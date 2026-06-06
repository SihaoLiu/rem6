use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::Address;
use rem6_system::{
    traffic_trace_replay_controller_control_completion,
    traffic_trace_replay_controller_runtime_sideband_events,
    traffic_trace_replay_controller_target_outcome, traffic_trace_replay_runtime_sideband_events,
    TrafficTraceReplayControllerRuntime, TrafficTraceReplaySidebandEvent,
    TrafficTraceReplaySidebandRuntime,
};
use rem6_traffic::{
    TrafficTraceCacheKind, TrafficTraceDiagnosticKind, TrafficTraceHtmKind, TrafficTraceTlbKind,
};
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport, ResponseDelivery};

mod support;

use support::traffic_trace::{
    controller_for_packets, endpoint, PacketFields, GEM5_FLUSH_REQ, GEM5_HTM_ABORT,
    GEM5_MEM_FENCE_REQ, GEM5_MEM_FENCE_RESP, GEM5_PRINT_REQ, GEM5_READ_REQ,
    GEM5_READ_RESP_WITH_INVALIDATE, GEM5_TLBI_EXT_SYNC,
};

#[test]
fn traffic_trace_replay_sideband_runtime_schedules_non_memory_trace_events() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 5,
            command: GEM5_TLBI_EXT_SYNC,
            address: Some(0),
            size: Some(64),
            packet_id: Some(10),
        },
        PacketFields {
            tick: 7,
            command: GEM5_FLUSH_REQ,
            address: Some(0x4000),
            size: Some(64),
            packet_id: Some(11),
        },
        PacketFields {
            tick: 9,
            command: GEM5_PRINT_REQ,
            address: Some(0x5000),
            size: Some(1),
            packet_id: Some(12),
        },
        PacketFields {
            tick: 11,
            command: GEM5_HTM_ABORT,
            address: None,
            size: None,
            packet_id: Some(13),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let runtime = Arc::new(Mutex::new(TrafficTraceReplaySidebandRuntime::default()));
    let mut tick = 0;
    for _ in 0..4 {
        let batch = controller.next_event(tick, 0).unwrap().unwrap();
        tick = batch.events()[0].tick_for_test();
        runtime.lock().unwrap().record_batch(&batch).unwrap();
    }

    let replay = Arc::clone(&runtime);
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(2, 2).unwrap();
    scheduler
        .schedule_at(PartitionId::new(0), 3, move |context| {
            assert_eq!(
                traffic_trace_replay_runtime_sideband_events(
                    Arc::clone(&replay),
                    context.now(),
                    context,
                ),
                4
            );
        })
        .unwrap();
    scheduler.run_until_idle_conservative();

    let records = runtime.lock().unwrap().sideband_events().to_vec();
    assert_eq!(records.len(), 4);
    assert_eq!(records[0].tick(), 5);
    assert!(matches!(
        records[0].event(),
        TrafficTraceReplaySidebandEvent::Tlb(event)
            if event.kind() == TrafficTraceTlbKind::ExternalSync
    ));
    assert_eq!(records[1].tick(), 7);
    assert!(matches!(
        records[1].event(),
        TrafficTraceReplaySidebandEvent::Cache(event)
            if event.kind() == TrafficTraceCacheKind::Flush
                && event.address() == Address::new(0x4000)
    ));
    assert_eq!(records[2].tick(), 9);
    assert!(matches!(
        records[2].event(),
        TrafficTraceReplaySidebandEvent::Diagnostic(event)
            if event.kind() == TrafficTraceDiagnosticKind::Print
                && event.address() == Some(Address::new(0x5000))
    ));
    assert_eq!(records[3].tick(), 11);
    assert!(matches!(
        records[3].event(),
        TrafficTraceReplaySidebandEvent::Htm(event)
            if event.kind() == TrafficTraceHtmKind::Abort
    ));
    assert!(runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_controller_runtime_preserves_sideband_while_target_advances() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x8000),
            size: Some(8),
            packet_id: Some(20),
        },
        PacketFields {
            tick: 5,
            command: GEM5_FLUSH_REQ,
            address: Some(0x8040),
            size: Some(64),
            packet_id: Some(21),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x8000),
            size: Some(8),
            packet_id: Some(20),
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
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
            MemoryTrace::new(),
            move |delivery, context| {
                let outcome = traffic_trace_replay_controller_target_outcome(
                    Arc::clone(&replay),
                    Arc::clone(&trace_controller),
                    &delivery,
                    context,
                    0,
                )
                .unwrap();
                assert_eq!(
                    traffic_trace_replay_controller_runtime_sideband_events(
                        Arc::clone(&replay),
                        context.now(),
                        context,
                    ),
                    0
                );
                outcome
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

    scheduler.run_until_idle_conservative();

    assert_eq!(*responses.lock().unwrap(), vec![(13, core, req.id())]);
    let records = runtime.lock().unwrap().sideband_events().to_vec();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tick(), 5);
    assert!(matches!(
        records[0].event(),
        TrafficTraceReplaySidebandEvent::Cache(event)
            if event.kind() == TrafficTraceCacheKind::Flush
                && event.address() == Address::new(0x8040)
    ));
    assert!(runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_controller_target_outcome_schedules_sideband_while_advancing() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x8800),
            size: Some(8),
            packet_id: Some(25),
        },
        PacketFields {
            tick: 5,
            command: GEM5_FLUSH_REQ,
            address: Some(0x8840),
            size: Some(64),
            packet_id: Some(26),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x8800),
            size: Some(8),
            packet_id: Some(25),
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
    let response_log = Arc::clone(&responses);
    transport
        .submit(
            &mut scheduler,
            route,
            req.clone(),
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
                response_log.lock().unwrap().push((
                    delivery.tick(),
                    delivery.endpoint().clone(),
                    delivery.response().request_id(),
                ));
            },
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    assert_eq!(*responses.lock().unwrap(), vec![(13, core, req.id())]);
    let records = runtime.lock().unwrap().sideband_events().to_vec();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tick(), 5);
    assert!(matches!(
        records[0].event(),
        TrafficTraceReplaySidebandEvent::Cache(event)
            if event.kind() == TrafficTraceCacheKind::Flush
                && event.address() == Address::new(0x8840)
    ));
    assert!(runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_controller_control_completion_schedules_sideband_while_advancing() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_MEM_FENCE_REQ,
            address: None,
            size: None,
            packet_id: Some(28),
        },
        PacketFields {
            tick: 5,
            command: GEM5_FLUSH_REQ,
            address: Some(0x8940),
            size: Some(64),
            packet_id: Some(29),
        },
        PacketFields {
            tick: 7,
            command: GEM5_MEM_FENCE_RESP,
            address: None,
            size: None,
            packet_id: Some(28),
        },
    ]);

    assert!(controller.start(0).unwrap().is_empty());
    let sync_batch = controller.next_event(0, 0).unwrap().unwrap();
    assert!(sync_batch.trace_sync().unwrap().requires_response());

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

    assert_eq!(runtime.lock().unwrap().control_acks().len(), 1);
    assert_eq!(runtime.lock().unwrap().control_acks()[0].tick(), 7);
    let records = runtime.lock().unwrap().sideband_events().to_vec();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tick(), 5);
    assert!(matches!(
        records[0].event(),
        TrafficTraceReplaySidebandEvent::Cache(event)
            if event.kind() == TrafficTraceCacheKind::Flush
                && event.address() == Address::new(0x8940)
    ));
    assert!(runtime.lock().unwrap().is_empty());
}

#[test]
fn traffic_trace_replay_controller_runtime_records_late_sideband_after_target_advances() {
    let mut controller = controller_for_packets(&[
        PacketFields {
            tick: 0,
            command: GEM5_READ_REQ,
            address: Some(0x9000),
            size: Some(8),
            packet_id: Some(30),
        },
        PacketFields {
            tick: 1,
            command: GEM5_FLUSH_REQ,
            address: Some(0x9040),
            size: Some(64),
            packet_id: Some(31),
        },
        PacketFields {
            tick: 8,
            command: GEM5_READ_RESP_WITH_INVALIDATE,
            address: Some(0x9000),
            size: Some(8),
            packet_id: Some(30),
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
    let core = endpoint("core0");
    let route = transport
        .add_route(
            MemoryRoute::new(
                core,
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
            req,
            MemoryTrace::new(),
            move |delivery, context| {
                let outcome = traffic_trace_replay_controller_target_outcome(
                    Arc::clone(&replay),
                    Arc::clone(&trace_controller),
                    &delivery,
                    context,
                    0,
                )
                .unwrap();
                assert_eq!(
                    traffic_trace_replay_controller_runtime_sideband_events(
                        Arc::clone(&replay),
                        context.now(),
                        context,
                    ),
                    0
                );
                outcome
            },
            |_| {},
        )
        .unwrap();

    scheduler.run_until_idle_conservative();

    let records = runtime.lock().unwrap().sideband_events().to_vec();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].tick(), 3);
    assert!(matches!(
        records[0].event(),
        TrafficTraceReplaySidebandEvent::Cache(event)
            if event.kind() == TrafficTraceCacheKind::Flush
                && event.address() == Address::new(0x9040)
    ));
    assert!(runtime.lock().unwrap().is_empty());
}

trait TrafficControllerEventTestTick {
    fn tick_for_test(&self) -> u64;
}

impl TrafficControllerEventTestTick for rem6_traffic::TrafficControllerEvent {
    fn tick_for_test(&self) -> u64 {
        match self {
            Self::TraceTlb(event) => event.tick(),
            Self::TraceCache(event) => event.tick(),
            Self::TraceDiagnostic(event) => event.tick(),
            Self::TraceHtm(event) => event.tick(),
            event => panic!("unexpected test event {event:?}"),
        }
    }
}
