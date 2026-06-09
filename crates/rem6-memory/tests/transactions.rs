use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, CoherenceIntent, LineMemoryStore,
    MemoryAccessOrdering, MemoryAtomicOp, MemoryBarrierSet, MemoryError, MemoryOperation,
    MemoryRequest, MemoryRequestCheckpointPayload, MemoryRequestId, MemoryRequestSnapshot,
    MemoryResponse, MemoryResponseCheckpointPayload, MemoryResponseSnapshot, ResponseStatus,
};

const OVERSIZED_VECTOR_LENGTH: u64 = isize::MAX as u64 + 1;

fn line_layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(7), sequence)
}

fn agent_request_id(agent: u32, sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(agent), sequence)
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
fn request_checkpoint_payload_round_trips_cacheable_strict_order() {
    let snapshot = MemoryRequestSnapshot::new(
        request_id(17),
        MemoryOperation::ReadShared,
        Address::new(0x2200),
        AccessSize::new(8).unwrap(),
        line_layout(),
        MemoryAccessOrdering::none(),
        false,
        true,
        None,
        None,
        None,
    )
    .unwrap();
    let payload = MemoryRequestCheckpointPayload::from_snapshot(snapshot.clone()).unwrap();
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(restored.operation(), MemoryOperation::ReadShared);
    assert!(!restored.is_uncacheable());
    assert!(restored.is_strict_ordered());
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
fn atomic_no_return_request_carries_data_without_returning_old_data() {
    let size = AccessSize::new(8).unwrap();
    let mask = ByteMask::full(size).unwrap();
    let request = MemoryRequest::atomic_no_return(
        request_id(24),
        Address::new(0x20c0),
        size,
        MemoryAtomicOp::Swap,
        vec![0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11],
        mask.clone(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(request.operation(), MemoryOperation::AtomicNoReturn);
    assert_eq!(request.atomic_op(), Some(MemoryAtomicOp::Swap));
    assert_eq!(request.coherence_intent(), CoherenceIntent::WriteUnique);
    assert_eq!(request.range().start(), Address::new(0x20c0));
    assert_eq!(
        request.data(),
        Some(&[0x18, 0x17, 0x16, 0x15, 0x14, 0x13, 0x12, 0x11][..])
    );
    assert_eq!(request.byte_mask(), Some(&mask));
    assert!(request.carries_data());
    assert!(request.requires_writable());
    assert!(request.requires_response());
    assert!(!request.returns_data());
}

#[test]
fn locked_rmw_requests_preserve_read_and_write_half_semantics() {
    let read = MemoryRequest::locked_rmw_read(
        request_id(22),
        Address::new(0x2108),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(read.operation(), MemoryOperation::LockedRmwRead);
    assert_eq!(read.coherence_intent(), CoherenceIntent::ReadUnique);
    assert_eq!(read.range().start(), Address::new(0x2108));
    assert_eq!(read.size(), AccessSize::new(8).unwrap());
    assert_eq!(read.data(), None);
    assert_eq!(read.byte_mask(), None);
    assert_eq!(read.atomic_op(), None);
    assert!(!read.carries_data());
    assert!(read.requires_writable());
    assert!(read.requires_response());
    assert!(read.returns_data());

    let size = AccessSize::new(8).unwrap();
    let mask = ByteMask::full(size).unwrap();
    let write = MemoryRequest::locked_rmw_write(
        request_id(23),
        Address::new(0x2110),
        size,
        vec![0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe],
        mask.clone(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(write.operation(), MemoryOperation::LockedRmwWrite);
    assert_eq!(write.coherence_intent(), CoherenceIntent::WriteUnique);
    assert_eq!(write.range().start(), Address::new(0x2110));
    assert_eq!(write.size(), size);
    assert_eq!(
        write.data(),
        Some(&[0x10, 0x32, 0x54, 0x76, 0x98, 0xba, 0xdc, 0xfe][..])
    );
    assert_eq!(write.byte_mask(), Some(&mask));
    assert_eq!(write.atomic_op(), None);
    assert!(write.carries_data());
    assert!(write.requires_writable());
    assert!(write.requires_response());
    assert!(!write.returns_data());
}

#[test]
fn llsc_requests_preserve_load_and_store_conditional_semantics() {
    let load = MemoryRequest::load_locked(
        request_id(24),
        Address::new(0x2208),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(load.operation(), MemoryOperation::LoadLocked);
    assert_eq!(load.coherence_intent(), CoherenceIntent::ReadShared);
    assert_eq!(load.range().start(), Address::new(0x2208));
    assert_eq!(load.size(), AccessSize::new(8).unwrap());
    assert_eq!(load.data(), None);
    assert_eq!(load.byte_mask(), None);
    assert_eq!(load.atomic_op(), None);
    assert!(!load.carries_data());
    assert!(!load.requires_writable());
    assert!(load.requires_response());
    assert!(load.returns_data());

    let size = AccessSize::new(4).unwrap();
    let mask = ByteMask::from_bits(vec![true, false, true, true]).unwrap();
    let store = MemoryRequest::store_conditional(
        request_id(25),
        Address::new(0x2210),
        size,
        vec![0x10, 0x20, 0x30, 0x40],
        mask.clone(),
        line_layout(),
    )
    .unwrap();

    assert_eq!(store.operation(), MemoryOperation::StoreConditional);
    assert_eq!(store.coherence_intent(), CoherenceIntent::WriteUnique);
    assert_eq!(store.range().start(), Address::new(0x2210));
    assert_eq!(store.size(), size);
    assert_eq!(store.data(), Some(&[0x10, 0x20, 0x30, 0x40][..]));
    assert_eq!(store.byte_mask(), Some(&mask));
    assert_eq!(store.atomic_op(), None);
    assert!(store.carries_data());
    assert!(store.requires_writable());
    assert!(store.requires_response());
    assert!(!store.returns_data());

    let mut memory = LineMemoryStore::new(line_layout());
    memory
        .insert_line(Address::new(0x2200), vec![0x55; 64])
        .unwrap();

    let no_reservation_response = memory.respond(&store).unwrap().unwrap();
    assert_eq!(
        no_reservation_response.status(),
        ResponseStatus::StoreConditionalFailed
    );
    assert_eq!(no_reservation_response.data(), None);
    let read_back =
        MemoryRequest::read_shared(request_id(27), Address::new(0x2210), size, line_layout())
            .unwrap();
    let read_back = memory.respond(&read_back).unwrap().unwrap();
    assert_eq!(read_back.data(), Some(&[0x55; 4][..]));

    let load_response = memory.respond(&load).unwrap().unwrap();
    assert_eq!(load_response.data(), Some(&[0x55; 8][..]));

    let adjacent_store_response = memory.respond(&store).unwrap().unwrap();
    assert_eq!(
        adjacent_store_response.status(),
        ResponseStatus::StoreConditionalFailed
    );
    let read_back =
        MemoryRequest::read_shared(request_id(28), Address::new(0x2210), size, line_layout())
            .unwrap();
    let read_back = memory.respond(&read_back).unwrap().unwrap();
    assert_eq!(read_back.data(), Some(&[0x55; 4][..]));

    let matching_load = MemoryRequest::load_locked(
        request_id(29),
        Address::new(0x2218),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let matching_store = MemoryRequest::store_conditional(
        request_id(31),
        Address::new(0x2210),
        size,
        vec![0x10, 0x20, 0x30, 0x40],
        mask.clone(),
        line_layout(),
    )
    .unwrap();
    memory.respond(&matching_load).unwrap().unwrap();
    let matching_store_response = memory.respond(&matching_store).unwrap().unwrap();
    assert_eq!(matching_store_response.status(), ResponseStatus::Completed);
    assert_eq!(matching_store_response.data(), None);
    let read_back =
        MemoryRequest::read_shared(request_id(32), Address::new(0x2210), size, line_layout())
            .unwrap();
    let read_back = memory.respond(&read_back).unwrap().unwrap();
    assert_eq!(read_back.data(), Some(&[0x10, 0x55, 0x30, 0x40][..]));
}

#[test]
fn normal_writes_clear_matching_load_locked_reservations() {
    let size = AccessSize::new(4).unwrap();
    let mask = ByteMask::full(size).unwrap();
    let mut memory = LineMemoryStore::new(line_layout());
    memory
        .insert_line(Address::new(0x2300), vec![0x44; 64])
        .unwrap();

    let load = MemoryRequest::load_locked(
        agent_request_id(7, 31),
        Address::new(0x2318),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let normal_write = MemoryRequest::write(
        agent_request_id(8, 32),
        Address::new(0x2310),
        size,
        vec![0xaa, 0xbb, 0xcc, 0xdd],
        mask.clone(),
        line_layout(),
    )
    .unwrap();
    let store = MemoryRequest::store_conditional(
        agent_request_id(7, 33),
        Address::new(0x2318),
        size,
        vec![0x10, 0x20, 0x30, 0x40],
        mask,
        line_layout(),
    )
    .unwrap();

    memory.respond(&load).unwrap().unwrap();
    memory.respond(&normal_write).unwrap().unwrap();
    let store_response = memory.respond(&store).unwrap().unwrap();

    assert_eq!(
        store_response.status(),
        ResponseStatus::StoreConditionalFailed
    );
    let read_back =
        MemoryRequest::read_shared(request_id(34), Address::new(0x2310), size, line_layout())
            .unwrap();
    let read_back = memory.respond(&read_back).unwrap().unwrap();
    assert_eq!(read_back.data(), Some(&[0xaa, 0xbb, 0xcc, 0xdd][..]));
}

#[test]
fn full_line_replacements_clear_all_overlapping_load_locked_reservations() {
    let size = AccessSize::new(4).unwrap();
    let mask = ByteMask::full(size).unwrap();
    let mut memory = LineMemoryStore::new(line_layout());
    memory
        .insert_line(Address::new(0x2400), vec![0x44; 64])
        .unwrap();

    let load = MemoryRequest::load_locked(
        agent_request_id(9, 35),
        Address::new(0x2438),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let replacement = MemoryRequest::writeback_dirty(
        agent_request_id(8, 36),
        Address::new(0x2400),
        vec![0x77; 64],
        line_layout(),
    )
    .unwrap();
    let store = MemoryRequest::store_conditional(
        agent_request_id(9, 37),
        Address::new(0x2438),
        size,
        vec![0x10, 0x20, 0x30, 0x40],
        mask,
        line_layout(),
    )
    .unwrap();

    memory.respond(&load).unwrap().unwrap();
    assert!(memory.respond(&replacement).unwrap().is_none());
    let store_response = memory.respond(&store).unwrap().unwrap();

    assert_eq!(
        store_response.status(),
        ResponseStatus::StoreConditionalFailed
    );
    let read_back =
        MemoryRequest::read_shared(request_id(38), Address::new(0x2438), size, line_layout())
            .unwrap();
    let read_back = memory.respond(&read_back).unwrap().unwrap();
    assert_eq!(read_back.data(), Some(&[0x77, 0x77, 0x77, 0x77][..]));
}

#[test]
fn insert_line_replacements_clear_all_overlapping_load_locked_reservations() {
    let size = AccessSize::new(4).unwrap();
    let mask = ByteMask::full(size).unwrap();
    let mut memory = LineMemoryStore::new(line_layout());
    memory
        .insert_line(Address::new(0x2500), vec![0x44; 64])
        .unwrap();

    let load = MemoryRequest::load_locked(
        agent_request_id(9, 39),
        Address::new(0x2538),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let store = MemoryRequest::store_conditional(
        agent_request_id(9, 40),
        Address::new(0x2538),
        size,
        vec![0x10, 0x20, 0x30, 0x40],
        mask,
        line_layout(),
    )
    .unwrap();

    memory.respond(&load).unwrap().unwrap();
    memory
        .insert_line(Address::new(0x2500), vec![0x66; 64])
        .unwrap();
    let store_response = memory.respond(&store).unwrap().unwrap();

    assert_eq!(
        store_response.status(),
        ResponseStatus::StoreConditionalFailed
    );
    let read_back =
        MemoryRequest::read_shared(request_id(41), Address::new(0x2538), size, line_layout())
            .unwrap();
    let read_back = memory.respond(&read_back).unwrap().unwrap();
    assert_eq!(read_back.data(), Some(&[0x66, 0x66, 0x66, 0x66][..]));
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

    let cache_block_zero =
        MemoryRequest::cache_block_zero(request_id(42), Address::new(0x5400), line_layout())
            .unwrap();
    assert_eq!(
        cache_block_zero.operation(),
        MemoryOperation::CacheBlockZero
    );
    assert_eq!(
        cache_block_zero.coherence_intent(),
        CoherenceIntent::CacheBlockZero
    );
    assert!(!cache_block_zero.carries_data());
    assert_eq!(cache_block_zero.byte_mask(), None);
    assert!(cache_block_zero.requires_writable());
    assert!(cache_block_zero.requires_response());
    assert!(!cache_block_zero.returns_data());

    let unaligned_cache_block_zero =
        MemoryRequest::cache_block_zero(request_id(43), Address::new(0x5404), line_layout())
            .unwrap_err();
    assert_eq!(
        unaligned_cache_block_zero,
        MemoryError::UnalignedLineAddress {
            address: Address::new(0x5404),
            line_size: 64
        }
    );

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

    let invalidate_writable =
        MemoryRequest::invalidate_writable(request_id(24), Address::new(0x7000), line_layout())
            .unwrap();
    assert_eq!(
        invalidate_writable.operation(),
        MemoryOperation::InvalidateWritable
    );
    assert_eq!(
        invalidate_writable.coherence_intent(),
        CoherenceIntent::InvalidateWritable
    );
    assert_eq!(invalidate_writable.byte_mask(), None);
    assert!(!invalidate_writable.carries_data());
    assert!(invalidate_writable.requires_writable());
    assert!(invalidate_writable.requires_response());
    assert!(!invalidate_writable.returns_data());

    let unaligned_invalidate_writable =
        MemoryRequest::invalidate_writable(request_id(25), Address::new(0x7004), line_layout())
            .unwrap_err();
    assert_eq!(
        unaligned_invalidate_writable,
        MemoryError::UnalignedLineAddress {
            address: Address::new(0x7004),
            line_size: 64
        }
    );

    let prefetch_read = MemoryRequest::prefetch_read(
        request_id(26),
        Address::new(0x8008),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    assert_eq!(prefetch_read.operation(), MemoryOperation::PrefetchRead);
    assert_eq!(
        prefetch_read.coherence_intent(),
        CoherenceIntent::ReadShared
    );
    assert_eq!(prefetch_read.byte_mask(), None);
    assert!(!prefetch_read.carries_data());
    assert!(!prefetch_read.requires_writable());
    assert!(!prefetch_read.requires_response());
    assert!(!prefetch_read.returns_data());

    let prefetch_write = MemoryRequest::prefetch_write(
        request_id(27),
        Address::new(0x8010),
        AccessSize::new(16).unwrap(),
        line_layout(),
    )
    .unwrap();
    assert_eq!(prefetch_write.operation(), MemoryOperation::PrefetchWrite);
    assert_eq!(
        prefetch_write.coherence_intent(),
        CoherenceIntent::WriteUnique
    );
    assert_eq!(prefetch_write.byte_mask(), None);
    assert!(!prefetch_write.carries_data());
    assert!(prefetch_write.requires_writable());
    assert!(!prefetch_write.requires_response());
    assert!(!prefetch_write.returns_data());
}

#[test]
fn prefetch_requests_can_require_response_without_returning_data() {
    let default_prefetch = MemoryRequest::prefetch_read(
        request_id(28),
        Address::new(0x8020),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    assert!(!default_prefetch.requires_response());
    assert_eq!(
        MemoryResponse::completed(&default_prefetch, None).unwrap_err(),
        MemoryError::ResponseNotExpected {
            request: request_id(28),
        }
    );

    let response_prefetch = default_prefetch.with_response_required();
    assert!(response_prefetch.requires_response());
    assert!(!response_prefetch.returns_data());

    let response = MemoryResponse::completed(&response_prefetch, None).unwrap();
    assert_eq!(response.request_id(), request_id(28));
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), None);

    assert_eq!(
        MemoryResponse::completed(&response_prefetch, Some(vec![0xaa])).unwrap_err(),
        MemoryError::UnexpectedResponseData {
            request: request_id(28),
        }
    );
}

#[test]
fn line_store_completes_response_required_prefetch_without_data() {
    let mut store = LineMemoryStore::new(line_layout());
    store
        .insert_line(Address::new(0x8000), vec![0x11; 64])
        .unwrap();
    let request = MemoryRequest::prefetch_read(
        request_id(29),
        Address::new(0x8008),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap()
    .with_response_required();

    let response = store.respond(&request).unwrap().unwrap();

    assert_eq!(response.request_id(), request_id(29));
    assert_eq!(response.status(), ResponseStatus::Completed);
    assert_eq!(response.data(), None);
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
fn memory_response_checkpoint_payload_uses_stable_store_conditional_failed_bytes() {
    let size = AccessSize::new(4).unwrap();
    let store = MemoryRequest::store_conditional(
        request_id(45),
        Address::new(0x7d10),
        size,
        vec![0x11, 0x22, 0x33, 0x44],
        ByteMask::full(size).unwrap(),
        line_layout(),
    )
    .unwrap();
    let failed = MemoryResponse::store_conditional_failed(&store).unwrap();

    let stable_payload = vec![
        b'M', b'R', b'E', b'S', 1, 0, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 45, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let decoded = MemoryResponseCheckpointPayload::decode(&stable_payload).unwrap();
    let restored = MemoryResponse::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(
        MemoryResponseCheckpointPayload::from_response(&failed).encode(),
        stable_payload
    );
    assert_eq!(decoded.snapshot(), &failed.snapshot());
    assert_eq!(restored, failed);
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
fn memory_request_checkpoint_payload_round_trips_atomic_no_return() {
    let size = AccessSize::new(8).unwrap();
    let mask = ByteMask::full(size).unwrap();
    let request = MemoryRequest::atomic_no_return(
        request_id(25),
        Address::new(0x6020),
        size,
        MemoryAtomicOp::Or,
        vec![0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27],
        mask.clone(),
        line_layout(),
    )
    .unwrap()
    .with_ordering(MemoryAccessOrdering::new(
        Some(MemoryBarrierSet::new(false, true)),
        Some(MemoryBarrierSet::memory()),
    ));

    let snapshot = request.snapshot();
    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(decoded.snapshot(), &snapshot);
    assert_eq!(restored, request);
    assert_eq!(restored.operation(), MemoryOperation::AtomicNoReturn);
    assert_eq!(restored.atomic_op(), Some(MemoryAtomicOp::Or));
    assert_eq!(restored.byte_mask(), Some(&mask));
    assert!(restored.requires_response());
    assert!(!restored.returns_data());
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
fn memory_request_checkpoint_payload_round_trips_cache_block_zero() {
    let request =
        MemoryRequest::cache_block_zero(request_id(25), Address::new(0x7800), line_layout())
            .unwrap()
            .with_ordering(MemoryAccessOrdering::new(
                Some(MemoryBarrierSet::new(false, true)),
                Some(MemoryBarrierSet::memory()),
            ))
            .with_evict_next();

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(restored, request);
    assert_eq!(restored.operation(), MemoryOperation::CacheBlockZero);
    assert_eq!(restored.data(), None);
    assert_eq!(restored.byte_mask(), None);
    assert!(restored.is_evict_next());
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
fn memory_request_checkpoint_payload_round_trips_invalidate_writable() {
    let request =
        MemoryRequest::invalidate_writable(request_id(25), Address::new(0x7800), line_layout())
            .unwrap()
            .with_ordering(MemoryAccessOrdering::new(
                Some(MemoryBarrierSet::memory()),
                None,
            ));

    let payload = MemoryRequestCheckpointPayload::from_request(&request);
    let decoded = MemoryRequestCheckpointPayload::decode(payload.encode().as_slice()).unwrap();
    let restored = MemoryRequest::from_snapshot(decoded.snapshot()).unwrap();

    assert_eq!(restored, request);
    assert_eq!(restored.operation(), MemoryOperation::InvalidateWritable);
    assert_eq!(restored.data(), None);
    assert_eq!(restored.byte_mask(), None);
    assert!(restored.requires_writable());
}

#[test]
fn memory_request_checkpoint_payload_round_trips_locked_rmw_requests() {
    let read = MemoryRequest::locked_rmw_read(
        request_id(46),
        Address::new(0x7d08),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let read_payload = MemoryRequestCheckpointPayload::from_request(&read);
    let read_decoded =
        MemoryRequestCheckpointPayload::decode(read_payload.encode().as_slice()).unwrap();
    let read_restored = MemoryRequest::from_snapshot(read_decoded.snapshot()).unwrap();

    assert_eq!(read_decoded.snapshot(), &read.snapshot());
    assert_eq!(read_restored, read);
    assert_eq!(read_restored.operation(), MemoryOperation::LockedRmwRead);
    assert!(read_restored.requires_writable());
    assert!(read_restored.returns_data());

    let size = AccessSize::new(8).unwrap();
    let write = MemoryRequest::locked_rmw_write(
        request_id(47),
        Address::new(0x7d10),
        size,
        vec![0xa0, 0xa1, 0xa2, 0xa3, 0xa4, 0xa5, 0xa6, 0xa7],
        ByteMask::full(size).unwrap(),
        line_layout(),
    )
    .unwrap();
    let write_payload = MemoryRequestCheckpointPayload::from_request(&write);
    let write_decoded =
        MemoryRequestCheckpointPayload::decode(write_payload.encode().as_slice()).unwrap();
    let write_restored = MemoryRequest::from_snapshot(write_decoded.snapshot()).unwrap();

    assert_eq!(write_decoded.snapshot(), &write.snapshot());
    assert_eq!(write_restored, write);
    assert_eq!(write_restored.operation(), MemoryOperation::LockedRmwWrite);
    assert!(write_restored.requires_writable());
    assert!(!write_restored.returns_data());
}

#[test]
fn memory_request_checkpoint_payload_round_trips_llsc_requests() {
    let load = MemoryRequest::load_locked(
        request_id(48),
        Address::new(0x7e08),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let load_payload = MemoryRequestCheckpointPayload::from_request(&load);
    let load_decoded =
        MemoryRequestCheckpointPayload::decode(load_payload.encode().as_slice()).unwrap();
    let load_restored = MemoryRequest::from_snapshot(load_decoded.snapshot()).unwrap();

    assert_eq!(load_decoded.snapshot(), &load.snapshot());
    assert_eq!(load_restored, load);
    assert_eq!(load_restored.operation(), MemoryOperation::LoadLocked);
    assert!(!load_restored.requires_writable());
    assert!(load_restored.returns_data());

    let size = AccessSize::new(8).unwrap();
    let store = MemoryRequest::store_conditional(
        request_id(49),
        Address::new(0x7e10),
        size,
        vec![0xb0, 0xb1, 0xb2, 0xb3, 0xb4, 0xb5, 0xb6, 0xb7],
        ByteMask::full(size).unwrap(),
        line_layout(),
    )
    .unwrap();
    let store_payload = MemoryRequestCheckpointPayload::from_request(&store);
    let store_decoded =
        MemoryRequestCheckpointPayload::decode(store_payload.encode().as_slice()).unwrap();
    let store_restored = MemoryRequest::from_snapshot(store_decoded.snapshot()).unwrap();

    assert_eq!(store_decoded.snapshot(), &store.snapshot());
    assert_eq!(store_restored, store);
    assert_eq!(
        store_restored.operation(),
        MemoryOperation::StoreConditional
    );
    assert!(store_restored.requires_writable());
    assert!(!store_restored.returns_data());
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
fn memory_request_checkpoint_payload_uses_stable_llsc_operation_bytes() {
    let load = MemoryRequest::load_locked(
        request_id(55),
        Address::new(0x8f08),
        AccessSize::new(8).unwrap(),
        line_layout(),
    )
    .unwrap();
    let load_stable_payload = vec![
        b'M', b'R', b'E', b'Q', 1, 0, 0, 0, 17, 0, 0, 0, 0, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 55, 0,
        0, 0, 0, 0, 0, 0, 0x08, 0x8f, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0,
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
    ];
    let load_decoded = MemoryRequestCheckpointPayload::decode(&load_stable_payload).unwrap();
    let load_restored = MemoryRequest::from_snapshot(load_decoded.snapshot()).unwrap();

    assert_eq!(
        MemoryRequestCheckpointPayload::from_request(&load).encode(),
        load_stable_payload
    );
    assert_eq!(load_decoded.snapshot(), &load.snapshot());
    assert_eq!(load_restored, load);

    let size = AccessSize::new(4).unwrap();
    let store = MemoryRequest::store_conditional(
        request_id(56),
        Address::new(0x9008),
        size,
        vec![0x11, 0x22, 0x33, 0x44],
        ByteMask::from_bits(vec![true, false, true, true]).unwrap(),
        line_layout(),
    )
    .unwrap();
    let store_stable_payload = vec![
        b'M', b'R', b'E', b'Q', 1, 0, 0, 0, 18, 0, 0, 0, 3, 0, 0, 0, 7, 0, 0, 0, 0, 0, 0, 0, 56, 0,
        0, 0, 0, 0, 0, 0, 0x08, 0x90, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 64, 0, 0, 0, 0, 0,
        0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x11, 0x22,
        0x33, 0x44, 1, 0, 1, 1,
    ];
    let store_decoded = MemoryRequestCheckpointPayload::decode(&store_stable_payload).unwrap();
    let store_restored = MemoryRequest::from_snapshot(store_decoded.snapshot()).unwrap();

    assert_eq!(
        MemoryRequestCheckpointPayload::from_request(&store).encode(),
        store_stable_payload
    );
    assert_eq!(store_decoded.snapshot(), &store.snapshot());
    assert_eq!(store_restored, store);
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
        .copy_from_slice(&3u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::UnsupportedRequestCheckpointVersion { version: 3 }
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
        .copy_from_slice(&0x0080_0000u32.to_le_bytes());

    assert_eq!(
        MemoryRequestCheckpointPayload::decode(&payload).unwrap_err(),
        MemoryError::InvalidRequestCheckpointFlags { flags: 0x0080_0000 }
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
