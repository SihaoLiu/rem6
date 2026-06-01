use std::sync::Arc;

use rem6_storage::{
    RawStorageImage, SimpleDisk, SimpleDiskError, SimpleDiskGuestMemory, SimpleDiskTransfer,
    StorageError, StorageImageLayer, StorageSectorId, STORAGE_SECTOR_BYTES,
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

#[derive(Debug)]
struct GuestMemory {
    bytes: Vec<u8>,
}

impl GuestMemory {
    fn filled(bytes: usize, value: u8) -> Self {
        Self {
            bytes: vec![value; bytes],
        }
    }

    fn write_seed(&mut self, address: u64, data: &[u8]) {
        let address = address as usize;
        self.bytes[address..address + data.len()].copy_from_slice(data);
    }

    fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

impl SimpleDiskGuestMemory for GuestMemory {
    fn read_bytes(&mut self, address: u64, bytes: u64) -> Result<Vec<u8>, SimpleDiskError> {
        let range = self.range(address, bytes)?;
        Ok(self.bytes[range].to_vec())
    }

    fn write_bytes(&mut self, address: u64, data: &[u8]) -> Result<(), SimpleDiskError> {
        let range = self.range(address, data.len() as u64)?;
        self.bytes[range].copy_from_slice(data);
        Ok(())
    }
}

impl GuestMemory {
    fn range(&self, address: u64, bytes: u64) -> Result<std::ops::Range<usize>, SimpleDiskError> {
        let end = address
            .checked_add(bytes)
            .ok_or(SimpleDiskError::GuestAddressOverflow { address, bytes })?;
        if end > self.bytes.len() as u64 {
            return Err(SimpleDiskError::GuestMemory {
                operation: "access",
                address,
                bytes,
                capacity_bytes: self.bytes.len() as u64,
            });
        }
        Ok(address as usize..end as usize)
    }
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
fn simple_disk_reads_sectors_into_guest_memory() {
    let image = Arc::new(RawStorageImage::from_bytes(image_bytes(&[0x10, 0x20, 0x30])).unwrap());
    let disk = SimpleDisk::new(image.clone() as Arc<dyn StorageImageLayer>);
    let mut guest = GuestMemory::filled(2048, 0xee);

    let transfer = disk
        .read_to_guest(&mut guest, 128, StorageSectorId::new(1), 1024)
        .unwrap();

    assert_eq!(
        transfer,
        SimpleDiskTransfer::new(128, StorageSectorId::new(1), 2, 1024)
    );
    assert_eq!(&guest.bytes()[0..128], vec![0xee; 128].as_slice());
    assert_eq!(
        &guest.bytes()[128..1152],
        [sector(0x20), sector(0x30)].concat().as_slice()
    );
    assert_eq!(&guest.bytes()[1152..], vec![0xee; 896].as_slice());
}

#[test]
fn simple_disk_writes_guest_memory_to_sectors() {
    let image = Arc::new(RawStorageImage::from_bytes(image_bytes(&[0x00, 0x00, 0x33])).unwrap());
    let disk = SimpleDisk::new(image.clone() as Arc<dyn StorageImageLayer>);
    let mut guest = GuestMemory::filled(2048, 0xee);
    guest.write_seed(256, &[sector(0xaa), sector(0xbb)].concat());

    let transfer = disk
        .write_from_guest(&mut guest, 256, StorageSectorId::new(0), 1024)
        .unwrap();
    disk.flush().unwrap();

    assert_eq!(
        transfer,
        SimpleDiskTransfer::new(256, StorageSectorId::new(0), 2, 1024)
    );
    assert_eq!(
        image.read(StorageSectorId::new(0), 3).unwrap(),
        [sector(0xaa), sector(0xbb), sector(0x33)].concat()
    );
    assert_eq!(image.flush_count(), 1);
}

#[test]
fn simple_disk_rejects_transfer_above_vec_capacity_before_allocation() {
    let image = Arc::new(HugeStorageImage);
    let disk = SimpleDisk::new(image.clone() as Arc<dyn StorageImageLayer>);
    let mut guest = GuestMemory::filled(1, 0);
    let bytes = (isize::MAX as u64 / STORAGE_SECTOR_BYTES + 1) * STORAGE_SECTOR_BYTES;

    assert!(matches!(
        disk.read_to_guest(&mut guest, 0, StorageSectorId::new(0), bytes),
        Err(SimpleDiskError::TransferTooLarge { bytes: actual }) if actual == bytes
    ));
}

#[test]
fn simple_disk_rejects_bad_requests_before_mutation() {
    let image = Arc::new(RawStorageImage::from_bytes(image_bytes(&[0x11, 0x22])).unwrap());
    let disk = SimpleDisk::new(image.clone() as Arc<dyn StorageImageLayer>);
    let mut guest = GuestMemory::filled(2048, 0xee);

    assert!(matches!(
        disk.read_to_guest(&mut guest, 128, StorageSectorId::new(0), 513),
        Err(SimpleDiskError::InvalidTransferByteCount { bytes: 513 })
    ));
    assert_eq!(guest.bytes(), vec![0xee; 2048].as_slice());

    assert!(matches!(
        disk.read_to_guest(&mut guest, 128, StorageSectorId::new(1), 1024),
        Err(SimpleDiskError::Storage(StorageError::OutOfRange {
            sector,
            sectors: 2,
            capacity_sectors: 2,
        })) if sector == StorageSectorId::new(1)
    ));
    assert_eq!(guest.bytes(), vec![0xee; 2048].as_slice());

    let read_only =
        Arc::new(RawStorageImage::from_read_only_bytes(image_bytes(&[0x41, 0x42])).unwrap());
    let read_only_disk = SimpleDisk::new(read_only.clone() as Arc<dyn StorageImageLayer>);
    guest.write_seed(512, &[sector(0xcc), sector(0xdd)].concat());

    assert!(matches!(
        read_only_disk.write_from_guest(&mut guest, 512, StorageSectorId::new(0), 1024),
        Err(SimpleDiskError::Storage(StorageError::ReadOnly))
    ));
    assert_eq!(
        read_only.read(StorageSectorId::new(0), 2).unwrap(),
        [sector(0x41), sector(0x42)].concat()
    );
}
