use std::sync::Arc;

use rem6_storage::{
    IdeBarWriteOutcome, IdeChannelId, IdeController, IdeControllerBar, IdeControllerDispatch,
    IdeControllerError, IdeControllerSnapshot, IdeDeviceId, IdeDisk, RawStorageImage,
    StorageImageLayer, IDE_BMI_CHANNEL_BYTES, IDE_BMI_COMMAND_OFFSET, IDE_BMI_PRD_TABLE_OFFSET,
    IDE_BMI_STATUS_DMA_CAP0, IDE_BMI_STATUS_DMA_CAP1, IDE_BMI_STATUS_INTERRUPT,
    IDE_BMI_STATUS_OFFSET, IDE_COMMAND_IDENTIFY, IDE_COMMAND_READ, IDE_CONTROL_OFFSET,
    IDE_DRIVE_DEVICE1, IDE_DRIVE_LBA, IDE_DRIVE_OFFSET, IDE_HCYL_OFFSET, IDE_LCYL_OFFSET,
    IDE_NSECTOR_OFFSET, IDE_SECTOR_OFFSET, IDE_STATUS_DRDY, IDE_STATUS_DRQ, IDE_STATUS_OFFSET,
};

fn sector(byte: u8) -> [u8; 512] {
    [byte; 512]
}

fn disk(byte: u8, device_id: IdeDeviceId) -> IdeDisk {
    let image = Arc::new(RawStorageImage::from_bytes(sector(byte).to_vec()).unwrap());
    IdeDisk::new(image as Arc<dyn StorageImageLayer>, device_id).unwrap()
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
fn ide_controller_reports_unsupported_dma_start_as_typed_error() {
    let primary0 = disk(0x10, IdeDeviceId::Device0);
    let mut controller = IdeController::new([Some(primary0), None, None, None]).unwrap();

    assert!(matches!(
        controller.write_bmi_u8(IdeChannelId::Primary, IDE_BMI_COMMAND_OFFSET, 1),
        Err(IdeControllerError::DmaUnsupported {
            channel: IdeChannelId::Primary,
        })
    ));
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
