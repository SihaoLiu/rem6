use std::sync::Arc;

use rem6_storage::{
    IdeBarWriteOutcome, IdeChannelId, IdeController, IdeControllerBar, IdeControllerDispatch,
    IdeControllerError, IdeControllerGuestMemory, IdeControllerSnapshot, IdeDeviceId, IdeDisk,
    RawStorageImage, StorageImageLayer, StorageSectorId, IDE_BMI_CHANNEL_BYTES,
    IDE_BMI_COMMAND_OFFSET, IDE_BMI_COMMAND_RW, IDE_BMI_COMMAND_START, IDE_BMI_PRD_TABLE_OFFSET,
    IDE_BMI_STATUS_ACTIVE, IDE_BMI_STATUS_DMA_CAP0, IDE_BMI_STATUS_DMA_CAP1,
    IDE_BMI_STATUS_INTERRUPT, IDE_BMI_STATUS_OFFSET, IDE_COMMAND_IDENTIFY, IDE_COMMAND_READ,
    IDE_COMMAND_READ_DMA, IDE_COMMAND_WRITE_DMA, IDE_CONTROL_OFFSET, IDE_DRIVE_DEVICE1,
    IDE_DRIVE_LBA, IDE_DRIVE_OFFSET, IDE_HCYL_OFFSET, IDE_LCYL_OFFSET, IDE_NSECTOR_OFFSET,
    IDE_SECTOR_OFFSET, IDE_STATUS_DRDY, IDE_STATUS_DRQ, IDE_STATUS_OFFSET,
};

fn sector(byte: u8) -> [u8; 512] {
    [byte; 512]
}

fn disk(byte: u8, device_id: IdeDeviceId) -> IdeDisk {
    let image = Arc::new(RawStorageImage::from_bytes(sector(byte).to_vec()).unwrap());
    IdeDisk::new(image as Arc<dyn StorageImageLayer>, device_id).unwrap()
}

fn disk_with_image(byte: u8, device_id: IdeDeviceId) -> (IdeDisk, Arc<RawStorageImage>) {
    let image = Arc::new(RawStorageImage::from_bytes(sector(byte).to_vec()).unwrap());
    (
        IdeDisk::new(image.clone() as Arc<dyn StorageImageLayer>, device_id).unwrap(),
        image,
    )
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
    ) -> Result<std::ops::Range<usize>, IdeControllerError> {
        let end = address
            .checked_add(bytes)
            .ok_or(IdeControllerError::GuestAddressOverflow { address, bytes })?;
        if end > self.bytes.len() as u64 {
            return Err(IdeControllerError::GuestMemory {
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
    fn validate_read(&self, address: u64, bytes: u64) -> Result<(), IdeControllerError> {
        self.range("read", address, bytes).map(|_| ())
    }

    fn validate_write(&self, address: u64, bytes: u64) -> Result<(), IdeControllerError> {
        self.range("write", address, bytes).map(|_| ())
    }

    fn read_bytes(&mut self, address: u64, bytes: u64) -> Result<Vec<u8>, IdeControllerError> {
        let range = self.range("read", address, bytes)?;
        Ok(self.bytes[range].to_vec())
    }

    fn write_bytes(&mut self, address: u64, data: &[u8]) -> Result<(), IdeControllerError> {
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

fn read_sector(controller: &mut IdeController, channel: IdeChannelId) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(512);
    for _ in 0..256 {
        bytes.extend_from_slice(&controller.read_data_u16(channel).unwrap().to_le_bytes());
    }
    bytes
}

fn issue_lba_read(controller: &mut IdeController, channel: IdeChannelId) {
    controller
        .write_command_u8(channel, IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_NSECTOR_OFFSET, 1)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_SECTOR_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_LCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_HCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_STATUS_OFFSET, IDE_COMMAND_READ)
        .unwrap();
}

fn issue_dma_command(controller: &mut IdeController, channel: IdeChannelId, command: u8) {
    controller
        .write_command_u8(channel, IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_NSECTOR_OFFSET, 1)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_SECTOR_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_LCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_HCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(channel, IDE_STATUS_OFFSET, command)
        .unwrap();
}

#[test]
fn ide_controller_routes_pio_to_selected_devices() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let primary1 = disk(0x20, IdeDeviceId::Device1);
    let mut controller = IdeController::new([Some(primary0), Some(primary1), None, None]).unwrap();

    issue_lba_read(&mut controller, IdeChannelId::Primary);
    assert_eq!(
        controller.read_command_u8(IdeChannelId::Primary, IDE_STATUS_OFFSET),
        Ok(IDE_STATUS_DRDY | IDE_STATUS_DRQ)
    );
    assert_eq!(
        read_sector(&mut controller, IdeChannelId::Primary),
        sector(0x10)
    );

    controller
        .write_command_u8(
            IdeChannelId::Primary,
            IDE_DRIVE_OFFSET,
            IDE_DRIVE_LBA | IDE_DRIVE_DEVICE1,
        )
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_NSECTOR_OFFSET, 1)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_SECTOR_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_LCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_HCYL_OFFSET, 0)
        .unwrap();
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_STATUS_OFFSET, IDE_COMMAND_READ)
        .unwrap();

    assert_eq!(
        read_sector(&mut controller, IdeChannelId::Primary),
        sector(0x20)
    );
}

#[test]
fn ide_controller_tracks_shared_interrupt_state() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let secondary0 = disk(0x30, IdeDeviceId::Device0);
    let mut controller =
        IdeController::new([Some(primary0), None, Some(secondary0), None]).unwrap();

    controller
        .write_command_u8(
            IdeChannelId::Primary,
            IDE_STATUS_OFFSET,
            IDE_COMMAND_IDENTIFY,
        )
        .unwrap();
    controller
        .write_command_u8(
            IdeChannelId::Secondary,
            IDE_STATUS_OFFSET,
            IDE_COMMAND_IDENTIFY,
        )
        .unwrap();

    assert!(controller.channel_pending_interrupt(IdeChannelId::Primary));
    assert!(controller.channel_pending_interrupt(IdeChannelId::Secondary));
    assert!(controller.shared_interrupt_asserted());

    assert_eq!(
        controller.read_command_u8(IdeChannelId::Primary, IDE_STATUS_OFFSET),
        Ok(IDE_STATUS_DRDY | IDE_STATUS_DRQ)
    );
    assert!(!controller.channel_pending_interrupt(IdeChannelId::Primary));
    assert!(controller.shared_interrupt_asserted());

    assert_eq!(
        controller.read_command_u8(IdeChannelId::Secondary, IDE_STATUS_OFFSET),
        Ok(IDE_STATUS_DRDY | IDE_STATUS_DRQ)
    );
    assert!(!controller.shared_interrupt_asserted());
}

#[test]
fn ide_controller_handles_missing_selected_device_without_panic() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();

    controller
        .write_command_u8(
            IdeChannelId::Primary,
            IDE_DRIVE_OFFSET,
            IDE_DRIVE_LBA | IDE_DRIVE_DEVICE1,
        )
        .unwrap();

    assert_eq!(
        controller.read_command_u8(IdeChannelId::Primary, IDE_STATUS_OFFSET),
        Ok(0)
    );
    controller
        .write_command_u8(
            IdeChannelId::Primary,
            IDE_STATUS_OFFSET,
            IDE_COMMAND_IDENTIFY,
        )
        .unwrap();
    assert!(!controller.shared_interrupt_asserted());

    controller
        .write_command_u8(IdeChannelId::Primary, IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    assert_eq!(
        controller.read_command_u8(IdeChannelId::Primary, IDE_STATUS_OFFSET),
        Ok(IDE_STATUS_DRDY)
    );
}

#[test]
fn ide_controller_bmi_registers_preserve_typed_status() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();

    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1)
    );

    controller
        .write_command_u8(
            IdeChannelId::Primary,
            IDE_STATUS_OFFSET,
            IDE_COMMAND_IDENTIFY,
        )
        .unwrap();
    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1 | IDE_BMI_STATUS_INTERRUPT)
    );

    controller
        .write_bmi_u8(
            IdeChannelId::Primary,
            IDE_BMI_STATUS_OFFSET,
            IDE_BMI_STATUS_INTERRUPT,
        )
        .unwrap();
    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1)
    );
    assert!(!controller.shared_interrupt_asserted());

    controller
        .write_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET, 0x1234_5673)
        .unwrap();
    assert_eq!(
        controller.read_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET),
        Ok(0x1234_5670)
    );
}

#[test]
fn ide_controller_rejects_dma_start_without_dma_command_before_active() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();

    assert!(matches!(
        controller.write_bmi_u8(IdeChannelId::Primary, IDE_BMI_COMMAND_OFFSET, 1),
        Err(IdeControllerError::Disk {
            channel: IdeChannelId::Primary,
            source: rem6_storage::IdeDiskError::DmaNotReady { .. },
        })
    ));
    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1)
    );
}

#[test]
fn ide_controller_dispatches_command_and_control_bars_with_layout_policy() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let primary1 = disk(0x20, IdeDeviceId::Device1);
    let mut controller = IdeController::new([Some(primary0), Some(primary1), None, None]).unwrap();
    let dispatch = IdeControllerDispatch::new(1, u64::from(IDE_CONTROL_OFFSET)).unwrap();

    assert_eq!(
        controller.write_bar_u8(
            dispatch,
            IdeControllerBar::PrimaryCommand,
            u64::from(IDE_DRIVE_OFFSET) << 1,
            IDE_DRIVE_LBA | IDE_DRIVE_DEVICE1,
        ),
        Ok(IdeBarWriteOutcome::Applied)
    );
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::PrimaryCommand,
            u64::from(IDE_NSECTOR_OFFSET) << 1,
            1,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::PrimaryCommand,
            u64::from(IDE_SECTOR_OFFSET) << 1,
            0,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::PrimaryCommand,
            u64::from(IDE_LCYL_OFFSET) << 1,
            0,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::PrimaryCommand,
            u64::from(IDE_HCYL_OFFSET) << 1,
            0,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::PrimaryCommand,
            u64::from(IDE_STATUS_OFFSET) << 1,
            IDE_COMMAND_READ,
        )
        .unwrap();

    assert_eq!(
        controller.read_bar_u8(dispatch, IdeControllerBar::PrimaryControl, 0),
        Ok(IDE_STATUS_DRDY | IDE_STATUS_DRQ)
    );
    let mut first_word_bytes = Vec::new();
    first_word_bytes.extend_from_slice(
        &controller
            .read_bar_u16(dispatch, IdeControllerBar::PrimaryCommand, 0)
            .unwrap()
            .to_le_bytes(),
    );
    assert_eq!(first_word_bytes, vec![0x20, 0x20]);
}

#[test]
fn ide_controller_dispatches_secondary_and_bmi_bar_windows() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let secondary0 = disk(0x30, IdeDeviceId::Device0);
    let mut controller =
        IdeController::new([Some(primary0), None, Some(secondary0), None]).unwrap();
    let dispatch = IdeControllerDispatch::new(0, u64::from(IDE_CONTROL_OFFSET))
        .unwrap()
        .with_bus_master_enabled(true);

    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::SecondaryCommand,
            u64::from(IDE_STATUS_OFFSET),
            IDE_COMMAND_IDENTIFY,
        )
        .unwrap();

    assert_eq!(
        controller.read_bar_u8(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_CHANNEL_BYTES + IDE_BMI_STATUS_OFFSET),
        ),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1 | IDE_BMI_STATUS_INTERRUPT)
    );
    assert_eq!(
        controller.write_bar_u8(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_CHANNEL_BYTES + IDE_BMI_STATUS_OFFSET),
            IDE_BMI_STATUS_INTERRUPT,
        ),
        Ok(IdeBarWriteOutcome::Applied)
    );
    assert_eq!(
        controller.read_bar_u8(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_CHANNEL_BYTES + IDE_BMI_STATUS_OFFSET),
        ),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1)
    );

    assert_eq!(
        controller.write_bar_u32(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_PRD_TABLE_OFFSET),
            0x9988_7765,
        ),
        Ok(IdeBarWriteOutcome::Applied)
    );
    assert_eq!(
        controller.read_bar_u32(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_PRD_TABLE_OFFSET),
        ),
        Ok(0x9988_7764)
    );
}

#[test]
fn ide_controller_ignores_bus_master_writes_when_disabled() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();
    let dispatch = IdeControllerDispatch::new(0, u64::from(IDE_CONTROL_OFFSET)).unwrap();

    assert_eq!(
        controller.write_bar_u32(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_PRD_TABLE_OFFSET),
            0x1111_2220,
        ),
        Ok(IdeBarWriteOutcome::IgnoredBusMasterDisabled)
    );
    assert_eq!(
        controller.read_bar_u32(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_PRD_TABLE_OFFSET),
        ),
        Ok(0)
    );
}

#[test]
fn ide_controller_snapshot_restores_channel_selection_bmi_and_disk_transfer() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let primary1 = disk(0x20, IdeDeviceId::Device1);
    let secondary0 = disk(0x30, IdeDeviceId::Device0);
    let mut controller =
        IdeController::new([Some(primary0), Some(primary1), Some(secondary0), None]).unwrap();
    let dispatch = IdeControllerDispatch::new(0, u64::from(IDE_CONTROL_OFFSET))
        .unwrap()
        .with_bus_master_enabled(true);

    controller
        .write_command_u8(
            IdeChannelId::Primary,
            IDE_DRIVE_OFFSET,
            IDE_DRIVE_LBA | IDE_DRIVE_DEVICE1,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::SecondaryCommand,
            u64::from(IDE_DRIVE_OFFSET),
            IDE_DRIVE_LBA,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::SecondaryCommand,
            u64::from(IDE_NSECTOR_OFFSET),
            1,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::SecondaryCommand,
            u64::from(IDE_SECTOR_OFFSET),
            0,
        )
        .unwrap();
    controller
        .write_bar_u8(
            dispatch,
            IdeControllerBar::SecondaryCommand,
            u64::from(IDE_STATUS_OFFSET),
            IDE_COMMAND_READ,
        )
        .unwrap();
    controller
        .write_bar_u32(
            dispatch,
            IdeControllerBar::BusMaster,
            u64::from(IDE_BMI_CHANNEL_BYTES + IDE_BMI_PRD_TABLE_OFFSET),
            0x1234_5678,
        )
        .unwrap();

    let snapshot = controller.snapshot();
    assert_eq!(
        snapshot.channel(IdeChannelId::Primary).selected_device(),
        IdeDeviceId::Device1
    );
    assert_eq!(
        snapshot.channel(IdeChannelId::Secondary).bmi().prd_table(),
        0x1234_5678
    );

    assert_eq!(
        controller.read_bar_u16(dispatch, IdeControllerBar::SecondaryCommand, 0),
        Ok(0x3030)
    );
    controller
        .write_command_u8(IdeChannelId::Primary, IDE_DRIVE_OFFSET, IDE_DRIVE_LBA)
        .unwrap();
    controller.restore(&snapshot).unwrap();

    assert_eq!(
        controller
            .snapshot()
            .channel(IdeChannelId::Primary)
            .selected_device(),
        IdeDeviceId::Device1
    );
    assert_eq!(
        controller.read_bar_u16(dispatch, IdeControllerBar::SecondaryCommand, 0),
        Ok(0x3030)
    );
}

#[test]
fn ide_controller_restore_rejects_shape_mismatch_before_mutation() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();
    let before = controller.snapshot();
    let wrong = IdeControllerSnapshot::from_channels([
        before.channel(IdeChannelId::Primary).clone(),
        before.channel(IdeChannelId::Primary).clone(),
    ]);

    assert!(matches!(
        controller.restore(&wrong),
        Err(IdeControllerError::SnapshotChannelMismatch {
            channel: IdeChannelId::Secondary,
        })
    ));
    assert_eq!(controller.snapshot(), before);
}

#[test]
fn ide_controller_executes_read_dma_from_disk_to_guest_memory() {
    let primary0 = disk(0x5a, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();
    let mut guest = GuestMemory::filled(4096, 0xee);
    write_prd(&mut guest, 0x80, 0x200, 512, true);
    controller
        .write_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET, 0x80)
        .unwrap();
    issue_dma_command(&mut controller, IdeChannelId::Primary, IDE_COMMAND_READ_DMA);
    controller
        .write_bmi_u8(
            IdeChannelId::Primary,
            IDE_BMI_COMMAND_OFFSET,
            IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW,
        )
        .unwrap();

    assert_eq!(
        controller.execute_dma(IdeChannelId::Primary, &mut guest),
        Ok(())
    );

    assert_eq!(&guest.bytes()[0x200..0x400], sector(0x5a).as_slice());
    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_COMMAND_OFFSET),
        Ok(IDE_BMI_COMMAND_RW)
    );
    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1 | IDE_BMI_STATUS_INTERRUPT)
    );
    assert!(controller.shared_interrupt_asserted());
}

#[test]
fn ide_controller_snapshot_restores_started_dma_transfer() {
    let primary0 = disk(0x6c, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();
    let mut guest = GuestMemory::filled(4096, 0xee);
    write_prd(&mut guest, 0x80, 0x200, 512, true);
    controller
        .write_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET, 0x80)
        .unwrap();
    issue_dma_command(&mut controller, IdeChannelId::Primary, IDE_COMMAND_READ_DMA);
    controller
        .write_bmi_u8(
            IdeChannelId::Primary,
            IDE_BMI_COMMAND_OFFSET,
            IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW,
        )
        .unwrap();

    let snapshot = controller.snapshot();
    controller
        .write_bmi_u8(IdeChannelId::Primary, IDE_BMI_COMMAND_OFFSET, 0)
        .unwrap();
    controller.restore(&snapshot).unwrap();

    assert_eq!(
        controller.execute_dma(IdeChannelId::Primary, &mut guest),
        Ok(())
    );
    assert_eq!(&guest.bytes()[0x200..0x400], sector(0x6c).as_slice());
}

#[test]
fn ide_controller_executes_write_dma_from_guest_memory_to_disk() {
    let (primary0, image) = disk_with_image(0x00, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();
    let mut guest = GuestMemory::filled(4096, 0xee);
    guest.write_seed(0x200, &sector(0xa5));
    write_prd(&mut guest, 0x80, 0x200, 512, true);
    controller
        .write_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET, 0x80)
        .unwrap();
    issue_dma_command(
        &mut controller,
        IdeChannelId::Primary,
        IDE_COMMAND_WRITE_DMA,
    );
    controller
        .write_bmi_u8(
            IdeChannelId::Primary,
            IDE_BMI_COMMAND_OFFSET,
            IDE_BMI_COMMAND_START,
        )
        .unwrap();

    assert_eq!(
        controller.execute_dma(IdeChannelId::Primary, &mut guest),
        Ok(())
    );

    assert_eq!(
        image.read(StorageSectorId::new(0), 1).unwrap(),
        sector(0xa5).to_vec()
    );
    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1 | IDE_BMI_STATUS_INTERRUPT)
    );
}

#[test]
fn ide_controller_dma_rejects_bad_prd_without_mutation() {
    let primary0 = disk(0x33, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();
    let mut guest = GuestMemory::filled(4096, 0xee);
    write_prd(&mut guest, 0x80, 0x200, 256, true);
    controller
        .write_bmi_u32(IdeChannelId::Primary, IDE_BMI_PRD_TABLE_OFFSET, 0x80)
        .unwrap();
    issue_dma_command(&mut controller, IdeChannelId::Primary, IDE_COMMAND_READ_DMA);
    controller
        .write_bmi_u8(
            IdeChannelId::Primary,
            IDE_BMI_COMMAND_OFFSET,
            IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW,
        )
        .unwrap();
    let before = controller.snapshot();
    let guest_before = guest.bytes().to_vec();

    assert!(matches!(
        controller.execute_dma(IdeChannelId::Primary, &mut guest),
        Err(IdeControllerError::InvalidPrdByteCount {
            channel: IdeChannelId::Primary,
            bytes: 256,
        })
    ));

    assert_eq!(controller.snapshot(), before);
    assert_eq!(guest.bytes(), guest_before.as_slice());
    assert_eq!(
        controller.read_bmi_u8(IdeChannelId::Primary, IDE_BMI_STATUS_OFFSET),
        Ok(IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1 | IDE_BMI_STATUS_ACTIVE)
    );
}
