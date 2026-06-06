use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_system::{
    traffic_trace_replay_controller_control_completion,
    traffic_trace_replay_controller_control_event, TrafficTraceReplayControlEvent,
    TrafficTraceReplayControllerControlError, TrafficTraceReplayControllerRuntime,
};
use rem6_traffic::TrafficTraceErrorKind;

mod support;

use support::traffic_trace::{
    controller_for_packets, PacketFields, GEM5_HTM_REQ, GEM5_INVALID_DEST_ERROR,
    GEM5_MEM_FENCE_REQ, GEM5_MEM_FENCE_RESP,
};

#[test]
fn traffic_trace_replay_control_completion_does_not_consume_later_ack_for_missing_source() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(90),
        },
        PacketFields {
            tick: 1,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(91),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(91),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let first_source_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(first_source_batch.trace_sync().unwrap().requires_response());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&first_source_batch)
        .unwrap();

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
    assert!(runtime.lock().unwrap().control_acks().is_empty());
    assert!(!runtime.lock().unwrap().is_empty());

    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    scheduler
        .schedule_at(PartitionId::new(0), 4, move |context| {
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
fn traffic_trace_replay_control_completion_accepts_out_of_order_control_acks() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(90),
        },
        PacketFields {
            tick: 1,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(91),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(91),
        },
        PacketFields {
            tick: 9,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(90),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let first_source_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(first_source_batch.trace_sync().unwrap().requires_response());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime
        .lock()
        .unwrap()
        .record_batch(&first_source_batch)
        .unwrap();

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

    assert!(!runtime.lock().unwrap().is_empty());
    assert_eq!(runtime.lock().unwrap().control_acks().len(), 1);
    assert_eq!(runtime.lock().unwrap().control_acks()[0].tick(), 9);
    assert_eq!(runtime.lock().unwrap().control_acks()[0].trace_tick(), 9);

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    scheduler
        .schedule_at(PartitionId::new(0), 4, move |context| {
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
    assert_eq!(runtime.lock().unwrap().control_acks().len(), 2);
    assert_eq!(runtime.lock().unwrap().control_acks()[1].tick(), 7);
    assert_eq!(runtime.lock().unwrap().control_acks()[1].trace_tick(), 7);
    assert!(runtime.lock().unwrap().control_failures().is_empty());
}

#[test]
fn traffic_trace_replay_control_event_exposes_controller_failure_to_consumer() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_HTM_REQ,
            address: Some(0x7100),
            size: Some(16),
            packet_id: Some(92),
        },
        PacketFields {
            tick: 7,
            command: GEM5_INVALID_DEST_ERROR,
            address: Some(0x7100),
            size: Some(16),
            packet_id: Some(92),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let htm_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(htm_batch.trace_htm().unwrap().requires_response());

    let controller = Arc::new(Mutex::new(controller));
    let runtime = Arc::new(Mutex::new(TrafficTraceReplayControllerRuntime::default()));
    runtime.lock().unwrap().record_batch(&htm_batch).unwrap();

    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    let failures = Arc::new(Mutex::new(Vec::new()));
    let replay = Arc::clone(&runtime);
    let trace_controller = Arc::clone(&controller);
    let failure_log = Arc::clone(&failures);
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            let event = traffic_trace_replay_controller_control_event(
                Arc::clone(&replay),
                Arc::clone(&trace_controller),
                context.now(),
                context,
                0,
            )
            .unwrap();
            match event {
                TrafficTraceReplayControlEvent::ControlFailure { delay, record } => {
                    failure_log.lock().unwrap().push((
                        context.now(),
                        delay,
                        record.tick(),
                        record.failure().error(),
                    ));
                }
                TrafficTraceReplayControlEvent::ControlAck { .. } => {
                    panic!("HTM error replay must expose a control failure event");
                }
            }
        })
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(
        *failures.lock().unwrap(),
        vec![(3, 4, 7, TrafficTraceErrorKind::InvalidDestination)]
    );
    assert!(runtime.lock().unwrap().control_failures().is_empty());
    assert!(runtime.lock().unwrap().control_acks().is_empty());
    assert!(runtime.lock().unwrap().is_empty());
}
