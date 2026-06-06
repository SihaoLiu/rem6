use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{
    TrafficGeneratorError, TrafficTrace, TrafficTraceConfig, TrafficTraceGenerator,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_FLAG_PRIVILEGED: u32 = 0x0000_8000;
const GEM5_FLAG_EVICT_NEXT: u32 = 0x0400_0000;
const GEM5_FLAG_SECURE: u32 = 0x1000_0000;
const GEM5_FLAG_PT_WALK: u32 = 0x2000_0000;
const GEM5_FLAG_KERNEL: u32 = 0x0000_1000;
const GEM5_FLAG_NO_ACCESS: u32 = 0x0008_0000;

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
    TrafficTraceConfig::new(AgentId::new(11), line_layout(), 99, trace).unwrap()
}

#[test]
fn trace_traffic_generator_maps_gem5_request_attribute_flags() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 3,
                    command: 1,
                    address: 0x200,
                    size: 8,
                    flags: Some(GEM5_FLAG_PRIVILEGED | GEM5_FLAG_SECURE | GEM5_FLAG_PT_WALK),
                },
                PacketFields {
                    tick: 5,
                    command: 4,
                    address: 0x240,
                    size: 16,
                    flags: Some(GEM5_FLAG_PRIVILEGED | GEM5_FLAG_SECURE),
                },
                PacketFields {
                    tick: 7,
                    command: 1,
                    address: 0x280,
                    size: 8,
                    flags: Some(GEM5_FLAG_EVICT_NEXT),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(70);

    let read = generator.next_request(70, 0).unwrap().unwrap();
    let write = generator.next_request(read.tick(), 0).unwrap().unwrap();
    let eviction_candidate = generator.next_request(write.tick(), 0).unwrap().unwrap();

    assert_eq!(read.tick(), 73);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);
    assert!(read.request().is_privileged());
    assert!(read.request().is_secure());
    assert!(read.request().is_page_table_walk());

    assert_eq!(write.tick(), 75);
    assert_eq!(write.request().operation(), MemoryOperation::Write);
    assert!(write.request().is_privileged());
    assert!(write.request().is_secure());
    assert!(!write.request().is_page_table_walk());
    assert_eq!(write.request().range().start(), Address::new(0x240));
    assert_eq!(write.request().size(), AccessSize::new(16).unwrap());

    assert_eq!(eviction_candidate.tick(), 77);
    assert_eq!(
        eviction_candidate.request().operation(),
        MemoryOperation::ReadShared
    );
    assert!(eviction_candidate.request().is_evict_next());
    assert_eq!(
        eviction_candidate.request().range().start(),
        Address::new(0x280)
    );
}

#[test]
fn trace_parser_still_rejects_gpu_kernel_sync_flag_without_native_event() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 1,
                    address: 0x100,
                    size: 8,
                    flags: Some(GEM5_FLAG_KERNEL),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_FLAG_KERNEL,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_gem5_no_access_flag_to_native_request() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 3,
                    command: 1,
                    address: 0x200,
                    size: 8,
                    flags: Some(GEM5_FLAG_NO_ACCESS | GEM5_FLAG_PRIVILEGED),
                },
                PacketFields {
                    tick: 5,
                    command: 4,
                    address: 0x240,
                    size: 16,
                    flags: Some(GEM5_FLAG_NO_ACCESS | GEM5_FLAG_SECURE),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(70);

    let read = generator.next_request(70, 0).unwrap().unwrap();
    let write = generator.next_request(read.tick(), 0).unwrap().unwrap();

    assert_eq!(read.tick(), 73);
    assert_eq!(read.request().operation(), MemoryOperation::NoAccess);
    assert_eq!(read.request().range().start(), Address::new(0x200));
    assert_eq!(read.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(read.request().data(), None);
    assert_eq!(read.request().byte_mask(), None);
    assert!(read.request().is_privileged());
    assert!(!read.request().requires_writable());
    assert!(read.request().requires_response());
    assert!(!read.request().returns_data());

    assert_eq!(write.tick(), 75);
    assert_eq!(write.request().operation(), MemoryOperation::NoAccess);
    assert_eq!(write.request().range().start(), Address::new(0x240));
    assert_eq!(write.request().size(), AccessSize::new(16).unwrap());
    assert_eq!(write.request().data(), None);
    assert_eq!(write.request().byte_mask(), None);
    assert!(write.request().is_secure());
    assert!(!write.request().requires_writable());
    assert!(write.request().requires_response());
    assert!(!write.request().returns_data());
}

#[test]
fn trace_parser_rejects_no_access_on_maintenance_packets() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 10,
                    address: 0x100,
                    size: 64,
                    flags: Some(GEM5_FLAG_NO_ACCESS),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_FLAG_NO_ACCESS,
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
