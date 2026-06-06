use std::sync::{Arc, Mutex};

use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{Address, AgentId, CacheLineLayout};
use rem6_system::{
    traffic_trace_replay_controller_runtime_sideband_events,
    traffic_trace_replay_controller_target_outcome, traffic_trace_replay_runtime_sideband_events,
    TrafficTraceReplayControllerRuntime, TrafficTraceReplaySidebandEvent,
    TrafficTraceReplaySidebandRuntime,
};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficStateGenerator,
    TrafficStateGraphConfig, TrafficStateId, TrafficStateSpec, TrafficTrace, TrafficTraceCacheKind,
    TrafficTraceConfig, TrafficTraceDiagnosticKind, TrafficTraceHtmKind, TrafficTraceTlbKind,
    TrafficTransition, TrafficTransitionProbability, TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTransport, ResponseDelivery, TransportEndpointId,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_READ_RESP_WITH_INVALIDATE: u32 = 3;
const GEM5_PRINT_REQ: u32 = 52;
const GEM5_FLUSH_REQ: u32 = 53;
const GEM5_HTM_ABORT: u32 = 58;
const GEM5_TLBI_EXT_SYNC: u32 = 59;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    packet_id: Option<u64>,
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn endpoint(name: &str) -> TransportEndpointId {
    TransportEndpointId::new(name).unwrap()
}

fn controller_for_packets(packets: &[PacketFields]) -> TrafficController {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(TICK_FREQUENCY, packets),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, u64::MAX)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(rem6_traffic::TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    TrafficController::new(controller_config)
}

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
                    1
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
                    1
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
