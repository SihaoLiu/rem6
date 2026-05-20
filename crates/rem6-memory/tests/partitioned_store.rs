use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, ByteMask, CacheLineLayout, MemoryError,
    MemoryRequest, MemoryRequestId, MemoryTargetId, PartitionedMemoryStore, ResponseStatus,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(64).unwrap()
}

fn request_id(sequence: u64) -> MemoryRequestId {
    MemoryRequestId::new(AgentId::new(5), sequence)
}

fn line_data(base: u8) -> Vec<u8> {
    (0..64).map(|offset| base.wrapping_add(offset)).collect()
}

fn range(start: u64, size: u64) -> AddressRange {
    AddressRange::new(Address::new(start), AccessSize::new(size).unwrap()).unwrap()
}

fn read(address: u64, size: u64, sequence: u64) -> MemoryRequest {
    MemoryRequest::read_shared(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(size).unwrap(),
        layout(),
    )
    .unwrap()
}

fn write(address: u64, bytes: &[u8], mask: &[bool], sequence: u64) -> MemoryRequest {
    MemoryRequest::write(
        request_id(sequence),
        Address::new(address),
        AccessSize::new(bytes.len() as u64).unwrap(),
        bytes.to_vec(),
        ByteMask::from_bits(mask.to_vec()).unwrap(),
        layout(),
    )
    .unwrap()
}

fn writeback(address: u64, bytes: Vec<u8>, sequence: u64) -> MemoryRequest {
    MemoryRequest::writeback_dirty(request_id(sequence), Address::new(address), bytes, layout())
        .unwrap()
}

fn mapped_store() -> (PartitionedMemoryStore, MemoryTargetId, MemoryTargetId) {
    let low = MemoryTargetId::new(10);
    let high = MemoryTargetId::new(20);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(high, layout()).unwrap();
    store.add_partition(low, layout()).unwrap();
    store
        .map_region(low, Address::new(0x0000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    store
        .map_region(high, Address::new(0x8000), AccessSize::new(0x4000).unwrap())
        .unwrap();
    store
        .insert_line(low, Address::new(0x1000), line_data(0x10))
        .unwrap();
    store
        .insert_line(high, Address::new(0x8000), line_data(0x80))
        .unwrap();
    (store, low, high)
}

#[test]
fn partitioned_store_routes_requests_by_address_region() {
    let (mut store, low, high) = mapped_store();

    let low_outcome = store.respond(&read(0x1004, 4, 1)).unwrap();
    assert_eq!(low_outcome.target(), low);
    let low_response = low_outcome.response().unwrap();
    assert_eq!(low_response.status(), ResponseStatus::Completed);
    assert_eq!(low_response.data().unwrap(), &[0x14, 0x15, 0x16, 0x17]);

    let high_outcome = store.respond(&read(0x8008, 4, 2)).unwrap();
    assert_eq!(high_outcome.target(), high);
    assert_eq!(
        high_outcome.response().unwrap().data().unwrap(),
        &[0x88, 0x89, 0x8a, 0x8b]
    );

    assert_eq!(store.partition_count(), 2);
    assert_eq!(store.region_count(), 2);
    assert_eq!(store.line_count(low).unwrap(), 1);
    assert_eq!(store.line_count(high).unwrap(), 1);
}

#[test]
fn partitioned_store_applies_writes_only_to_decoded_target() {
    let (mut store, low, high) = mapped_store();
    let request = write(
        0x1002,
        &[0xaa, 0xbb, 0xcc, 0xdd],
        &[true, false, true, false],
        3,
    );

    let outcome = store.respond(&request).unwrap();
    assert_eq!(outcome.target(), low);
    assert_eq!(outcome.response().unwrap().data(), None);

    assert_eq!(
        &store.line_data(low, Address::new(0x1000)).unwrap()[..8],
        &[0x10, 0x11, 0xaa, 0x13, 0xcc, 0x15, 0x16, 0x17]
    );
    assert_eq!(
        &store.line_data(high, Address::new(0x8000)).unwrap()[..8],
        &[0x80, 0x81, 0x82, 0x83, 0x84, 0x85, 0x86, 0x87]
    );
}

#[test]
fn partitioned_store_handles_writebacks_without_responses() {
    let (mut store, _low, high) = mapped_store();
    let replacement = line_data(0x40);

    let outcome = store
        .respond(&writeback(0x8000, replacement.clone(), 4))
        .unwrap();
    assert_eq!(outcome.target(), high);
    assert_eq!(outcome.response(), None);
    assert_eq!(
        store.line_data(high, Address::new(0x8000)).unwrap(),
        replacement
    );
}

#[test]
fn partitioned_store_snapshots_and_restores_regions_partitions_and_lines() {
    let (mut store, low, high) = mapped_store();

    let snapshot = store.snapshot();

    assert_eq!(
        snapshot.regions(),
        &[(low, range(0x0000, 0x4000)), (high, range(0x8000, 0x4000))]
    );
    assert_eq!(
        snapshot
            .partitions()
            .iter()
            .map(|partition| partition.target())
            .collect::<Vec<_>>(),
        vec![low, high]
    );
    assert_eq!(
        snapshot.partitions()[0].lines()[0].line(),
        Address::new(0x1000)
    );
    assert_eq!(snapshot.partitions()[0].lines()[0].data(), &line_data(0x10));
    assert_eq!(
        snapshot.partitions()[1].lines()[0].line(),
        Address::new(0x8000)
    );
    assert_eq!(snapshot.partitions()[1].lines()[0].data(), &line_data(0x80));

    store
        .respond(&write(
            0x1000,
            &[0xaa, 0xbb, 0xcc, 0xdd],
            &[true, true, true, true],
            5,
        ))
        .unwrap();
    store
        .respond(&writeback(0x8000, line_data(0x40), 6))
        .unwrap();

    store.restore(&snapshot).unwrap();

    assert_eq!(store.regions(), snapshot.regions());
    assert_eq!(store.partition_count(), 2);
    assert_eq!(store.line_count(low).unwrap(), 1);
    assert_eq!(store.line_count(high).unwrap(), 1);
    assert_eq!(
        store.line_data(low, Address::new(0x1000)).unwrap(),
        line_data(0x10)
    );
    assert_eq!(
        store.line_data(high, Address::new(0x8000)).unwrap(),
        line_data(0x80)
    );
    assert_eq!(
        store
            .respond(&read(0x1004, 4, 7))
            .unwrap()
            .response()
            .unwrap()
            .data()
            .unwrap(),
        &[0x14, 0x15, 0x16, 0x17]
    );
}

#[test]
fn partitioned_store_allows_disjoint_regions_for_one_target() {
    let target = MemoryTargetId::new(30);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    store
        .map_region(
            target,
            Address::new(0x1000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    store
        .insert_line(target, Address::new(0x1000), line_data(0x30))
        .unwrap();
    store
        .insert_line(target, Address::new(0x8000), line_data(0x90))
        .unwrap();

    assert!(store.contains_partition(target));
    assert_eq!(store.partition_count(), 1);
    assert_eq!(store.partition_layout(target).unwrap(), layout());
    assert_eq!(
        store.regions(),
        &[
            (target, range(0x1000, 0x1000)),
            (target, range(0x8000, 0x1000))
        ]
    );
    assert_eq!(store.respond(&read(0x1001, 2, 5)).unwrap().target(), target);
    assert_eq!(store.respond(&read(0x8001, 2, 6)).unwrap().target(), target);
}

#[test]
fn partitioned_store_reports_missing_lines_after_decode() {
    let target = MemoryTargetId::new(40);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x4000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();

    assert_eq!(
        store.line_data(target, Address::new(0x4040)).unwrap_err(),
        MemoryError::UnmappedLine {
            line: Address::new(0x4040)
        }
    );
    assert_eq!(
        store.respond(&read(0x4044, 4, 7)).unwrap_err(),
        MemoryError::UnmappedLine {
            line: Address::new(0x4040)
        }
    );
}

#[test]
fn partitioned_store_rejects_duplicate_unknown_and_overlapping_mappings() {
    let target = MemoryTargetId::new(1);
    let unknown = MemoryTargetId::new(9);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();

    assert_eq!(
        store.add_partition(target, layout()).unwrap_err(),
        MemoryError::DuplicateMemoryTarget { target }
    );
    assert_eq!(
        store
            .map_region(
                unknown,
                Address::new(0x0000),
                AccessSize::new(0x1000).unwrap()
            )
            .unwrap_err(),
        MemoryError::UnknownMemoryTarget { target: unknown }
    );

    store
        .map_region(
            target,
            Address::new(0x0000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    assert!(matches!(
        store
            .map_region(
                target,
                Address::new(0x0800),
                AccessSize::new(0x1000).unwrap()
            )
            .unwrap_err(),
        MemoryError::OverlappingAddressRegion { .. }
    ));

    assert_eq!(
        store.line_count(unknown).unwrap_err(),
        MemoryError::UnknownMemoryTarget { target: unknown }
    );
    assert_eq!(
        store.partition_layout(unknown).unwrap_err(),
        MemoryError::UnknownMemoryTarget { target: unknown }
    );
}

#[test]
fn partitioned_store_rejects_unmapped_cross_region_and_layout_mismatch() {
    let target = MemoryTargetId::new(7);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x4000),
            AccessSize::new(0x100).unwrap(),
        )
        .unwrap();
    store
        .insert_line(target, Address::new(0x4000), line_data(0x20))
        .unwrap();

    assert_eq!(
        store.respond(&read(0x3000, 4, 5)).unwrap_err(),
        MemoryError::UnmappedAddress {
            address: Address::new(0x3000)
        }
    );
    assert!(matches!(
        store.respond(&read(0x40f8, 16, 6)).unwrap_err(),
        MemoryError::RequestCrossesAddressRegion { .. }
    ));

    let actual = CacheLineLayout::new(128).unwrap();
    let mismatched = MemoryRequest::read_shared(
        request_id(7),
        Address::new(0x4000),
        AccessSize::new(8).unwrap(),
        actual,
    )
    .unwrap();
    assert_eq!(
        store.respond(&mismatched).unwrap_err(),
        MemoryError::LineLayoutMismatch {
            request: mismatched.id(),
            expected: layout(),
            actual,
        }
    );
}
