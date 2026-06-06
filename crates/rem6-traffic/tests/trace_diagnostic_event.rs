use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficGeneratorError,
    TrafficRequestKind, TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId,
    TrafficStateSpec, TrafficTrace, TrafficTraceConfig, TrafficTraceDiagnosticKind,
    TrafficTraceEvent, TrafficTraceGenerator, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_PRINT_REQ: u32 = 52;
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

fn diagnostic_trace() -> TrafficTrace {
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
                    command: GEM5_PRINT_REQ,
                    address: Some(0x4000),
                    size: Some(1),
                    flags: None,
                    packet_id: Some(2),
                    pc: Some(0x1004),
                },
                PacketFields {
                    tick: 9,
                    command: GEM5_PRINT_REQ,
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
fn trace_generator_emits_print_req_diagnostic_events() {
    let mut generator = TrafficTraceGenerator::new(trace_config(diagnostic_trace()));
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
    let print = match generator.next_event(read.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Diagnostic(event) => event,
        _ => panic!("PrintReq should emit a diagnostic event"),
    };
    assert_eq!(print.tick(), 107);
    assert_eq!(print.sequence(), 1);
    assert_eq!(print.kind(), TrafficTraceDiagnosticKind::Print);
    assert_eq!(print.address(), Some(Address::new(0x4000)));
    assert_eq!(print.size_bytes(), Some(1));
    assert!(print.is_print());
    assert_eq!(print.trace_packet_id(), Some(2));
    assert_eq!(print.trace_pc(), Some(Address::new(0x1004)));

    assert_eq!(generator.schedule_tick(print.tick(), 0).unwrap(), 109);
    let print_without_probe = match generator.next_event(print.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Diagnostic(event) => event,
        _ => panic!("PrintReq should emit a diagnostic event"),
    };
    assert_eq!(print_without_probe.tick(), 109);
    assert_eq!(print_without_probe.sequence(), 2);
    assert_eq!(
        print_without_probe.kind(),
        TrafficTraceDiagnosticKind::Print
    );
    assert_eq!(print_without_probe.address(), None);
    assert_eq!(print_without_probe.size_bytes(), None);
    assert!(print_without_probe.is_print());
    assert_eq!(print_without_probe.trace_packet_id(), Some(3));
    assert_eq!(print_without_probe.trace_pc(), Some(Address::new(0x1008)));

    assert_eq!(
        generator
            .schedule_tick(print_without_probe.tick(), 0)
            .unwrap(),
        113
    );
    let write = match generator
        .next_event(print_without_probe.tick(), 0)
        .unwrap()
        .unwrap()
    {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("write packet should emit a request event"),
    };
    assert_eq!(write.tick(), 113);
    assert_eq!(write.sequence(), 3);
    assert_eq!(write.kind(), TrafficRequestKind::Write);
    assert_eq!(write.request().operation(), MemoryOperation::Write);

    assert_eq!(generator.summary().packet_count(), 4);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 4);
    assert_eq!(generator.summary().first_tick(), Some(105));
    assert_eq!(generator.summary().last_tick(), Some(113));
}

#[test]
fn trace_diagnostic_kind_preserves_gem5_print_policy() {
    assert!(
        TrafficTraceDiagnosticKind::Print.is_print(),
        "{} print policy should match gem5",
        TrafficTraceDiagnosticKind::Print.gem5_name()
    );
}

#[test]
fn trace_generator_next_request_reports_diagnostic_event_boundary() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_PRINT_REQ,
                address: Some(0x4000),
                size: Some(1),
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
        TrafficGeneratorError::TraceDiagnosticEventRequiresNextEvent {
            command: "PrintReq",
        }
    );

    let diagnostic = match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Diagnostic(event) => event,
        _ => panic!("PrintReq should remain pending"),
    };
    assert_eq!(diagnostic.tick(), 5);
    assert_eq!(diagnostic.sequence(), 0);
    assert_eq!(diagnostic.kind(), TrafficTraceDiagnosticKind::Print);
}

#[test]
fn trace_parser_rejects_print_req_flags() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 5,
                    command: GEM5_PRINT_REQ,
                    address: Some(0x4000),
                    size: Some(1),
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
fn traffic_controller_emits_trace_diagnostic_event() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_PRINT_REQ,
                address: Some(0x4000),
                size: Some(1),
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
    let diagnostic = batch.trace_diagnostic().unwrap();
    assert_eq!(diagnostic.tick(), 25);
    assert_eq!(diagnostic.sequence(), 0);
    assert_eq!(diagnostic.kind(), TrafficTraceDiagnosticKind::Print);
    assert_eq!(diagnostic.address(), Some(Address::new(0x4000)));
    assert_eq!(diagnostic.size_bytes(), Some(1));
    assert_eq!(diagnostic.trace_packet_id(), Some(9));
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
