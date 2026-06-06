use std::io::Write;

use flate2::{write::GzEncoder, Compression};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryAtomicOp, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    TrafficGeneratorError, TrafficRequestKind, TrafficTrace, TrafficTraceConfig,
    TrafficTraceExitStatus, TrafficTraceGenerator, TrafficTraceSnapshot,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_FLAG_STRICT_ORDER: u32 = 0x0000_0800;

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
    TrafficTraceConfig::new(AgentId::new(7), line_layout(), 99, trace).unwrap()
}

fn read_write_trace() -> TrafficTrace {
    TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 5,
                    command: 1,
                    address: 0x20,
                    size: 8,
                    flags: None,
                },
                PacketFields {
                    tick: 9,
                    command: 4,
                    address: 0x30,
                    size: 4,
                    flags: Some(0),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap()
}

#[test]
fn trace_parser_decodes_minimal_gem5_packet_trace() {
    let trace = read_write_trace();

    assert_eq!(trace.tick_frequency(), TICK_FREQUENCY);
    assert_eq!(trace.len(), 2);
    assert!(!trace.is_empty());
}

#[test]
fn trace_parser_decodes_gzip_wrapped_gem5_packet_trace() {
    let plain = gem5_packet_trace(
        TICK_FREQUENCY,
        &[
            PacketFields {
                tick: 5,
                command: 1,
                address: 0x20,
                size: 8,
                flags: None,
            },
            PacketFields {
                tick: 9,
                command: 4,
                address: 0x30,
                size: 4,
                flags: Some(0),
            },
        ],
    );
    let compressed = gzip_bytes(&plain);

    let trace = TrafficTrace::from_gem5_packet_trace(&compressed, TICK_FREQUENCY).unwrap();

    assert_eq!(trace.tick_frequency(), TICK_FREQUENCY);
    assert_eq!(trace.len(), 2);
}

#[test]
fn trace_parser_rejects_invalid_gzip_packet_trace() {
    let error = TrafficTrace::from_gem5_packet_trace(&[0x1f, 0x8b, 0x08, 0x00], TICK_FREQUENCY)
        .unwrap_err();

    let TrafficGeneratorError::TraceGzipDecode { message } = error else {
        panic!("invalid gzip packet trace should report gzip decoding");
    };
    assert!(!message.is_empty());
}

#[test]
fn trace_parser_rejects_invalid_gem5_packet_traces() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(&[0, 0, 0, 0], TICK_FREQUENCY).unwrap_err(),
        TrafficGeneratorError::TraceBadMagic {
            actual: [0, 0, 0, 0],
        }
    );

    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(&GEM5_MAGIC, TICK_FREQUENCY).unwrap_err(),
        TrafficGeneratorError::TraceMissingHeader
    );

    let mut oversized_record = GEM5_MAGIC.to_vec();
    append_varint(&mut oversized_record, u64::from(u32::MAX) + 1);
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(&oversized_record, TICK_FREQUENCY).unwrap_err(),
        TrafficGeneratorError::TraceMessageTooLarge {
            offset: 4,
            length: u64::from(u32::MAX) + 1,
        }
    );

    let mut overlong_record_length = GEM5_MAGIC.to_vec();
    overlong_record_length.extend_from_slice(&[0x81, 0x80, 0x80, 0x80, 0x80, 0x00]);
    overlong_record_length.push(0);
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(&overlong_record_length, TICK_FREQUENCY).unwrap_err(),
        TrafficGeneratorError::TraceVarint32TooLong { offset: 4 }
    );

    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(TICK_FREQUENCY + 1, &[]),
            TICK_FREQUENCY
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceTickFrequencyMismatch {
            expected: TICK_FREQUENCY,
            actual: TICK_FREQUENCY + 1,
        }
    );

    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 36,
                    address: 0,
                    size: 8,
                    flags: None,
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedCommand { command: 36 }
    );

    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 1,
                    address: 0,
                    size: 8,
                    flags: Some(1),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags { flags: 1 }
    );

    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 1,
                    address: 0,
                    size: 8,
                    flags: Some(GEM5_FLAG_STRICT_ORDER),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_FLAG_STRICT_ORDER,
        }
    );

    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 1,
                    address: 0,
                    size: 0,
                    flags: None,
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceZeroSize
    );
}

#[test]
fn trace_traffic_generator_replays_read_write_packets_with_offset_and_summary() {
    let config = trace_config(read_write_trace())
        .with_addr_offset(0x1000)
        .unwrap();
    let mut generator = TrafficTraceGenerator::new(config);

    generator.enter(100);

    assert_eq!(generator.config().duration(), 99);
    assert_eq!(generator.schedule_tick(100, 0).unwrap(), 105);

    let first = generator.next_request(100, 0).unwrap().unwrap();
    assert_eq!(first.tick(), 105);
    assert_eq!(first.sequence(), 0);
    assert_eq!(first.kind(), TrafficRequestKind::Read);
    assert_eq!(first.address(), Address::new(0x1020));
    assert_eq!(
        first.request().id(),
        MemoryRequestId::new(AgentId::new(7), 0)
    );
    assert_eq!(first.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(first.request().range().start(), Address::new(0x1020));
    assert_eq!(first.request().size(), AccessSize::new(8).unwrap());

    assert_eq!(generator.schedule_tick(105, 0).unwrap(), 109);

    let second = generator.next_request(105, 0).unwrap().unwrap();
    assert_eq!(second.tick(), 109);
    assert_eq!(second.sequence(), 1);
    assert_eq!(second.kind(), TrafficRequestKind::Write);
    assert_eq!(second.address(), Address::new(0x1030));
    assert_eq!(second.request().operation(), MemoryOperation::Write);
    assert_eq!(second.request().data(), Some(&vec![7; 4][..]));
    assert_eq!(second.request().byte_mask().unwrap().len(), 4);

    assert_eq!(generator.schedule_tick(109, 0).unwrap(), u64::MAX);
    assert_eq!(generator.next_request(109, 0).unwrap(), None);
    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 4);
    assert_eq!(generator.summary().first_tick(), Some(105));
    assert_eq!(generator.summary().last_tick(), Some(109));
}

#[test]
fn trace_traffic_generator_maps_read_exclusive_packet_to_read_unique() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 22,
                address: 0x80,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(50);

    let event = generator.next_request(50, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 57);
    assert_eq!(event.kind(), TrafficRequestKind::Read);
    assert_eq!(event.address(), Address::new(0x80));
    assert_eq!(event.request().operation(), MemoryOperation::ReadUnique);
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 64);
    assert_eq!(generator.summary().write_count(), 0);
}

#[test]
fn trace_traffic_generator_rejects_read_exclusive_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 22,
                address: 0x80,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(50);

    assert_eq!(
        generator.next_request(50, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheReadSizeMismatch {
            command: "ReadExReq",
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_rejects_read_exclusive_packet_with_unaligned_address() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 22,
                address: 0x88,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(50);

    assert_eq!(
        generator.next_request(50, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheReadUnalignedAddress {
            command: "ReadExReq",
            address: Address::new(0x88),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_rejects_read_exclusive_packet_unaligned_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 22,
                address: 0x80,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(8).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(50);

    assert_eq!(
        generator.next_request(50, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheReadUnalignedAddress {
            command: "ReadExReq",
            address: Address::new(0x88),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_cache_read_packets_to_read_shared() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 3,
                    command: 24,
                    address: 0x40,
                    size: 64,
                    flags: None,
                },
                PacketFields {
                    tick: 6,
                    command: 25,
                    address: 0x80,
                    size: 64,
                    flags: None,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let clean = generator.next_request(20, 0).unwrap().unwrap();
    let shared = generator.next_request(clean.tick(), 0).unwrap().unwrap();

    assert_eq!(clean.tick(), 23);
    assert_eq!(clean.kind(), TrafficRequestKind::Read);
    assert_eq!(clean.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(shared.tick(), 26);
    assert_eq!(shared.kind(), TrafficRequestKind::Read);
    assert_eq!(shared.request().operation(), MemoryOperation::ReadShared);
    assert_eq!(generator.summary().read_count(), 2);
    assert_eq!(generator.summary().bytes_read(), 128);
    assert_eq!(generator.summary().write_count(), 0);
}

#[test]
fn trace_traffic_generator_rejects_cache_read_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 24,
                address: 0x40,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    assert_eq!(
        generator.next_request(20, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheReadSizeMismatch {
            command: "ReadCleanReq",
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_rejects_cache_read_packet_with_unaligned_address() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 25,
                address: 0x48,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    assert_eq!(
        generator.next_request(20, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheReadUnalignedAddress {
            command: "ReadSharedReq",
            address: Address::new(0x48),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_validates_cache_read_alignment_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 3,
                command: 24,
                address: 0x40,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(8).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(20);

    assert_eq!(
        generator.next_request(20, 0).unwrap_err(),
        TrafficGeneratorError::TraceCacheReadUnalignedAddress {
            command: "ReadCleanReq",
            address: Address::new(0x48),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_prefetch_packets_to_prefetch_operations() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 2,
                    command: 11,
                    address: 0x140,
                    size: 8,
                    flags: None,
                },
                PacketFields {
                    tick: 5,
                    command: 13,
                    address: 0x180,
                    size: 16,
                    flags: None,
                },
                PacketFields {
                    tick: 9,
                    command: 12,
                    address: 0x1c0,
                    size: 32,
                    flags: None,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(40);

    let soft = generator.next_request(40, 0).unwrap().unwrap();
    let hard = generator.next_request(soft.tick(), 0).unwrap().unwrap();
    let exclusive = generator.next_request(hard.tick(), 0).unwrap().unwrap();

    assert_eq!(soft.tick(), 42);
    assert_eq!(soft.kind(), TrafficRequestKind::Read);
    assert_eq!(soft.request().operation(), MemoryOperation::PrefetchRead);
    assert_eq!(soft.request().size(), AccessSize::new(8).unwrap());
    assert!(!soft.request().requires_writable());
    assert!(!soft.request().requires_response());
    assert!(!soft.request().returns_data());
    assert_eq!(soft.request().byte_mask(), None);
    assert_eq!(soft.request().data(), None);

    assert_eq!(hard.tick(), 45);
    assert_eq!(hard.kind(), TrafficRequestKind::Read);
    assert_eq!(hard.request().operation(), MemoryOperation::PrefetchRead);
    assert_eq!(hard.request().size(), AccessSize::new(16).unwrap());
    assert!(!hard.request().requires_writable());
    assert!(!hard.request().requires_response());
    assert!(!hard.request().returns_data());
    assert_eq!(hard.request().byte_mask(), None);
    assert_eq!(hard.request().data(), None);

    assert_eq!(exclusive.tick(), 49);
    assert_eq!(exclusive.kind(), TrafficRequestKind::Read);
    assert_eq!(
        exclusive.request().operation(),
        MemoryOperation::PrefetchWrite
    );
    assert_eq!(exclusive.request().size(), AccessSize::new(32).unwrap());
    assert!(exclusive.request().requires_writable());
    assert!(!exclusive.request().requires_response());
    assert_eq!(exclusive.request().byte_mask(), None);
    assert_eq!(exclusive.request().data(), None);

    assert_eq!(generator.summary().read_count(), 3);
    assert_eq!(generator.summary().bytes_read(), 56);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_traffic_generator_maps_write_line_packet_to_full_line_write() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 4,
                command: 16,
                address: 0x80,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let event = generator.next_request(30, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 34);
    assert_eq!(event.kind(), TrafficRequestKind::Write);
    assert_eq!(event.address(), Address::new(0x80));
    assert_eq!(event.request().operation(), MemoryOperation::Write);
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(event.request().data(), Some(&vec![7; 64][..]));
    assert_eq!(event.request().byte_mask().unwrap().len(), 64);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_written(), 64);
    assert_eq!(generator.summary().read_count(), 0);
}

#[test]
fn trace_traffic_generator_rejects_write_line_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 4,
                command: 16,
                address: 0x80,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let error = generator.next_request(30, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceWriteLineSizeMismatch {
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_rejects_write_line_packet_with_unaligned_address() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 4,
                command: 16,
                address: 0x84,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let error = generator.next_request(30, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceWriteLineUnalignedAddress {
            address: Address::new(0x84),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_writeback_packets_to_writeback_operations() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 4,
                    command: 7,
                    address: 0x80,
                    size: 64,
                    flags: None,
                },
                PacketFields {
                    tick: 9,
                    command: 8,
                    address: 0xc0,
                    size: 64,
                    flags: None,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let dirty = generator.next_request(30, 0).unwrap().unwrap();
    let clean = generator.next_request(dirty.tick(), 0).unwrap().unwrap();

    assert_eq!(dirty.tick(), 34);
    assert_eq!(dirty.kind(), TrafficRequestKind::Write);
    assert_eq!(dirty.address(), Address::new(0x80));
    assert_eq!(dirty.request().operation(), MemoryOperation::WritebackDirty);
    assert_eq!(dirty.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(dirty.request().data(), Some(&vec![7; 64][..]));
    assert_eq!(dirty.request().byte_mask(), None);
    assert!(!dirty.request().requires_response());
    assert_eq!(clean.tick(), 39);
    assert_eq!(clean.kind(), TrafficRequestKind::Write);
    assert_eq!(clean.address(), Address::new(0xc0));
    assert_eq!(clean.request().operation(), MemoryOperation::WritebackClean);
    assert_eq!(clean.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(clean.request().data(), Some(&vec![7; 64][..]));
    assert_eq!(clean.request().byte_mask(), None);
    assert!(!clean.request().requires_response());
    assert_eq!(generator.summary().write_count(), 2);
    assert_eq!(generator.summary().bytes_written(), 128);
    assert_eq!(generator.summary().read_count(), 0);
}

#[test]
fn trace_traffic_generator_maps_write_clean_packet_to_write_clean_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 9,
                address: 0x100,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let event = generator.next_request(20, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 27);
    assert_eq!(event.kind(), TrafficRequestKind::Write);
    assert_eq!(event.address(), Address::new(0x100));
    assert_eq!(event.request().operation(), MemoryOperation::WriteClean);
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(event.request().data(), Some(&vec![7; 64][..]));
    assert_eq!(event.request().byte_mask(), None);
    assert!(event.request().carries_data());
    assert!(!event.request().requires_writable());
    assert!(!event.request().requires_response());
    assert!(!event.request().returns_data());
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_written(), 64);
    assert_eq!(generator.summary().read_count(), 0);
}

#[test]
fn trace_traffic_generator_maps_swap_packet_to_atomic_swap_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 11,
                command: 34,
                address: 0x108,
                size: 8,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(70);

    let event = generator.next_request(70, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 81);
    assert_eq!(event.kind(), TrafficRequestKind::Atomic);
    assert_eq!(event.address(), Address::new(0x108));
    assert_eq!(event.request().operation(), MemoryOperation::Atomic);
    assert_eq!(event.request().atomic_op(), Some(MemoryAtomicOp::Swap));
    assert_eq!(event.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(event.request().data(), Some(&vec![7; 8][..]));
    assert_eq!(event.request().byte_mask().unwrap().len(), 8);
    assert!(event.request().carries_data());
    assert!(event.request().requires_writable());
    assert!(event.request().requires_response());
    assert!(event.request().returns_data());
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 8);
}

#[test]
fn trace_traffic_generator_maps_locked_rmw_packets_to_typed_locked_operations() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 13,
                    command: 30,
                    address: 0x208,
                    size: 8,
                    flags: None,
                },
                PacketFields {
                    tick: 17,
                    command: 32,
                    address: 0x208,
                    size: 8,
                    flags: None,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(90);

    let read = generator.next_request(90, 0).unwrap().unwrap();
    assert_eq!(read.tick(), 103);
    assert_eq!(read.kind(), TrafficRequestKind::Read);
    assert_eq!(read.address(), Address::new(0x208));
    assert_eq!(read.request().operation(), MemoryOperation::LockedRmwRead);
    assert_eq!(read.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(read.request().data(), None);
    assert_eq!(read.request().byte_mask(), None);
    assert_eq!(read.request().atomic_op(), None);
    assert!(read.request().requires_writable());
    assert!(read.request().returns_data());

    let write = generator.next_request(read.tick(), 0).unwrap().unwrap();
    assert_eq!(write.tick(), 107);
    assert_eq!(write.kind(), TrafficRequestKind::Write);
    assert_eq!(write.address(), Address::new(0x208));
    assert_eq!(write.request().operation(), MemoryOperation::LockedRmwWrite);
    assert_eq!(write.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(write.request().data(), Some(&vec![7; 8][..]));
    assert_eq!(write.request().byte_mask().unwrap().len(), 8);
    assert!(write
        .request()
        .byte_mask()
        .unwrap()
        .bits()
        .iter()
        .all(|bit| *bit));
    assert_eq!(write.request().atomic_op(), None);
    assert!(write.request().requires_writable());
    assert!(!write.request().returns_data());

    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 8);
}

#[test]
fn trace_traffic_generator_maps_llsc_packets_to_typed_operations() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 19,
                    command: 26,
                    address: 0x308,
                    size: 8,
                    flags: None,
                },
                PacketFields {
                    tick: 23,
                    command: 27,
                    address: 0x308,
                    size: 8,
                    flags: None,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(110);

    let load = generator.next_request(110, 0).unwrap().unwrap();
    assert_eq!(load.tick(), 129);
    assert_eq!(load.kind(), TrafficRequestKind::Read);
    assert_eq!(load.address(), Address::new(0x308));
    assert_eq!(load.request().operation(), MemoryOperation::LoadLocked);
    assert_eq!(load.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(load.request().data(), None);
    assert_eq!(load.request().byte_mask(), None);
    assert_eq!(load.request().atomic_op(), None);
    assert!(!load.request().requires_writable());
    assert!(load.request().returns_data());

    let store = generator.next_request(load.tick(), 0).unwrap().unwrap();
    assert_eq!(store.tick(), 133);
    assert_eq!(store.kind(), TrafficRequestKind::Write);
    assert_eq!(store.address(), Address::new(0x308));
    assert_eq!(
        store.request().operation(),
        MemoryOperation::StoreConditional
    );
    assert_eq!(store.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(store.request().data(), Some(&vec![7; 8][..]));
    assert_eq!(store.request().byte_mask().unwrap().len(), 8);
    assert!(store
        .request()
        .byte_mask()
        .unwrap()
        .bits()
        .iter()
        .all(|bit| *bit));
    assert_eq!(store.request().atomic_op(), None);
    assert!(store.request().requires_writable());
    assert!(!store.request().returns_data());

    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 8);
    assert_eq!(generator.summary().bytes_written(), 8);
}

#[test]
fn trace_traffic_generator_maps_store_cond_fail_packet_to_forced_fail_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 29,
                command: 28,
                address: 0x308,
                size: 8,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(110);

    let store = generator.next_request(110, 0).unwrap().unwrap();
    assert_eq!(store.tick(), 139);
    assert_eq!(store.sequence(), 0);
    assert_eq!(store.kind(), TrafficRequestKind::Write);
    assert_eq!(store.address(), Address::new(0x308));
    assert_eq!(
        store.request().operation(),
        MemoryOperation::StoreConditionalFail
    );
    assert_eq!(store.request().size(), AccessSize::new(8).unwrap());
    assert_eq!(store.request().data(), Some(&vec![7; 8][..]));
    assert_eq!(store.request().byte_mask().unwrap().len(), 8);
    assert!(store
        .request()
        .byte_mask()
        .unwrap()
        .bits()
        .iter()
        .all(|bit| *bit));
    assert_eq!(store.request().atomic_op(), None);
    assert!(store.request().requires_writable());
    assert!(!store.request().returns_data());

    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 0);
    assert_eq!(generator.summary().write_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 0);
    assert_eq!(generator.summary().bytes_written(), 8);
}

#[test]
fn trace_traffic_generator_rejects_write_clean_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 9,
                address: 0x100,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(20);

    let error = generator.next_request(20, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceWritebackSizeMismatch {
            command: "WriteClean",
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_validates_write_clean_alignment_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 9,
                address: 0x100,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(4).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(20);

    let error = generator.next_request(20, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceWritebackUnalignedAddress {
            command: "WriteClean",
            address: Address::new(0x104),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_rejects_writeback_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 4,
                command: 7,
                address: 0x80,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let error = generator.next_request(30, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceWritebackSizeMismatch {
            command: "WritebackDirty",
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_rejects_writeback_packet_with_unaligned_address() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 4,
                command: 8,
                address: 0x84,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let error = generator.next_request(30, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceWritebackUnalignedAddress {
            command: "WritebackClean",
            address: Address::new(0x84),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_validates_writeback_alignment_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 4,
                command: 7,
                address: 0x80,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(4).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(30);

    let error = generator.next_request(30, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceWritebackUnalignedAddress {
            command: "WritebackDirty",
            address: Address::new(0x84),
            line_size: 64,
        }
    );
}

#[test]
fn trace_parser_rejects_writeback_packet_with_nonzero_flags() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 7,
                    address: 0x80,
                    size: 64,
                    flags: Some(0x10),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags { flags: 0x10 }
    );
}

#[test]
fn trace_traffic_generator_maps_clean_evict_packet_to_maintenance_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 6,
                command: 10,
                address: 0x100,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(40);

    let event = generator.next_request(40, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 46);
    assert_eq!(event.kind(), TrafficRequestKind::Maintenance);
    assert_eq!(event.address(), Address::new(0x100));
    assert_eq!(event.request().operation(), MemoryOperation::CleanEvict);
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert!(!event.request().carries_data());
    assert!(!event.request().requires_response());
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 0);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_read(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_traffic_generator_rejects_clean_evict_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 6,
                command: 10,
                address: 0x100,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(40);

    let error = generator.next_request(40, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceCleanEvictSizeMismatch {
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_validates_clean_evict_alignment_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 6,
                command: 10,
                address: 0x100,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(4).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(40);

    let error = generator.next_request(40, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceCleanEvictUnalignedAddress {
            address: Address::new(0x104),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_clean_shared_packet_to_maintenance_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 42,
                address: 0x180,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(50);

    let event = generator.next_request(50, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 57);
    assert_eq!(event.kind(), TrafficRequestKind::Maintenance);
    assert_eq!(event.address(), Address::new(0x180));
    assert_eq!(event.request().operation(), MemoryOperation::CleanShared);
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert!(!event.request().carries_data());
    assert!(event.request().requires_response());
    assert!(!event.request().requires_writable());
    assert!(!event.request().returns_data());
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 0);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_read(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_traffic_generator_rejects_clean_shared_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 7,
                command: 42,
                address: 0x180,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(50);

    let error = generator.next_request(50, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceCleanMaintenanceSizeMismatch {
            command: "CleanSharedReq",
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_clean_invalid_packet_to_invalidate_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 9,
                command: 44,
                address: 0x1c0,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(70);

    let event = generator.next_request(70, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 79);
    assert_eq!(event.kind(), TrafficRequestKind::Maintenance);
    assert_eq!(event.address(), Address::new(0x1c0));
    assert_eq!(event.request().operation(), MemoryOperation::Invalidate);
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert!(!event.request().carries_data());
    assert!(event.request().requires_response());
    assert!(!event.request().requires_writable());
    assert!(!event.request().returns_data());
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 0);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_read(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_traffic_generator_validates_clean_invalid_alignment_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 9,
                command: 44,
                address: 0x1c0,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(4).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(70);

    let error = generator.next_request(70, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceCleanMaintenanceUnalignedAddress {
            command: "CleanInvalidReq",
            address: Address::new(0x1c4),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_invalidate_packet_to_writable_invalidate_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 11,
                command: 54,
                address: 0x200,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(90);

    let event = generator.next_request(90, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 101);
    assert_eq!(event.kind(), TrafficRequestKind::Maintenance);
    assert_eq!(event.address(), Address::new(0x200));
    assert_eq!(
        event.request().operation(),
        MemoryOperation::InvalidateWritable
    );
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert!(!event.request().carries_data());
    assert!(event.request().requires_response());
    assert!(event.request().requires_writable());
    assert!(!event.request().returns_data());
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 0);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_read(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_traffic_generator_rejects_invalidate_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 11,
                command: 54,
                address: 0x200,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(90);

    let error = generator.next_request(90, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceInvalidateSizeMismatch {
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_validates_invalidate_alignment_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 11,
                command: 54,
                address: 0x200,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(4).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(90);

    let error = generator.next_request(90, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceInvalidateUnalignedAddress {
            address: Address::new(0x204),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_maps_upgrade_packet_to_maintenance_operation() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 8,
                command: 17,
                address: 0x140,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(60);

    let event = generator.next_request(60, 0).unwrap().unwrap();

    assert_eq!(event.tick(), 68);
    assert_eq!(event.kind(), TrafficRequestKind::Maintenance);
    assert_eq!(event.address(), Address::new(0x140));
    assert_eq!(event.request().operation(), MemoryOperation::Upgrade);
    assert_eq!(event.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(event.request().data(), None);
    assert_eq!(event.request().byte_mask(), None);
    assert!(!event.request().carries_data());
    assert!(event.request().requires_response());
    assert!(event.request().requires_writable());
    assert!(!event.request().returns_data());
    assert_eq!(generator.summary().packet_count(), 1);
    assert_eq!(generator.summary().read_count(), 0);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_read(), 0);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_traffic_generator_rejects_upgrade_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 8,
                command: 17,
                address: 0x140,
                size: 32,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(60);

    let error = generator.next_request(60, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceUpgradeSizeMismatch {
            size: 32,
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_validates_upgrade_alignment_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 8,
                command: 17,
                address: 0x140,
                size: 64,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(4).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(60);

    let error = generator.next_request(60, 0).unwrap_err();

    assert_eq!(
        error,
        TrafficGeneratorError::TraceUpgradeUnalignedAddress {
            address: Address::new(0x144),
            line_size: 64,
        }
    );
}

#[test]
fn trace_traffic_generator_applies_elastic_delay_and_non_elastic_clamp() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: 1,
                address: 0x20,
                size: 8,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();

    let mut elastic = TrafficTraceGenerator::new(trace_config(trace.clone()).with_elastic(true));
    let mut non_elastic = TrafficTraceGenerator::new(trace_config(trace).with_elastic(false));

    elastic.enter(100);
    non_elastic.enter(100);

    assert_eq!(elastic.schedule_tick(100, 3).unwrap(), 108);
    assert_eq!(non_elastic.schedule_tick(110, 3).unwrap(), 110);
}

#[test]
fn trace_traffic_generator_does_not_double_apply_elastic_delay_after_schedule_polling() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: 1,
                address: 0x20,
                size: 8,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace).with_elastic(true));
    generator.enter(100);

    assert_eq!(generator.schedule_tick(100, 3).unwrap(), 108);
    assert_eq!(generator.schedule_tick(100, 3).unwrap(), 108);

    let event = generator.next_request(100, 3).unwrap().unwrap();
    assert_eq!(event.tick(), 108);
}

#[test]
fn trace_traffic_generator_snapshot_restores_cursor_summary_and_tick_offset() {
    let mut generator = TrafficTraceGenerator::new(trace_config(read_write_trace()));
    generator.enter(100);

    let first = generator.next_request(100, 0).unwrap().unwrap();
    let snapshot = generator.snapshot();
    let mut restored = TrafficTraceGenerator::restore(snapshot).unwrap();

    let next = restored.next_request(first.tick(), 0).unwrap().unwrap();

    assert_eq!(next.sequence(), 1);
    assert_eq!(next.address(), Address::new(0x30));
    assert_eq!(restored.summary().packet_count(), 2);
    assert_eq!(restored.summary().bytes_read(), 8);
    assert_eq!(restored.summary().bytes_written(), 4);
}

#[test]
fn trace_traffic_generator_resets_trace_cursor_on_exit_and_reenter() {
    let mut generator = TrafficTraceGenerator::new(trace_config(read_write_trace()));
    generator.enter(100);
    assert!(generator.next_request(100, 0).unwrap().is_some());

    let status = generator.exit();
    assert_eq!(status, TrafficTraceExitStatus::incomplete());
    assert_eq!(generator.schedule_tick(100, 0).unwrap(), u64::MAX);

    generator.enter(200);
    let first = generator.next_request(200, 0).unwrap().unwrap();

    assert_eq!(first.sequence(), 0);
    assert_eq!(first.tick(), 205);
    assert_eq!(first.address(), Address::new(0x20));
}

#[test]
fn trace_traffic_generator_rejects_overflowing_tick_address_and_snapshot_cursor() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 5,
                command: 1,
                address: u64::MAX,
                size: 8,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator =
        TrafficTraceGenerator::new(trace_config(trace).with_addr_offset(1).unwrap());
    generator.enter(0);

    assert_eq!(
        generator.next_request(0, 0).unwrap_err(),
        TrafficGeneratorError::AddressOverflow {
            label: "trace_address",
            value: u64::MAX,
            increment: 1,
        }
    );

    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 10,
                command: 1,
                address: 0,
                size: 8,
                flags: None,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(u64::MAX);

    assert_eq!(
        generator.schedule_tick(u64::MAX, 0).unwrap_err(),
        TrafficGeneratorError::TickOverflow {
            tick: u64::MAX,
            delta: 10,
        }
    );

    let snapshot = TrafficTraceSnapshot::new(
        trace_config(read_write_trace()),
        3,
        0,
        Default::default(),
        0,
        true,
    );

    assert_eq!(
        TrafficTraceGenerator::restore(snapshot).unwrap_err(),
        TrafficGeneratorError::TraceSnapshotCursorOutsideTrace {
            cursor: 3,
            length: 2,
        }
    );
}

fn gem5_packet_trace(tick_frequency: u64, packets: &[PacketFields]) -> Vec<u8> {
    let mut trace = GEM5_MAGIC.to_vec();
    append_message(&mut trace, &header_message(tick_frequency));

    for packet in packets {
        append_message(&mut trace, &packet_message(*packet));
    }

    trace
}

fn append_message(trace: &mut Vec<u8>, message: &[u8]) {
    append_varint(trace, message.len() as u64);
    trace.extend_from_slice(message);
}

fn header_message(tick_frequency: u64) -> Vec<u8> {
    let mut message = Vec::new();
    append_field_varint(&mut message, 3, tick_frequency);
    message
}

fn packet_message(packet: PacketFields) -> Vec<u8> {
    let mut message = Vec::new();
    append_field_varint(&mut message, 1, packet.tick);
    append_field_varint(&mut message, 2, u64::from(packet.command));
    append_field_varint(&mut message, 3, packet.address);
    append_field_varint(&mut message, 4, u64::from(packet.size));
    if let Some(flags) = packet.flags {
        append_field_varint(&mut message, 5, u64::from(flags));
    }
    message
}

fn append_field_varint(message: &mut Vec<u8>, field: u32, value: u64) {
    append_varint(message, u64::from(field << 3));
    append_varint(message, value);
}

fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
    loop {
        let mut byte = (value & 0x7f) as u8;
        value >>= 7;
        if value != 0 {
            byte |= 0x80;
        }
        bytes.push(byte);
        if value == 0 {
            break;
        }
    }
}

fn gzip_bytes(bytes: &[u8]) -> Vec<u8> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(bytes).unwrap();
    encoder.finish().unwrap()
}
