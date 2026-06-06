use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficController, TrafficControllerConfig, TrafficControllerState, TrafficGeneratorError,
    TrafficRequestKind, TrafficStateGenerator, TrafficStateGraphConfig, TrafficStateId,
    TrafficStateSpec, TrafficTrace, TrafficTraceConfig, TrafficTraceErrorKind, TrafficTraceEvent,
    TrafficTraceGenerator, TrafficTransition, TrafficTransitionProbability,
    TRAFFIC_TRANSITION_PROBABILITY_SCALE,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_READ_REQ: u32 = 1;
const GEM5_WRITE_REQ: u32 = 4;
const GEM5_INVALID_DEST_ERROR: u32 = 46;
const GEM5_BAD_ADDRESS_ERROR: u32 = 47;
const GEM5_READ_ERROR: u32 = 48;
const GEM5_WRITE_ERROR: u32 = 49;
const GEM5_FUNCTIONAL_READ_ERROR: u32 = 50;
const GEM5_FUNCTIONAL_WRITE_ERROR: u32 = 51;
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

#[test]
fn trace_generator_emits_error_response_events() {
    let trace = TrafficTrace::from_gem5_packet_trace(
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
                    command: GEM5_BAD_ADDRESS_ERROR,
                    address: Some(0x4000),
                    size: Some(8),
                    flags: None,
                    packet_id: Some(2),
                    pc: Some(0x1004),
                },
                PacketFields {
                    tick: 9,
                    command: GEM5_FUNCTIONAL_WRITE_ERROR,
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
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(100);

    let read = match generator.next_event(100, 0).unwrap().unwrap() {
        TrafficTraceEvent::Request(event) => event,
        _ => panic!("read packet should emit a request event"),
    };
    assert_eq!(read.tick(), 105);
    assert_eq!(read.sequence(), 0);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);

    let bad_address = match generator.next_event(read.tick(), 0).unwrap().unwrap() {
        TrafficTraceEvent::Error(event) => event,
        _ => panic!("BadAddressError should emit an error event"),
    };
    assert_eq!(bad_address.tick(), 107);
    assert_eq!(bad_address.sequence(), 1);
    assert_eq!(bad_address.kind(), TrafficTraceErrorKind::BadAddress);
    assert_eq!(bad_address.address(), Some(Address::new(0x4000)));
    assert_eq!(bad_address.size_bytes(), Some(8));
    assert_eq!(bad_address.trace_packet_id(), Some(2));
    assert_eq!(bad_address.trace_pc(), Some(Address::new(0x1004)));

    let functional_write = match generator
        .next_event(bad_address.tick(), 0)
        .unwrap()
        .unwrap()
    {
        TrafficTraceEvent::Error(event) => event,
        _ => panic!("FunctionalWriteError should emit an error event"),
    };
    assert_eq!(functional_write.tick(), 109);
    assert_eq!(functional_write.sequence(), 2);
    assert_eq!(
        functional_write.kind(),
        TrafficTraceErrorKind::FunctionalWrite
    );
    assert_eq!(functional_write.address(), None);
    assert_eq!(functional_write.size_bytes(), None);
    assert_eq!(functional_write.trace_packet_id(), Some(3));
    assert_eq!(functional_write.trace_pc(), Some(Address::new(0x1008)));

    let write = match generator
        .next_event(functional_write.tick(), 0)
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
fn trace_generator_next_request_reports_error_event_boundary() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_READ_ERROR,
                address: Some(0x4000),
                size: Some(8),
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
        TrafficGeneratorError::TraceErrorEventRequiresNextEvent {
            command: "ReadError",
        }
    );

    let error = match generator.next_event(0, 0).unwrap().unwrap() {
        TrafficTraceEvent::Error(event) => event,
        _ => panic!("ReadError should remain pending"),
    };
    assert_eq!(error.tick(), 5);
    assert_eq!(error.sequence(), 0);
    assert_eq!(error.kind(), TrafficTraceErrorKind::Read);
    assert_eq!(error.address(), Some(Address::new(0x4000)));
    assert_eq!(error.size_bytes(), Some(8));
}

#[test]
fn trace_parser_rejects_error_response_flags() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 5,
                    command: GEM5_WRITE_ERROR,
                    address: Some(0x4000),
                    size: Some(8),
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
fn traffic_controller_emits_trace_error_event() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: GEM5_INVALID_DEST_ERROR,
                address: Some(0x4000),
                size: Some(8),
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
    assert!(batch.trace_cache().is_none());
    assert!(batch.trace_htm().is_none());
    assert!(batch.trace_diagnostic().is_none());
    let error = batch.trace_error().unwrap();
    assert_eq!(error.tick(), 25);
    assert_eq!(error.sequence(), 0);
    assert_eq!(error.kind(), TrafficTraceErrorKind::InvalidDestination);
    assert_eq!(error.address(), Some(Address::new(0x4000)));
    assert_eq!(error.size_bytes(), Some(8));
    assert_eq!(error.trace_packet_id(), Some(9));
}

#[test]
fn trace_generator_maps_all_gem5_error_response_kinds() {
    let commands = [
        GEM5_INVALID_DEST_ERROR,
        GEM5_BAD_ADDRESS_ERROR,
        GEM5_READ_ERROR,
        GEM5_WRITE_ERROR,
        GEM5_FUNCTIONAL_READ_ERROR,
        GEM5_FUNCTIONAL_WRITE_ERROR,
    ];
    let packets = commands
        .into_iter()
        .enumerate()
        .map(|(index, command)| PacketFields {
            tick: u64::try_from(index).unwrap() + 1,
            command,
            address: None,
            size: None,
            flags: None,
            packet_id: None,
            pc: None,
        })
        .collect::<Vec<_>>();
    let trace =
        TrafficTrace::from_gem5_packet_trace(&gem5_packet_trace(TICK_FREQUENCY, &packets), 1_000)
            .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(0);

    let kinds = (0..commands.len())
        .map(|_| match generator.next_event(0, 0).unwrap().unwrap() {
            TrafficTraceEvent::Error(event) => event.kind(),
            _ => panic!("gem5 error command should emit an error event"),
        })
        .collect::<Vec<_>>();

    assert_eq!(
        kinds,
        vec![
            TrafficTraceErrorKind::InvalidDestination,
            TrafficTraceErrorKind::BadAddress,
            TrafficTraceErrorKind::Read,
            TrafficTraceErrorKind::Write,
            TrafficTraceErrorKind::FunctionalRead,
            TrafficTraceErrorKind::FunctionalWrite,
        ]
    );
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
