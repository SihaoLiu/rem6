use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, CoherenceIntent, MemoryAccessOrdering,
    MemoryAtomicOp, MemoryBarrierSet, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestCheckpointPayload, MemoryRequestId, MemoryResponse,
    MemoryResponseCheckpointPayload, MemoryResponseSnapshot, ResponseStatus,
};

const OVERSIZED_VECTOR_LENGTH: u64 = isize::MAX as u64 + 1;

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

#[test]
fn read_request_derives_range_and_line_metadata() {
    let request = MemoryRequest::read_shared(
        request_id(11),
        Address::new(0x1003),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.id(), request_id(11));
    assert_eq!(request.operation(), MemoryOperation::ReadShared);
    assert_eq!(request.coherence_intent(), CoherenceIntent::ReadShared);
    assert_eq!(request.range().start(), Address::new(0x1003));
    assert_eq!(request.range().end(), Address::new(0x100b));
    assert_eq!(request.line_address(), Address::new(0x1000));
    assert_eq!(request.line_offset(), 3);
    assert_eq!(request.line_span(), 1);
    assert!(request.requires_response());
    assert!(request.returns_data());
    assert!(!request.carries_data());
}

#[test]
fn request_reports_cross_line_accesses() {
    let request = MemoryRequest::read_shared(
        request_id(12),
        Address::new(0x103e),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.range().start(), Address::new(0x103e));
    assert_eq!(request.range().end(), Address::new(0x1042));
    assert_eq!(request.line_address(), Address::new(0x1000));
    assert_eq!(request.line_offset(), 62);
    assert_eq!(request.line_span(), 2);
}

#[test]
fn request_default_ordering_is_empty_and_builder_preserves_edges() {
    let request = MemoryRequest::read_shared(
        request_id(15),
        Address::new(0x2000),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.ordering(), MemoryAccessOrdering::none());

    let ordered = request.with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        Some(MemoryBarrierSet::memory()),
    ));

    assert_eq!(
        ordered.ordering(),
        MemoryAccessOrdering::new(
            Some(MemoryBarrierSet::memory()),
            Some(MemoryBarrierSet::memory())
        )
    );
}

#[test]
fn request_uncacheable_builder_sets_strict_order_independently_of_barriers() {
    let request = MemoryRequest::read_shared(
        request_id(16),
        Address::new(0x2100),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert!(!request.is_uncacheable());
    assert!(!request.is_strict_ordered());
    assert_eq!(request.ordering(), MemoryAccessOrdering::none());

    let marked = request.with_uncacheable_strict_order();

    assert!(marked.is_uncacheable());
    assert!(marked.is_strict_ordered());
    assert_eq!(marked.ordering(), MemoryAccessOrdering::none());
}

#[test]
fn request_rejects_invalid_sizes_and_address_overflow() {
    assert_eq!(AccessSize::new(0).unwrap_err(), MemoryError::ZeroAccessSize);

    let overflow = MemoryRequest::read_shared(
        request_id(13),
        Address::new(u64::MAX - 3),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap_err();

    assert_eq!(
        overflow,
        MemoryError::AddressOverflow {
            start: Address::new(u64::MAX - 3),
            size: AccessSize::new(8).unwrap(),
        }
    );
}

#[test]
fn byte_mask_full_rejects_sizes_above_vec_capacity_before_allocation() {
    let size = AccessSize::new(OVERSIZED_VECTOR_LENGTH).unwrap();

    assert_eq!(
        ByteMask::full(size),
        Err(MemoryError::AccessSizeTooLarge { size })
    );
}

#[test]
fn line_layout_requires_nonzero_power_of_two_bytes() {
    assert_eq!(
        CacheLineLayout::new(0).unwrap_err(),
        MemoryError::ZeroCacheLineSize
    );
    assert_eq!(
        CacheLineLayout::new(48).unwrap_err(),
        MemoryError::NonPowerOfTwoCacheLineSize { bytes: 48 }
    );
    assert_eq!(
        CacheLineLayout::new(OVERSIZED_VECTOR_LENGTH).unwrap_err(),
        MemoryError::CacheLineSizeTooLarge {
            bytes: OVERSIZED_VECTOR_LENGTH
        }
    );

    let layout = CacheLineLayout::new(128).unwrap();
    assert_eq!(layout.bytes(), 128);
    assert_eq!(
        layout.line_address(Address::new(0x10ff)),
        Address::new(0x1080)
    );
    assert_eq!(layout.line_offset(Address::new(0x10ff)), 127);
}

#[test]
fn write_request_validates_payload_and_byte_mask() {
    let size = AccessSize::new(4).unwrap();
    let mask = ByteMask::from_bits(vec![true, false, true, true]).unwrap();
    let request = MemoryRequest::write(
        request_id(14),
        Address::new(0x2000),
        size,
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        mask.clone(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.operation(), MemoryOperation::Write);
    assert_eq!(request.coherence_intent(), CoherenceIntent::WriteUnique);
    assert_eq!(request.data(), Some(&[0xaa, 0xbb, 0xcc, 0xdd][..]));
    assert_eq!(request.byte_mask(), Some(&mask));
    assert!(request.carries_data());
    assert!(request.requires_writable());

    let short_payload = MemoryRequest::write(
        request_id(15),
        Address::new(0x2000),
        size,
        vec![0xaa, 0xbb],
        ByteMask::full(size).unwrap(),
        line_layout(),
    )
    .unwrap_err();

    assert_eq!(
        short_payload,
        MemoryError::PayloadSizeMismatch {
            expected: size,
            actual: 2
        }
    );

    let short_mask = MemoryRequest::write(
        request_id(16),
        Address::new(0x2000),
        size,
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        ByteMask::from_bits(vec![true, true]).unwrap(),
        line_layout(),
    )
    .unwrap_err();

    assert_eq!(
        short_mask,
        MemoryError::ByteMaskSizeMismatch {
            expected: size,
            actual: 2
        }
    );
}

#[test]
fn atomic_request_carries_data_and_returns_data() {
    let size = AccessSize::new(8).unwrap();
    let mask = ByteMask::full(size).unwrap();
    let request = MemoryRequest::atomic(
        request_id(21),
        Address::new(0x2080),
        size,
        vec![0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01],
        mask.clone(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.operation(), MemoryOperation::Atomic);
    assert_eq!(request.coherence_intent(), CoherenceIntent::WriteUnique);
    assert_eq!(request.range().start(), Address::new(0x2080));
    assert_eq!(
        request.data(),
        Some(&[0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01][..])
    );
    assert_eq!(request.byte_mask(), Some(&mask));
    assert!(request.carries_data());
    assert!(request.requires_writable());
    assert!(request.returns_data());
}

#[test]
fn coherence_operations_expose_protocol_relevant_attributes() {
    let upgrade = MemoryRequest::upgrade(
        request_id(17),
        Address::new(0x3000),
        AccessSize::new(1).unwrap(),
        line_layout(),
    )
    .unwrap();
    assert_eq!(upgrade.operation(), MemoryOperation::Upgrade);
    assert_eq!(upgrade.coherence_intent(), CoherenceIntent::Upgrade);
    assert!(upgrade.requires_writable());
    assert!(upgrade.requires_response());
    assert!(!upgrade.returns_data());

    let writeback = MemoryRequest::writeback_dirty(
        request_id(18),
        Address::new(0x4000),
        vec![0x5a; 64],
        line_layout(),
    )
    .unwrap();
    assert_eq!(writeback.operation(), MemoryOperation::WritebackDirty);
    assert_eq!(
        writeback.coherence_intent(),
        CoherenceIntent::WritebackDirty
    );
    assert!(writeback.carries_data());
    assert!(!writeback.requires_response());

    let unaligned_writeback = MemoryRequest::writeback_dirty(
        request_id(19),
        Address::new(0x4004),
        vec![0x5a; 64],
        line_layout(),
    )
    .unwrap_err();
    assert_eq!(
        unaligned_writeback,
        MemoryError::UnalignedLineAddress {
            address: Address::new(0x4004),
            line_size: 64
        }
    );

    let write_clean = MemoryRequest::write_clean(
        request_id(20),
        Address::new(0x5000),
        vec![0xa5; 64],
        line_layout(),
    )
    .unwrap();
    assert_eq!(write_clean.operation(), MemoryOperation::WriteClean);
    assert_eq!(write_clean.coherence_intent(), CoherenceIntent::WriteClean);
    assert!(write_clean.carries_data());
    assert_eq!(write_clean.byte_mask(), None);
    assert!(!write_clean.requires_writable());
    assert!(!write_clean.requires_response());
    assert!(!write_clean.returns_data());

    let unaligned_write_clean = MemoryRequest::write_clean(
        request_id(21),
        Address::new(0x5004),
        vec![0xa5; 64],
        line_layout(),
    )
    .unwrap_err();
    assert_eq!(
        unaligned_write_clean,
        MemoryError::UnalignedLineAddress {
            address: Address::new(0x5004),
            line_size: 64
        }
    );

    let clean_shared =
        MemoryRequest::clean_shared(request_id(22), Address::new(0x6000), line_layout()).unwrap();
    assert_eq!(clean_shared.operation(), MemoryOperation::CleanShared);
    assert_eq!(
        clean_shared.coherence_intent(),
        CoherenceIntent::CleanShared
    );
    assert_eq!(clean_shared.byte_mask(), None);
    assert!(!clean_shared.carries_data());
    assert!(!clean_shared.requires_writable());
    assert!(clean_shared.requires_response());
    assert!(!clean_shared.returns_data());

    let unaligned_clean_shared =
        MemoryRequest::clean_shared(request_id(23), Address::new(0x6004), line_layout())
            .unwrap_err();
    assert_eq!(
        unaligned_clean_shared,
        MemoryError::UnalignedLineAddress {
            address: Address::new(0x6004),
            line_size: 64
        }
    );
}

#[test]
fn responses_validate_request_data_contracts() {
    let read = MemoryRequest::read_shared(
        request_id(20),
        Address::new(0x5000),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let read_response =
        MemoryResponse::completed(&read, Some(vec![0xde, 0xad, 0xbe, 0xef])).unwrap();

    assert_eq!(read_response.request_id(), read.id());
    assert_eq!(read_response.status(), ResponseStatus::Completed);
    assert_eq!(read_response.data(), Some(&[0xde, 0xad, 0xbe, 0xef][..]));

    let missing_read_data = MemoryResponse::completed(&read, None).unwrap_err();
    assert_eq!(
        missing_read_data,
        MemoryError::MissingResponseData { request: read.id() }
    );

    let write = MemoryRequest::write(
        request_id(21),
        Address::new(0x5008),
        AccessSize::new(2).unwrap(),
        vec![0x11, 0x22],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        line_layout(),
    )
    .unwrap();
    let write_response = MemoryResponse::completed(&write, None).unwrap();
    assert_eq!(write_response.status(), ResponseStatus::Completed);
    assert!(write_response.data().is_none());

    let unexpected_write_data = MemoryResponse::completed(&write, Some(vec![0x33])).unwrap_err();
    assert_eq!(
        unexpected_write_data,
        MemoryError::UnexpectedResponseData {
            request: write.id()
        }
    );

    let retry = MemoryResponse::retry(&read);
    assert_eq!(retry.status(), ResponseStatus::Retry);
    assert!(retry.data().is_none());
}

#[test]
fn response_snapshot_rejects_empty_data_payload() {
    assert_eq!(
        MemoryResponseSnapshot::new(request_id(34), ResponseStatus::Completed, Some(Vec::new()))
            .unwrap_err(),
        MemoryError::InvalidResponseDataLength {
            request: request_id(34),
            length: 0
        }
    );
}

#[test]
fn memory_response_checkpoint_payload_round_trips_completed_data_and_retry() {
    let read = MemoryRequest::read_shared(
        request_id(29),
        Address::new(0x7600),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let completed = MemoryResponse::completed(&read, Some(vec![0xde, 0xad, 0xbe, 0xef])).unwrap();
    let completed_payload = MemoryResponseCheckpointPayload::from_response(&completed);
    let decoded =
        MemoryResponseCheckpointPayload::decode(completed_payload.encode().as_slice()).unwrap();
    let restored = MemoryResponse::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &completed.snapshot());
    assert_eq!(restored, completed);
    assert_eq!(restored.request_id(), request_id(29));
    assert_eq!(restored.status(), ResponseStatus::Completed);
    assert_eq!(restored.data(), Some(&[0xde, 0xad, 0xbe, 0xef][..]));

    let retry = MemoryResponse::retry(&read);
    let retry_payload = MemoryResponseCheckpointPayload::from_response(&retry);
    let retry_decoded =
        MemoryResponseCheckpointPayload::decode(retry_payload.encode().as_slice()).unwrap();
    let retry_restored = MemoryResponse::from_snapshot(retry_decoded.snapshot()).unwrap();

    assert_eq!(retry_decoded.snapshot(), &retry.snapshot());
    assert_eq!(retry_restored, retry);
    assert_eq!(retry_restored.status(), ResponseStatus::Retry);
    assert!(retry_restored.data().is_none());
}

#[test]
fn memory_response_checkpoint_payload_round_trips_completed_without_data() {
    let write = MemoryRequest::write(
        request_id(35),
        Address::new(0x7b00),
        AccessSize::new(2).unwrap(),
        vec![0x11, 0x22],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        line_layout(),
    )
    .unwrap();
    let completed = MemoryResponse::completed(&write, None).unwrap();
    let payload = MemoryResponseCheckpointPayload::from_response(&completed);
    let decoded = MemoryResponseCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryResponse::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &completed.snapshot());
    assert_eq!(restored, completed);
    assert_eq!(restored.request_id(), request_id(35));
    assert_eq!(restored.status(), ResponseStatus::Completed);
    assert!(restored.data().is_none());
}

#[test]
fn memory_response_checkpoint_payload_uses_stable_completed_data_bytes() {
    let read = MemoryRequest::read_shared(
        request_id(36),
        Address::new(0x7c00),
        AccessSize::new(2).unwrap(),
        line_layout(),
    )
    .unwrap();
    let completed = MemoryResponse::completed(&read, Some(vec![0xaa, 0xbb])).unwrap();

    let stable_payload = vec![
        b'M', b'R', b'E', b'S', 1, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 36, 0,
        0, 0, 0, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0xaa, 0xbb,
    ];
    let decoded = MemoryResponseCheckpointPayload::decode(&stable_payload).unwrap();
    let restored = MemoryResponse::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(
        MemoryResponseCheckpointPayload::from_response(&completed).encode(),
        stable_payload
    );
    assert_eq!(decoded.snapshot(), &completed.snapshot());
    assert_eq!(restored, completed);
}

#[test]
fn memory_response_checkpoint_payload_uses_stable_completed_without_data_bytes() {
    let write = MemoryRequest::write(
        request_id(37),
        Address::new(0x7d00),
        AccessSize::new(2).unwrap(),
        vec![0x11, 0x22],
        ByteMask::full(AccessSize::new(2).unwrap()).unwrap(),
        line_layout(),
    )
    .unwrap();
    let completed = MemoryResponse::completed(&write, None).unwrap();

    let stable_payload = vec![
        b'M', b'R', b'E', b'S', 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 37, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let decoded = MemoryResponseCheckpointPayload::decode(&stable_payload).unwrap();
    let restored = MemoryResponse::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(
        MemoryResponseCheckpointPayload::from_response(&completed).encode(),
        stable_payload
    );
    assert_eq!(decoded.snapshot(), &completed.snapshot());
    assert_eq!(restored, completed);
}

#[test]
fn memory_response_checkpoint_payload_rejects_invalid_status_code() {
    let response = MemoryResponse::retry(
        &MemoryRequest::read_shared(
            request_id(30),
            Address::new(0x7700),
            AccessSize::new(4).unwrap(),
            line_layout(),
        )
        .unwrap(),
    );
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_STATUS_OFFSET..RESPONSE_CHECKPOINT_STATUS_OFFSET + 4]
        .copy_from_slice(&99u32.to_le_bytes());

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointStatus { code: 99 }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_invalid_magic() {
    let response = MemoryResponse::retry(
        &MemoryRequest::read_shared(
            request_id(38),
            Address::new(0x7e00),
            AccessSize::new(4).unwrap(),
            line_layout(),
        )
        .unwrap(),
    );
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[0] = b'X';

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointMagic
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_unsupported_version() {
    let response = MemoryResponse::retry(
        &MemoryRequest::read_shared(
            request_id(39),
            Address::new(0x7f00),
            AccessSize::new(4).unwrap(),
            line_layout(),
        )
        .unwrap(),
    );
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_VERSION_OFFSET..RESPONSE_CHECKPOINT_VERSION_OFFSET + 4]
        .copy_from_slice(&2u32.to_le_bytes());

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::UnsupportedResponseCheckpointVersion { version: 2 }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_reserved_flag_bits() {
    let response = MemoryResponse::retry(
        &MemoryRequest::read_shared(
            request_id(31),
            Address::new(0x7800),
            AccessSize::new(4).unwrap(),
            line_layout(),
        )
        .unwrap(),
    );
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_FLAGS_OFFSET..RESPONSE_CHECKPOINT_FLAGS_OFFSET + 4]
        .copy_from_slice(&0x8000_0000u32.to_le_bytes());

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointFlags { flags: 0x8000_0000 }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_reserved_field() {
    let response = MemoryResponse::retry(
        &MemoryRequest::read_shared(
            request_id(40),
            Address::new(0x8000),
            AccessSize::new(4).unwrap(),
            line_layout(),
        )
        .unwrap(),
    );
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_RESERVED_OFFSET..RESPONSE_CHECKPOINT_RESERVED_OFFSET + 4]
        .copy_from_slice(&1u32.to_le_bytes());

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointReserved { value: 1 }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_short_payload() {
    let response = MemoryResponse::retry(
        &MemoryRequest::read_shared(
            request_id(41),
            Address::new(0x8100),
            AccessSize::new(4).unwrap(),
            line_layout(),
        )
        .unwrap(),
    );
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload.truncate(RESPONSE_CHECKPOINT_HEADER_SIZE - 1);

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointPayloadSize {
            expected: RESPONSE_CHECKPOINT_HEADER_SIZE,
            actual: RESPONSE_CHECKPOINT_HEADER_SIZE - 1
        }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_declared_data_larger_than_payload() {
    let read = MemoryRequest::read_shared(
        request_id(42),
        Address::new(0x8200),
        AccessSize::new(2).unwrap(),
        line_layout(),
    )
    .unwrap();
    let response = MemoryResponse::completed(&read, Some(vec![0x10, 0x20])).unwrap();
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET..RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET + 8]
        .copy_from_slice(&3u64.to_le_bytes());

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointPayloadSize {
            expected: RESPONSE_CHECKPOINT_HEADER_SIZE + 3,
            actual: RESPONSE_CHECKPOINT_HEADER_SIZE + 2
        }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_declared_data_smaller_than_payload() {
    let read = MemoryRequest::read_shared(
        request_id(43),
        Address::new(0x8300),
        AccessSize::new(2).unwrap(),
        line_layout(),
    )
    .unwrap();
    let response = MemoryResponse::completed(&read, Some(vec![0x10, 0x20])).unwrap();
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET..RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET + 8]
        .copy_from_slice(&1u64.to_le_bytes());

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointPayloadSize {
            expected: RESPONSE_CHECKPOINT_HEADER_SIZE + 1,
            actual: RESPONSE_CHECKPOINT_HEADER_SIZE + 2
        }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_absent_data_with_nonzero_length() {
    let response = MemoryResponse::retry(
        &MemoryRequest::read_shared(
            request_id(32),
            Address::new(0x7900),
            AccessSize::new(4).unwrap(),
            line_layout(),
        )
        .unwrap(),
    );
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET..RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET + 8]
        .copy_from_slice(&1u64.to_le_bytes());
    payload.push(0);

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointDataLength { length: 1 }
    );
}

#[test]
fn memory_response_checkpoint_payload_rejects_present_data_with_zero_length() {
    let read = MemoryRequest::read_shared(
        request_id(33),
        Address::new(0x7a00),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let response = MemoryResponse::completed(&read, Some(vec![0x10, 0x20, 0x30, 0x40])).unwrap();
    let mut payload = MemoryResponseCheckpointPayload::from_response(&response).encode();
    payload[RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET..RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET + 8]
        .copy_from_slice(&0u64.to_le_bytes());
    payload.truncate(RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET + 8);

    assert_eq!(
        MemoryResponseCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidResponseCheckpointDataLength { length: 0 }
    );
}

#[test]
fn memory_request_checkpoint_payload_round_trips_atomic_ordering_and_flags() {
    let size = AccessSize::new(8).unwrap();
    let mask =
        ByteMask::from_bits(vec![true, false, true, false, true, false, true, false]).unwrap();
    let request = MemoryRequest::atomic_with_op(
        request_id(22),
        Address::new(0x6008),
        size,
        MemoryAtomicOp::Xor,
        vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
        mask.clone(),
        line_layout(),
    )
    .unwrap()
    .with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        Some(MemoryBarrierSet::new(true, false)),
    ))
    .with_uncacheable_strict_order();

    let snapshot = request.snapshot();
    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(restored, request);
    assert_eq!(restored.atomic_op(), Some(MemoryAtomicOp::Xor));
    assert_eq!(restored.byte_mask(), Some(&mask));
    assert!(restored.is_uncacheable());
    assert!(restored.is_strict_ordered());
}

#[test]
fn memory_request_checkpoint_payload_round_trips_write_clean() {
    let request = MemoryRequest::write_clean(
        request_id(23),
        Address::new(0x7000),
        vec![0x7c; 64],
        line_layout(),
    )
    .unwrap()
    .with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::new(false, true)),
        None,
    ));

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(restored, request);
    assert_eq!(restored.operation(), MemoryOperation::WriteClean);
    assert_eq!(restored.data(), Some(&vec![0x7c; 64][..]));
    assert_eq!(restored.byte_mask(), None);
}

#[test]
fn memory_request_checkpoint_payload_round_trips_clean_shared() {
    let request = MemoryRequest::clean_shared(request_id(24), Address::new(0x7400), line_layout())
        .unwrap()
        .with_ordering(MemoryAccessOrdering::new(
            Some(MemoryBarrierSet::new(true, false)),
            Some(MemoryBarrierSet::new(false, true)),
        ));

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(restored, request);
    assert_eq!(restored.operation(), MemoryOperation::CleanShared);
    assert_eq!(restored.data(), None);
    assert_eq!(restored.byte_mask(), None);
}

#[test]
fn memory_request_checkpoint_payload_rejects_invalid_operation_code() {
    let request = MemoryRequest::read_shared(
        request_id(23),
        Address::new(0x7000),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_OPERATION_OFFSET..REQUEST_CHECKPOINT_OPERATION_OFFSET + 4]
        .copy_from_slice(&99u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointOperation { code: 99 }
    );
}

#[test]
fn memory_request_checkpoint_payload_uses_stable_read_shared_bytes() {
    let request = MemoryRequest::read_shared(
        request_id(44),
        Address::new(0x8400),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();

    let stable_payload = vec![
        b'M', b'R', b'E', b'Q', 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 44, 0,
        0, 0, 0, 0, 0, 0, 0, 0x84, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let decoded = MemoryRequestCheckpointPayload::decode(&stable_payload).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(
        MemoryRequestCheckpointPayload::from_request(&request).encode(),
        stable_payload
    );
    assert_eq!(decoded.snapshot(), &request.snapshot());
    assert_eq!(restored, request);
}

#[test]
fn memory_request_checkpoint_payload_uses_stable_atomic_ordering_bytes() {
    let size = AccessSize::new(8).unwrap();
    let mask =
        ByteMask::from_bits(vec![true, false, true, false, true, false, true, false]).unwrap();
    let request = MemoryRequest::atomic_with_op(
        request_id(54),
        Address::new(0x8e08),
        size,
        MemoryAtomicOp::Xor,
        vec![0x10, 0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80],
        mask,
        line_layout(),
    )
    .unwrap()
    .with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::memory()),
        Some(MemoryBarrierSet::new(true, false)),
    ))
    .with_uncacheable_strict_order();

    let stable_payload = vec![
        b'M', b'R', b'E', b'Q', 1, 0, 0, 0, 7, 0, 0, 0, 0xff, 0x06, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0,
        54, 0, 0, 0, 0, 0, 0, 0, 0x08, 0x8e, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0,
        0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 0, 0, 0, 0, 0x10,
        0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x80, 1, 0, 1, 0, 1, 0, 1, 0,
    ];
    let decoded = MemoryRequestCheckpointPayload::decode(&stable_payload).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(
        MemoryRequestCheckpointPayload::from_request(&request).encode(),
        stable_payload
    );
    assert_eq!(decoded.snapshot(), &request.snapshot());
    assert_eq!(restored, request);
}

#[test]
fn memory_request_checkpoint_payload_rejects_invalid_magic() {
    let request = MemoryRequest::read_shared(
        request_id(45),
        Address::new(0x8500),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[0] = b'X';

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointMagic
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_unsupported_version() {
    let request = MemoryRequest::read_shared(
        request_id(46),
        Address::new(0x8600),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_VERSION_OFFSET..REQUEST_CHECKPOINT_VERSION_OFFSET + 4]
        .copy_from_slice(&2u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::UnsupportedRequestCheckpointVersion { version: 2 }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_reserved_flag_bits() {
    let request = MemoryRequest::read_shared(
        request_id(24),
        Address::new(0x7100),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_FLAGS_OFFSET..REQUEST_CHECKPOINT_FLAGS_OFFSET + 4]
        .copy_from_slice(&0x8000_0000u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointFlags { flags: 0x8000_0000 }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_primary_reserved_field() {
    let request = MemoryRequest::read_shared(
        request_id(47),
        Address::new(0x8700),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_PRIMARY_RESERVED_OFFSET
        ..REQUEST_CHECKPOINT_PRIMARY_RESERVED_OFFSET + 4]
        .copy_from_slice(&1u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointReserved { value: 1 }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_secondary_reserved_field() {
    let request = MemoryRequest::read_shared(
        request_id(48),
        Address::new(0x8800),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_SECONDARY_RESERVED_OFFSET
        ..REQUEST_CHECKPOINT_SECONDARY_RESERVED_OFFSET + 4]
        .copy_from_slice(&1u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointReserved { value: 1 }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_short_payload() {
    let request = MemoryRequest::read_shared(
        request_id(49),
        Address::new(0x8900),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload.truncate(REQUEST_CHECKPOINT_HEADER_SIZE - 1);

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointPayloadSize {
            expected: REQUEST_CHECKPOINT_HEADER_SIZE,
            actual: REQUEST_CHECKPOINT_HEADER_SIZE - 1
        }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_declared_data_larger_than_payload() {
    let request = MemoryRequest::write(
        request_id(50),
        Address::new(0x8a00),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::from_bits(vec![true, false]).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_DATA_LENGTH_OFFSET..REQUEST_CHECKPOINT_DATA_LENGTH_OFFSET + 8]
        .copy_from_slice(&3u64.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointPayloadSize {
            expected: REQUEST_CHECKPOINT_HEADER_SIZE + 3 + 2,
            actual: REQUEST_CHECKPOINT_HEADER_SIZE + 2 + 2
        }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_declared_data_smaller_than_payload() {
    let request = MemoryRequest::write(
        request_id(51),
        Address::new(0x8b00),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::from_bits(vec![true, false]).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_DATA_LENGTH_OFFSET..REQUEST_CHECKPOINT_DATA_LENGTH_OFFSET + 8]
        .copy_from_slice(&1u64.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointPayloadSize {
            expected: REQUEST_CHECKPOINT_HEADER_SIZE + 1 + 2,
            actual: REQUEST_CHECKPOINT_HEADER_SIZE + 2 + 2
        }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_declared_mask_larger_than_payload() {
    let request = MemoryRequest::write(
        request_id(52),
        Address::new(0x8c00),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::from_bits(vec![true, false]).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_MASK_LENGTH_OFFSET..REQUEST_CHECKPOINT_MASK_LENGTH_OFFSET + 8]
        .copy_from_slice(&3u64.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointPayloadSize {
            expected: REQUEST_CHECKPOINT_HEADER_SIZE + 2 + 3,
            actual: REQUEST_CHECKPOINT_HEADER_SIZE + 2 + 2
        }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_declared_mask_smaller_than_payload() {
    let request = MemoryRequest::write(
        request_id(53),
        Address::new(0x8d00),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::from_bits(vec![true, false]).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_MASK_LENGTH_OFFSET..REQUEST_CHECKPOINT_MASK_LENGTH_OFFSET + 8]
        .copy_from_slice(&1u64.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointPayloadSize {
            expected: REQUEST_CHECKPOINT_HEADER_SIZE + 2 + 1,
            actual: REQUEST_CHECKPOINT_HEADER_SIZE + 2 + 2
        }
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_ordering_bits_without_edge_presence() {
    let request = MemoryRequest::read_shared(
        request_id(25),
        Address::new(0x7200),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    payload[REQUEST_CHECKPOINT_FLAGS_OFFSET..REQUEST_CHECKPOINT_FLAGS_OFFSET + 4]
        .copy_from_slice(&REQUEST_CHECKPOINT_BEFORE_READ_FLAG.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointFlags {
            flags: REQUEST_CHECKPOINT_BEFORE_READ_FLAG
        }
    );
}

#[test]
fn memory_request_checkpoint_payload_preserves_empty_ordering_edges() {
    let request = MemoryRequest::read_shared(
        request_id(27),
        Address::new(0x7400),
        AccessSize::new(4).unwrap(),
        line_layout(),
    )
    .unwrap()
    .with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::new(false, false)),
        Some(MemoryBarrierSet::new(false, false)),
    ));

    let snapshot = request.snapshot();
    let payload = MemoryRequestCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(
        decoded.snapshot().ordering().before(),
        Some(MemoryBarrierSet::new(false, false))
    );
    assert_eq!(
        decoded.snapshot().ordering().after(),
        Some(MemoryBarrierSet::new(false, false))
    );
}

#[test]
fn memory_request_checkpoint_payload_rejects_invalid_mask_byte() {
    let request = MemoryRequest::write(
        request_id(28),
        Address::new(0x7500),
        AccessSize::new(2).unwrap(),
        vec![0xaa, 0xbb],
        ByteMask::from_bits(vec![true, false]).unwrap(),
        line_layout(),
    )
    .unwrap();
    let mut payload = MemoryRequestCheckpointPayload::from_request(&request).encode();
    let last = payload.last_mut().unwrap();
    *last = 2;

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointMaskBit { value: 2 }
    );
}

const REQUEST_CHECKPOINT_HEADER_SIZE: usize = 80;
const REQUEST_CHECKPOINT_VERSION_OFFSET: usize = 4;
const REQUEST_CHECKPOINT_OPERATION_OFFSET: usize = 8;
const REQUEST_CHECKPOINT_FLAGS_OFFSET: usize = 12;
const REQUEST_CHECKPOINT_PRIMARY_RESERVED_OFFSET: usize = 20;
const REQUEST_CHECKPOINT_DATA_LENGTH_OFFSET: usize = 56;
const REQUEST_CHECKPOINT_MASK_LENGTH_OFFSET: usize = 64;
const REQUEST_CHECKPOINT_SECONDARY_RESERVED_OFFSET: usize = 76;
const REQUEST_CHECKPOINT_BEFORE_READ_FLAG: u32 = 1 << 5;
const RESPONSE_CHECKPOINT_HEADER_SIZE: usize = 40;
const RESPONSE_CHECKPOINT_VERSION_OFFSET: usize = 4;
const RESPONSE_CHECKPOINT_STATUS_OFFSET: usize = 8;
const RESPONSE_CHECKPOINT_FLAGS_OFFSET: usize = 12;
const RESPONSE_CHECKPOINT_RESERVED_OFFSET: usize = 20;
const RESPONSE_CHECKPOINT_DATA_LENGTH_OFFSET: usize = 32;
