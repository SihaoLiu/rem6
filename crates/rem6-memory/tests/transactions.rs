use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, CoherenceIntent, MemoryAccessOrdering,
    MemoryBarrierSet, MemoryError, MemoryOperation, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};

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
fn line_layout_requires_nonzero_power_of_two_bytes() {
    assert_eq!(
        CacheLineLayout::new(0).unwrap_err(),
        MemoryError::ZeroCacheLineSize
    );
    assert_eq!(
        CacheLineLayout::new(48).unwrap_err(),
        MemoryError::NonPowerOfTwoCacheLineSize { bytes: 48 }
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
