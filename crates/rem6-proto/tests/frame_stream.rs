use rem6_memory::Address;
use rem6_proto::{
    DependencyRecord, DependencyRecordKind, DependencyTrace, DependencyTraceHeader,
    InstructionEncoding, InstructionKind, InstructionRecord, MemoryAccess, PacketCommand,
    PacketRecord, ProtoError, ProtoTrace, TraceFrame, TraceFrameKind, TraceFrameStream,
    TraceFrameStreamCursor, TraceFrameStreamIndex, TraceHeader, TraceSourceId,
};

fn instruction_trace(source: &str, tick: u64) -> ProtoTrace {
    ProtoTrace::builder(
        TraceHeader::new(TraceSourceId::new(source).unwrap(), 1_000_000_000).unwrap(),
    )
    .add_instruction(
        InstructionRecord::new(
            0x8000 + tick,
            InstructionEncoding::word(0x0000_2083),
            0,
            0,
            tick,
            InstructionKind::MemRead,
        )
        .unwrap()
        .with_memory_access(MemoryAccess::new(Address::new(0x9000 + tick), 8, 0).unwrap())
        .unwrap(),
    )
    .add_packet(
        PacketRecord::new(
            tick + 1,
            PacketCommand::Read,
            Address::new(0x9000 + tick),
            8,
        )
        .unwrap(),
    )
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
    .add_record(
        DependencyRecord::new(2, DependencyRecordKind::Load)
            .unwrap()
            .with_order_dependency(1)
            .unwrap()
            .with_physical_address(Address::new(0x9000))
            .with_virtual_address(Address::new(0xffff_9000))
            .with_size(8)
            .unwrap(),
    )
    .build()
    .unwrap()
}

#[test]
fn trace_frame_stream_round_trips_ordered_frames() {
    let first_trace = instruction_trace("cpu0.proto", 10);
    let second_trace = instruction_trace("cpu1.proto", 20);
    let dependency_trace = dependency_trace();

    let first = TraceFrame::from_proto_trace(&first_trace, vec![1, 2, 3]).unwrap();
    let dependency = TraceFrame::from_dependency_trace(&dependency_trace, vec![8, 9]).unwrap();
    let second = TraceFrame::from_proto_trace(&second_trace, vec![4, 5, 6]).unwrap();

    let stream =
        TraceFrameStream::new(vec![first.clone(), dependency.clone(), second.clone()]).unwrap();
    let encoded = stream.encode();
    let decoded = TraceFrameStream::decode(&encoded).unwrap();

    assert_eq!(decoded.frames(), &[first, dependency, second]);
    assert_eq!(
        decoded.frames()[0].kind(),
        TraceFrameKind::InstructionPacketTrace
    );
    assert_eq!(decoded.frames()[1].kind(), TraceFrameKind::DependencyTrace);
    assert_eq!(
        decoded.frames()[2].identity(),
        second_trace.identity().as_str()
    );
}

#[test]
fn trace_frame_stream_cursor_reads_ordered_frames_with_byte_offsets() {
    let first_trace = instruction_trace("cpu0.proto", 10);
    let dependency_trace = dependency_trace();
    let first = TraceFrame::from_proto_trace(&first_trace, vec![1, 2, 3]).unwrap();
    let dependency = TraceFrame::from_dependency_trace(&dependency_trace, vec![8, 9]).unwrap();
    let first_len = first.encode().len();
    let dependency_len = dependency.encode().len();
    let encoded = TraceFrameStream::new(vec![first.clone(), dependency.clone()])
        .unwrap()
        .encode();

    let mut cursor = TraceFrameStreamCursor::new(&encoded).unwrap();
    assert_eq!(cursor.next_index(), 0);
    assert_eq!(cursor.byte_position(), 6);

    let first_record = cursor.next_frame().unwrap().unwrap();
    assert_eq!(first_record.index(), 0);
    assert_eq!(first_record.length_offset(), 6);
    assert!(first_record.frame_offset() > first_record.length_offset());
    assert_eq!(first_record.frame_len(), first_len);
    assert_eq!(first_record.frame(), &first);
    assert_eq!(cursor.next_index(), 1);
    assert_eq!(
        cursor.byte_position(),
        first_record.frame_offset() + first_record.frame_len()
    );

    let second_length_offset = cursor.byte_position();
    let second_record = cursor.next_frame().unwrap().unwrap();
    assert_eq!(second_record.index(), 1);
    assert_eq!(second_record.length_offset(), second_length_offset);
    assert_eq!(second_record.frame_len(), dependency_len);
    assert_eq!(second_record.frame(), &dependency);
    assert!(cursor.is_finished());
    assert!(cursor.next_frame().unwrap().is_none());

    cursor.reset();
    assert_eq!(cursor.next_index(), 0);
    assert_eq!(cursor.byte_position(), 6);
    assert_eq!(cursor.next_frame().unwrap().unwrap().frame(), &first);
}

#[test]
fn trace_frame_stream_cursor_rejects_bad_input_without_advancing() {
    let header_len = 6;
    let frame =
        TraceFrame::from_proto_trace(&instruction_trace("cpu0.proto", 10), vec![1, 2, 3]).unwrap();
    let encoded = TraceFrameStream::new(vec![frame]).unwrap().encode();

    assert_eq!(
        TraceFrameStreamCursor::new(&encoded[..header_len]).unwrap_err(),
        ProtoError::EmptyFrameStream,
    );

    let mut bad_magic = encoded.clone();
    bad_magic[0] = b'X';
    assert_eq!(
        TraceFrameStreamCursor::new(&bad_magic).unwrap_err(),
        ProtoError::InvalidFrameStreamMagic,
    );

    let mut truncated_length = encoded[..header_len].to_vec();
    truncated_length.push(0x80);
    let mut truncated_cursor = TraceFrameStreamCursor::new(&truncated_length).unwrap();
    assert_eq!(
        truncated_cursor.next_frame().unwrap_err(),
        ProtoError::TruncatedFrameStream,
    );
    assert_eq!(truncated_cursor.byte_position(), header_len);
    assert_eq!(truncated_cursor.next_index(), 0);

    let mut corrupt_frame = encoded;
    let last = corrupt_frame.len() - 1;
    corrupt_frame[last] ^= 0xff;
    let mut corrupt_cursor = TraceFrameStreamCursor::new(&corrupt_frame).unwrap();
    assert_eq!(
        corrupt_cursor.next_frame().unwrap_err(),
        ProtoError::FrameChecksumMismatch,
    );
    assert_eq!(corrupt_cursor.byte_position(), header_len);
    assert_eq!(corrupt_cursor.next_index(), 0);
}

#[test]
fn trace_frame_stream_index_exposes_stable_record_metadata_for_partitioned_ingestion() {
    let first_trace = instruction_trace("cpu0.proto", 10);
    let second_trace = instruction_trace("cpu1.proto", 20);
    let dependency_trace = dependency_trace();
    let first = TraceFrame::from_proto_trace(&first_trace, vec![1, 2, 3]).unwrap();
    let dependency = TraceFrame::from_dependency_trace(&dependency_trace, vec![8, 9]).unwrap();
    let second = TraceFrame::from_proto_trace(&second_trace, vec![4, 5, 6, 7]).unwrap();
    let first_len = first.encode().len();
    let dependency_len = dependency.encode().len();
    let second_len = second.encode().len();
    let encoded = TraceFrameStream::new(vec![first.clone(), dependency.clone(), second.clone()])
        .unwrap()
        .encode();

    let index = TraceFrameStreamIndex::from_bytes(&encoded).unwrap();

    assert_eq!(index.len(), 3);
    assert!(!index.is_empty());
    assert_eq!(
        index.total_frame_bytes(),
        first_len + dependency_len + second_len
    );
    assert_eq!(index.total_payload_bytes(), 3 + 2 + 4);
    assert_eq!(index.count_kind(TraceFrameKind::InstructionPacketTrace), 2);
    assert_eq!(index.count_kind(TraceFrameKind::DependencyTrace), 1);

    let records = index.records();
    assert_eq!(records[0].index(), 0);
    assert_eq!(records[0].kind(), TraceFrameKind::InstructionPacketTrace);
    assert_eq!(records[0].identity(), first_trace.identity().as_str());
    assert_eq!(records[0].length_offset(), 6);
    assert_eq!(records[0].frame_len(), first_len);
    assert_eq!(records[0].payload_len(), 3);
    assert_eq!(
        records[0].byte_range(),
        records[0].frame_offset()..records[0].frame_offset() + first_len
    );

    assert_eq!(records[1].kind(), TraceFrameKind::DependencyTrace);
    assert_eq!(records[1].identity(), dependency_trace.identity().as_str());
    assert_eq!(records[1].payload_len(), 2);

    let instruction_identities = index
        .records_for_kind(TraceFrameKind::InstructionPacketTrace)
        .map(|record| record.identity())
        .collect::<Vec<_>>();
    assert_eq!(
        instruction_identities,
        vec![
            first_trace.identity().as_str(),
            second_trace.identity().as_str()
        ]
    );
}

#[test]
fn trace_frame_stream_index_rejects_corrupt_streams() {
    let frame =
        TraceFrame::from_proto_trace(&instruction_trace("cpu0.proto", 10), vec![1, 2, 3]).unwrap();
    let encoded = TraceFrameStream::new(vec![frame]).unwrap().encode();

    let mut bad_magic = encoded.clone();
    bad_magic[0] = b'X';
    assert_eq!(
        TraceFrameStreamIndex::from_bytes(&bad_magic).unwrap_err(),
        ProtoError::InvalidFrameStreamMagic,
    );

    let mut corrupt_frame = encoded;
    let last = corrupt_frame.len() - 1;
    corrupt_frame[last] ^= 0xff;
    assert_eq!(
        TraceFrameStreamIndex::from_bytes(&corrupt_frame).unwrap_err(),
        ProtoError::FrameChecksumMismatch,
    );
}

#[test]
fn trace_frame_stream_rejects_ambiguous_or_corrupt_records() {
    assert_eq!(
        TraceFrameStream::new(Vec::new()).unwrap_err(),
        ProtoError::EmptyFrameStream,
    );

    let frame =
        TraceFrame::from_proto_trace(&instruction_trace("cpu0.proto", 10), vec![1, 2, 3]).unwrap();
    let encoded = TraceFrameStream::new(vec![frame]).unwrap().encode();

    let header_len = 6;
    assert_eq!(
        TraceFrameStream::decode(&encoded[..header_len]).unwrap_err(),
        ProtoError::EmptyFrameStream,
    );

    let mut bad_magic = encoded.clone();
    bad_magic[0] = b'X';
    assert_eq!(
        TraceFrameStream::decode(&bad_magic).unwrap_err(),
        ProtoError::InvalidFrameStreamMagic,
    );

    let mut bad_version = encoded.clone();
    bad_version[4] = 2;
    assert_eq!(
        TraceFrameStream::decode(&bad_version).unwrap_err(),
        ProtoError::UnsupportedFrameStreamVersion { version: 2 },
    );

    let mut truncated_length = encoded[..header_len].to_vec();
    truncated_length.push(0x80);
    assert_eq!(
        TraceFrameStream::decode(&truncated_length).unwrap_err(),
        ProtoError::TruncatedFrameStream,
    );

    let mut overlong_length = encoded[..header_len].to_vec();
    overlong_length.extend_from_slice(&[0xff, 0xff, 0xff, 0xff, 0x10]);
    assert_eq!(
        TraceFrameStream::decode(&overlong_length).unwrap_err(),
        ProtoError::InvalidFrameStreamLength,
    );

    let mut empty_record = encoded[..header_len].to_vec();
    empty_record.push(0);
    assert_eq!(
        TraceFrameStream::decode(&empty_record).unwrap_err(),
        ProtoError::InvalidFrameStreamLength,
    );

    let mut truncated_frame = encoded[..header_len].to_vec();
    truncated_frame.extend_from_slice(&[10]);
    truncated_frame.extend_from_slice(&[1, 2, 3]);
    assert_eq!(
        TraceFrameStream::decode(&truncated_frame).unwrap_err(),
        ProtoError::TruncatedFrameStream,
    );

    let mut corrupt_frame = encoded;
    let last = corrupt_frame.len() - 1;
    corrupt_frame[last] ^= 0xff;
    assert_eq!(
        TraceFrameStream::decode(&corrupt_frame).unwrap_err(),
        ProtoError::FrameChecksumMismatch,
    );
}
