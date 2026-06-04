use std::io::Write;

use flate2::{write::GzEncoder, Compression};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_traffic::{
    TrafficGeneratorError, TrafficRequestKind, TrafficTrace, TrafficTraceConfig,
    TrafficTraceExitStatus, TrafficTraceGenerator, TrafficTraceSnapshot,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;

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
                    command: 2,
                    address: 0,
                    size: 8,
                    flags: None,
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedCommand { command: 2 }
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
                size: 16,
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
    assert_eq!(event.request().size(), AccessSize::new(16).unwrap());
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().bytes_read(), 16);
    assert_eq!(generator.summary().write_count(), 0);
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
                    size: 8,
                    flags: None,
                },
                PacketFields {
                    tick: 6,
                    command: 25,
                    address: 0x48,
                    size: 8,
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
    assert_eq!(generator.summary().bytes_read(), 16);
    assert_eq!(generator.summary().write_count(), 0);
}

#[test]
fn trace_parser_rejects_read_exclusive_packet_with_nonzero_flags() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 22,
                    address: 0,
                    size: 8,
                    flags: Some(0x400),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags { flags: 0x400 }
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
