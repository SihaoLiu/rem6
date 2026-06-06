use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_system::{
    traffic_trace_replay_controller_control_completion,
    traffic_trace_replay_controller_target_outcome,
    traffic_trace_replay_runtime_control_completion, traffic_trace_replay_runtime_target_outcome,
    traffic_trace_replay_target_event, traffic_trace_replay_target_outcome,
    TrafficTraceReplayControlError, TrafficTraceReplayControlRuntime,
    TrafficTraceReplayControllerControlError, TrafficTraceReplayControllerRuntime,
    TrafficTraceReplayControllerTargetError, TrafficTraceReplayTargetError,
    TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetRuntime,
};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerEvent,
    TrafficControllerEventBatch, TrafficControllerState, TrafficStateGenerator,
    TrafficStateGeneratorSnapshot, TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec,
    TrafficTrace, TrafficTraceConfig, TrafficTraceControlFailure, TrafficTraceErrorKind,
    TrafficTraceMemoryFailure, TrafficTraceReplayAction, TrafficTraceReplayActionQueue,
    TrafficTransition, TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, ResponseDelivery,
    TargetOutcome, TransportEndpointId,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_READ_RESP_WITH_INVALIDATE: u32 = 3;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_WRITEBACK_DIRTY: u32 = 7;
const GEM5_MEM_FENCE_REQ: u32 = 38;
const GEM5_MEM_FENCE_RESP: u32 = 41;
const GEM5_INVALID_DEST_ERROR: u32 = 46;
const GEM5_WRITE_ERROR: u32 = 49;
const GEM5_HTM_REQ: u32 = 56;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    packet_id: Option<u64>,
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request(sequence: u64) -> MemoryRequest {
    request_from(1, sequence)
}

fn request_from(agent: u32, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(agent), sequence),
        Address::new(0x4000 + sequence * 0x40),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap()
}

fn completed_response(request: &MemoryRequest, data: &[u8]) -> MemoryResponse {
    MemoryResponse::completed(request, Some(data.to_vec())).unwrap()
}

fn controller_for_packets(packets: &[PacketFields]) -> TrafficController {
    controller_for_packets_with_state_duration(packets, u64::MAX)
}

fn controller_for_packets_with_state_duration(
    packets: &[PacketFields],
    duration: u64,
) -> TrafficController {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, duration)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(rem6_traffic::TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    TrafficController::new(controller_config)
}

fn trace_cursor(controller: &TrafficController) -> usize {
    match controller.snapshot().generators()[0].generator() {
        TrafficStateGeneratorSnapshot::Trace(snapshot) => snapshot.cursor(),
        _ => panic!("traffic replay test controller must use trace generator"),
    }
}

#[test]
fn traffic_trace_replay_target_outcome_drives_transport_response_timing() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let action_queue = Arc::new(Mutex::new(TrafficTraceReplayActionQueue::default()));

    let core = endpoint("core0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                memory,
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(10);
    action_queue
        .lock()
        .unwrap()
        .record_action(TrafficTraceReplayAction::MemoryResponse {
            tick: 7,
            response: completed_response(&req, &[0xde, 0xad, 0xbe, 0xef, 0x44, 0x55, 0x66, 0x77]),
        })
        .unwrap();

    let queue = Arc::clone(&action_queue);
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery, _context| {
                assert_eq!(delivery.tick(), 3);
                traffic_trace_replay_target_outcome(&mut queue.lock().unwrap(), &delivery).unwrap()
            },
            move |delivery: ResponseDelivery| {
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.endpoint().clone(),
                    delivery.response().data().unwrap().to_vec(),
                ));
            },
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.final_tick(), 12);
    assert_eq!(
        *responses.lock().unwrap(),
        vec![(
            12,
            core.clone(),
            vec![0xde, 0xad, 0xbe, 0xef, 0x44, 0x55, 0x66, 0x77]
        )]
    );
    assert!(action_queue.lock().unwrap().is_empty());
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                core.clone(),
                MemoryTraceKind::RequestSent,
                req.id()
            ),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("memory0"),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
            MemoryTraceEvent::response(12, route, core, req.id(), ResponseStatus::Completed),
        ]
    );
}

#[test]
fn traffic_trace_replay_target_outcome_rejects_wrong_request_response() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let action_queue = Arc::new(Mutex::new(TrafficTraceReplayActionQueue::default()));
    let errors = Arc::new(Mutex::new(Vec::new()));

    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(20);
    let wrong_req = request_from(2, 99);
    action_queue
        .lock()
        .unwrap()
        .record_action(TrafficTraceReplayAction::MemoryResponse {
            tick: 7,
            response: completed_response(&wrong_req, &[0xaa; 8]),
        })
        .unwrap();

    let queue = Arc::clone(&action_queue);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, _context| match traffic_trace_replay_target_outcome(
                &mut queue.lock().unwrap(),
                &delivery,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("mismatched trace response must not reach the requester"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayTargetError::RequestMismatch {
            request: req.id(),
            response: wrong_req.id(),
        }]
    );
    assert!(!action_queue.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_target_event_consumes_memory_failure_without_response_delivery() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let action_queue = Arc::new(Mutex::new(TrafficTraceReplayActionQueue::default()));
    let failures = Arc::new(Mutex::new(Vec::new()));

    let core = endpoint("core0");
    let memory = endpoint("memory0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                memory,
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(30);
    let failure = TrafficTraceMemoryFailure::new(req.id(), TrafficTraceErrorKind::Write);
    action_queue
        .lock()
        .unwrap()
        .record_action(TrafficTraceReplayAction::MemoryFailure { tick: 7, failure })
        .unwrap();

    let queue = Arc::clone(&action_queue);
    let failure_log = Arc::clone(&failures);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery, context| {
                assert_eq!(delivery.tick(), 3);
                let event =
                    traffic_trace_replay_target_event(&mut queue.lock().unwrap(), &delivery)
                        .unwrap();
                match &event {
                    TrafficTraceReplayTargetEvent::MemoryFailure { delay, record } => {
                        assert_eq!(*delay, 4);
                        let record = *record;
                        let failure_log = Arc::clone(&failure_log);
                        context
                            .schedule_local_after(event.target_delay(), move |context| {
                                failure_log.lock().unwrap().push((context.now(), record));
                            })
                            .unwrap();
                    }
                    TrafficTraceReplayTargetEvent::MemoryResponse(_) => {
                        panic!("trace memory failure must not become a response event");
                    }
                }
                event.into_target_outcome()
            },
            |_| panic!("trace memory failure must not reach the requester as a response"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(failures.lock().unwrap().len(), 1);
    let (failure_tick, record) = failures.lock().unwrap()[0];
    assert_eq!(failure_tick, 7);
    assert_eq!(record.tick(), 7);
    assert_eq!(record.failure(), failure);
    assert!(action_queue.lock().unwrap().is_empty());
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                core.clone(),
                MemoryTraceKind::RequestSent,
                req.id()
            ),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("memory0"),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
        ]
    );
}

#[test]
fn traffic_trace_replay_target_event_preserves_memory_failure_on_request_mismatch() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let action_queue = Arc::new(Mutex::new(TrafficTraceReplayActionQueue::default()));
    let errors = Arc::new(Mutex::new(Vec::new()));

    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(40);
    let wrong_req = request_from(2, 100);
    let failure = TrafficTraceMemoryFailure::new(wrong_req.id(), TrafficTraceErrorKind::Read);
    action_queue
        .lock()
        .unwrap()
        .record_action(TrafficTraceReplayAction::MemoryFailure { tick: 7, failure })
        .unwrap();

    let queue = Arc::clone(&action_queue);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, _context| match traffic_trace_replay_target_event(
                &mut queue.lock().unwrap(),
                &delivery,
            ) {
                Ok(event) => event.into_target_outcome(),
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("mismatched trace failure must not reach the requester"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayTargetError::FailureRequestMismatch {
            request: req.id(),
            failure: wrong_req.id(),
        }]
    );
    assert!(!action_queue.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_target_runtime_records_batch_and_drives_memory_response() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayTargetRuntime::default()));

    let core = endpoint("core0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(50);
    runtime
        .lock()
        .unwrap()
        .record_batch(&TrafficControllerEventBatch::new(vec![
            TrafficControllerEvent::TraceReplayAction(TrafficTraceReplayAction::MemoryResponse {
                tick: 7,
                response: completed_response(
                    &req,
                    &[0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57],
                ),
            }),
        ]))
        .unwrap();

    let replay = Arc::clone(&runtime);
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery, context| {
                traffic_trace_replay_runtime_target_outcome(Arc::clone(&replay), &delivery, context)
                    .unwrap()
            },
            move |delivery: ResponseDelivery| {
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.endpoint().clone(),
                    delivery.response().data().unwrap().to_vec(),
                ));
            },
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *responses.lock().unwrap(),
        vec![(
            12,
            core,
            vec![0x50, 0x51, 0x52, 0x53, 0x54, 0x55, 0x56, 0x57]
        )]
    );
    assert!(runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
}

#[test]
fn traffic_trace_replay_target_runtime_reports_pending_request_as_not_empty() {
    let mut controller = controller_for_packets(&[PacketFields {
        tick: 0,
        command: GEM5_READ_REQ,
        address: Some(0x4400),
        size: Some(8),
        packet_id: Some(17),
    }]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    let req = request_batch.request().unwrap().request().clone();

    let mut runtime = TrafficTraceReplayTargetRuntime::default();
    runtime.record_batch(&request_batch).unwrap();

    assert_eq!(runtime.request_tick(req.id()), Some(0));
    assert!(!runtime.is_empty());
}

#[test]
fn traffic_trace_replay_target_runtime_schedules_memory_failure_from_batch() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayTargetRuntime::default()));

    let core = endpoint("core0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();
    let req = request(60);
    let failure = TrafficTraceMemoryFailure::new(req.id(), TrafficTraceErrorKind::Read);
    runtime
        .lock()
        .unwrap()
        .record_batch(&TrafficControllerEventBatch::new(vec![
            TrafficControllerEvent::TraceReplayAction(TrafficTraceReplayAction::MemoryFailure {
                tick: 7,
                failure,
            }),
        ]))
        .unwrap();

    let replay = Arc::clone(&runtime);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery, context| {
                traffic_trace_replay_runtime_target_outcome(Arc::clone(&replay), &delivery, context)
                    .unwrap()
            },
            |_| panic!("trace replay memory failure must not deliver a response"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(runtime.lock().unwrap().is_empty());
    assert_eq!(runtime.lock().unwrap().memory_failures().len(), 1);
    let scheduled_failure = runtime.lock().unwrap().memory_failures()[0];
    assert_eq!(scheduled_failure.tick(), 7);
    assert_eq!(scheduled_failure.record().tick(), 7);
    assert_eq!(scheduled_failure.record().failure(), failure);
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(0, route, core, MemoryTraceKind::RequestSent, req.id()),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("memory0"),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
        ]
    );
}

#[test]
fn traffic_trace_replay_controller_target_outcome_advances_controller_for_transport_response() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(70),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(70),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(request_batch.trace_replay_action().is_none());
    let req = request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();
    assert!(!runtime.lock().unwrap().is_empty());

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let core = endpoint("core0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let submitted_req = req.clone();
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            submitted_req.clone(),
            trace.clone(),
            move |delivery, context| {
                assert_eq!(delivery.tick(), 3);
                assert_eq!(delivery.request().id(), submitted_req.id());
                traffic_trace_replay_controller_target_outcome(
                    Arc::clone(&replay),
                    Arc::clone(&trace_controller),
                    &delivery,
                    context,
                    0,
                )
                .unwrap()
            },
            move |delivery: ResponseDelivery| {
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.endpoint().clone(),
                    delivery.response().status(),
                    delivery.response().data().unwrap().len(),
                ));
            },
        )
        .unwrap();

    let summary = scheduler.run_until_idle_conservative();

    assert_eq!(summary.final_tick(), 12);
    assert_eq!(
        *responses.lock().unwrap(),
        vec![(12, core.clone(), ResponseStatus::Completed, 8)]
    );
    assert!(runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(
                0,
                route,
                core.clone(),
                MemoryTraceKind::RequestSent,
                req.id()
            ),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("memory0"),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
            MemoryTraceEvent::response(12, route, core, req.id(), ResponseStatus::Completed),
        ]
    );
}

#[test]
fn traffic_trace_replay_controller_target_outcome_advances_controller_for_memory_failure() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(71),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(71),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(request_batch.trace_replay_action().is_none());
    let req = request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let trace = MemoryTrace::new();
    let core = endpoint("core0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            trace.clone(),
            move |delivery, context| {
                traffic_trace_replay_controller_target_outcome(
                    Arc::clone(&replay),
                    Arc::clone(&trace_controller),
                    &delivery,
                    context,
                    0,
                )
                .unwrap()
            },
            |_| panic!("trace replay memory failure must not deliver a response"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(runtime.lock().unwrap().is_empty());
    assert_eq!(runtime.lock().unwrap().memory_failures().len(), 1);
    let scheduled_failure = runtime.lock().unwrap().memory_failures()[0];
    assert_eq!(scheduled_failure.tick(), 7);
    assert_eq!(scheduled_failure.record().tick(), 7);
    assert_eq!(scheduled_failure.record().failure().request_id(), req.id());
    assert_eq!(
        scheduled_failure.record().failure().error(),
        TrafficTraceErrorKind::Write
    );
    assert_eq!(
        trace.snapshot(),
        vec![
            MemoryTraceEvent::request(0, route, core, MemoryTraceKind::RequestSent, req.id()),
            MemoryTraceEvent::request(
                3,
                route,
                endpoint("memory0"),
                MemoryTraceKind::RequestArrived,
                req.id()
            ),
        ]
    );
}

#[test]
fn traffic_trace_replay_controller_target_outcome_rejects_response_before_delivery() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(72),
        },
        PacketFields {
            tick: 2,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(72),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    let req = request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| match traffic_trace_replay_controller_target_outcome(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                &delivery,
                context,
                0,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("pre-delivery trace response must not reach the requester"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControllerTargetError::Target(
            TrafficTraceReplayTargetError::ResponseBeforeRequest {
                request: req.id(),
                delivery_tick: 3,
                response_tick: 2,
            },
        )]
    );
    assert!(!runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_controller_target_outcome_rejects_failure_before_delivery() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(73),
        },
        PacketFields {
            tick: 2,
            command: GEM5_WRITE_ERROR,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(73),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    let req = request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| match traffic_trace_replay_controller_target_outcome(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                &delivery,
                context,
                0,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("pre-delivery trace failure must not deliver a response"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControllerTargetError::Target(
            TrafficTraceReplayTargetError::FailureBeforeRequest {
                request: req.id(),
                delivery_tick: 3,
                failure_tick: 2,
            },
        )]
    );
    assert!(!runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
}

#[test]
fn traffic_trace_replay_controller_runtime_preserves_control_action_while_target_advances() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x8800),
            size: Some(8),
            packet_id: Some(84),
        },
        PacketFields {
            tick: 1,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(85),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(85),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x8800),
            size: Some(8),
            packet_id: Some(84),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    let req = request_batch.request().unwrap().request().clone();
    let sync_batch = controller.next_event(1, 0).unwrap().unwrap();
    assert!(sync_batch.trace_sync().unwrap().requires_response());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();
    runtime.lock().unwrap().record_batch(&sync_batch).unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let responses = Arc::new(Mutex::new(Vec::new()));
    let errors = Arc::new(Mutex::new(Vec::new()));
    let core = endpoint("core0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core.clone(),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let response_log = Arc::clone(&responses);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| match traffic_trace_replay_controller_target_outcome(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                &delivery,
                context,
                0,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(format!("{error}"));
                    TargetOutcome::NoResponse
                }
            },
            move |delivery: ResponseDelivery| {
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.endpoint().clone(),
                    delivery.response().request_id(),
                ));
            },
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    scheduler
        .schedule_at(PartitionId::new(1), 3, move |context| {
            if let Err(error) = traffic_trace_replay_controller_control_completion(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                context.now(),
                context,
                0,
            ) {
                error_log.lock().unwrap().push(format!("{error}"));
            }
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(errors.lock().unwrap().is_empty());
    assert_eq!(*responses.lock().unwrap(), vec![(13, core, req.id())]);
    assert_eq!(runtime.lock().unwrap().control_acks().len(), 1);
    let ack = runtime.lock().unwrap().control_acks()[0];
    assert_eq!(ack.tick(), 7);
    assert_eq!(ack.trace_tick(), 7);
    assert!(runtime.lock().unwrap().control_failures().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
    assert!(runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_controller_target_outcome_reports_missing_after_control_ack_trace_exit() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9000),
            size: Some(8),
            packet_id: Some(76),
        },
        PacketFields {
            tick: 1,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(77),
        },
        PacketFields {
            tick: 2,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(77),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    let req = request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| match traffic_trace_replay_controller_target_outcome(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                &delivery,
                context,
                0,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("missing trace memory response must not reach the requester"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControllerTargetError::ReplayActionMissing { request: req.id() }]
    );
    assert!(!runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
    assert!(runtime.lock().unwrap().control_acks().is_empty());
}

#[test]
fn traffic_trace_replay_controller_target_outcome_accepts_no_response_memory_request() {
    let mut controller = controller_for_packets(&[PacketFields {
        tick: 0,
        command: GEM5_WRITEBACK_DIRTY,
        address: Some(0xa000),
        size: Some(64),
        packet_id: Some(78),
    }]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    let req = request_batch.request().unwrap().request().clone();
    assert!(!req.requires_response());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| match traffic_trace_replay_controller_target_outcome(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                &delivery,
                context,
                0,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("no-response trace memory request must not deliver a response"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(errors.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
}

#[test]
fn traffic_trace_replay_controller_target_outcome_reports_missing_replay_action() {
    let mut controller = controller_for_packets(&[PacketFields {
        tick: 0,
        command: GEM5_READ_REQ,
        address: Some(0x6000),
        size: Some(8),
        packet_id: Some(72),
    }]);

    assert!(controller.start(0).unwrap().is_empty());
    let request_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(request_batch.trace_replay_action().is_none());
    let req = request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| match traffic_trace_replay_controller_target_outcome(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                &delivery,
                context,
                0,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("missing trace replay action must not deliver a response"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControllerTargetError::ReplayActionMissing { request: req.id() }]
    );
    assert!(!runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
}

#[test]
fn traffic_trace_replay_controller_target_outcome_reports_missing_on_replayed_request() {
    let packets = [PacketFields {
        tick: 0,
        command: GEM5_READ_REQ,
        address: Some(0x6400),
        size: Some(8),
        packet_id: Some(73),
    }];

    let mut source_controller = controller_for_packets_with_state_duration(&packets, 10);
    assert!(source_controller.start(0).unwrap().is_empty());
    let request_batch = source_controller.next_event(0, 0).unwrap().unwrap();
    assert!(request_batch.trace_exit().is_none());
    assert_eq!(trace_cursor(&source_controller), 1);
    let req = request_batch.request().unwrap().request().clone();

    let mut replay_controller = controller_for_packets_with_state_duration(&packets, 10);
    assert!(replay_controller.start(0).unwrap().is_empty());
    assert_eq!(trace_cursor(&replay_controller), 0);

    let controller = Arc::new(Mutex::new(replay_controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let route = transport
        .add_route(
            MemoryRoute::new(
                endpoint("core0"),
                PartitionId::new(0),
                endpoint("memory0"),
                PartitionId::new(1),
                3,
                5,
            )
            .unwrap(),
        )
        .unwrap();

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| match traffic_trace_replay_controller_target_outcome(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                &delivery,
                context,
                0,
            ) {
                Ok(outcome) => outcome,
                Err(error) => {
                    error_log.lock().unwrap().push(error);
                    TargetOutcome::NoResponse
                }
            },
            |_| panic!("replayed request without response action must not deliver a response"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControllerTargetError::ReplayActionMissing { request: req.id() }]
    );
    assert_eq!(trace_cursor(&controller.lock().unwrap()), 1);
    assert!(!runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
}

#[test]
fn traffic_trace_replay_control_runtime_schedules_control_ack_from_batch() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControlRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&TrafficControllerEventBatch::new(vec![
            TrafficControllerEvent::TraceReplayAction(TrafficTraceReplayAction::ControlAck {
                tick: 7,
            }),
        ]))
        .unwrap();

    let replay = Arc::clone(&runtime);
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            traffic_trace_replay_runtime_control_completion(
                Arc::clone(&replay),
                context.now(),
                context,
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(runtime.lock().unwrap().is_empty());
    assert_eq!(runtime.lock().unwrap().control_acks().len(), 1);
    let ack = runtime.lock().unwrap().control_acks()[0];
    assert_eq!(ack.tick(), 7);
    assert_eq!(ack.trace_tick(), 7);
    assert!(runtime.lock().unwrap().control_failures().is_empty());
}

#[test]
fn traffic_trace_replay_control_runtime_schedules_control_failure_from_batch() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControlRuntime::default()));
    let failure = TrafficTraceControlFailure::new(TrafficTraceErrorKind::InvalidDestination);
    runtime
        .lock()
        .unwrap()
        .record_batch(&TrafficControllerEventBatch::new(vec![
            TrafficControllerEvent::TraceReplayAction(TrafficTraceReplayAction::ControlFailure {
                tick: 9,
                failure,
            }),
        ]))
        .unwrap();

    let replay = Arc::clone(&runtime);
    scheduler
        .schedule_at(PartitionId::new(0), 4, move |context| {
            traffic_trace_replay_runtime_control_completion(
                Arc::clone(&replay),
                context.now(),
                context,
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(runtime.lock().unwrap().is_empty());
    assert_eq!(runtime.lock().unwrap().control_failures().len(), 1);
    let scheduled_failure = runtime.lock().unwrap().control_failures()[0];
    assert_eq!(scheduled_failure.tick(), 9);
    assert_eq!(scheduled_failure.record().tick(), 9);
    assert_eq!(scheduled_failure.record().failure(), failure);
    assert!(runtime.lock().unwrap().control_acks().is_empty());
}

#[test]
fn traffic_trace_replay_control_runtime_rejects_ack_before_delivery() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControlRuntime::default()));
    let errors = Arc::new(Mutex::new(Vec::new()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&TrafficControllerEventBatch::new(vec![
            TrafficControllerEvent::TraceReplayAction(TrafficTraceReplayAction::ControlAck {
                tick: 2,
            }),
        ]))
        .unwrap();

    let replay = Arc::clone(&runtime);
    let error_log = Arc::clone(&errors);
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            if let Err(error) = traffic_trace_replay_runtime_control_completion(
                Arc::clone(&replay),
                context.now(),
                context,
            ) {
                error_log.lock().unwrap().push(error);
            }
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControlError::AckBeforeDelivery {
            delivery_tick: 3,
            ack_tick: 2,
        }]
    );
    assert!(!runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().control_acks().is_empty());
}

#[test]
fn traffic_trace_replay_control_runtime_rejects_failure_before_delivery() {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControlRuntime::default()));
    let errors = Arc::new(Mutex::new(Vec::new()));
    let failure = TrafficTraceControlFailure::new(TrafficTraceErrorKind::InvalidDestination);
    runtime
        .lock()
        .unwrap()
        .record_batch(&TrafficControllerEventBatch::new(vec![
            TrafficControllerEvent::TraceReplayAction(TrafficTraceReplayAction::ControlFailure {
                tick: 2,
                failure,
            }),
        ]))
        .unwrap();

    let replay = Arc::clone(&runtime);
    let error_log = Arc::clone(&errors);
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            if let Err(error) = traffic_trace_replay_runtime_control_completion(
                Arc::clone(&replay),
                context.now(),
                context,
            ) {
                error_log.lock().unwrap().push(error);
            }
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControlError::FailureBeforeDelivery {
            delivery_tick: 3,
            failure_tick: 2,
        }]
    );
    assert!(!runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().control_failures().is_empty());
}

#[test]
fn traffic_trace_replay_controller_control_completion_advances_controller_for_sync_ack() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(80),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(80),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let sync_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(sync_batch.trace_sync().unwrap().requires_response());
    assert!(sync_batch.trace_replay_action().is_none());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime.lock().unwrap().record_batch(&sync_batch).unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            traffic_trace_replay_controller_control_completion(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                context.now(),
                context,
                0,
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(runtime.lock().unwrap().is_empty());
    assert_eq!(runtime.lock().unwrap().control_acks().len(), 1);
    assert_eq!(runtime.lock().unwrap().control_acks()[0].tick(), 7);
    assert!(runtime.lock().unwrap().control_failures().is_empty());
}

#[test]
fn traffic_trace_replay_controller_control_completion_advances_controller_for_htm_failure() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_HTM_REQ,
            address: Some(0xb000),
            size: Some(16),
            packet_id: Some(81),
        },
        PacketFields {
            tick: 7,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0xb000),
            size: Some(16),
            packet_id: Some(81),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let htm_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(htm_batch.trace_htm().unwrap().requires_response());
    assert!(htm_batch.trace_replay_action().is_none());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime.lock().unwrap().record_batch(&htm_batch).unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            traffic_trace_replay_controller_control_completion(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                context.now(),
                context,
                0,
            )
            .unwrap();
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().control_acks().is_empty());
    assert_eq!(runtime.lock().unwrap().control_failures().len(), 1);
    let scheduled_failure = runtime.lock().unwrap().control_failures()[0];
    assert_eq!(scheduled_failure.tick(), 7);
    assert_eq!(scheduled_failure.record().tick(), 7);
    assert_eq!(
        scheduled_failure.record().failure().error(),
        TrafficTraceErrorKind::InvalidDestination
    );
}

#[test]
fn traffic_trace_replay_controller_control_completion_reports_missing_control_action() {
    let mut controller = controller_for_packets(&[PacketFields {
        tick: 0,
        command: GEM5_MEM_FENCE_REQ,
        address: None,
        size: None,
        packet_id: Some(82),
    }]);

    assert!(controller.start(0).unwrap().is_empty());
    let sync_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(sync_batch.trace_sync().unwrap().requires_response());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime.lock().unwrap().record_batch(&sync_batch).unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let errors = Arc::new(Mutex::new(Vec::new()));
    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let error_log = Arc::clone(&errors);
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            if let Err(error) = traffic_trace_replay_controller_control_completion(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                context.now(),
                context,
                0,
            ) {
                error_log.lock().unwrap().push(error);
            }
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![TrafficTraceReplayControllerControlError::ReplayActionMissing { delivery_tick: 3 }]
    );
    assert!(runtime.lock().unwrap().is_empty());
}

fn state(id: u32, duration: u64) -> TrafficStateSpec {
    TrafficStateSpec::new(TrafficStateId::new(id), duration)
}

fn transition(from: u32, to: u32) -> TrafficTransition {
    TrafficTransition::new(
        TrafficStateId::new(from),
        TrafficStateId::new(to),
        TrafficTransitionProbability::from_micros(TRAFFIC_TRANSITION_PROBABILITY_SCALE).unwrap(),
    )
}

fn graph(
    states: Vec<TrafficStateSpec>,
    transitions: Vec<TrafficTransition>,
) -> TrafficStateGraphConfig {
    TrafficStateGraphConfig::new(states, TrafficStateId::new(0), transitions).unwrap()
}

fn gem5_packet_trace(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut bytes = GEM5_MAGIC.to_vec();
    let mut header = Vec::new();
    append_key(&mut header, 3, 0);
    append_varint(&mut header, tick_frequency);
    append_record(&mut bytes, &header);

    for packet in packets {
        let mut message = Vec::new();
        append_key(&mut message, 1, 0);
        append_varint(&mut message, packet.tick);
        append_key(&mut message, 2, 0);
        append_varint(&mut message, u64::from(packet.command));
        if let Some(address) = packet.address {
            append_key(&mut message, 3, 0);
            append_varint(&mut message, address);
        }
        if let Some(size) = packet.size {
            append_key(&mut message, 4, 0);
            append_varint(&mut message, u64::from(size));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        append_record(&mut bytes, &message);
    }

    bytes
}

fn append_record(bytes: &mut Vec<u8>, message: &[u8]) {
    append_varint(
        bytes,
        u64::try_from(message.len()).expect("test message length fits u64"),
    );
    bytes.extend_from_slice(message);
}

fn append_key(bytes: &mut Vec<u8>, field: u32, wire_type: u8) {
    append_varint(bytes, (u64::from(field) << 3) | u64::from(wire_type));
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        bytes.push((value as u8) | 0x80);
        value >>= 7;
    }
    bytes.push(value as u8);
}
