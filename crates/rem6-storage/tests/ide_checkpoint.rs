use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointRegistry};
use rem6_storage::{
    IdeController, IdeControllerCheckpointBank, IdeControllerCheckpointPort,
    IdeControllerGuestMemory, IdeControllerSnapshot, IdeDeviceId, IdeDisk, RawStorageImage,
    StorageCheckpointError, StorageImageLayer, IDE_BMI_COMMAND_OFFSET, IDE_BMI_COMMAND_RW,
    IDE_BMI_COMMAND_START, IDE_BMI_PRD_TABLE_OFFSET, IDE_COMMAND_READ_DMA, IDE_DRIVE_LBA,
    IDE_DRIVE_OFFSET, IDE_HCYL_OFFSET, IDE_LCYL_OFFSET, IDE_NSECTOR_OFFSET, IDE_SECTOR_OFFSET,
    IDE_STATUS_OFFSET,
};

fn sector(byte: u8) -> [u8; 512] {
    [byte; 512]
}

fn disk(byte: u8, device_id: IdeDeviceId) -> IdeDisk {
    let image = Arc::new(RawStorageImage::from_bytes(sector(byte).to_vec()).unwrap());
    IdeDisk::new(image as Arc<dyn StorageImageLayer>, device_id).unwrap()
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

    fn range(
        &self,
        operation: &'static str,
        address: u64,
        bytes: u64,
    ) -> Result<std::ops::Range<usize>, rem6_storage::IdeControllerError> {
        let end = address
            .checked_add(bytes)
            .ok_or(rem6_storage::IdeControllerError::GuestAddressOverflow { address, bytes })?;
        if end > self.bytes.len() as u64 {
            return Err(rem6_storage::IdeControllerError::GuestMemory {
                operation,
                address,
                bytes,
                capacity_bytes: self.bytes.len() as u64,
            });
        }
        Ok(address as usize..end as usize)
    }
}

impl IdeControllerGuestMemory for GuestMemory {
    fn validate_read(
        &self,
        address: u64,
        bytes: u64,
    ) -> Result<(), rem6_storage::IdeControllerError> {
        self.range("read", address, bytes).map(|_| ())
    }

    fn validate_write(
        &self,
        address: u64,
        bytes: u64,
    ) -> Result<(), rem6_storage::IdeControllerError> {
        self.range("write", address, bytes).map(|_| ())
    }

    fn read_bytes(
        &mut self,
        address: u64,
        bytes: u64,
    ) -> Result<Vec<u8>, rem6_storage::IdeControllerError> {
        let range = self.range("read", address, bytes)?;
        Ok(self.bytes[range].to_vec())
    }

    fn write_bytes(
        &mut self,
        address: u64,
        data: &[u8],
    ) -> Result<(), rem6_storage::IdeControllerError> {
        let range = self.range("write", address, data.len() as u64)?;
        self.bytes[range].copy_from_slice(data);
        Ok(())
    }
}

fn write_prd(guest: &mut GuestMemory, table: u64, base: u32, byte_count: u16, end: bool) {
    let mut entry = [0_u8; 8];
    entry[0..4].copy_from_slice(&base.to_le_bytes());
    entry[4..6].copy_from_slice(&byte_count.to_le_bytes());
    entry[6..8].copy_from_slice(&(if end { 0x8000_u16 } else { 0 }).to_le_bytes());
    guest.write_seed(table, &entry);
}

fn issue_read_dma(controller: &mut IdeController) {
    controller
        .write_command_u8(
            rem6_storage::IdeChannelId::Primary,
            IDE_DRIVE_OFFSET,
            IDE_DRIVE_LBA,
        )
        .unwrap();
    controller
        .write_command_u8(rem6_storage::IdeChannelId::Primary, IDE_NSECTOR_OFFSET, 1)
        .unwrap();
    controller
        .write_command_u8(rem6_storage::IdeChannelId::Primary, IDE_SECTOR_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(rem6_storage::IdeChannelId::Primary, IDE_LCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(rem6_storage::IdeChannelId::Primary, IDE_HCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(
            rem6_storage::IdeChannelId::Primary,
            IDE_STATUS_OFFSET,
            IDE_COMMAND_READ_DMA,
        )
        .unwrap();
}

#[test]
fn ide_controller_checkpoint_bank_restores_active_dma_snapshot() {
    let component = CheckpointComponentId::new("storage.ide0").unwrap();
    let controller = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x6d, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));
    let mut guest = GuestMemory::filled(4096, 0xee);
    write_prd(&mut guest, 0x80, 0x200, 512, true);
    {
        let mut locked = controller.lock().unwrap();
        locked
            .write_bmi_u32(
                rem6_storage::IdeChannelId::Primary,
                IDE_BMI_PRD_TABLE_OFFSET,
                0x80,
            )
            .unwrap();
        issue_read_dma(&mut locked);
        locked
            .write_bmi_u8(
                rem6_storage::IdeChannelId::Primary,
                IDE_BMI_COMMAND_OFFSET,
                IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW,
            )
            .unwrap();
    }
    let captured_snapshot = controller.lock().unwrap().snapshot();
    let bank = IdeControllerCheckpointBank::new([IdeControllerCheckpointPort::new(
        component.clone(),
        controller.clone(),
    )])
    .unwrap();
    let mut registry = CheckpointRegistry::new();

    bank.register_all(&mut registry).unwrap();
    let captured = bank.capture_all_into(&mut registry).unwrap();

    assert_eq!(captured[0].component(), &component);
    assert_eq!(captured[0].snapshot(), &captured_snapshot);
    assert!(registry.chunk(&component, "ide-controller").is_some());

    controller
        .lock()
        .unwrap()
        .write_bmi_u8(
            rem6_storage::IdeChannelId::Primary,
            IDE_BMI_COMMAND_OFFSET,
            0,
        )
        .unwrap();
    let restored = bank.restore_all_from(&registry).unwrap();

    assert_eq!(restored[0].snapshot(), &captured_snapshot);
    controller
        .lock()
        .unwrap()
        .execute_dma(rem6_storage::IdeChannelId::Primary, &mut guest)
        .unwrap();
    assert_eq!(&guest.bytes()[0x200..0x400], sector(0x6d).as_slice());
}

#[test]
fn ide_controller_checkpoint_bank_rejects_bad_chunk_without_partial_restore() {
    let first_component = CheckpointComponentId::new("storage.ide0").unwrap();
    let second_component = CheckpointComponentId::new("storage.ide1").unwrap();
    let first = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x10, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));
    let second = Arc::new(Mutex::new(
        IdeController::new([Some(disk(0x20, IdeDeviceId::Device0)), None, None, None]).unwrap(),
    ));
    let bank = IdeControllerCheckpointBank::new([
        IdeControllerCheckpointPort::new(first_component.clone(), first.clone()),
        IdeControllerCheckpointPort::new(second_component.clone(), second.clone()),
    ])
    .unwrap();
    let mut registry = CheckpointRegistry::new();
    bank.register_all(&mut registry).unwrap();
    bank.capture_all_into(&mut registry).unwrap();
    first
        .lock()
        .unwrap()
        .write_command_u8(
            rem6_storage::IdeChannelId::Primary,
            IDE_DRIVE_OFFSET,
            IDE_DRIVE_LBA,
        )
        .unwrap();
    second
        .lock()
        .unwrap()
        .write_command_u8(
            rem6_storage::IdeChannelId::Primary,
            IDE_DRIVE_OFFSET,
            IDE_DRIVE_LBA,
        )
        .unwrap();
    let first_before: IdeControllerSnapshot = first.lock().unwrap().snapshot();
    let second_before: IdeControllerSnapshot = second.lock().unwrap().snapshot();
    registry
        .write_chunk(&second_component, "ide-controller", vec![0xff])
        .unwrap();

    let error = bank.restore_all_from(&registry).unwrap_err();

    match error {
        StorageCheckpointError::InvalidChunk { component, .. } => {
            assert_eq!(component, second_component);
        }
        other => panic!("unexpected error: {other:?}"),
    }
    assert_eq!(first.lock().unwrap().snapshot(), first_before);
    assert_eq!(second.lock().unwrap().snapshot(), second_before);
}
