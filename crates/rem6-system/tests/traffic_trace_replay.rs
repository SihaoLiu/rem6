use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_system::{
    traffic_trace_replay_runtime_target_outcome, traffic_trace_replay_target_event,
    traffic_trace_replay_target_outcome, TrafficTraceReplayTargetError,
    TrafficTraceReplayTargetEvent, TrafficTraceReplayTargetRuntime,
};
use rem6_traffic::{
    TrafficControllerEvent, TrafficControllerEventBatch, TrafficTraceErrorKind,
    TrafficTraceMemoryFailure, TrafficTraceReplayAction, TrafficTraceReplayActionQueue,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport, ResponseDelivery,
    TargetOutcome, TransportEndpointId,
};

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
