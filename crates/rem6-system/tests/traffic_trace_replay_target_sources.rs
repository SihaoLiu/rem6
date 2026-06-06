use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::ResponseStatus;
use rem6_system::{
    traffic_trace_replay_controller_target_event, traffic_trace_replay_controller_target_outcome,
    TrafficTraceReplayControllerRuntime, TrafficTraceReplayControllerTargetError,
    TrafficTraceReplayTargetEvent,
};
use rem6_traffic::TrafficTraceErrorKind;
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport, ResponseDelivery, TargetOutcome};

mod support;

use support::traffic_trace::{
    controller_for_packets, endpoint, PacketFields, GEM5_READ_REQ, GEM5_READ_RESP_WITH_INVALIDATE,
    GEM5_WRITE_ERROR, GEM5_WRITE_REQ,
};

#[test]
fn traffic_trace_replay_target_outcome_accepts_out_of_order_memory_responses() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(90),
        },
        PacketFields {
            tick: 1,
            command: GEM5_READ_REQ,
            address: Some(0x4040),
            size: Some(8),
            packet_id: Some(91),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x4040),
            size: Some(8),
            packet_id: Some(91),
        },
        PacketFields {
            tick: 9,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x4000),
            size: Some(8),
            packet_id: Some(90),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let first_request_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(first_request_batch.trace_replay_action().is_none());
    let req = first_request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&first_request_batch)
        .unwrap();

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
    let error_log = Arc::clone(&errors);
    let response_log = Arc::clone(&responses);
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
            move |delivery: ResponseDelivery| {
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.endpoint().clone(),
                    delivery.response().request_id(),
                    delivery.response().status(),
                ));
            },
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(errors.lock().unwrap().is_empty());
    assert_eq!(
        *responses.lock().unwrap(),
        vec![(14, core, req.id(), ResponseStatus::Completed)]
    );
    assert!(!runtime.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
}

#[test]
fn traffic_trace_replay_target_outcome_accepts_out_of_order_memory_failures() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(92),
        },
        PacketFields {
            tick: 1,
            command: GEM5_WRITE_REQ,
            address: Some(0x5040),
            size: Some(8),
            packet_id: Some(93),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x5040),
            size: Some(8),
            packet_id: Some(93),
        },
        PacketFields {
            tick: 9,
            command: GEM5_WRITE_ERROR,
            address: Some(0x5000),
            size: Some(8),
            packet_id: Some(92),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let first_request_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(first_request_batch.trace_replay_action().is_none());
    let req = first_request_batch.request().unwrap().request().clone();

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&first_request_batch)
        .unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let responses = Arc::new(Mutex::new(Vec::new()));
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
    let response_log = Arc::clone(&responses);
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
            move |delivery: ResponseDelivery| {
                response_log
                    .lock()
                    .unwrap()
                    .push(delivery.response().request_id());
            },
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert!(errors.lock().unwrap().is_empty());
    assert!(responses.lock().unwrap().is_empty());
    assert_eq!(runtime.lock().unwrap().memory_failures().len(), 1);
    let failure = runtime.lock().unwrap().memory_failures()[0];
    assert_eq!(failure.tick(), 9);
    assert_eq!(failure.record().tick(), 9);
    assert_eq!(failure.record().failure().request_id(), req.id());
    assert!(!runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_target_outcome_drops_reported_missing_request_before_later_response() {
    let packets = [
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x6000),
            size: Some(8),
            packet_id: Some(94),
        },
        PacketFields {
            tick: 1,
            command: GEM5_READ_REQ,
            address: Some(0x6040),
            size: Some(8),
            packet_id: Some(95),
        },
        PacketFields {
            tick: 7,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x6040),
            size: Some(8),
            packet_id: Some(95),
        },
    ];

    let mut source_controller = controller_for_packets(&packets);
    assert!(source_controller.start(0).unwrap().is_empty());
    let first_request_batch = source_controller.next_event(0, 0).unwrap().unwrap();
    let first_req = first_request_batch.request().unwrap().request().clone();
    let second_request_batch = source_controller.next_event(1, 0).unwrap().unwrap();
    let second_req = second_request_batch.request().unwrap().request().clone();

    let mut replay_controller = controller_for_packets(&packets);
    assert!(replay_controller.start(0).unwrap().is_empty());
    let replayed_first_batch = replay_controller.next_event(0, 0).unwrap().unwrap();
    assert_eq!(
        replayed_first_batch.request().unwrap().request().id(),
        first_req.id()
    );

    let controller = Arc::new(Mutex::new(replay_controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&first_request_batch)
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
            first_req.clone(),
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
            |_| panic!("missing first trace response must not reach the requester"),
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *errors.lock().unwrap(),
        vec![
            TrafficTraceReplayControllerTargetError::ReplayActionMissing {
                request: first_req.id()
            }
        ]
    );
    assert!(!runtime.lock().unwrap().is_empty());

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let mut transport = MemoryTransport::new();
    let responses = Arc::new(Mutex::new(Vec::new()));
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
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            second_req.clone(),
            MemoryTrace::new(),
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
            move |delivery: ResponseDelivery| {
                response_log
                    .lock()
                    .unwrap()
                    .push((delivery.tick(), delivery.response().request_id()));
            },
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(*responses.lock().unwrap(), vec![(12, second_req.id())]);
    assert!(runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_target_event_exposes_controller_memory_failure_to_consumer() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_WRITE_REQ,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(96),
        },
        PacketFields {
            tick: 7,
            command: GEM5_WRITE_ERROR,
            address: Some(0x7000),
            size: Some(8),
            packet_id: Some(96),
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
    let failures = Arc::new(Mutex::new(Vec::new()));
    let responses = Arc::new(Mutex::new(Vec::new()));
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
    let failure_log = Arc::clone(&failures);
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| {
                let event = traffic_trace_replay_controller_target_event(
                    Arc::clone(&replay),
                    Arc::clone(&trace_controller),
                    &delivery,
                    context,
                    0,
                )
                .unwrap()
                .unwrap();
                match event {
                    TrafficTraceReplayTargetEvent::MemoryFailure { delay, record } => {
                        failure_log.lock().unwrap().push((
                            context.now(),
                            delay,
                            record.tick(),
                            record.failure().request_id(),
                            record.failure().error(),
                        ));
                        TargetOutcome::NoResponse
                    }
                    TrafficTraceReplayTargetEvent::MemoryResponse(outcome) => outcome,
                }
            },
            move |delivery: ResponseDelivery| {
                response_log
                    .lock()
                    .unwrap()
                    .push(delivery.response().request_id());
            },
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *failures.lock().unwrap(),
        vec![(3, 4, 7, req.id(), TrafficTraceErrorKind::Write)]
    );
    assert!(responses.lock().unwrap().is_empty());
    assert!(runtime.lock().unwrap().memory_failures().is_empty());
    assert!(runtime.lock().unwrap().is_empty());
}
