use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_traffic::{TrafficRequestKind, TrafficTrace, TrafficTraceConfig, TrafficTraceGenerator};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: u64,
    size: u32,
}

fn trace_config(trace: TrafficTrace) -> TrafficTraceConfig {
    TrafficTraceConfig::new(
        AgentId::new(7),
        CacheLineLayout::new(64).unwrap(),
        99,
        trace,
    )
    .unwrap()
}

#[test]
fn trace_traffic_generator_maps_sc_upgrade_packets_to_llsc_upgrade_operations() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 4,
                    command: 18,
                    address: 0x180,
                    size: 64,
                },
                PacketFields {
                    tick: 9,
                    command: 20,
                    address: 0x1c0,
                    size: 64,
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(30);

    let upgrade = generator.next_request(30, 0).unwrap().unwrap();
    assert_eq!(upgrade.tick(), 34);
    assert_eq!(upgrade.kind(), TrafficRequestKind::Maintenance);
    assert_eq!(upgrade.address(), Address::new(0x180));
    assert_eq!(
        upgrade.request().operation(),
        MemoryOperation::StoreConditionalUpgrade
    );
    assert_eq!(upgrade.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(upgrade.request().data(), None);
    assert_eq!(upgrade.request().byte_mask(), None);
    assert!(!upgrade.request().carries_data());
    assert!(upgrade.request().requires_response());
    assert!(upgrade.request().requires_writable());
    assert!(!upgrade.request().returns_data());

    let fail = generator.next_request(upgrade.tick(), 0).unwrap().unwrap();
    assert_eq!(fail.tick(), 39);
    assert_eq!(fail.kind(), TrafficRequestKind::Read);
    assert_eq!(fail.address(), Address::new(0x1c0));
    assert_eq!(
        fail.request().operation(),
        MemoryOperation::StoreConditionalUpgradeFail
    );
    assert_eq!(fail.request().size(), AccessSize::new(64).unwrap());
    assert_eq!(fail.request().data(), None);
    assert_eq!(fail.request().byte_mask(), None);
    assert!(!fail.request().carries_data());
    assert!(fail.request().requires_response());
    assert!(fail.request().requires_writable());
    assert!(fail.request().returns_data());

    assert_eq!(generator.summary().packet_count(), 2);
    assert_eq!(generator.summary().read_count(), 1);
    assert_eq!(generator.summary().write_count(), 0);
    assert_eq!(generator.summary().bytes_read(), 64);
    assert_eq!(generator.summary().bytes_written(), 0);
}

#[test]
fn trace_traffic_generator_rejects_sc_upgrade_packet_with_partial_line_size() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 4,
                command: 18,
                address: 0x180,
                size: 32,
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
        rem6_traffic::TrafficGeneratorError::TraceUpgradeSizeMismatch {
            command: "SCUpgradeReq",
            size: 32,
            line_size: 64,
        }
    );
    assert_eq!(
        error.to_string(),
        "gem5 packet trace SCUpgradeReq size 32 does not match cache line size 64"
    );
}

#[test]
fn trace_traffic_generator_rejects_sc_upgrade_fail_packet_unaligned_after_addr_offset() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[PacketFields {
                tick: 9,
                command: 20,
                address: 0x1c0,
                size: 64,
            }],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let config = trace_config(trace).with_addr_offset(8).unwrap();
    let mut generator = TrafficTraceGenerator::new(config);
    generator.enter(30);

    let error = generator.next_request(30, 0).unwrap_err();

    assert_eq!(
        error,
        rem6_traffic::TrafficGeneratorError::TraceUpgradeUnalignedAddress {
            command: "SCUpgradeFailReq",
            address: Address::new(0x1c8),
            line_size: 64,
        }
    );
    assert_eq!(
        error.to_string(),
        "gem5 packet trace SCUpgradeFailReq address 0x1c8 is not aligned to cache line size 64"
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
        push_message(
            &mut bytes,
            &[
                field_varint(1, packet.tick),
                field_varint(2, u64::from(packet.command)),
                field_varint(3, packet.address),
                field_varint(4, u64::from(packet.size)),
            ],
        );
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
