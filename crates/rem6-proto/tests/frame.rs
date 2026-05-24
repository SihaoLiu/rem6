use rem6_memory::Address;
use rem6_proto::{
    DependencyRecord, DependencyRecordKind, DependencyTrace, DependencyTraceHeader,
    InstructionEncoding, InstructionKind, InstructionRecord, MemoryAccess, PacketCommand,
    PacketRecord, ProtoError, ProtoTrace, TraceFrame, TraceFrameKind, TraceHeader, TraceSourceId,
};

fn instruction_trace() -> ProtoTrace {
    ProtoTrace::builder(
        TraceHeader::new(TraceSourceId::new("cpu0.proto").unwrap(), 1_000_000_000).unwrap(),
    )
    .add_instruction(
        InstructionRecord::new(
            0x8000,
            InstructionEncoding::word(0x0000_2083),
            0,
            0,
            10,
            InstructionKind::MemRead,
        )
        .unwrap()
        .with_memory_access(MemoryAccess::new(Address::new(0x9000), 8, 0).unwrap())
        .unwrap(),
    )
    .add_packet(PacketRecord::new(11, PacketCommand::Read, Address::new(0x9000), 8).unwrap())
    .build()
    .unwrap()
}

fn dependency_trace() -> DependencyTrace {
    DependencyTrace::builder(
        DependencyTraceHeader::new(TraceSourceId::new("cpu0.dep").unwrap(), 1_000_000_000, 64)
            .unwrap(),
    )
    .add_record(
        DependencyRecord::new(1, DependencyRecordKind::Compute)
            .unwrap()
            .with_compute_delay(4)
            .with_pc(0x8000),
    )
    .build()
    .unwrap()
}

#[test]
fn trace_frame_round_trips_typed_trace_identity_and_payload() {
    let trace = instruction_trace();
    let frame = TraceFrame::from_proto_trace(&trace, vec![1, 2, 3, 4]).unwrap();

    assert_eq!(frame.kind(), TraceFrameKind::InstructionPacketTrace);
    assert_eq!(frame.identity(), trace.identity().as_str());
    assert_eq!(frame.payload(), &[1, 2, 3, 4]);

    let bytes = frame.encode();
    let decoded = TraceFrame::decode(&bytes).unwrap();
    assert_eq!(decoded, frame);

    let dep_trace = dependency_trace();
    let dep_frame = TraceFrame::from_dependency_trace(&dep_trace, vec![8, 9]).unwrap();
    let dep_decoded = TraceFrame::decode(&dep_frame.encode()).unwrap();
    assert_eq!(dep_decoded.kind(), TraceFrameKind::DependencyTrace);
    assert_eq!(dep_decoded.identity(), dep_trace.identity().as_str());
}

#[test]
fn trace_frame_rejects_corrupt_or_ambiguous_binary_records() {
    assert_eq!(
        TraceFrame::new(TraceFrameKind::InstructionPacketTrace, "", vec![1]).unwrap_err(),
        ProtoError::EmptyFrameIdentity,
    );
    assert_eq!(
        TraceFrame::new(
            TraceFrameKind::InstructionPacketTrace,
            "0123456789abcdef",
            Vec::new(),
        )
        .unwrap_err(),
        ProtoError::EmptyFramePayload,
    );

    let frame = TraceFrame::from_proto_trace(&instruction_trace(), vec![1, 2, 3]).unwrap();
    let bytes = frame.encode();

    let mut bad_magic = bytes.clone();
    bad_magic[0] = b'X';
    assert_eq!(
        TraceFrame::decode(&bad_magic).unwrap_err(),
        ProtoError::InvalidFrameMagic,
    );

    let mut bad_kind = bytes.clone();
    bad_kind[6] = 99;
    assert_eq!(
        TraceFrame::decode(&bad_kind).unwrap_err(),
        ProtoError::UnknownFrameKind { kind: 99 },
    );

    let truncated = &bytes[..bytes.len() - 2];
    assert_eq!(
        TraceFrame::decode(truncated).unwrap_err(),
        ProtoError::TruncatedFrame,
    );

    let mut bad_checksum = bytes;
    let last = bad_checksum.len() - 1;
    bad_checksum[last] ^= 0xff;
    assert_eq!(
        TraceFrame::decode(&bad_checksum).unwrap_err(),
        ProtoError::FrameChecksumMismatch,
    );
}
