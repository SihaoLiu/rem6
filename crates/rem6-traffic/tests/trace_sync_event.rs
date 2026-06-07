use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficGeneratorError,
    TrafficRequestKind, TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId,
    TrafficStateSpec, TrafficTrace, TrafficTraceConfig, TrafficTraceEvent, TrafficTraceGenerator,
    TrafficTraceSyncKind, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_MEM_FENCE_REQ: u32 = 38;
const GEM5_MEM_SYNC_REQ: u32 = 39;
const GEM5_FLAG_PHYSICAL: u32 = 0x0000_0200;
const GEM5_FLAG_KERNEL: u32 = 0x0000_1000;
const GEM5_SYNC_INV_L1: u32 = 0x0000_0001;
const GEM5_SYNC_INV_L2: u32 = 0x0000_0040;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: Option<u64>,
    size: Option<u32>,
    flags: Option<u32>,
    packet_id: Option<u64>,
    pc: Option<u64>,
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn trace_config(trace: TrafficTrace) -> TrafficTraceConfig {
    TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap()
}

fn sync_trace() -> TrafficTrace {
    TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 5,
                    command: GEM5_READ_REQ,
                    address: Some(0x20),
                    size: Some(8),
                    flags: None,
                    packet_id: Some(1),
                    pc: Some(0x1000),
                },
                PacketFields {
                    tick: 7,
                    command: GEM5_MEM_FENCE_REQ,
                    address: None,
                    size: None,
                    flags: None,
                    packet_id: Some(2),
                    pc: Some(0x1004),
                },
                PacketFields {
                    tick: 9,
                    command: GEM5_MEM_SYNC_REQ,
                    address: None,
                    size: None,
                    flags: None,
                    packet_id: Some(3),
                    pc: Some(0x1008),
                },
                PacketFields {
                    tick: 13,
                    command: GEM5_WRITE_REQ,
                    address: Some(0x30),
                    size: Some(4),
                    flags: None,
                    packet_id: Some(4),
                    pc: Some(0x100c),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap()
}

#[test]
fn trace_generator_emits_mem_fence_and_mem_sync_events() {
    let mut generator = TrafficTraceGenerator::new(trace_config(sync_trace()));
    generator.enter(100);

    assert_eq!(generator.schedule_tick(100, 0).unwrap(), 105);
    let read = match generator.next_event(100, 0).unwrap().unwrap() {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("read packet should emit a request event"),
    };
    assert_eq!(read.tick(), 105);
    assert_eq!(read.sequence(), 0);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(read.trace_packet_id(), Some(1));
    assert_eq!(read.trace_pc(), Some(Address::new(0x1000)));

    assert_eq!(generator.schedule_tick(read.tick(), 0).unwrap(), 107);
    let fence = match generator.next_event(read.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Sync(event) => event,
        _ => panic!("MemFenceReq should emit a sync event"),
    };
    assert_eq!(fence.tick(), 107);
    assert_eq!(fence.sequence(), 1);
    assert_eq!(fence.kind(), TrafficTraceSyncKind::MemFence);
    assert!(!fence.kernel_sync());
    assert!(fence.is_request());
    assert!(fence.requires_response());
    assert_eq!(fence.trace_packet_id(), Some(2));
    assert_eq!(fence.trace_pc(), Some(Address::new(0x1004)));

    assert_eq!(generator.schedule_tick(fence.tick(), 0).unwrap(), 109);
    let sync = match generator.next_event(fence.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Sync(event) => event,
        _ => panic!("MemSyncReq should emit a sync event"),
    };
    assert_eq!(sync.tick(), 109);
    assert_eq!(sync.sequence(), 2);
    assert_eq!(sync.kind(), TrafficTraceSyncKind::MemSync);
    assert!(!sync.kernel_sync());
    assert!(sync.is_request());
    assert!(sync.requires_response());
    assert_eq!(sync.trace_packet_id(), Some(3));
    assert_eq!(sync.trace_pc(), Some(Address::new(0x1008)));

    assert_eq!(generator.schedule_tick(sync.tick(), 0).unwrap(), 113);
    let write = match generator.next_event(sync.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("write packet should emit a request event"),
    };
    assert_eq!(write.tick(), 113);
    assert_eq!(write.sequence(), 3);
    assert_eq!(write.kind(), TrafficRequestKind::Write);
    assert_eq!(write.request().operation(), MemoryOperation::Write);
    assert_eq!(write.trace_packet_id(), Some(4));
    assert_eq!(write.trace_pc(), Some(Address::new(0x100c)));

    assert_eq!(generator.schedule_tick(write.tick(), 0).unwrap(), u64::MAX);
    assert_eq!(generator.summary().packet_count(), 4);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 4);
    assert_eq!(generator.summary().first_tick(), Some(105));
    assert_eq!(generator.summary().last_tick(), Some(113));
}

#[test]
fn trace_sync_kind_preserves_gem5_request_response_policy() {
    for kind in [
        TrafficTraceSyncKind::MemFence,
        TrafficTraceSyncKind::MemSync,
    ] {
        assert!(
            kind.is_request(),
            "{} request policy should match gem5",
            kind.gem5_name()
        );
        assert!(
            kind.requires_response(),
            "{} response policy should match gem5",
            kind.gem5_name()
        );
    }
}

#[test]
fn trace_generator_next_request_reports_sync_event_boundary() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_MEM_FENCE_REQ,
                address: None,
                size: None,
                flags: None,
                packet_id: None,
                pc: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(0);

    assert_eq!(
        generator.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::TraceSyncEventRequiresNextEvent {
            command: "MemFenceReq",
        }
    );

    let sync = match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Sync(event) => event,
        _ => panic!("MemFenceReq should remain pending"),
    };
    assert_eq!(sync.tick(), 5);
    assert_eq!(sync.sequence(), 0);
    assert_eq!(sync.kind(), TrafficTraceSyncKind::MemFence);
    assert!(!sync.kernel_sync());
}

#[test]
fn trace_generator_accepts_probe_addr_size_on_sync_packets() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 11,
                command: GEM5_MEM_SYNC_REQ,
                address: Some(0),
                size: Some(0),
                flags: Some(GEM5_FLAG_KERNEL),
                packet_id: Some(12),
                pc: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let sync = match generator.next_event(20, 0).unwrap().unwrap() {
        TrafficTraceEvent::Sync(event) => event,
        _ => panic!("MemSyncReq should emit a sync event"),
    };

    assert_eq!(sync.tick(), 31);
    assert_eq!(sync.sequence(), 0);
    assert_eq!(sync.kind(), TrafficTraceSyncKind::MemSync);
    assert!(sync.kernel_sync());
    assert_eq!(sync.trace_packet_id(), Some(12));
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 0);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_read(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_generator_preserves_mem_sync_cache_invalidation_policy() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 11,
                command: GEM5_MEM_SYNC_REQ,
                address: None,
                size: None,
                flags: Some(GEM5_FLAG_KERNEL | GEM5_SYNC_INV_L1),
                packet_id: Some(13),
                pc: Some(0x2000),
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let l1_sync = match generator.next_event(20, 0).unwrap().unwrap() {
        TrafficTraceEvent::Sync(event) => event,
        _ => panic!("MemSyncReq should emit a sync event"),
    };
    assert_eq!(l1_sync.tick(), 31);
    assert_eq!(l1_sync.kind(), TrafficTraceSyncKind::MemSync);
    assert!(l1_sync.kernel_sync());
    assert!(l1_sync.invalidates_l1());
    assert_eq!(l1_sync.trace_packet_id(), Some(13));
    assert_eq!(l1_sync.trace_pc(), Some(Address::new(0x2000)));
}

#[test]
fn trace_parser_rejects_non_kernel_sync_flags() {
    for flags in [GEM5_FLAG_PHYSICAL, GEM5_SYNC_INV_L2] {
        assert_eq!(
            TrafficTrace::from_gem5_packet_trace(
                &gem5_packet_trace(
                    TICK_FREQUENCY,
                    &[PacketFields {
                        tick: 11,
                        command: GEM5_MEM_SYNC_REQ,
                        address: Some(0),
                        size: Some(0),
                        flags: Some(flags),
                        packet_id: None,
                        pc: None,
                    }],
                ),
                TICK_FREQUENCY,
            )
            .unwrap_err(),
            TrafficGeneratorError::TraceUnsupportedFlags { flags }
        );
    }
}

#[test]
fn traffic_controller_emits_trace_sync_event() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 5,
                    command: GEM5_MEM_FENCE_REQ,
                    address: None,
                    size: None,
                    flags: None,
                    packet_id: Some(9),
                    pc: None,
                },
                PacketFields {
                    tick: 8,
                    command: GEM5_READ_REQ,
                    address: Some(0x80),
                    size: Some(8),
                    flags: None,
                    packet_id: None,
                    pc: None,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap();
    let controller_config = TrafficControllerConfig::new(
        graph(vec![state(0, u64::MAX)], vec![transition(0, 0)]),
        vec![TrafficControllerState::new(
            TrafficStateId::new(0),
            TrafficStateGenerator::Trace(TrafficTraceGenerator::new(config)),
        )],
    )
    .unwrap();
    let mut controller = TrafficController::new(controller_config);

    assert!(controller.start(20).unwrap().is_empty());
    let sync_batch = controller.next_event(20, 0).unwrap().unwrap();
    assert!(sync_batch.request().is_none());
    let sync = sync_batch.trace_sync().unwrap();
    assert_eq!(sync.tick(), 25);
    assert_eq!(sync.sequence(), 0);
    assert_eq!(sync.kind(), TrafficTraceSyncKind::MemFence);
    assert_eq!(sync.trace_packet_id(), Some(9));

    let request_batch = controller.next_event(sync.tick(), 0).unwrap().unwrap();
    let request = request_batch.request().unwrap();
    assert_eq!(request.tick(), 28);
    assert_eq!(request.sequence(), 1);
    assert_eq!(request.address(), Address::new(0x80));
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
        if let Some(flags) = packet.flags {
            append_key(&mut message, 5, 0);
            append_varint(&mut message, u64::from(flags));
        }
        if let Some(packet_id) = packet.packet_id {
            append_key(&mut message, 6, 0);
            append_varint(&mut message, packet_id);
        }
        if let Some(pc) = packet.pc {
            append_key(&mut message, 7, 0);
            append_varint(&mut message, pc);
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
