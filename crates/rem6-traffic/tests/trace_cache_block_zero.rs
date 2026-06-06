use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficGeneratorError, TrafficTrace, TrafficTraceConfig, TrafficTraceGenerator,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_FLAG_CACHE_BLOCK_ZERO: u32 = 0x0001_0000;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: u64,
    size: u32,
    flags: Option<u32>,
}

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn trace_config(trace: TrafficTrace) -> TrafficTraceConfig {
    TrafficTraceConfig::new(AgentId::new(9), line_layout(), 99, trace).unwrap()
}

#[test]
fn trace_traffic_generator_maps_cache_block_zero_flag_to_native_request() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 4,
                address: 0x200,
                size: 64,
                flags: Some(GEM5_FLAG_CACHE_BLOCK_ZERO),
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(70);

    let event = generator.next_request(70, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 73);
    assert_eq!(event.request().operation(), MemoryOperation::CacheBlockZero);
    assert_eq!(event.request().range().start(), Address::new(0x200));
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert!(event.request().requires_writable());
    assert!(event.request().requires_response());
    assert!(!event.request().returns_data());
}

#[test]
fn trace_generator_rejects_cache_block_zero_with_non_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 4,
                address: 0x200,
                size: 32,
                flags: Some(GEM5_FLAG_CACHE_BLOCK_ZERO),
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(70);

    assert_eq!(
        generator.next_request(70, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheBlockZeroSizeMismatch {
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_generator_rejects_cache_block_zero_with_unaligned_address() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 4,
                address: 0x204,
                size: 64,
                flags: Some(GEM5_FLAG_CACHE_BLOCK_ZERO),
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(70);

    assert_eq!(
        generator.next_request(70, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheBlockZeroUnalignedAddress {
            address: Address::new(0x204),
            line_size: 64,
        }
    );
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
        if let Some(flags) = packet.flags {
            fields.push(field_varint(5, u64::from(flags)));
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

fn push_varint(out: &mut Vec<u8>, mut value: u64) {
    while value >= 0x80 {
        out.push((value as u8) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}
