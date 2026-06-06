use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::ResponseStatus;
use rem6_system::{
    traffic_trace_replay_controller_target_outcome, TrafficTraceReplayControllerRuntime,
};
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
