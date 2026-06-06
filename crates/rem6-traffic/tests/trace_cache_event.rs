use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficGeneratorError,
    TrafficRequestKind, TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId,
    TrafficStateSpec, TrafficTrace, TrafficTraceCacheKind, TrafficTraceConfig, TrafficTraceEvent,
    TrafficTraceGenerator, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_FLUSH_REQ: u32 = 53;
const GEM5_FLAG_PHYSICAL: u32 = 0x0000_0200;

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

fn cache_trace() -> TrafficTrace {
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
                    command: GEM5_FLUSH_REQ,
                    address: Some(0x4000),
                    size: Some(64),
                    flags: None,
                    packet_id: Some(2),
                    pc: Some(0x1004),
                },
                PacketFields {
                    tick: 13,
                    command: GEM5_WRITE_REQ,
                    address: Some(0x30),
                    size: Some(4),
                    flags: None,
                    packet_id: Some(3),
                    pc: Some(0x100c),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap()
}

#[test]
fn trace_generator_emits_flush_req_cache_event() {
    let mut generator = TrafficTraceGenerator::new(trace_config(cache_trace()));
    generator.enter(100);

    let read = match generator.next_event(100, 0).unwrap().unwrap() {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("read packet should emit a request event"),
    };
    assert_eq!(read.tick(), 105);
    assert_eq!(read.sequence(), 0);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);

    assert_eq!(generator.schedule_tick(read.tick(), 0).unwrap(), 107);
    let flush = match generator.next_event(read.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Cache(event) => event,
        _ => panic!("FlushReq should emit a cache event"),
    };
    assert_eq!(flush.tick(), 107);
    assert_eq!(flush.sequence(), 1);
    assert_eq!(flush.kind(), TrafficTraceCacheKind::Flush);
    assert_eq!(flush.address(), Address::new(0x4000));
    assert_eq!(flush.size_bytes(), 64);
    assert_eq!(flush.trace_packet_id(), Some(2));
    assert_eq!(flush.trace_pc(), Some(Address::new(0x1004)));

    assert_eq!(generator.schedule_tick(flush.tick(), 0).unwrap(), 113);
    let write = match generator.next_event(flush.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("write packet should emit a request event"),
    };
    assert_eq!(write.tick(), 113);
    assert_eq!(write.sequence(), 2);
    assert_eq!(write.kind(), TrafficRequestKind::Write);
    assert_eq!(write.request().operation(), MemoryOperation::Write);

    assert_eq!(generator.summary().packet_count(), 3);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 4);
    assert_eq!(generator.summary().first_tick(), Some(105));
    assert_eq!(generator.summary().last_tick(), Some(113));
}

#[test]
fn trace_generator_next_request_reports_cache_event_boundary() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_FLUSH_REQ,
                address: Some(0x4000),
                size: Some(64),
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
        TrafficGeneratorError::TraceCacheEventRequiresNextEvent {
            command: "FlushReq",
        }
    );

    let flush = match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Cache(event) => event,
        _ => panic!("FlushReq should remain pending"),
    };
    assert_eq!(flush.tick(), 5);
    assert_eq!(flush.sequence(), 0);
    assert_eq!(flush.kind(), TrafficTraceCacheKind::Flush);
    assert_eq!(flush.address(), Address::new(0x4000));
    assert_eq!(flush.size_bytes(), 64);
}

#[test]
fn trace_generator_applies_addr_offset_to_flush_req_cache_event() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_FLUSH_REQ,
                address: Some(0x4000),
                size: Some(64),
                flags: None,
                packet_id: None,
                pc: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(0x40).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(0);

    let flush = match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Cache(event) => event,
        _ => panic!("FlushReq should emit a cache event"),
    };
    assert_eq!(flush.tick(), 5);
    assert_eq!(flush.sequence(), 0);
    assert_eq!(flush.kind(), TrafficTraceCacheKind::Flush);
    assert_eq!(flush.address(), Address::new(0x4040));
    assert_eq!(flush.size_bytes(), 64);
}

#[test]
fn trace_generator_rejects_flush_req_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_FLUSH_REQ,
                address: Some(0x4000),
                size: Some(32),
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
        generator.next_event(0, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheEventSizeMismatch {
            command: "FlushReq",
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_generator_rejects_flush_req_unaligned_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_FLUSH_REQ,
                address: Some(0x4000),
                size: Some(64),
                flags: None,
                packet_id: None,
                pc: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(8).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(0);

    assert_eq!(
        generator.next_event(0, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheEventUnalignedAddress {
            command: "FlushReq",
            address: Address::new(0x4008),
            line_size: 64,
        }
    );
}

#[test]
fn trace_parser_rejects_flush_req_flags() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 5,
                    command: GEM5_FLUSH_REQ,
                    address: Some(0x4000),
                    size: Some(64),
                    flags: Some(GEM5_FLAG_PHYSICAL),
                    packet_id: None,
                    pc: None,
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_FLAG_PHYSICAL,
        }
    );
}

#[test]
fn traffic_controller_emits_trace_cache_event() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_FLUSH_REQ,
                address: Some(0x4000),
                size: Some(64),
                flags: None,
                packet_id: Some(9),
                pc: None,
            }],
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
    let batch = controller.next_event(20, 0).unwrap().unwrap();
    assert!(batch.request().is_none());
    assert!(batch.trace_sync().is_none());
    assert!(batch.trace_tlb().is_none());
    assert!(batch.trace_htm().is_none());
    assert!(batch.trace_diagnostic().is_none());
    let flush = batch.trace_cache().unwrap();
    assert_eq!(flush.tick(), 25);
    assert_eq!(flush.sequence(), 0);
    assert_eq!(flush.kind(), TrafficTraceCacheKind::Flush);
    assert_eq!(flush.address(), Address::new(0x4000));
    assert_eq!(flush.size_bytes(), 64);
    assert_eq!(flush.trace_packet_id(), Some(9));
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
