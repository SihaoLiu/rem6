use rem6_boot::{BootError, BootImage, BootLineWrite, BootLoadReport};
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, LineMemoryStore, MemoryError,
    MemoryTargetId, PartitionedMemoryStore,
};

fn layout() -> CacheLineLayout {
    CacheLineLayout::new(16).unwrap()
}

fn line(fill: u8) -> Vec<u8> {
    vec![fill; 16]
}

#[test]
fn boot_image_loads_segments_across_lines_and_preserves_existing_bytes() {
    let mut store = LineMemoryStore::new(layout());
    store.insert_line(Address::new(0x1000), line(0x55)).unwrap();
    let image = BootImage::new(Address::new(0x1004))
        .add_segment(Address::new(0x100e), vec![0xa0, 0xa1, 0xa2, 0xa3])
        .unwrap()
        .add_segment(Address::new(0x1020), vec![0xb0, 0xb1, 0xb2])
        .unwrap();

    let report = image.load_into_line_store(&mut store).unwrap();

    assert_eq!(
        report,
        BootLoadReport::new(
            Address::new(0x1004),
            vec![
                BootLineWrite::new(Address::new(0x1000), 14, 2),
                BootLineWrite::new(Address::new(0x1010), 0, 2),
                BootLineWrite::new(Address::new(0x1020), 0, 3),
            ],
        )
    );
    let first = store.line_data(Address::new(0x1000)).unwrap();
    assert_eq!(&first[0..14], &[0x55; 14]);
    assert_eq!(&first[14..16], &[0xa0, 0xa1]);
    let second = store.line_data(Address::new(0x1010)).unwrap();
    assert_eq!(&second[0..2], &[0xa2, 0xa3]);
    assert_eq!(&second[2..16], &[0; 14]);
    let third = store.line_data(Address::new(0x1020)).unwrap();
    assert_eq!(&third[0..3], &[0xb0, 0xb1, 0xb2]);
    assert_eq!(&third[3..16], &[0; 13]);
}

#[test]
fn boot_image_loads_into_partitioned_store_target() {
    let target = MemoryTargetId::new(7);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(target, layout()).unwrap();
    store
        .map_region(
            target,
            Address::new(0x8000),
            AccessSize::new(0x1000).unwrap(),
        )
        .unwrap();
    let image = BootImage::new(Address::new(0x8004))
        .add_segment(Address::new(0x8008), vec![1, 2, 3, 4])
        .unwrap();

    let report = image
        .load_into_partitioned_store(&mut store, target)
        .unwrap();

    assert_eq!(
        report,
        BootLoadReport::new(
            Address::new(0x8004),
            vec![BootLineWrite::new(Address::new(0x8000), 8, 4)],
        )
    );
    let data = store.line_data(target, Address::new(0x8000)).unwrap();
    assert_eq!(&data[0..8], &[0; 8]);
    assert_eq!(&data[8..12], &[1, 2, 3, 4]);
    assert_eq!(&data[12..16], &[0; 4]);
}

#[test]
fn boot_image_loads_partitioned_store_segments_by_address_region() {
    let code = MemoryTargetId::new(1);
    let data = MemoryTargetId::new(2);
    let mut store = PartitionedMemoryStore::new();
    store.add_partition(code, layout()).unwrap();
    store.add_partition(data, layout()).unwrap();
    store
        .map_region(code, Address::new(0x8000), AccessSize::new(0x1000).unwrap())
        .unwrap();
    store
        .map_region(data, Address::new(0xa000), AccessSize::new(0x1000).unwrap())
        .unwrap();
    store
        .insert_line(data, Address::new(0xa000), line(0x77))
        .unwrap();
    let image = BootImage::new(Address::new(0x8000))
        .add_segment(Address::new(0x8004), vec![1, 2, 3, 4])
        .unwrap()
        .add_segment(Address::new(0xa00e), vec![0xa0, 0xa1, 0xa2, 0xa3])
        .unwrap();

    let report = image
        .load_into_partitioned_store_by_address(&mut store)
        .unwrap();

    assert_eq!(
        report,
        BootLoadReport::new(
            Address::new(0x8000),
            vec![
                BootLineWrite::new(Address::new(0x8000), 4, 4),
                BootLineWrite::new(Address::new(0xa000), 14, 2),
                BootLineWrite::new(Address::new(0xa010), 0, 2),
            ],
        )
    );
    assert_eq!(
        &store.line_data(code, Address::new(0x8000)).unwrap()[4..8],
        &[1, 2, 3, 4],
    );
    let first_data = store.line_data(data, Address::new(0xa000)).unwrap();
    assert_eq!(&first_data[0..14], &[0x77; 14]);
    assert_eq!(&first_data[14..16], &[0xa0, 0xa1]);
    let second_data = store.line_data(data, Address::new(0xa010)).unwrap();
    assert_eq!(&second_data[0..2], &[0xa2, 0xa3]);
    assert_eq!(&second_data[2..16], &[0; 14]);
}

#[test]
fn boot_image_rejects_bad_segments_and_unknown_partition() {
    assert_eq!(
        BootImage::new(Address::new(0)).add_segment(Address::new(0x1000), Vec::new()),
        Err(BootError::EmptySegment {
            start: Address::new(0x1000),
        })
    );
    assert_eq!(
        BootImage::new(Address::new(0))
            .add_segment(Address::new(u64::MAX), vec![1, 2])
            .unwrap_err(),
        BootError::Memory(MemoryError::AddressOverflow {
            start: Address::new(u64::MAX),
            size: AccessSize::new(2).unwrap(),
        })
    );

    let overlap = BootImage::new(Address::new(0))
        .add_segment(Address::new(0x2000), vec![0; 8])
        .unwrap()
        .add_segment(Address::new(0x2004), vec![0; 8])
        .unwrap_err();
    assert_eq!(
        overlap,
        BootError::OverlappingSegment {
            existing: AddressRange::new(Address::new(0x2000), AccessSize::new(8).unwrap()).unwrap(),
            requested: AddressRange::new(Address::new(0x2004), AccessSize::new(8).unwrap())
                .unwrap(),
        }
    );

    let unknown = MemoryTargetId::new(9);
    let mut store = PartitionedMemoryStore::new();
    let image = BootImage::new(Address::new(0))
        .add_segment(Address::new(0x3000), vec![0xaa])
        .unwrap();
    assert_eq!(
        image
            .load_into_partitioned_store(&mut store, unknown)
            .unwrap_err(),
        BootError::Memory(MemoryError::UnknownMemoryTarget { target: unknown })
    );
}
