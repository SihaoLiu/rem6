use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryAccessOrdering, MemoryBarrierSet,
    MemoryOperation,
};
use rem6_traffic::{
    TrafficGeneratorError, TrafficRequestKind, TrafficTrace, TrafficTraceConfig,
    TrafficTraceGenerator,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_UNSUPPORTED_FLAG: u32 = 0x0000_0010;
const GEM5_FLAG_INST_FETCH: u32 = 0x0000_0100;
const GEM5_FLAG_PHYSICAL: u32 = 0x0000_0200;
const GEM5_FLAG_UNCACHEABLE: u32 = 0x0000_0400;
const GEM5_FLAG_STRICT_ORDER: u32 = 0x0000_0800;
const GEM5_FLAG_ACQUIRE_PC: u32 = 0x0000_2000;
const GEM5_FLAG_PRIVILEGED: u32 = 0x0000_8000;
const GEM5_FLAG_ACQUIRE: u32 = 0x0002_0000;
const GEM5_FLAG_RELEASE: u32 = 0x0004_0000;
const GEM5_FLAG_EVICT_NEXT: u32 = 0x0400_0000;
const GEM5_FLAG_SECURE: u32 = 0x1000_0000;
const GEM5_FLAG_PT_WALK: u32 = 0x2000_0000;
const GEM5_FLAG_KERNEL: u32 = 0x0000_1000;
const GEM5_FLAG_NO_ACCESS: u32 = 0x0008_0000;
const GEM5_FLAG_PREFETCH: u32 = 0x0100_0000;
const GEM5_FLAG_PF_EXCLUSIVE: u32 = 0x0200_0000;

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
                    flags: Some(GEM5_FLAG_EVICT_NEXT | GEM5_FLAG_KERNEL),
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
    assert!(eviction_candidate.request().is_kernel_sync());
}

#[test]
fn trace_traffic_generator_maps_gem5_kernel_sync_flag_to_native_attribute() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 1,
                    command: 1,
                    address: 0x100,
                    size: 8,
                    flags: Some(GEM5_FLAG_KERNEL),
                },
                PacketFields {
                    tick: 2,
                    command: 4,
                    address: 0x140,
                    size: 4,
                    flags: Some(GEM5_FLAG_KERNEL | GEM5_FLAG_NO_ACCESS),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let read = generator.next_request(20, 0).unwrap().unwrap();
    let no_access = generator.next_request(read.tick(), 0).unwrap().unwrap();

    assert_eq!(read.tick(), 21);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);
    assert!(read.request().is_kernel_sync());

    assert_eq!(no_access.tick(), 22);
    assert_eq!(no_access.request().operation(), MemoryOperation::NoAccess);
    assert!(no_access.request().is_kernel_sync());
}

#[test]
fn trace_traffic_generator_maps_gem5_ordering_flags() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 2,
                    command: 4,
                    address: 0x40,
                    size: 8,
                    flags: Some(GEM5_FLAG_UNCACHEABLE | GEM5_FLAG_STRICT_ORDER | GEM5_FLAG_RELEASE),
                },
                PacketFields {
                    tick: 7,
                    command: 1,
                    address: 0x80,
                    size: 16,
                    flags: Some(GEM5_FLAG_ACQUIRE),
                },
                PacketFields {
                    tick: 11,
                    command: 1,
                    address: 0xc0,
                    size: 4,
                    flags: Some(GEM5_FLAG_UNCACHEABLE),
                },
                PacketFields {
                    tick: 15,
                    command: 1,
                    address: 0x100,
                    size: 8,
                    flags: Some(GEM5_FLAG_ACQUIRE_PC),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(10);

    let release = generator.next_request(10, 0).unwrap().unwrap();
    assert_eq!(release.tick(), 12);
    assert_eq!(release.request().operation(), MemoryOperation::Write);
    assert!(release.request().is_uncacheable());
    assert!(release.request().is_strict_ordered());
    assert_eq!(
        release.request().ordering(),
        MemoryAccessOrdering::new(Some(MemoryBarrierSet::memory()), None)
    );

    let acquire = generator.next_request(12, 0).unwrap().unwrap();
    assert_eq!(acquire.tick(), 17);
    assert_eq!(acquire.request().operation(), MemoryOperation::ReadShared);
    assert!(!acquire.request().is_uncacheable());
    assert!(!acquire.request().is_strict_ordered());
    assert_eq!(
        acquire.request().ordering(),
        MemoryAccessOrdering::new(None, Some(MemoryBarrierSet::memory()))
    );

    let uncacheable = generator.next_request(17, 0).unwrap().unwrap();
    assert_eq!(uncacheable.tick(), 21);
    assert_eq!(
        uncacheable.request().operation(),
        MemoryOperation::ReadShared
    );
    assert!(uncacheable.request().is_uncacheable());
    assert!(!uncacheable.request().is_strict_ordered());
    assert_eq!(
        uncacheable.request().ordering(),
        MemoryAccessOrdering::none()
    );

    let acquire_pc = generator.next_request(21, 0).unwrap().unwrap();
    assert_eq!(acquire_pc.tick(), 25);
    assert_eq!(
        acquire_pc.request().operation(),
        MemoryOperation::ReadShared
    );
    assert_eq!(
        acquire_pc.request().ordering(),
        MemoryAccessOrdering::new(None, Some(MemoryBarrierSet::memory()))
    );
}

#[test]
fn trace_traffic_generator_accepts_gem5_physical_flag_as_trace_address_metadata() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 2,
                    command: 1,
                    address: 0x40,
                    size: 8,
                    flags: Some(GEM5_FLAG_PHYSICAL | GEM5_FLAG_ACQUIRE),
                },
                PacketFields {
                    tick: 5,
                    command: 4,
                    address: 0x80,
                    size: 4,
                    flags: Some(GEM5_FLAG_PHYSICAL),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let read = generator.next_request(30, 0).unwrap().unwrap();
    assert_eq!(read.tick(), 32);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(
        read.request().ordering(),
        MemoryAccessOrdering::new(None, Some(MemoryBarrierSet::memory()))
    );
    assert!(!read.request().is_uncacheable());

    let write = generator.next_request(32, 0).unwrap().unwrap();
    assert_eq!(write.tick(), 35);
    assert_eq!(write.kind(), TrafficRequestKind::Write);
    assert_eq!(write.request().operation(), MemoryOperation::Write);
    assert_eq!(write.request().ordering(), MemoryAccessOrdering::none());
    assert!(!write.request().is_uncacheable());
}

#[test]
fn trace_traffic_generator_maps_gem5_inst_fetch_flag() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 1,
                address: 0x100,
                size: 4,
                flags: Some(GEM5_FLAG_INST_FETCH),
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let event = generator.next_request(20, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 23);
    assert_eq!(event.kind(), TrafficRequestKind::Read);
    assert_eq!(event.address(), Address::new(0x100));
    assert_eq!(
        event.request().operation(),
        MemoryOperation::InstructionFetch
    );
    assert_eq!(event.request().range().start(), Address::new(0x100));
    assert_eq!(event.request().size(), AccessSize::new(4).unwrap());
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 4);
}

#[test]
fn trace_traffic_generator_prefetch_flag_takes_priority_over_inst_fetch_flag() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 1,
                address: 0x180,
                size: 8,
                flags: Some(GEM5_FLAG_INST_FETCH | GEM5_FLAG_PREFETCH),
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(40);

    let event = generator.next_request(40, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 43);
    assert_eq!(event.kind(), TrafficRequestKind::Read);
    assert_eq!(event.address(), Address::new(0x180));
    assert_eq!(event.request().operation(), MemoryOperation::PrefetchRead);
    assert!(!event.request().requires_writable());
    assert!(!event.request().requires_response());
    assert_eq!(event.request().range().start(), Address::new(0x180));
    assert_eq!(event.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
}

#[test]
fn trace_parser_rejects_gem5_inst_fetch_flag_on_non_fetch_packet() {
    for command in [4, 22] {
        assert_eq!(
            TrafficTrace::from_gem5_packet_trace(
                &gem5_packet_trace(
                    TICK_FREQUENCY,
                    &[PacketFields {
                        tick: 1,
                        command,
                        address: 0x100,
                        size: 4,
                        flags: Some(GEM5_FLAG_INST_FETCH),
                    }],
                ),
                TICK_FREQUENCY,
            )
            .unwrap_err(),
            TrafficGeneratorError::TraceUnsupportedFlags {
                flags: GEM5_FLAG_INST_FETCH,
            }
        );
    }
}

#[test]
fn trace_traffic_generator_maps_gem5_prefetch_request_flags() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 3,
                    command: 1,
                    address: 0x200,
                    size: 8,
                    flags: Some(GEM5_FLAG_PREFETCH),
                },
                PacketFields {
                    tick: 5,
                    command: 1,
                    address: 0x240,
                    size: 16,
                    flags: Some(GEM5_FLAG_PREFETCH | GEM5_FLAG_PF_EXCLUSIVE),
                },
                PacketFields {
                    tick: 8,
                    command: 4,
                    address: 0x280,
                    size: 32,
                    flags: Some(GEM5_FLAG_PREFETCH | GEM5_FLAG_PF_EXCLUSIVE),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(90);

    let read = generator.next_request(90, 0).unwrap().unwrap();
    let read_exclusive = generator.next_request(read.tick(), 0).unwrap().unwrap();
    let write_exclusive = generator
        .next_request(read_exclusive.tick(), 0)
        .unwrap()
        .unwrap();

    assert_eq!(read.tick(), 93);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.request().operation(), MemoryOperation::PrefetchRead);
    assert!(!read.request().requires_writable());
    assert!(!read.request().requires_response());
    assert_eq!(read.request().data(), None);
    assert_eq!(read.request().byte_mask(), None);

    assert_eq!(read_exclusive.tick(), 95);
    assert_eq!(read_exclusive.kind(), TrafficRequestKind::Read);
    assert_eq!(
        read_exclusive.request().operation(),
        MemoryOperation::PrefetchWrite
    );
    assert!(read_exclusive.request().requires_writable());
    assert!(!read_exclusive.request().requires_response());
    assert_eq!(read_exclusive.request().data(), None);
    assert_eq!(read_exclusive.request().byte_mask(), None);

    assert_eq!(write_exclusive.tick(), 98);
    assert_eq!(write_exclusive.kind(), TrafficRequestKind::Read);
    assert_eq!(
        write_exclusive.request().operation(),
        MemoryOperation::PrefetchWrite
    );
    assert!(write_exclusive.request().requires_writable());
    assert!(!write_exclusive.request().requires_response());
    assert_eq!(write_exclusive.request().data(), None);
    assert_eq!(write_exclusive.request().byte_mask(), None);

    assert_eq!(generator.summary().read_count(), 3);
    assert_eq!(generator.summary().bytes_read(), 56);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_parser_rejects_gem5_write_packet_with_nonexclusive_prefetch_flag() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 4,
                    address: 0x200,
                    size: 8,
                    flags: Some(GEM5_FLAG_PREFETCH),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_FLAG_PREFETCH,
        }
    );
}

#[test]
fn trace_parser_rejects_read_exclusive_packet_with_unsupported_flags() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 22,
                    address: 0,
                    size: 8,
                    flags: Some(GEM5_UNSUPPORTED_FLAG),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_UNSUPPORTED_FLAG,
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
