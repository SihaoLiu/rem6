use std::sync::Arc;

use rem6_storage::{
    IdeDeviceId, IdeDisk, IdeDiskError, RawStorageImage, StorageError, StorageImageLayer,
    StorageSectorId, IDE_COMMAND_ATAPI_IDENTIFY_DEVICE, IDE_COMMAND_IDENTIFY, IDE_COMMAND_READ,
    IDE_COMMAND_READ_NATIVE_MAX, IDE_COMMAND_WRITE, IDE_CONTROL_OFFSET, IDE_CONTROL_RST,
    IDE_DRIVE_LBA, IDE_DRIVE_OFFSET, IDE_ERROR_ABORT, IDE_ERROR_OFFSET, IDE_HCYL_OFFSET,
    IDE_LCYL_OFFSET, IDE_NSECTOR_OFFSET, IDE_SECTOR_OFFSET, IDE_STATUS_DRDY, IDE_STATUS_DRQ,
    IDE_STATUS_ERR, IDE_STATUS_OFFSET, IDE_STATUS_SEEK,
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

fn read_words(disk: &mut IdeDisk, words: usize) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(words * 2);
    for _ in 0..words {
        bytes.extend_from_slice(&disk.read_data_u16().unwrap().to_le_bytes());
    }
    bytes
}

fn word(bytes: &[u8], index: usize) -> u16 {
    u16::from_le_bytes(bytes[index * 2..index * 2 + 2].try_into().unwrap())
}

#[test]
fn ide_disk_identify_reports_geometry_capacity_and_model() {
    let image = Arc::new(RawStorageImage::from_bytes(vec![0; 128 * 512]).unwrap());
    let mut disk = IdeDisk::new(image as Arc<dyn StorageImageLayer>, IdeDeviceId::Device0).unwrap();

    assert_eq!(disk.status(), IDE_STATUS_DRDY);
    assert_eq!(disk.read_command_u8(IDE_ERROR_OFFSET).unwrap(), 0x01);

    disk.write_command_u8(IDE_STATUS_OFFSET, IDE_COMMAND_IDENTIFY)
        .unwrap();

    assert_eq!(disk.status(), IDE_STATUS_DRDY | IDE_STATUS_DRQ);
    assert!(disk.pending_interrupt());

    let identify = read_words(&mut disk, 256);

    assert_eq!(word(&identify, 1), 1);
    assert_eq!(word(&identify, 3), 2);
    assert_eq!(word(&identify, 6), 63);
    assert_eq!(&identify[54..66], b"5MI EDD si k");
    assert_eq!(word(&identify, 47), 128);
    assert_eq!(word(&identify, 49), 0x0700);
    assert_eq!(word(&identify, 53), 0x0006);
    assert_eq!(
        u32::from_le_bytes(identify[120..124].try_into().unwrap()),
        128
    );
    assert_eq!(word(&identify, 80), 0x0080);
    assert_eq!(word(&identify, 88), 0x001f);
    assert_eq!(word(&identify, 93), 0x4001);
    assert_eq!(disk.status(), IDE_STATUS_DRDY | IDE_STATUS_SEEK);
}

#[test]
fn ide_disk_reads_and_writes_lba_pio_sectors() {
    let image = Arc::new(RawStorageImage::from_bytes(image_bytes(&[0x11, 0x22, 0x33])).unwrap());
    let mut disk = IdeDisk::new(
        image.clone() as Arc<dyn StorageImageLayer>,
        IdeDeviceId::Device0,
    )
    .unwrap();

    disk.write_command_u8(IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    disk.write_command_u8(IDE_NSECTOR_OFFSET, 2).unwrap();
    disk.write_command_u8(IDE_SECTOR_OFFSET, 1).unwrap();
    disk.write_command_u8(IDE_LCYL_OFFSET, 0).unwrap();
    disk.write_command_u8(IDE_HCYL_OFFSET, 0).unwrap();
    disk.write_command_u8(IDE_STATUS_OFFSET, IDE_COMMAND_READ)
        .unwrap();

    assert_eq!(disk.status(), IDE_STATUS_DRDY | IDE_STATUS_DRQ);
    let read_payload = read_words(&mut disk, 512);
    assert_eq!(read_payload, [sector(0x22), sector(0x33)].concat());
    assert_eq!(disk.status(), IDE_STATUS_DRDY | IDE_STATUS_SEEK);

    disk.write_command_u8(IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    disk.write_command_u8(IDE_NSECTOR_OFFSET, 2).unwrap();
    disk.write_command_u8(IDE_SECTOR_OFFSET, 0).unwrap();
    disk.write_command_u8(IDE_LCYL_OFFSET, 0).unwrap();
    disk.write_command_u8(IDE_HCYL_OFFSET, 0).unwrap();
    disk.write_command_u8(IDE_STATUS_OFFSET, IDE_COMMAND_WRITE)
        .unwrap();

    assert_eq!(disk.status(), IDE_STATUS_DRDY | IDE_STATUS_DRQ);
    for chunk in [sector(0xaa), sector(0xbb)].concat().chunks_exact(2) {
        disk.write_data_u16(u16::from_le_bytes(chunk.try_into().unwrap()))
            .unwrap();
    }

    assert_eq!(disk.status(), IDE_STATUS_DRDY | IDE_STATUS_SEEK);
    assert_eq!(
        image.read(StorageSectorId::new(0), 3).unwrap(),
        [sector(0xaa), sector(0xbb), sector(0x33)].concat()
    );
}

#[test]
fn ide_disk_validates_commands_before_mutation() {
    let image = Arc::new(RawStorageImage::from_bytes(image_bytes(&[0x41, 0x42])).unwrap());
    let mut disk = IdeDisk::new(
        image.clone() as Arc<dyn StorageImageLayer>,
        IdeDeviceId::Device0,
    )
    .unwrap();

    disk.write_command_u8(IDE_NSECTOR_OFFSET, 1).unwrap();
    disk.write_command_u8(IDE_SECTOR_OFFSET, 0).unwrap();
    assert!(matches!(
        disk.write_command_u8(IDE_STATUS_OFFSET, IDE_COMMAND_READ),
        Err(IdeDiskError::ChsAccessUnsupported {
            command: IDE_COMMAND_READ,
            drive: 0,
        })
    ));
    assert_eq!(disk.status(), IDE_STATUS_DRDY);

    disk.write_command_u8(IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    disk.write_command_u8(IDE_NSECTOR_OFFSET, 2).unwrap();
    disk.write_command_u8(IDE_SECTOR_OFFSET, 1).unwrap();
    assert!(matches!(
        disk.write_command_u8(IDE_STATUS_OFFSET, IDE_COMMAND_READ),
        Err(IdeDiskError::Storage(StorageError::OutOfRange {
            sector,
            sectors: 2,
            capacity_sectors: 2,
        })) if sector == StorageSectorId::new(1)
    ));
    assert_eq!(disk.status(), IDE_STATUS_DRDY);

    disk.write_command_u8(IDE_STATUS_OFFSET, IDE_COMMAND_ATAPI_IDENTIFY_DEVICE)
        .unwrap();
    let error = disk.read_command_u8(IDE_ERROR_OFFSET).unwrap();
    assert_eq!(error & IDE_ERROR_ABORT, IDE_ERROR_ABORT);
    assert_eq!(error & 0x01, 0x01);
    assert_eq!(
        disk.status(),
        IDE_STATUS_DRDY | IDE_STATUS_SEEK | IDE_STATUS_ERR
    );

    assert!(matches!(
        disk.write_data_u16(0xaaaa),
        Err(IdeDiskError::DataWriteNotReady)
    ));
    assert_eq!(
        image.read(StorageSectorId::new(0), 2).unwrap(),
        [sector(0x41), sector(0x42)].concat()
    );
}

#[test]
fn ide_disk_control_reset_and_native_max_are_typed() {
    let image = Arc::new(RawStorageImage::from_bytes(image_bytes(&[0x01, 0x02, 0x03])).unwrap());
    let mut disk = IdeDisk::new(image as Arc<dyn StorageImageLayer>, IdeDeviceId::Device1).unwrap();

    assert_eq!(disk.device_id(), IdeDeviceId::Device1);
    assert_eq!(disk.status(), IDE_STATUS_DRDY);

    disk.write_control_u8(IDE_CONTROL_OFFSET, IDE_CONTROL_RST)
        .unwrap();
    assert!(disk.status() & IDE_STATUS_DRQ == 0);
    disk.write_control_u8(IDE_CONTROL_OFFSET, 0).unwrap();

    assert_eq!(disk.status(), IDE_STATUS_DRDY);
    assert_eq!(disk.read_command_u8(IDE_ERROR_OFFSET).unwrap(), 0x01);

    disk.write_command_u8(IDE_STATUS_OFFSET, IDE_COMMAND_READ_NATIVE_MAX)
        .unwrap();
    assert_eq!(disk.read_command_u8(IDE_SECTOR_OFFSET).unwrap(), 2);
    assert_eq!(disk.read_command_u8(IDE_LCYL_OFFSET).unwrap(), 0);
    assert_eq!(disk.read_command_u8(IDE_HCYL_OFFSET).unwrap(), 0);
    assert_eq!(disk.read_command_u8(IDE_DRIVE_OFFSET).unwrap() & 0x0f, 0);
    assert_eq!(disk.status(), IDE_STATUS_DRDY | IDE_STATUS_SEEK);
}
