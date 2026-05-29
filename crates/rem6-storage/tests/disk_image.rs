use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use rem6_storage::{
    CowStorageImage, FileStorageImage, RawStorageImage, StorageError, StorageFileOperation,
    StorageImageLayer, StorageSectorId, STORAGE_SECTOR_BYTES,
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
