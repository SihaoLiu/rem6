use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{TrafficTrace, TrafficTraceConfig, TrafficTraceGenerator};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: u64,
    size: u32,
    pkt_id: Option<u64>,
    pc: Option<u64>,
}

fn trace_config(trace: TrafficTrace) -> TrafficTraceConfig {
    TrafficTraceConfig::new(
        AgentId::new(17),
        CacheLineLayout::new(64).unwrap(),
        99,
        trace,
    )
    .unwrap()
}

#[test]
fn trace_request_events_preserve_gem5_packet_id_and_pc_metadata() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 3,
                    command: 1,
                    address: 0x200,
                    size: 8,
                    pkt_id: Some(77),
                    pc: Some(0x4000),
                },
                PacketFields {
                    tick: 5,
                    command: 4,
                    address: 0x240,
                    size: 4,
                    pkt_id: None,
                    pc: None,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let read = generator.next_request(20, 0).unwrap().unwrap();
    let write = generator.next_request(read.tick(), 0).unwrap().unwrap();

    assert_eq!(read.tick(), 23);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(read.trace_packet_id(), Some(77));
    assert_eq!(read.trace_pc(), Some(Address::new(0x4000)));

    assert_eq!(write.tick(), 25);
    assert_eq!(write.request().operation(), MemoryOperation::Write);
    assert_eq!(write.trace_packet_id(), None);
    assert_eq!(write.trace_pc(), None);
}

#[test]
fn trace_snapshot_restore_preserves_pending_packet_metadata() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 1,
                    command: 1,
                    address: 0x100,
                    size: 8,
                    pkt_id: Some(1),
                    pc: Some(0x1000),
                },
                PacketFields {
                    tick: 4,
                    command: 1,
                    address: 0x140,
                    size: 8,
                    pkt_id: Some(2),
                    pc: Some(0x1400),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(10);

    let first = generator.next_request(10, 0).unwrap().unwrap();
    let snapshot = generator.snapshot();
    let mut restored = TrafficTraceGenerator::restore(snapshot).unwrap();

    let second = restored.next_request(first.tick(), 0).unwrap().unwrap();

    assert_eq!(second.tick(), 14);
    assert_eq!(second.trace_packet_id(), Some(2));
    assert_eq!(second.trace_pc(), Some(Address::new(0x1400)));
}

#[test]
fn trace_preserves_gem5_packet_header_metadata() {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&GEM5_MAGIC);
    push_message(
        &mut bytes,
        &[
            field_length(1, b"system.cpu.trace"),
            field_varint(2, 7),
            field_varint(3, TICK_FREQUENCY),
            id_string_entry(3, b"cpu0"),
            id_string_entry(9, b"dma"),
        ],
    );
    push_message(
        &mut bytes,
        &[
            field_varint(1, 1),
            field_varint(2, 1),
            field_varint(3, 0x100),
            field_varint(4, 8),
        ],
    );

    let trace = TrafficTrace::from_gem5_packet_trace(&bytes, TICK_FREQUENCY).unwrap();

    assert_eq!(trace.object_id(), Some("system.cpu.trace"));
    assert_eq!(trace.header_version(), 7);
    assert_eq!(trace.id_strings().len(), 2);
    assert_eq!(trace.id_strings()[0].key(), 3);
    assert_eq!(trace.id_strings()[0].value(), "cpu0");
    assert_eq!(trace.id_strings()[1].key(), 9);
    assert_eq!(trace.id_strings()[1].value(), "dma");
    assert_eq!(trace.id_string(9), Some("dma"));
    assert_eq!(trace.id_string(4), None);
}

fn gem5_packet_trace(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&GEM5_MAGIC);
    push_message(
        &mut bytes,
        &[
            field_length(1, b"rem6-test"),
            field_varint(2, 0),
            field_varint(3, tick_frequency),
        ],
    );
    for packet in packets {
        let mut fields = vec![
            field_varint(1, packet.tick),
            field_varint(2, u64::from(packet.command)),
            field_varint(3, packet.address),
            field_varint(4, u64::from(packet.size)),
        ];
        if let Some(pkt_id) = packet.pkt_id {
            fields.push(field_varint(6, pkt_id));
        }
        if let Some(pc) = packet.pc {
            fields.push(field_varint(7, pc));
        }
        push_message(&mut bytes, &fields);
    }
    bytes
}

fn push_message(bytes: &mut Vec<u8>, fields: &[Vec<u8>]) {
    let payload: Vec<u8> = fields.iter().flatten().copied().collect();
    push_varint(bytes, payload.len() as u64);
    bytes.extend_from_slice(&payload);
}

fn field_varint(field: u64, value: u64) -> Vec<u8> {
    let mut out = Vec::new();
    push_varint(&mut out, field << 3);
    push_varint(&mut out, value);
    out
}

fn field_length(field: u64, value: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    push_varint(&mut out, (field << 3) | 2);
    push_varint(&mut out, value.len() as u64);
    out.extend_from_slice(value);
    out
}

fn id_string_entry(key: u32, value: &[u8]) -> Vec<u8> {
    let fields = [field_varint(1, u64::from(key)), field_length(2, value)];
    field_length(4, &fields.iter().flatten().copied().collect::<Vec<_>>())
}

fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}
