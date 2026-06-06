use rem6_memory::{AgentId, CacheLineLayout, MemoryAtomicOp, MemoryOperation};
use rem6_traffic::{
    TrafficGeneratorError, TrafficTrace, TrafficTraceConfig, TrafficTraceGenerator,
};

const GEM5_MAGIC: [u8; 4] = [0x67, 0x65, 0x6d, 0x35];
const TICK_FREQUENCY: u64 = 1_000;
const GEM5_FLAG_LOCKED_RMW: u32 = 0x0010_0000;
const GEM5_FLAG_LLSC: u32 = 0x0020_0000;
const GEM5_FLAG_MEM_SWAP: u32 = 0x0040_0000;
const GEM5_FLAG_MEM_SWAP_COND: u32 = 0x0080_0000;
const GEM5_FLAG_ATOMIC_RETURN_OP: u32 = 0x4000_0000;
const GEM5_FLAG_ATOMIC_NO_RETURN_OP: u32 = 0x8000_0000;

#[derive(Clone, Copy)]
struct PacketFields {
    tick: u64,
    command: u32,
    address: u64,
    size: u32,
    flags: Option<u32>,
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
fn trace_accepts_gem5_operation_request_flags_on_matching_packets() {
    let trace = TrafficTrace::from_gem5_packet_trace(
        &gem5_packet_trace(
            TICK_FREQUENCY,
            &[
                PacketFields {
                    tick: 2,
                    command: 26,
                    address: 0x300,
                    size: 8,
                    flags: Some(GEM5_FLAG_LLSC),
                },
                PacketFields {
                    tick: 5,
                    command: 27,
                    address: 0x300,
                    size: 8,
                    flags: Some(GEM5_FLAG_LLSC),
                },
                PacketFields {
                    tick: 8,
                    command: 30,
                    address: 0x340,
                    size: 16,
                    flags: Some(GEM5_FLAG_LOCKED_RMW),
                },
                PacketFields {
                    tick: 11,
                    command: 32,
                    address: 0x340,
                    size: 16,
                    flags: Some(GEM5_FLAG_LOCKED_RMW),
                },
                PacketFields {
                    tick: 14,
                    command: 34,
                    address: 0x380,
                    size: 8,
                    flags: Some(GEM5_FLAG_MEM_SWAP),
                },
                PacketFields {
                    tick: 17,
                    command: 34,
                    address: 0x3c0,
                    size: 8,
                    flags: Some(GEM5_FLAG_MEM_SWAP_COND),
                },
                PacketFields {
                    tick: 20,
                    command: 34,
                    address: 0x400,
                    size: 8,
                    flags: Some(GEM5_FLAG_ATOMIC_RETURN_OP),
                },
            ],
        ),
        TICK_FREQUENCY,
    )
    .unwrap();
    let mut generator = TrafficTraceGenerator::new(trace_config(trace));
    generator.enter(50);

    let load_locked = generator.next_request(50, 0).unwrap().unwrap();
    let store_conditional = generator
        .next_request(load_locked.tick(), 0)
        .unwrap()
        .unwrap();
    let locked_read = generator
        .next_request(store_conditional.tick(), 0)
        .unwrap()
        .unwrap();
    let locked_write = generator
        .next_request(locked_read.tick(), 0)
        .unwrap()
        .unwrap();
    let swap = generator
        .next_request(locked_write.tick(), 0)
        .unwrap()
        .unwrap();
    let swap_conditional = generator.next_request(swap.tick(), 0).unwrap().unwrap();
    let atomic_return = generator
        .next_request(swap_conditional.tick(), 0)
        .unwrap()
        .unwrap();

    assert_eq!(
        load_locked.request().operation(),
        MemoryOperation::LoadLocked
    );
    assert_eq!(
        store_conditional.request().operation(),
        MemoryOperation::StoreConditional
    );
    assert_eq!(
        locked_read.request().operation(),
        MemoryOperation::LockedRmwRead
    );
    assert_eq!(
        locked_write.request().operation(),
        MemoryOperation::LockedRmwWrite
    );
    assert_eq!(swap.request().operation(), MemoryOperation::Atomic);
    assert_eq!(swap.request().atomic_op(), Some(MemoryAtomicOp::Swap));
    assert_eq!(
        swap_conditional.request().operation(),
        MemoryOperation::Atomic
    );
    assert_eq!(
        swap_conditional.request().atomic_op(),
        Some(MemoryAtomicOp::Swap)
    );
    assert_eq!(atomic_return.request().operation(), MemoryOperation::Atomic);
    assert_eq!(
        atomic_return.request().atomic_op(),
        Some(MemoryAtomicOp::Swap)
    );
    assert!(atomic_return.request().requires_response());
    assert!(atomic_return.request().returns_data());

    assert_eq!(generator.summary().packet_count(), 7);
    assert_eq!(generator.summary().read_count(), 5);
    assert_eq!(generator.summary().write_count(), 5);
}

#[test]
fn trace_rejects_gem5_operation_request_flags_on_mismatched_packets() {
    for (command, flags) in [
        (1, GEM5_FLAG_LLSC),
        (4, GEM5_FLAG_LOCKED_RMW),
        (1, GEM5_FLAG_MEM_SWAP),
        (4, GEM5_FLAG_MEM_SWAP_COND),
        (1, GEM5_FLAG_ATOMIC_RETURN_OP),
    ] {
        assert_eq!(
            TrafficTrace::from_gem5_packet_trace(
                &gem5_packet_trace(
                    TICK_FREQUENCY,
                    &[PacketFields {
                        tick: 1,
                        command,
                        address: 0x200,
                        size: 8,
                        flags: Some(flags),
                    }],
                ),
                TICK_FREQUENCY,
            )
            .unwrap_err(),
            TrafficGeneratorError::TraceUnsupportedFlags { flags }
        );
    }
}

#[test]
fn trace_rejects_gem5_atomic_no_return_flag_until_request_policy_can_represent_it() {
    assert_eq!(
        TrafficTrace::from_gem5_packet_trace(
            &gem5_packet_trace(
                TICK_FREQUENCY,
                &[PacketFields {
                    tick: 1,
                    command: 34,
                    address: 0x200,
                    size: 8,
                    flags: Some(GEM5_FLAG_ATOMIC_NO_RETURN_OP),
                }],
            ),
            TICK_FREQUENCY,
        )
        .unwrap_err(),
        TrafficGeneratorError::TraceUnsupportedFlags {
            flags: GEM5_FLAG_ATOMIC_NO_RETURN_OP,
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
