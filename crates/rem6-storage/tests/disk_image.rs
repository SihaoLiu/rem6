use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_storage::{
    CowStorageImage, FileStorageImage, RawStorageImage, StorageCheckpointError, StorageError,
    StorageFileOperation, StorageImageCheckpointBank, StorageImageCheckpointPort,
    StorageImageCheckpointRecord, StorageImageCheckpointSnapshot, StorageImageLayer,
    StorageSectorId, STORAGE_SECTOR_BYTES,
};

fn sector(byte: u8) -> [u8; 512] {
    [byte; 512]
}

fn image_bytes(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .flat_map(|byte| sector(*byte))
        .collect::<Vec<_>>()
}

fn temp_image_path(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("rem6-storage-{name}-{unique}.img"))
}

#[derive(Debug)]
struct HugeStorageImage;

impl StorageImageLayer for HugeStorageImage {
    fn capacity_sectors(&self) -> u64 {
        u64::MAX
    }

    fn read_sector(&self, _sector: StorageSectorId) -> Result<[u8; 512], StorageError> {
        Ok(sector(0))
    }

    fn write_sector(&self, _sector: StorageSectorId, _data: [u8; 512]) -> Result<(), StorageError> {
        Ok(())
    }

    fn flush(&self) -> Result<(), StorageError> {
        Ok(())
    }
}

#[test]
fn raw_storage_image_reads_writes_flushes_and_restores_snapshot() {
    let image = RawStorageImage::from_bytes(image_bytes(&[0x11, 0x22, 0x33])).unwrap();

    assert_eq!(image.capacity_sectors(), 3);
    assert_eq!(
        image.read(StorageSectorId::new(0), 2).unwrap(),
        [sector(0x11), sector(0x22)].concat()
    );

    image
        .write_sector(StorageSectorId::new(1), sector(0xaa))
        .unwrap();
    image.flush().unwrap();
    let snapshot = image.snapshot();

    assert_eq!(image.flush_count(), 1);
    assert_eq!(snapshot.capacity_sectors(), 3);
    assert_eq!(snapshot.flush_count(), 1);
    assert_eq!(snapshot.bytes().len() as u64, 3 * STORAGE_SECTOR_BYTES);

    image
        .write_sector(StorageSectorId::new(1), sector(0xbb))
        .unwrap();
    image.restore(&snapshot).unwrap();

    assert_eq!(
        image.read_sector(StorageSectorId::new(1)).unwrap(),
        sector(0xaa)
    );
    assert_eq!(image.flush_count(), 1);
}

#[test]
fn cow_storage_image_overlays_child_and_writeback_is_explicit() {
    let child = RawStorageImage::from_bytes(image_bytes(&[0x01, 0x02, 0x03])).unwrap();
    let cow = CowStorageImage::new(Arc::new(child.clone()));

    assert_eq!(cow.capacity_sectors(), 3);
    assert_eq!(
        cow.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0x01)
    );

    cow.write_sector(StorageSectorId::new(0), sector(0xaa))
        .unwrap();
    cow.write_sector(StorageSectorId::new(2), sector(0xcc))
        .unwrap();
    cow.flush().unwrap();

    assert_eq!(
        cow.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0xaa)
    );
    assert_eq!(
        cow.read_sector(StorageSectorId::new(1)).unwrap(),
        sector(0x02)
    );
    assert_eq!(
        child.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0x01)
    );
    assert_eq!(
        cow.dirty_sectors(),
        vec![StorageSectorId::new(0), StorageSectorId::new(2)]
    );

    let snapshot = cow.snapshot();
    assert_eq!(snapshot.capacity_sectors(), 3);
    assert_eq!(snapshot.flush_count(), 1);
    assert_eq!(snapshot.dirty_sectors().len(), 2);

    cow.write_sector(StorageSectorId::new(1), sector(0xbb))
        .unwrap();
    cow.restore(&snapshot).unwrap();
    assert_eq!(
        cow.read_sector(StorageSectorId::new(1)).unwrap(),
        sector(0x02)
    );

    cow.writeback().unwrap();
    assert_eq!(
        child.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0xaa)
    );
    assert_eq!(
        child.read_sector(StorageSectorId::new(2)).unwrap(),
        sector(0xcc)
    );
}

#[test]
fn storage_images_reject_bad_requests_before_mutation() {
    assert!(matches!(
        RawStorageImage::from_bytes(vec![0; 7]),
        Err(StorageError::InvalidImageSize { bytes: 7 })
    ));

    let image = RawStorageImage::from_read_only_bytes(image_bytes(&[0x10])).unwrap();
    assert!(matches!(
        image.write_sector(StorageSectorId::new(0), sector(0xee)),
        Err(StorageError::ReadOnly)
    ));
    assert_eq!(
        image.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0x10)
    );

    assert!(matches!(
        image.read(StorageSectorId::new(1), 1),
        Err(StorageError::OutOfRange {
            sector,
            sectors: 1,
            capacity_sectors: 1,
        }) if sector == StorageSectorId::new(1)
    ));

    let child = RawStorageImage::from_read_only_bytes(image_bytes(&[0x20])).unwrap();
    let cow = CowStorageImage::new(Arc::new(child.clone()));
    cow.write_sector(StorageSectorId::new(0), sector(0xdd))
        .unwrap();
    assert!(matches!(cow.writeback(), Err(StorageError::ReadOnly)));
    assert_eq!(
        child.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0x20)
    );
    assert_eq!(
        cow.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0xdd)
    );
}

#[test]
fn cow_storage_image_rejects_read_byte_count_overflow_before_allocation() {
    let cow = CowStorageImage::new(Arc::new(HugeStorageImage));
    let sectors = u64::MAX / STORAGE_SECTOR_BYTES + 1;

    assert!(matches!(
        cow.read(StorageSectorId::new(0), sectors),
        Err(StorageError::CapacityOverflow { sectors: actual }) if actual == sectors
    ));
}

#[test]
fn nested_cow_storage_images_read_from_nearest_dirty_layer() {
    let raw = RawStorageImage::from_bytes(image_bytes(&[0x41, 0x42])).unwrap();
    let lower = CowStorageImage::new(Arc::new(raw.clone()));
    lower
        .write_sector(StorageSectorId::new(0), sector(0xa0))
        .unwrap();
    let upper = CowStorageImage::new(Arc::new(lower.clone()) as Arc<dyn StorageImageLayer>);
    upper
        .write_sector(StorageSectorId::new(1), sector(0xb1))
        .unwrap();

    assert_eq!(
        upper.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0xa0)
    );
    assert_eq!(
        upper.read_sector(StorageSectorId::new(1)).unwrap(),
        sector(0xb1)
    );
    assert_eq!(
        lower.read_sector(StorageSectorId::new(1)).unwrap(),
        sector(0x42)
    );
    assert_eq!(
        raw.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0x41)
    );
}

#[test]
fn file_storage_image_reads_writes_and_flushes_host_file_explicitly() {
    let path = temp_image_path("read-write");
    fs::write(&path, image_bytes(&[0x51, 0x52])).unwrap();

    let image = FileStorageImage::open(&path).unwrap();
    assert_eq!(image.path(), path.as_path());
    assert_eq!(image.capacity_sectors(), 2);
    assert_eq!(
        image.read_sector(StorageSectorId::new(0)).unwrap(),
        sector(0x51)
    );

    image
        .write_sector(StorageSectorId::new(1), sector(0xaa))
        .unwrap();
    assert_eq!(image.flush_count(), 0);
    image.flush().unwrap();

    assert_eq!(image.flush_count(), 1);
    assert_eq!(
        fs::read(&path).unwrap(),
        [sector(0x51), sector(0xaa)].concat()
    );

    fs::remove_file(path).unwrap();
}

#[test]
fn file_storage_image_snapshots_and_restores_host_file_bytes() {
    let path = temp_image_path("snapshot");
    fs::write(&path, image_bytes(&[0x41, 0x42])).unwrap();

    let image = FileStorageImage::open(&path).unwrap();
    image
        .write_sector(StorageSectorId::new(1), sector(0xaa))
        .unwrap();
    image.flush().unwrap();
    let snapshot = image.snapshot().unwrap();

    assert_eq!(snapshot.capacity_sectors(), 2);
    assert_eq!(snapshot.flush_count(), 1);
    assert!(!snapshot.read_only());
    assert_eq!(snapshot.bytes(), [sector(0x41), sector(0xaa)].concat());

    image
        .write_sector(StorageSectorId::new(0), sector(0xbb))
        .unwrap();
    image.restore(&snapshot).unwrap();

    assert_eq!(image.flush_count(), 1);
    assert_eq!(
        fs::read(&path).unwrap(),
        [sector(0x41), sector(0xaa)].concat()
    );

    fs::remove_file(path).unwrap();
}

#[test]
fn file_storage_image_rejects_bad_files_and_writes_before_mutation() {
    let missing_path = temp_image_path("missing");
    assert!(matches!(
        FileStorageImage::open(&missing_path),
        Err(StorageError::FileOperationFailed {
            operation: StorageFileOperation::Open,
            ..
        })
    ));

    let bad_size_path = temp_image_path("bad-size");
    fs::write(&bad_size_path, vec![0; 7]).unwrap();
    assert!(matches!(
        FileStorageImage::open(&bad_size_path),
        Err(StorageError::InvalidImageSize { bytes: 7 })
    ));

    let path = temp_image_path("rejects");
    fs::write(&path, image_bytes(&[0x61])).unwrap();
    let read_only = FileStorageImage::open_read_only(&path).unwrap();
    assert!(matches!(
        read_only.write_sector(StorageSectorId::new(0), sector(0xff)),
        Err(StorageError::ReadOnly)
    ));
    assert_eq!(fs::read(&path).unwrap(), image_bytes(&[0x61]));

    let writable = FileStorageImage::open(&path).unwrap();
    assert!(matches!(
        writable.write_sector(StorageSectorId::new(1), sector(0xee)),
        Err(StorageError::OutOfRange {
            sector,
            sectors: 1,
            capacity_sectors: 1,
        }) if sector == StorageSectorId::new(1)
    ));
    assert_eq!(fs::read(&path).unwrap(), image_bytes(&[0x61]));

    fs::remove_file(bad_size_path).unwrap();
    fs::remove_file(path).unwrap();
}

#[test]
fn storage_image_checkpoint_bank_captures_and_restores_raw_and_cow_images() {
    let raw_component = CheckpointComponentId::new("storage.raw0").unwrap();
    let cow_component = CheckpointComponentId::new("storage.cow0").unwrap();
    let raw = RawStorageImage::from_bytes(image_bytes(&[0x11, 0x22])).unwrap();
    let cow_child = RawStorageImage::from_bytes(image_bytes(&[0x33, 0x44])).unwrap();
    let cow = CowStorageImage::new(Arc::new(cow_child.clone()));
    raw.write_sector(StorageSectorId::new(1), sector(0xaa))
        .unwrap();
    cow.write_sector(StorageSectorId::new(0), sector(0xbb))
        .unwrap();
    cow.flush().unwrap();
    let raw_snapshot = raw.snapshot();
    let cow_snapshot = cow.snapshot();
    let bank = StorageImageCheckpointBank::new([
        StorageImageCheckpointPort::raw(raw_component.clone(), raw.clone()),
        StorageImageCheckpointPort::cow(cow_component.clone(), cow.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    let captured = bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(
        bank.components(),
        vec![cow_component.clone(), raw_component.clone()]
    );
    assert_eq!(
        captured,
        vec![
            StorageImageCheckpointRecord::new(
                cow_component.clone(),
                StorageImageCheckpointSnapshot::Cow(cow_snapshot.clone()),
            ),
            StorageImageCheckpointRecord::new(
                raw_component.clone(),
                StorageImageCheckpointSnapshot::Raw(raw_snapshot.clone()),
            ),
        ]
    );
    assert!(registry.chunk(&raw_component, "storage-image").is_some());
    assert!(registry.chunk(&cow_component, "storage-image").is_some());

    raw.write_sector(StorageSectorId::new(1), sector(0xcc))
        .unwrap();
    cow.write_sector(StorageSectorId::new(0), sector(0xdd))
        .unwrap();

    let restored = bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(raw.snapshot(), raw_snapshot);
    assert_eq!(cow.snapshot(), cow_snapshot);
}

#[test]
fn storage_image_checkpoint_bank_captures_and_restores_file_images() {
    let component = CheckpointComponentId::new("storage.file0").unwrap();
    let path = temp_image_path("checkpoint-file");
    fs::write(&path, image_bytes(&[0x21, 0x22])).unwrap();
    let file = FileStorageImage::open(&path).unwrap();
    file.write_sector(StorageSectorId::new(0), sector(0xaa))
        .unwrap();
    file.flush().unwrap();
    let file_snapshot = file.snapshot().unwrap();
    let bank = StorageImageCheckpointBank::new([StorageImageCheckpointPort::file(
        component.clone(),
        file.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    let captured = bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(
        captured,
        vec![StorageImageCheckpointRecord::new(
            component.clone(),
            StorageImageCheckpointSnapshot::File(file_snapshot.clone()),
        )]
    );
    assert!(registry.chunk(&component, "storage-image").is_some());

    file.write_sector(StorageSectorId::new(1), sector(0xbb))
        .unwrap();

    let restored = bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored, captured);
    assert_eq!(file.snapshot().unwrap(), file_snapshot);
    assert_eq!(
        fs::read(&path).unwrap(),
        [sector(0xaa), sector(0x22)].concat()
    );

    fs::remove_file(path).unwrap();
}

#[test]
fn storage_image_checkpoint_bank_rejects_bad_chunk_without_partial_restore() {
    let first_component = CheckpointComponentId::new("storage.raw0").unwrap();
    let second_component = CheckpointComponentId::new("storage.raw1").unwrap();
    let first = RawStorageImage::from_bytes(image_bytes(&[0x10])).unwrap();
    let second = RawStorageImage::from_bytes(image_bytes(&[0x20])).unwrap();
    let bank = StorageImageCheckpointBank::new([
        StorageImageCheckpointPort::raw(first_component.clone(), first.clone()),
        StorageImageCheckpointPort::raw(second_component.clone(), second.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    first
        .write_sector(StorageSectorId::new(0), sector(0xa0))
        .unwrap();
    second
        .write_sector(StorageSectorId::new(0), sector(0xb0))
        .unwrap();
    let first_before = first.snapshot();
    let second_before = second.snapshot();
    registry
        .write_chunk(&second_component, "storage-image", vec![0xff])
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    match error {
        StorageCheckpointError::InvalidChunk { component, .. } => {
            assert_eq!(component, second_component);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(first.snapshot(), first_before);
    assert_eq!(second.snapshot(), second_before);
}
