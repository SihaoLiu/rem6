use rem6_boot::{BootElfError, BootError, BootImage, BootLineWrite, BootLoadReport};
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

fn write_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn write_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

#[derive(Clone, Copy)]
struct ElfProgramHeaderSpec {
    kind: u32,
    offset: u64,
    physical: u64,
    file_size: u64,
    memory_size: u64,
}

fn elf64_image(entry: u64, headers: &[ElfProgramHeaderSpec], data: &[(usize, &[u8])]) -> Vec<u8> {
    let mut size = 64 + headers.len() * 56;
    for (offset, bytes) in data {
        size = size.max(offset + bytes.len());
    }
    let mut bytes = vec![0; size];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 2;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, 243);
    write_u32(&mut bytes, 20, 1);
    write_u64(&mut bytes, 24, entry);
    write_u64(&mut bytes, 32, 64);
    write_u16(&mut bytes, 52, 64);
    write_u16(&mut bytes, 54, 56);
    write_u16(&mut bytes, 56, headers.len() as u16);

    for (index, header) in headers.iter().enumerate() {
        let base = 64 + index * 56;
        write_u32(&mut bytes, base, header.kind);
        write_u32(&mut bytes, base + 4, 5);
        write_u64(&mut bytes, base + 8, header.offset);
        write_u64(&mut bytes, base + 16, header.physical);
        write_u64(&mut bytes, base + 24, header.physical);
        write_u64(&mut bytes, base + 32, header.file_size);
        write_u64(&mut bytes, base + 40, header.memory_size);
        write_u64(&mut bytes, base + 48, 0x1000);
    }

    for (offset, payload) in data {
        bytes[*offset..*offset + payload.len()].copy_from_slice(payload);
    }
    bytes
}

fn elf32_image(entry: u32, headers: &[ElfProgramHeaderSpec], data: &[(usize, &[u8])]) -> Vec<u8> {
    let mut size = 52 + headers.len() * 32;
    for (offset, bytes) in data {
        size = size.max(offset + bytes.len());
    }
    let mut bytes = vec![0; size];
    bytes[0..4].copy_from_slice(b"\x7fELF");
    bytes[4] = 1;
    bytes[5] = 1;
    bytes[6] = 1;
    write_u16(&mut bytes, 16, 2);
    write_u16(&mut bytes, 18, 243);
    write_u32(&mut bytes, 20, 1);
    write_u32(&mut bytes, 24, entry);
    write_u32(&mut bytes, 28, 52);
    write_u16(&mut bytes, 40, 52);
    write_u16(&mut bytes, 42, 32);
    write_u16(&mut bytes, 44, headers.len() as u16);

    for (index, header) in headers.iter().enumerate() {
        let base = 52 + index * 32;
        write_u32(&mut bytes, base, header.kind);
        write_u32(&mut bytes, base + 4, header.offset as u32);
        write_u32(&mut bytes, base + 8, header.physical as u32);
        write_u32(&mut bytes, base + 12, header.physical as u32);
        write_u32(&mut bytes, base + 16, header.file_size as u32);
        write_u32(&mut bytes, base + 20, header.memory_size as u32);
        write_u32(&mut bytes, base + 24, 5);
        write_u32(&mut bytes, base + 28, 0x1000);
    }

    for (offset, payload) in data {
        bytes[*offset..*offset + payload.len()].copy_from_slice(payload);
    }
    bytes
}

#[test]
fn boot_image_loads_elf64_loadable_segments_with_zero_fill() {
    let elf = elf64_image(
        0x8004,
        &[
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x100,
                physical: 0x8000,
                file_size: 4,
                memory_size: 8,
            },
            ElfProgramHeaderSpec {
                kind: 4,
                offset: 0x108,
                physical: 0x8800,
                file_size: 4,
                memory_size: 4,
            },
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x110,
                physical: 0x9002,
                file_size: 3,
                memory_size: 3,
            },
        ],
        &[
            (0x100, &[0x13, 0x05, 0x00, 0x00]),
            (0x108, &[0xde, 0xad, 0xbe, 0xef]),
            (0x110, &[0xa0, 0xa1, 0xa2]),
        ],
    );

    let image = BootImage::from_elf64_le(&elf).unwrap();

    assert_eq!(image.entry(), Address::new(0x8004));
    assert_eq!(image.segments().len(), 2);
    assert_eq!(image.segments()[0].range().start(), Address::new(0x8000));
    assert_eq!(
        image.segments()[0].data(),
        &[0x13, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    );
    assert_eq!(image.segments()[1].range().start(), Address::new(0x9002));
    assert_eq!(image.segments()[1].data(), &[0xa0, 0xa1, 0xa2]);
}

#[test]
fn boot_image_loads_elf32_loadable_segments_with_zero_fill() {
    let elf = elf32_image(
        0x8040,
        &[
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x100,
                physical: 0x8000,
                file_size: 4,
                memory_size: 8,
            },
            ElfProgramHeaderSpec {
                kind: 4,
                offset: 0x108,
                physical: 0x9000,
                file_size: 4,
                memory_size: 4,
            },
            ElfProgramHeaderSpec {
                kind: 1,
                offset: 0x110,
                physical: 0xa002,
                file_size: 3,
                memory_size: 3,
            },
        ],
        &[
            (0x100, &[0x13, 0x05, 0x00, 0x00]),
            (0x108, &[0xde, 0xad, 0xbe, 0xef]),
            (0x110, &[0xb0, 0xb1, 0xb2]),
        ],
    );

    let image = BootImage::from_elf32_le(&elf).unwrap();

    assert_eq!(image.entry(), Address::new(0x8040));
    assert_eq!(image.segments().len(), 2);
    assert_eq!(image.segments()[0].range().start(), Address::new(0x8000));
    assert_eq!(
        image.segments()[0].data(),
        &[0x13, 0x05, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    );
    assert_eq!(image.segments()[1].range().start(), Address::new(0xa002));
    assert_eq!(image.segments()[1].data(), &[0xb0, 0xb1, 0xb2]);
}

#[test]
fn boot_image_rejects_elf64_segment_memory_overflow_with_segment_context() {
    let elf = elf64_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: u64::MAX - 1,
            file_size: 2,
            memory_size: 4,
        }],
        &[(0x100, &[0xaa, 0xbb])],
    );

    assert_eq!(
        BootImage::from_elf64_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::SegmentMemoryRangeOverflow {
                segment: 0,
                physical: u64::MAX - 1,
                memory_size: 4,
            },
        },
    );
}

#[test]
fn boot_image_rejects_elf32_segment_memory_overflow_with_segment_context() {
    let elf = elf32_image(
        0x8000,
        &[ElfProgramHeaderSpec {
            kind: 1,
            offset: 0x100,
            physical: u32::MAX as u64 - 1,
            file_size: 2,
            memory_size: 4,
        }],
        &[(0x100, &[0xaa, 0xbb])],
    );

    assert_eq!(
        BootImage::from_elf32_le(&elf).unwrap_err(),
        BootError::InvalidElf {
            reason: BootElfError::SegmentMemoryRangeOverflow {
                segment: 0,
                physical: u32::MAX as u64 - 1,
                memory_size: 4,
            },
        },
    );
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
