use rem6_kernel::Tick;
use rem6_memory::Address;
use rem6_proto::{
    InstructionEncoding, InstructionKind, InstructionRecord, MemoryAccess, PacketCommand,
    PacketRecord, ProtoError, ProtoTrace, TraceHeader, TraceIdString, TraceSourceId,
};

fn header(source: &str) -> TraceHeader {
    TraceHeader::new(TraceSourceId::new(source).unwrap(), 1_000_000_000).unwrap()
}

fn mem_read(tick: Tick, pc: u64, addr: u64) -> InstructionRecord {
    InstructionRecord::new(
        pc,
        InstructionEncoding::word(0x0000_2083),
        0,
        1,
        tick,
        InstructionKind::MemRead,
    )
    .unwrap()
    .with_memory_access(MemoryAccess::new(Address::new(addr), 8, 0).unwrap())
    .unwrap()
}

#[test]
fn proto_trace_accepts_typed_instruction_and_packet_records() {
    let trace = ProtoTrace::builder(header("cpu0.icache"))
        .add_id_string(TraceIdString::new(9, "cpu0.data").unwrap())
        .add_id_string(TraceIdString::new(2, "cpu0.inst").unwrap())
        .add_instruction(mem_read(10, 0x8000, 0x9000))
        .add_packet(
            PacketRecord::new(11, PacketCommand::Read, Address::new(0x9000), 8)
                .unwrap()
                .with_packet_id(44)
                .with_pc(0x8000),
        )
        .build()
        .unwrap();

    assert_eq!(trace.header().source().as_str(), "cpu0.icache");
    assert_eq!(trace.id_strings()[0].key(), 2);
    assert_eq!(trace.id_strings()[1].key(), 9);
    assert_eq!(
        trace.instructions()[0].memory_accesses()[0].address(),
        Address::new(0x9000)
    );
    assert_eq!(trace.packets()[0].packet_id(), Some(44));
    assert!(!trace.identity().as_str().is_empty());

    let reordered_map_trace = ProtoTrace::builder(header("cpu0.icache"))
        .add_id_string(TraceIdString::new(2, "cpu0.inst").unwrap())
        .add_id_string(TraceIdString::new(9, "cpu0.data").unwrap())
        .add_instruction(mem_read(10, 0x8000, 0x9000))
        .add_packet(
            PacketRecord::new(11, PacketCommand::Read, Address::new(0x9000), 8)
                .unwrap()
                .with_packet_id(44)
                .with_pc(0x8000),
        )
        .build()
        .unwrap();
    assert_eq!(trace.identity(), reordered_map_trace.identity());
}

#[test]
fn proto_trace_rejects_untyped_or_ambiguous_external_records() {
    assert_eq!(
        TraceSourceId::new("").unwrap_err(),
        ProtoError::EmptyTraceSource,
    );
    assert_eq!(
        TraceHeader::new(TraceSourceId::new("cpu0").unwrap(), 0).unwrap_err(),
        ProtoError::ZeroTickFrequency,
    );
    assert_eq!(
        InstructionEncoding::bytes(Vec::new()).unwrap_err(),
        ProtoError::EmptyInstructionBytes,
    );
    assert_eq!(
        MemoryAccess::new(Address::new(0x1000), 0, 0).unwrap_err(),
        ProtoError::ZeroMemoryAccessSize,
    );
    assert_eq!(
        PacketRecord::new(1, PacketCommand::Write, Address::new(0x2000), 0).unwrap_err(),
        ProtoError::ZeroPacketSize,
    );
    let missing_mem_access = InstructionRecord::new(
        0x8000,
        InstructionEncoding::word(0x0000_0013),
        0,
        0,
        1,
        InstructionKind::MemWrite,
    )
    .unwrap();
    assert_eq!(
        ProtoTrace::builder(header("cpu0"))
            .add_instruction(missing_mem_access)
            .build()
            .unwrap_err(),
        ProtoError::MissingInstructionMemoryAccess {
            kind: InstructionKind::MemWrite,
        },
    );
    assert_eq!(
        InstructionRecord::new(
            0x8000,
            InstructionEncoding::word(0x0000_0013),
            0,
            0,
            1,
            InstructionKind::IntAlu,
        )
        .unwrap()
        .with_memory_access(MemoryAccess::new(Address::new(0x1000), 4, 0).unwrap())
        .unwrap_err(),
        ProtoError::UnexpectedInstructionMemoryAccess {
            kind: InstructionKind::IntAlu,
        },
    );
    assert_eq!(
        ProtoTrace::builder(header("cpu0"))
            .add_id_string(TraceIdString::new(1, "first").unwrap())
            .add_id_string(TraceIdString::new(1, "second").unwrap())
            .build()
            .unwrap_err(),
        ProtoError::DuplicateTraceIdString { key: 1 },
    );
}
