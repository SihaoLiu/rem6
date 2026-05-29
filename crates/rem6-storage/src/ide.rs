use std::sync::Arc;

use crate::{
    IdeBmiSnapshot, IdeChannelSnapshot, IdeControllerError, IdeControllerGuestMemory,
    IdeControllerSnapshot, IdeDiskError, IdeDiskSnapshot, IdeDiskTransferSnapshot, IdeDmaDirection,
    IdeDmaPlan, IdeDmaRequest, StorageError, StorageImageLayer, StorageSectorId,
    STORAGE_SECTOR_BYTES,
};

pub const IDE_DATA_OFFSET: u8 = 0;
pub const IDE_ERROR_OFFSET: u8 = 1;
pub const IDE_FEATURES_OFFSET: u8 = 1;
pub const IDE_NSECTOR_OFFSET: u8 = 2;
pub const IDE_SECTOR_OFFSET: u8 = 3;
pub const IDE_LCYL_OFFSET: u8 = 4;
pub const IDE_HCYL_OFFSET: u8 = 5;
pub const IDE_DRIVE_OFFSET: u8 = 6;
pub const IDE_STATUS_OFFSET: u8 = 7;
pub const IDE_COMMAND_OFFSET: u8 = 7;
pub const IDE_CONTROL_OFFSET: u8 = 2;
pub const IDE_ALTSTAT_OFFSET: u8 = 2;

pub const IDE_DRIVE_DEVICE1: u8 = 0x10;
pub const IDE_DRIVE_LBA: u8 = 0x40;
pub const IDE_CONTROL_RST: u8 = 0x04;
pub const IDE_CONTROL_IEN: u8 = 0x02;
pub const IDE_STATUS_BSY: u8 = 0x80;
pub const IDE_STATUS_DRDY: u8 = 0x40;
pub const IDE_STATUS_DF: u8 = 0x20;
pub const IDE_STATUS_SEEK: u8 = 0x10;
pub const IDE_STATUS_DRQ: u8 = 0x08;
pub const IDE_STATUS_ERR: u8 = 0x01;
pub const IDE_ERROR_ABORT: u8 = 0x04;

pub const IDE_COMMAND_READ: u8 = 0x20;
pub const IDE_COMMAND_WRITE: u8 = 0x30;
pub const IDE_COMMAND_READ_MULTI: u8 = 0xc4;
pub const IDE_COMMAND_WRITE_MULTI: u8 = 0xc5;
pub const IDE_COMMAND_READ_DMA: u8 = 0xc8;
pub const IDE_COMMAND_WRITE_DMA: u8 = 0xca;
pub const IDE_COMMAND_IDENTIFY: u8 = 0xec;
pub const IDE_COMMAND_ATAPI_IDENTIFY_DEVICE: u8 = 0xa1;
pub const IDE_COMMAND_READ_NATIVE_MAX: u8 = 0xf8;

pub const IDE_BMI_COMMAND_OFFSET: u8 = 0x0;
pub const IDE_BMI_STATUS_OFFSET: u8 = 0x2;
pub const IDE_BMI_PRD_TABLE_OFFSET: u8 = 0x4;
pub const IDE_BMI_COMMAND_START: u8 = 0x01;
pub const IDE_BMI_COMMAND_RW: u8 = 0x08;
pub const IDE_BMI_STATUS_ACTIVE: u8 = 0x01;
pub const IDE_BMI_STATUS_DMA_ERROR: u8 = 0x02;
pub const IDE_BMI_STATUS_INTERRUPT: u8 = 0x04;
pub const IDE_BMI_STATUS_DMA_CAP1: u8 = 0x20;
pub const IDE_BMI_STATUS_DMA_CAP0: u8 = 0x40;
pub const IDE_BMI_CHANNEL_BYTES: u8 = 8;

const IDE_IDENTIFY_BYTES: usize = 512;
const IDE_MAX_MULTI_SECTORS: u16 = 128;
const IDE_ATA7_MAJOR: u16 = 0x0080;
const IDE_MODEL: &[u8] = b"5MI EDD si k";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdeDeviceId {
    Device0,
    Device1,
}

impl IdeDeviceId {
    pub const fn as_u8(self) -> u8 {
        match self {
            Self::Device0 => 0,
            Self::Device1 => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdeChannelId {
    Primary,
    Secondary,
}

impl IdeChannelId {
    pub(crate) const fn index(self) -> usize {
        match self {
            Self::Primary => 0,
            Self::Secondary => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdeControllerBar {
    PrimaryCommand,
    PrimaryControl,
    SecondaryCommand,
    SecondaryControl,
    BusMaster,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdeControllerDispatch {
    io_shift: u8,
    control_offset: u64,
    bus_master_enabled: bool,
}

impl IdeControllerDispatch {
    pub fn new(io_shift: u8, control_offset: u64) -> Result<Self, IdeControllerError> {
        if io_shift > 7 {
            return Err(IdeControllerError::InvalidDispatch {
                io_shift,
                control_offset,
            });
        }
        Ok(Self {
            io_shift,
            control_offset,
            bus_master_enabled: false,
        })
    }

    pub const fn with_bus_master_enabled(mut self, enabled: bool) -> Self {
        self.bus_master_enabled = enabled;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdeBarWriteOutcome {
    Applied,
    IgnoredBusMasterDisabled,
}

#[derive(Clone, Debug)]
pub struct IdeController {
    channels: [IdeChannel; 2],
}

impl IdeController {
    pub fn new(disks: [Option<IdeDisk>; 4]) -> Result<Self, IdeControllerError> {
        let [primary0, primary1, secondary0, secondary1] = disks;
        Self::validate_slot(0, IdeDeviceId::Device0, primary0.as_ref())?;
        Self::validate_slot(1, IdeDeviceId::Device1, primary1.as_ref())?;
        Self::validate_slot(2, IdeDeviceId::Device0, secondary0.as_ref())?;
        Self::validate_slot(3, IdeDeviceId::Device1, secondary1.as_ref())?;

        Ok(Self {
            channels: [
                IdeChannel::new(IdeChannelId::Primary, primary0, primary1),
                IdeChannel::new(IdeChannelId::Secondary, secondary0, secondary1),
            ],
        })
    }

    pub fn snapshot(&self) -> IdeControllerSnapshot {
        IdeControllerSnapshot {
            channels: [
                self.channel(IdeChannelId::Primary).snapshot(),
                self.channel(IdeChannelId::Secondary).snapshot(),
            ],
        }
    }

    pub fn restore(&mut self, snapshot: &IdeControllerSnapshot) -> Result<(), IdeControllerError> {
        self.validate_snapshot(snapshot)?;
        for channel_snapshot in &snapshot.channels {
            let channel = self.channel_mut(channel_snapshot.id);
            channel.restore(channel_snapshot)?;
        }
        Ok(())
    }

    pub fn read_bar_u8(
        &mut self,
        dispatch: IdeControllerDispatch,
        bar: IdeControllerBar,
        offset: u64,
    ) -> Result<u8, IdeControllerError> {
        match bar {
            IdeControllerBar::PrimaryCommand => {
                self.read_command_u8(IdeChannelId::Primary, command_bar_offset(dispatch, offset)?)
            }
            IdeControllerBar::PrimaryControl => {
                self.read_control_u8(IdeChannelId::Primary, control_bar_offset(dispatch, offset)?)
            }
            IdeControllerBar::SecondaryCommand => {
                self.read_command_u8(IdeChannelId::Secondary, offset_u8(bar, offset, 1)?)
            }
            IdeControllerBar::SecondaryControl => self.read_control_u8(
                IdeChannelId::Secondary,
                control_bar_offset(dispatch, offset)?,
            ),
            IdeControllerBar::BusMaster => {
                let (channel, bmi_offset) = bmi_bar_offset(offset, 1)?;
                self.read_bmi_u8(channel, bmi_offset)
            }
        }
    }

    pub fn write_bar_u8(
        &mut self,
        dispatch: IdeControllerDispatch,
        bar: IdeControllerBar,
        offset: u64,
        value: u8,
    ) -> Result<IdeBarWriteOutcome, IdeControllerError> {
        match bar {
            IdeControllerBar::PrimaryCommand => {
                self.write_command_u8(
                    IdeChannelId::Primary,
                    command_bar_offset(dispatch, offset)?,
                    value,
                )?;
            }
            IdeControllerBar::PrimaryControl => {
                self.write_control_u8(
                    IdeChannelId::Primary,
                    control_bar_offset(dispatch, offset)?,
                    value,
                )?;
            }
            IdeControllerBar::SecondaryCommand => {
                self.write_command_u8(IdeChannelId::Secondary, offset_u8(bar, offset, 1)?, value)?;
            }
            IdeControllerBar::SecondaryControl => {
                self.write_control_u8(
                    IdeChannelId::Secondary,
                    control_bar_offset(dispatch, offset)?,
                    value,
                )?;
            }
            IdeControllerBar::BusMaster => {
                if !dispatch.bus_master_enabled {
                    return Ok(IdeBarWriteOutcome::IgnoredBusMasterDisabled);
                }
                let (channel, bmi_offset) = bmi_bar_offset(offset, 1)?;
                self.write_bmi_u8(channel, bmi_offset, value)?;
            }
        }
        Ok(IdeBarWriteOutcome::Applied)
    }

    pub fn read_bar_u16(
        &mut self,
        dispatch: IdeControllerDispatch,
        bar: IdeControllerBar,
        offset: u64,
    ) -> Result<u16, IdeControllerError> {
        match bar {
            IdeControllerBar::PrimaryCommand
                if command_bar_offset(dispatch, offset)? == IDE_DATA_OFFSET =>
            {
                self.read_data_u16(IdeChannelId::Primary)
            }
            IdeControllerBar::SecondaryCommand if offset_u8(bar, offset, 2)? == IDE_DATA_OFFSET => {
                self.read_data_u16(IdeChannelId::Secondary)
            }
            _ => Err(IdeControllerError::UnsupportedBarWidth {
                bar,
                offset,
                width: 2,
            }),
        }
    }

    pub fn write_bar_u16(
        &mut self,
        dispatch: IdeControllerDispatch,
        bar: IdeControllerBar,
        offset: u64,
        value: u16,
    ) -> Result<IdeBarWriteOutcome, IdeControllerError> {
        match bar {
            IdeControllerBar::PrimaryCommand
                if command_bar_offset(dispatch, offset)? == IDE_DATA_OFFSET =>
            {
                self.write_data_u16(IdeChannelId::Primary, value)?;
            }
            IdeControllerBar::SecondaryCommand if offset_u8(bar, offset, 2)? == IDE_DATA_OFFSET => {
                self.write_data_u16(IdeChannelId::Secondary, value)?;
            }
            _ => {
                return Err(IdeControllerError::UnsupportedBarWidth {
                    bar,
                    offset,
                    width: 2,
                });
            }
        }
        Ok(IdeBarWriteOutcome::Applied)
    }

    pub fn read_bar_u32(
        &self,
        _dispatch: IdeControllerDispatch,
        bar: IdeControllerBar,
        offset: u64,
    ) -> Result<u32, IdeControllerError> {
        if bar != IdeControllerBar::BusMaster {
            return Err(IdeControllerError::UnsupportedBarWidth {
                bar,
                offset,
                width: 4,
            });
        }
        let (channel, bmi_offset) = bmi_bar_offset(offset, 4)?;
        self.read_bmi_u32(channel, bmi_offset)
    }

    pub fn write_bar_u32(
        &mut self,
        dispatch: IdeControllerDispatch,
        bar: IdeControllerBar,
        offset: u64,
        value: u32,
    ) -> Result<IdeBarWriteOutcome, IdeControllerError> {
        if bar != IdeControllerBar::BusMaster {
            return Err(IdeControllerError::UnsupportedBarWidth {
                bar,
                offset,
                width: 4,
            });
        }
        if !dispatch.bus_master_enabled {
            return Ok(IdeBarWriteOutcome::IgnoredBusMasterDisabled);
        }
        let (channel, bmi_offset) = bmi_bar_offset(offset, 4)?;
        self.write_bmi_u32(channel, bmi_offset, value)?;
        Ok(IdeBarWriteOutcome::Applied)
    }

    pub fn channel_pending_interrupt(&self, channel: IdeChannelId) -> bool {
        self.channel(channel).pending_interrupt
    }

    pub fn shared_interrupt_asserted(&self) -> bool {
        self.channels
            .iter()
            .any(|channel| channel.pending_interrupt)
    }

    pub fn read_command_u8(
        &mut self,
        channel: IdeChannelId,
        offset: u8,
    ) -> Result<u8, IdeControllerError> {
        let command_channel = self.channel_mut(channel);
        let value = if let Some(disk) = command_channel.selected_mut() {
            disk.read_command_u8(offset)
                .map_err(|source| IdeControllerError::Disk { channel, source })?
        } else {
            0
        };
        command_channel.refresh_interrupt();
        Ok(value)
    }

    pub fn write_command_u8(
        &mut self,
        channel: IdeChannelId,
        offset: u8,
        value: u8,
    ) -> Result<(), IdeControllerError> {
        let command_channel = self.channel_mut(channel);
        if offset == IDE_DRIVE_OFFSET {
            command_channel.select(value & IDE_DRIVE_DEVICE1 != 0);
        }

        if let Some(disk) = command_channel.selected_mut() {
            disk.write_command_u8(offset, value)
                .map_err(|source| IdeControllerError::Disk { channel, source })?;
        }
        command_channel.refresh_interrupt();
        Ok(())
    }

    pub fn read_control_u8(
        &mut self,
        channel: IdeChannelId,
        offset: u8,
    ) -> Result<u8, IdeControllerError> {
        let command_channel = self.channel_mut(channel);
        let value = if let Some(disk) = command_channel.selected_mut() {
            disk.read_control_u8(offset)
                .map_err(|source| IdeControllerError::Disk { channel, source })?
        } else {
            0
        };
        command_channel.refresh_interrupt();
        Ok(value)
    }

    pub fn write_control_u8(
        &mut self,
        channel: IdeChannelId,
        offset: u8,
        value: u8,
    ) -> Result<(), IdeControllerError> {
        let command_channel = self.channel_mut(channel);
        if let Some(disk) = command_channel.selected_mut() {
            disk.write_control_u8(offset, value)
                .map_err(|source| IdeControllerError::Disk { channel, source })?;
        }
        command_channel.refresh_interrupt();
        Ok(())
    }

    pub fn read_data_u16(&mut self, channel: IdeChannelId) -> Result<u16, IdeControllerError> {
        let command_channel = self.channel_mut(channel);
        let selected_device = command_channel.selected_device;
        let disk = command_channel
            .selected_mut()
            .ok_or(IdeControllerError::NoSelectedDevice {
                channel,
                device: selected_device,
            })?;
        let value = disk
            .read_data_u16()
            .map_err(|source| IdeControllerError::Disk { channel, source })?;
        command_channel.refresh_interrupt();
        Ok(value)
    }

    pub fn write_data_u16(
        &mut self,
        channel: IdeChannelId,
        value: u16,
    ) -> Result<(), IdeControllerError> {
        let command_channel = self.channel_mut(channel);
        let selected_device = command_channel.selected_device;
        let disk = command_channel
            .selected_mut()
            .ok_or(IdeControllerError::NoSelectedDevice {
                channel,
                device: selected_device,
            })?;
        disk.write_data_u16(value)
            .map_err(|source| IdeControllerError::Disk { channel, source })?;
        command_channel.refresh_interrupt();
        Ok(())
    }

    pub fn read_bmi_u8(&self, channel: IdeChannelId, offset: u8) -> Result<u8, IdeControllerError> {
        self.channel(channel).bmi.read_u8(channel, offset)
    }

    pub fn write_bmi_u8(
        &mut self,
        channel: IdeChannelId,
        offset: u8,
        value: u8,
    ) -> Result<(), IdeControllerError> {
        let command_channel = self.channel_mut(channel);
        match offset {
            IDE_BMI_COMMAND_OFFSET => command_channel.write_bmi_command(value),
            IDE_BMI_STATUS_OFFSET => {
                command_channel.bmi.write_status(value);
                if value & IDE_BMI_STATUS_INTERRUPT != 0 {
                    command_channel.pending_interrupt = false;
                }
                Ok(())
            }
            _ => Err(IdeControllerError::InvalidBmiOffset {
                channel,
                offset,
                width: 1,
            }),
        }
    }

    pub fn read_bmi_u32(
        &self,
        channel: IdeChannelId,
        offset: u8,
    ) -> Result<u32, IdeControllerError> {
        self.channel(channel).bmi.read_u32(channel, offset)
    }

    pub fn write_bmi_u32(
        &mut self,
        channel: IdeChannelId,
        offset: u8,
        value: u32,
    ) -> Result<(), IdeControllerError> {
        match offset {
            IDE_BMI_PRD_TABLE_OFFSET => {
                self.channel_mut(channel).bmi.prd_table = value & !0x3;
                Ok(())
            }
            _ => Err(IdeControllerError::InvalidBmiOffset {
                channel,
                offset,
                width: 4,
            }),
        }
    }

    pub fn execute_dma(
        &mut self,
        channel: IdeChannelId,
        guest: &mut impl IdeControllerGuestMemory,
    ) -> Result<(), IdeControllerError> {
        let command_channel = self.channel(channel);
        let request = command_channel
            .selected()
            .ok_or(IdeControllerError::NoSelectedDevice {
                channel,
                device: command_channel.selected_device,
            })?
            .dma_request()
            .map_err(|source| IdeControllerError::Disk { channel, source })?;
        if command_channel.bmi.command & IDE_BMI_COMMAND_START == 0 {
            return Err(IdeControllerError::DmaNotActive { channel });
        }
        let expected_bytes = request
            .bytes()
            .ok_or(IdeControllerError::DmaByteCountOverflow { channel })?;
        let plan = IdeDmaPlan::decode(
            channel,
            guest,
            command_channel.bmi.prd_table,
            expected_bytes,
        )?;
        plan.validate_access(request.direction(), guest)?;

        let payload = match request.direction() {
            IdeDmaDirection::ToGuest => self
                .channel(channel)
                .selected()
                .unwrap()
                .read_dma_payload(request)
                .map_err(|source| IdeControllerError::Disk { channel, source })?,
            IdeDmaDirection::FromGuest => plan.read_payload(guest)?,
        };

        match request.direction() {
            IdeDmaDirection::ToGuest => plan.write_payload(guest, &payload)?,
            IdeDmaDirection::FromGuest => self
                .channel(channel)
                .selected()
                .unwrap()
                .commit_dma_payload(request, &payload)
                .map_err(|source| IdeControllerError::Disk { channel, source })?,
        }

        let command_channel = self.channel_mut(channel);
        command_channel
            .selected_mut()
            .unwrap()
            .complete_dma()
            .map_err(|source| IdeControllerError::Disk { channel, source })?;
        command_channel.complete_dma();
        Ok(())
    }

    fn validate_slot(
        slot: usize,
        expected: IdeDeviceId,
        disk: Option<&IdeDisk>,
    ) -> Result<(), IdeControllerError> {
        if let Some(disk) = disk {
            let actual = disk.device_id();
            if actual != expected {
                return Err(IdeControllerError::InvalidDeviceSlot {
                    slot,
                    expected,
                    actual,
                });
            }
        }
        Ok(())
    }

    fn channel(&self, channel: IdeChannelId) -> &IdeChannel {
        &self.channels[channel.index()]
    }

    fn channel_mut(&mut self, channel: IdeChannelId) -> &mut IdeChannel {
        &mut self.channels[channel.index()]
    }

    fn validate_snapshot(
        &self,
        snapshot: &IdeControllerSnapshot,
    ) -> Result<(), IdeControllerError> {
        if snapshot.channels[0].id != IdeChannelId::Primary {
            return Err(IdeControllerError::SnapshotChannelMismatch {
                channel: IdeChannelId::Primary,
            });
        }
        if snapshot.channels[1].id != IdeChannelId::Secondary {
            return Err(IdeControllerError::SnapshotChannelMismatch {
                channel: IdeChannelId::Secondary,
            });
        }
        for channel_snapshot in snapshot.channels() {
            self.channel(channel_snapshot.id)
                .validate_snapshot(channel_snapshot)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug)]
struct IdeChannel {
    id: IdeChannelId,
    device0: Option<IdeDisk>,
    device1: Option<IdeDisk>,
    selected_device: IdeDeviceId,
    pending_interrupt: bool,
    bmi: IdeBmiRegisters,
}

impl IdeChannel {
    fn new(id: IdeChannelId, device0: Option<IdeDisk>, device1: Option<IdeDisk>) -> Self {
        Self {
            id,
            device0,
            device1,
            selected_device: IdeDeviceId::Device0,
            pending_interrupt: false,
            bmi: IdeBmiRegisters::reset(),
        }
    }

    fn select(&mut self, select_device_1: bool) {
        self.selected_device = if select_device_1 {
            IdeDeviceId::Device1
        } else {
            IdeDeviceId::Device0
        };
    }

    fn selected_mut(&mut self) -> Option<&mut IdeDisk> {
        match self.selected_device {
            IdeDeviceId::Device0 => self.device0.as_mut(),
            IdeDeviceId::Device1 => self.device1.as_mut(),
        }
    }

    fn selected(&self) -> Option<&IdeDisk> {
        match self.selected_device {
            IdeDeviceId::Device0 => self.device0.as_ref(),
            IdeDeviceId::Device1 => self.device1.as_ref(),
        }
    }

    fn refresh_interrupt(&mut self) {
        self.pending_interrupt = self
            .selected()
            .is_some_and(|selected| selected.pending_interrupt());
        if self.pending_interrupt {
            self.bmi.status |= IDE_BMI_STATUS_INTERRUPT;
        } else {
            self.bmi.status &= !IDE_BMI_STATUS_INTERRUPT;
        }
    }

    fn write_bmi_command(&mut self, value: u8) -> Result<(), IdeControllerError> {
        let old_start = self.bmi.command & IDE_BMI_COMMAND_START != 0;
        let new_start = value & IDE_BMI_COMMAND_START != 0;
        if !old_start && new_start {
            let request = self
                .selected()
                .ok_or(IdeControllerError::NoSelectedDevice {
                    channel: self.id,
                    device: self.selected_device,
                })?
                .dma_request()
                .map_err(|source| IdeControllerError::Disk {
                    channel: self.id,
                    source,
                })?;
            let host_to_device = value & IDE_BMI_COMMAND_RW == 0;
            if host_to_device != (request.direction() == IdeDmaDirection::FromGuest) {
                return Err(IdeControllerError::DmaDirectionMismatch { channel: self.id });
            }
            self.bmi.status |= IDE_BMI_STATUS_ACTIVE;
        }
        if old_start && !new_start {
            self.bmi.status &= !IDE_BMI_STATUS_ACTIVE;
        }
        self.bmi.command = value & (IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW);
        Ok(())
    }

    fn complete_dma(&mut self) {
        self.bmi.command &= !IDE_BMI_COMMAND_START;
        self.bmi.status &= !IDE_BMI_STATUS_ACTIVE;
        self.bmi.status |= IDE_BMI_STATUS_INTERRUPT;
        self.pending_interrupt = true;
    }

    fn snapshot(&self) -> IdeChannelSnapshot {
        IdeChannelSnapshot {
            id: self.id,
            selected_device: self.selected_device,
            pending_interrupt: self.pending_interrupt,
            bmi: self.bmi.snapshot(),
            device0: self.device0.as_ref().map(IdeDisk::snapshot),
            device1: self.device1.as_ref().map(IdeDisk::snapshot),
        }
    }

    fn validate_snapshot(&self, snapshot: &IdeChannelSnapshot) -> Result<(), IdeControllerError> {
        if snapshot.id != self.id {
            return Err(IdeControllerError::SnapshotChannelMismatch { channel: self.id });
        }
        validate_channel_device_snapshot(
            self.id,
            IdeDeviceId::Device0,
            &self.device0,
            &snapshot.device0,
        )?;
        validate_channel_device_snapshot(
            self.id,
            IdeDeviceId::Device1,
            &self.device1,
            &snapshot.device1,
        )?;
        Ok(())
    }

    fn restore(&mut self, snapshot: &IdeChannelSnapshot) -> Result<(), IdeControllerError> {
        self.selected_device = snapshot.selected_device;
        self.pending_interrupt = snapshot.pending_interrupt;
        self.bmi.restore(snapshot.bmi);
        if let (Some(device), Some(device_snapshot)) = (&mut self.device0, &snapshot.device0) {
            device
                .restore(device_snapshot)
                .map_err(|source| IdeControllerError::Disk {
                    channel: self.id,
                    source,
                })?;
        }
        if let (Some(device), Some(device_snapshot)) = (&mut self.device1, &snapshot.device1) {
            device
                .restore(device_snapshot)
                .map_err(|source| IdeControllerError::Disk {
                    channel: self.id,
                    source,
                })?;
        }
        Ok(())
    }
}

fn validate_channel_device_snapshot(
    channel: IdeChannelId,
    device: IdeDeviceId,
    live: &Option<IdeDisk>,
    snapshot: &Option<IdeDiskSnapshot>,
) -> Result<(), IdeControllerError> {
    match (live, snapshot) {
        (Some(live), Some(snapshot)) => {
            if snapshot.device_id() != live.device_id() || snapshot.device_id() != device {
                return Err(IdeControllerError::SnapshotDeviceMismatch { channel, device });
            }
            live.validate_snapshot(snapshot)
                .map_err(|source| IdeControllerError::Disk { channel, source })
        }
        (None, None) => Ok(()),
        _ => Err(IdeControllerError::SnapshotDeviceMismatch { channel, device }),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IdeBmiRegisters {
    command: u8,
    status: u8,
    prd_table: u32,
}

impl IdeBmiRegisters {
    const fn reset() -> Self {
        Self {
            command: 0,
            status: IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1,
            prd_table: 0,
        }
    }

    fn read_u8(self, channel: IdeChannelId, offset: u8) -> Result<u8, IdeControllerError> {
        match offset {
            IDE_BMI_COMMAND_OFFSET => Ok(self.command),
            IDE_BMI_STATUS_OFFSET => Ok(self.status),
            _ => Err(IdeControllerError::InvalidBmiOffset {
                channel,
                offset,
                width: 1,
            }),
        }
    }

    fn read_u32(self, channel: IdeChannelId, offset: u8) -> Result<u32, IdeControllerError> {
        match offset {
            IDE_BMI_PRD_TABLE_OFFSET => Ok(self.prd_table),
            _ => Err(IdeControllerError::InvalidBmiOffset {
                channel,
                offset,
                width: 4,
            }),
        }
    }

    fn write_status(&mut self, value: u8) {
        let mut status = self.status
            & (IDE_BMI_STATUS_ACTIVE
                | IDE_BMI_STATUS_DMA_ERROR
                | IDE_BMI_STATUS_INTERRUPT
                | IDE_BMI_STATUS_DMA_CAP0
                | IDE_BMI_STATUS_DMA_CAP1);
        if value & IDE_BMI_STATUS_INTERRUPT != 0 {
            status &= !IDE_BMI_STATUS_INTERRUPT;
        }
        if value & IDE_BMI_STATUS_DMA_ERROR != 0 {
            status &= !IDE_BMI_STATUS_DMA_ERROR;
        }
        self.status = status | IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1;
    }

    const fn snapshot(self) -> IdeBmiSnapshot {
        IdeBmiSnapshot {
            command: self.command,
            status: self.status,
            prd_table: self.prd_table,
        }
    }

    fn restore(&mut self, snapshot: IdeBmiSnapshot) {
        self.command = snapshot.command;
        self.status = snapshot.status | IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1;
        self.prd_table = snapshot.prd_table & !0x3;
    }
}

fn command_bar_offset(
    dispatch: IdeControllerDispatch,
    offset: u64,
) -> Result<u8, IdeControllerError> {
    let shifted = offset
        .checked_shr(u32::from(dispatch.io_shift))
        .unwrap_or(0);
    offset_u8(IdeControllerBar::PrimaryCommand, shifted, 1)
}

fn control_bar_offset(
    dispatch: IdeControllerDispatch,
    offset: u64,
) -> Result<u8, IdeControllerError> {
    let adjusted = offset.checked_add(dispatch.control_offset).ok_or(
        IdeControllerError::BarOffsetOverflow {
            bar: IdeControllerBar::PrimaryControl,
            offset,
            addend: dispatch.control_offset,
        },
    )?;
    offset_u8(IdeControllerBar::PrimaryControl, adjusted, 1)
}

fn bmi_bar_offset(offset: u64, width: u8) -> Result<(IdeChannelId, u8), IdeControllerError> {
    let channel_bytes = u64::from(IDE_BMI_CHANNEL_BYTES);
    let total_bytes = channel_bytes * 2;
    let width_u64 = u64::from(width);
    let end = offset
        .checked_add(width_u64)
        .ok_or(IdeControllerError::BarOffsetOverflow {
            bar: IdeControllerBar::BusMaster,
            offset,
            addend: width_u64,
        })?;
    if end > total_bytes {
        return Err(IdeControllerError::InvalidBarOffset {
            bar: IdeControllerBar::BusMaster,
            offset,
            width,
        });
    }

    let (channel, local) = if offset < channel_bytes {
        (IdeChannelId::Primary, offset)
    } else {
        (IdeChannelId::Secondary, offset - channel_bytes)
    };
    if local + width_u64 > channel_bytes {
        return Err(IdeControllerError::InvalidBarOffset {
            bar: IdeControllerBar::BusMaster,
            offset,
            width,
        });
    }
    Ok((channel, local as u8))
}

fn offset_u8(bar: IdeControllerBar, offset: u64, width: u8) -> Result<u8, IdeControllerError> {
    let width_u64 = u64::from(width);
    let end = offset
        .checked_add(width_u64)
        .ok_or(IdeControllerError::BarOffsetOverflow {
            bar,
            offset,
            addend: width_u64,
        })?;
    if end > u64::from(u8::MAX) + 1 {
        return Err(IdeControllerError::InvalidBarOffset { bar, offset, width });
    }
    Ok(offset as u8)
}

#[derive(Clone, Debug)]
pub struct IdeDisk {
    image: Arc<dyn StorageImageLayer>,
    device_id: IdeDeviceId,
    registers: IdeTaskFile,
    status: u8,
    control: u8,
    pending_interrupt: bool,
    transfer: Option<IdeTransfer>,
}

impl IdeDisk {
    pub fn new(
        image: Arc<dyn StorageImageLayer>,
        device_id: IdeDeviceId,
    ) -> Result<Self, IdeDiskError> {
        let capacity = image.capacity_sectors();
        if capacity == 0 {
            return Err(IdeDiskError::InvalidCapacity { sectors: capacity });
        }
        if capacity > u64::from(u32::MAX) {
            return Err(IdeDiskError::InvalidCapacity { sectors: capacity });
        }

        Ok(Self {
            image,
            device_id,
            registers: IdeTaskFile::reset(),
            status: IDE_STATUS_DRDY,
            control: 0,
            pending_interrupt: false,
            transfer: None,
        })
    }

    pub fn snapshot(&self) -> IdeDiskSnapshot {
        IdeDiskSnapshot {
            device_id: self.device_id,
            task_file: self.registers,
            status: self.status,
            control: self.control,
            pending_interrupt: self.pending_interrupt,
            transfer: self.transfer.as_ref().map(IdeTransfer::snapshot),
        }
    }

    pub fn restore(&mut self, snapshot: &IdeDiskSnapshot) -> Result<(), IdeDiskError> {
        self.validate_snapshot(snapshot)?;
        self.device_id = snapshot.device_id;
        self.registers = snapshot.task_file;
        self.status = snapshot.status;
        self.control = snapshot.control;
        self.pending_interrupt = snapshot.pending_interrupt;
        self.transfer = snapshot
            .transfer
            .as_ref()
            .map(IdeTransfer::from_snapshot)
            .transpose()?;
        Ok(())
    }

    fn validate_snapshot(&self, snapshot: &IdeDiskSnapshot) -> Result<(), IdeDiskError> {
        if snapshot.device_id != self.device_id {
            return Err(IdeDiskError::SnapshotDeviceMismatch {
                expected: self.device_id,
                actual: snapshot.device_id,
            });
        }
        if let Some(transfer) = &snapshot.transfer {
            transfer.validate()?;
        }
        Ok(())
    }

    pub const fn device_id(&self) -> IdeDeviceId {
        self.device_id
    }

    pub const fn status(&self) -> u8 {
        self.status
    }

    pub const fn pending_interrupt(&self) -> bool {
        self.pending_interrupt
    }

    pub fn read_command_u8(&mut self, offset: u8) -> Result<u8, IdeDiskError> {
        match offset {
            IDE_ERROR_OFFSET => Ok(self.registers.error),
            IDE_NSECTOR_OFFSET => Ok(self.registers.sector_count),
            IDE_SECTOR_OFFSET => Ok(self.registers.sector_number),
            IDE_LCYL_OFFSET => Ok(self.registers.cylinder_low),
            IDE_HCYL_OFFSET => Ok(self.registers.cylinder_high),
            IDE_DRIVE_OFFSET => Ok(self.registers.drive),
            IDE_STATUS_OFFSET => {
                self.pending_interrupt = false;
                Ok(self.status)
            }
            _ => Err(IdeDiskError::InvalidCommandOffset { offset }),
        }
    }

    pub fn write_command_u8(&mut self, offset: u8, value: u8) -> Result<(), IdeDiskError> {
        match offset {
            IDE_FEATURES_OFFSET => Ok(()),
            IDE_NSECTOR_OFFSET => {
                self.registers.sector_count = value;
                Ok(())
            }
            IDE_SECTOR_OFFSET => {
                self.registers.sector_number = value;
                Ok(())
            }
            IDE_LCYL_OFFSET => {
                self.registers.cylinder_low = value;
                Ok(())
            }
            IDE_HCYL_OFFSET => {
                self.registers.cylinder_high = value;
                Ok(())
            }
            IDE_DRIVE_OFFSET => {
                self.registers.drive = value;
                Ok(())
            }
            IDE_COMMAND_OFFSET => self.start_command(value),
            _ => Err(IdeDiskError::InvalidCommandOffset { offset }),
        }
    }

    pub fn read_control_u8(&self, offset: u8) -> Result<u8, IdeDiskError> {
        match offset {
            IDE_ALTSTAT_OFFSET => Ok(self.status),
            _ => Err(IdeDiskError::InvalidControlOffset { offset }),
        }
    }

    pub fn write_control_u8(&mut self, offset: u8, value: u8) -> Result<(), IdeDiskError> {
        if offset != IDE_CONTROL_OFFSET {
            return Err(IdeDiskError::InvalidControlOffset { offset });
        }

        let was_reset = self.control & IDE_CONTROL_RST != 0;
        self.control = value;
        if value & IDE_CONTROL_RST != 0 {
            self.status = IDE_STATUS_BSY;
            self.pending_interrupt = false;
            self.transfer = None;
        } else if was_reset {
            self.registers = IdeTaskFile::reset();
            self.status = IDE_STATUS_DRDY;
            self.pending_interrupt = false;
            self.transfer = None;
        }
        Ok(())
    }

    pub fn read_data_u16(&mut self) -> Result<u16, IdeDiskError> {
        let transfer = self
            .transfer
            .as_mut()
            .ok_or(IdeDiskError::DataReadNotReady)?;
        if transfer.direction != IdeTransferDirection::Input {
            return Err(IdeDiskError::DataReadNotReady);
        }
        let word = transfer.read_word()?;
        if transfer.is_complete() {
            self.complete_data_transfer();
        } else if transfer.sector_boundary() && !self.interrupts_disabled() {
            self.pending_interrupt = true;
        }
        Ok(word)
    }

    pub fn write_data_u16(&mut self, value: u16) -> Result<(), IdeDiskError> {
        let transfer = self
            .transfer
            .as_mut()
            .ok_or(IdeDiskError::DataWriteNotReady)?;
        if transfer.direction != IdeTransferDirection::Output {
            return Err(IdeDiskError::DataWriteNotReady);
        }
        transfer.write_word(value)?;
        if transfer.is_complete() {
            let completed = self.transfer.take().unwrap();
            self.commit_write_transfer(completed)?;
            self.complete_data_transfer();
        }
        Ok(())
    }

    fn start_command(&mut self, command: u8) -> Result<(), IdeDiskError> {
        self.registers.command = command;
        self.registers.error &= !IDE_ERROR_ABORT;
        self.status &= !IDE_STATUS_ERR;
        self.pending_interrupt = false;
        self.transfer = None;

        match command {
            IDE_COMMAND_READ_NATIVE_MAX => {
                self.start_busy();
                let native_max = self.capacity_sectors_u32()? - 1;
                self.registers.sector_number = (native_max & 0xff) as u8;
                self.registers.cylinder_low = ((native_max >> 8) & 0xff) as u8;
                self.registers.cylinder_high = ((native_max >> 16) & 0xff) as u8;
                self.registers.drive =
                    (self.registers.drive & !0x0f) | (((native_max >> 24) & 0x0f) as u8);
                self.complete_command(false);
                Ok(())
            }
            IDE_COMMAND_IDENTIFY => {
                self.start_busy();
                let payload = self.identify_payload()?;
                self.prepare_input(payload);
                Ok(())
            }
            IDE_COMMAND_ATAPI_IDENTIFY_DEVICE => {
                self.start_busy();
                self.registers.error |= IDE_ERROR_ABORT;
                self.complete_command(true);
                Ok(())
            }
            IDE_COMMAND_READ | IDE_COMMAND_READ_MULTI => {
                let (start, sectors) = self.lba_transfer(command)?;
                self.start_busy();
                let mut payload = Vec::with_capacity(sectors as usize * IDE_IDENTIFY_BYTES);
                for offset in 0..sectors {
                    payload.extend_from_slice(
                        &self
                            .image
                            .read_sector(StorageSectorId::new(start + offset))?,
                    );
                }
                self.prepare_input(payload);
                Ok(())
            }
            IDE_COMMAND_WRITE | IDE_COMMAND_WRITE_MULTI => {
                let (start, sectors) = self.lba_transfer(command)?;
                self.start_busy();
                self.transfer = Some(IdeTransfer::new_output(start, sectors)?);
                self.status = IDE_STATUS_DRDY | IDE_STATUS_DRQ;
                Ok(())
            }
            IDE_COMMAND_READ_DMA | IDE_COMMAND_WRITE_DMA => {
                let (start, sectors) = self.lba_transfer(command)?;
                self.start_busy();
                let direction = if command == IDE_COMMAND_READ_DMA {
                    IdeDmaDirection::ToGuest
                } else {
                    IdeDmaDirection::FromGuest
                };
                self.transfer = Some(IdeTransfer::new_dma(direction, start, sectors)?);
                self.status = IDE_STATUS_DRDY | IDE_STATUS_DRQ;
                Ok(())
            }
            _ => Err(IdeDiskError::UnsupportedCommand { command }),
        }
    }

    fn start_busy(&mut self) {
        self.status |= IDE_STATUS_BSY;
        self.status &= !(IDE_STATUS_DRQ | IDE_STATUS_DF);
    }

    fn prepare_input(&mut self, payload: Vec<u8>) {
        self.transfer = Some(IdeTransfer::new_input(payload));
        self.status = IDE_STATUS_DRDY | IDE_STATUS_DRQ;
        if !self.interrupts_disabled() {
            self.pending_interrupt = true;
        }
    }

    fn complete_command(&mut self, error: bool) {
        self.status = IDE_STATUS_DRDY | IDE_STATUS_SEEK;
        if error {
            self.status |= IDE_STATUS_ERR;
        }
        if !self.interrupts_disabled() {
            self.pending_interrupt = true;
        }
    }

    fn complete_data_transfer(&mut self) {
        self.transfer = None;
        self.status = IDE_STATUS_DRDY | IDE_STATUS_SEEK;
        self.pending_interrupt = false;
    }

    fn dma_request(&self) -> Result<IdeDmaRequest, IdeDiskError> {
        match self.transfer.as_ref() {
            Some(transfer) => transfer.dma_request(),
            None => Err(IdeDiskError::DmaNotReady {
                command: self.registers.command,
            }),
        }
    }

    fn read_dma_payload(&self, request: IdeDmaRequest) -> Result<Vec<u8>, IdeDiskError> {
        if request.direction() != IdeDmaDirection::ToGuest {
            return Err(IdeDiskError::DmaDirectionMismatch {
                command: self.registers.command,
            });
        }
        let mut payload = Vec::with_capacity(request.bytes().unwrap_or(0) as usize);
        for offset in 0..request.sectors() {
            payload.extend_from_slice(
                &self
                    .image
                    .read_sector(StorageSectorId::new(request.start_sector() + offset))?,
            );
        }
        Ok(payload)
    }

    fn commit_dma_payload(
        &self,
        request: IdeDmaRequest,
        payload: &[u8],
    ) -> Result<(), IdeDiskError> {
        if request.direction() != IdeDmaDirection::FromGuest {
            return Err(IdeDiskError::DmaDirectionMismatch {
                command: self.registers.command,
            });
        }
        let expected = request.bytes().ok_or(IdeDiskError::TransferTooLarge {
            sectors: request.sectors(),
        })?;
        if payload.len() as u64 != expected {
            return Err(IdeDiskError::DmaByteCountMismatch {
                expected_bytes: expected,
                actual_bytes: payload.len() as u64,
            });
        }
        for (offset, chunk) in payload
            .chunks_exact(STORAGE_SECTOR_BYTES as usize)
            .enumerate()
        {
            let mut sector = [0_u8; STORAGE_SECTOR_BYTES as usize];
            sector.copy_from_slice(chunk);
            self.image.write_sector(
                StorageSectorId::new(request.start_sector() + offset as u64),
                sector,
            )?;
        }
        Ok(())
    }

    fn complete_dma(&mut self) -> Result<(), IdeDiskError> {
        self.dma_request()?;
        self.transfer = None;
        self.status = IDE_STATUS_DRDY | IDE_STATUS_SEEK;
        self.pending_interrupt = !self.interrupts_disabled();
        Ok(())
    }

    fn commit_write_transfer(&self, transfer: IdeTransfer) -> Result<(), IdeDiskError> {
        for (offset, chunk) in transfer
            .payload
            .chunks_exact(IDE_IDENTIFY_BYTES)
            .enumerate()
        {
            let mut sector = [0_u8; IDE_IDENTIFY_BYTES];
            sector.copy_from_slice(chunk);
            self.image.write_sector(
                StorageSectorId::new(transfer.start_sector + offset as u64),
                sector,
            )?;
        }
        Ok(())
    }

    fn lba_transfer(&self, command: u8) -> Result<(u64, u64), IdeDiskError> {
        if self.registers.drive & IDE_DRIVE_LBA == 0 {
            return Err(IdeDiskError::ChsAccessUnsupported {
                command,
                drive: self.registers.drive,
            });
        }

        let sectors = if self.registers.sector_count == 0 {
            256
        } else {
            u64::from(self.registers.sector_count)
        };
        let start = self.registers.lba_base();
        let end = start
            .checked_add(sectors)
            .ok_or(StorageError::RequestAddressOverflow {
                sector: StorageSectorId::new(start),
                sectors,
            })?;
        let capacity = self.image.capacity_sectors();
        if end > capacity {
            return Err(StorageError::OutOfRange {
                sector: StorageSectorId::new(start),
                sectors,
                capacity_sectors: capacity,
            }
            .into());
        }
        Ok((start, sectors))
    }

    fn identify_payload(&self) -> Result<Vec<u8>, IdeDiskError> {
        let capacity = self.capacity_sectors_u32()?;
        let geometry = IdeGeometry::from_capacity(capacity)?;
        let mut bytes = vec![0_u8; IDE_IDENTIFY_BYTES];
        put_word(&mut bytes, 1, geometry.cylinders);
        put_word(&mut bytes, 3, u16::from(geometry.heads));
        put_word(&mut bytes, 6, u16::from(geometry.sectors));
        bytes[54..54 + IDE_MODEL.len()].copy_from_slice(IDE_MODEL);
        put_word(&mut bytes, 47, IDE_MAX_MULTI_SECTORS);
        bytes[99] = 0x07;
        put_word(&mut bytes, 53, 0x0006);
        bytes[118] = IDE_MAX_MULTI_SECTORS as u8;
        bytes[119] = 0x01;
        put_dword(&mut bytes, 60, capacity);
        bytes[126] = 0x04;
        bytes[128] = 0x03;
        put_word(&mut bytes, 80, IDE_ATA7_MAJOR);
        bytes[176] = 0x1f;
        put_word(&mut bytes, 93, 0x4001);
        Ok(bytes)
    }

    fn capacity_sectors_u32(&self) -> Result<u32, IdeDiskError> {
        u32::try_from(self.image.capacity_sectors()).map_err(|_| IdeDiskError::InvalidCapacity {
            sectors: self.image.capacity_sectors(),
        })
    }

    const fn interrupts_disabled(&self) -> bool {
        self.control & IDE_CONTROL_IEN != 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdeTaskFile {
    error: u8,
    sector_count: u8,
    sector_number: u8,
    cylinder_low: u8,
    cylinder_high: u8,
    drive: u8,
    command: u8,
}

impl IdeTaskFile {
    const fn reset() -> Self {
        Self {
            error: 0x01,
            sector_count: 0,
            sector_number: 0,
            cylinder_low: 0,
            cylinder_high: 0,
            drive: 0,
            command: 0,
        }
    }

    fn lba_base(self) -> u64 {
        (u64::from(self.drive & 0x0f) << 24)
            | (u64::from(self.cylinder_high) << 16)
            | (u64::from(self.cylinder_low) << 8)
            | u64::from(self.sector_number)
    }

    pub const fn error(self) -> u8 {
        self.error
    }

    pub const fn sector_count(self) -> u8 {
        self.sector_count
    }

    pub const fn sector_number(self) -> u8 {
        self.sector_number
    }

    pub const fn cylinder_low(self) -> u8 {
        self.cylinder_low
    }

    pub const fn cylinder_high(self) -> u8 {
        self.cylinder_high
    }

    pub const fn drive(self) -> u8 {
        self.drive
    }

    pub const fn command(self) -> u8 {
        self.command
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IdeTransfer {
    direction: IdeTransferDirection,
    start_sector: u64,
    payload: Vec<u8>,
    cursor: usize,
}

impl IdeTransfer {
    fn new_input(payload: Vec<u8>) -> Self {
        Self {
            direction: IdeTransferDirection::Input,
            start_sector: 0,
            payload,
            cursor: 0,
        }
    }

    fn new_output(start_sector: u64, sectors: u64) -> Result<Self, IdeDiskError> {
        let bytes = sectors
            .checked_mul(STORAGE_SECTOR_BYTES)
            .ok_or(IdeDiskError::TransferTooLarge { sectors })?;
        let capacity =
            usize::try_from(bytes).map_err(|_| IdeDiskError::TransferTooLarge { sectors })?;
        Ok(Self {
            direction: IdeTransferDirection::Output,
            start_sector,
            payload: vec![0; capacity],
            cursor: 0,
        })
    }

    fn new_dma(
        direction: IdeDmaDirection,
        start_sector: u64,
        sectors: u64,
    ) -> Result<Self, IdeDiskError> {
        let cursor =
            usize::try_from(sectors).map_err(|_| IdeDiskError::TransferTooLarge { sectors })?;
        Ok(Self {
            direction: IdeTransferDirection::Dma(direction),
            start_sector,
            payload: Vec::new(),
            cursor,
        })
    }

    fn read_word(&mut self) -> Result<u16, IdeDiskError> {
        if self.cursor + 2 > self.payload.len() {
            return Err(IdeDiskError::DataReadNotReady);
        }
        let word = u16::from_le_bytes(
            self.payload[self.cursor..self.cursor + 2]
                .try_into()
                .unwrap(),
        );
        self.cursor += 2;
        Ok(word)
    }

    fn write_word(&mut self, value: u16) -> Result<(), IdeDiskError> {
        if self.cursor + 2 > self.payload.len() {
            return Err(IdeDiskError::DataWriteNotReady);
        }
        self.payload[self.cursor..self.cursor + 2].copy_from_slice(&value.to_le_bytes());
        self.cursor += 2;
        Ok(())
    }

    fn is_complete(&self) -> bool {
        self.cursor == self.payload.len()
    }

    fn sector_boundary(&self) -> bool {
        self.cursor > 0
            && self.cursor.is_multiple_of(STORAGE_SECTOR_BYTES as usize)
            && !self.is_complete()
    }

    fn snapshot(&self) -> IdeDiskTransferSnapshot {
        match self.direction {
            IdeTransferDirection::Input => IdeDiskTransferSnapshot::Input {
                start_sector: self.start_sector,
                cursor: self.cursor,
                payload: self.payload.clone(),
            },
            IdeTransferDirection::Output => IdeDiskTransferSnapshot::Output {
                start_sector: self.start_sector,
                cursor: self.cursor,
                payload: self.payload.clone(),
            },
            IdeTransferDirection::Dma(direction) => IdeDiskTransferSnapshot::Dma {
                direction,
                start_sector: self.start_sector,
                sectors: self.cursor as u64,
            },
        }
    }

    fn from_snapshot(snapshot: &IdeDiskTransferSnapshot) -> Result<Self, IdeDiskError> {
        snapshot.validate()?;
        let (direction, start_sector, cursor, payload) = match snapshot {
            IdeDiskTransferSnapshot::Input {
                start_sector,
                cursor,
                payload,
            } => (
                IdeTransferDirection::Input,
                *start_sector,
                *cursor,
                payload.clone(),
            ),
            IdeDiskTransferSnapshot::Output {
                start_sector,
                cursor,
                payload,
            } => (
                IdeTransferDirection::Output,
                *start_sector,
                *cursor,
                payload.clone(),
            ),
            IdeDiskTransferSnapshot::Dma {
                direction,
                start_sector,
                sectors,
            } => (
                IdeTransferDirection::Dma(*direction),
                *start_sector,
                usize::try_from(*sectors)
                    .map_err(|_| IdeDiskError::TransferTooLarge { sectors: *sectors })?,
                Vec::new(),
            ),
        };
        Ok(Self {
            direction,
            start_sector,
            payload,
            cursor,
        })
    }

    fn dma_request(&self) -> Result<IdeDmaRequest, IdeDiskError> {
        match self.direction {
            IdeTransferDirection::Dma(direction) => Ok(IdeDmaRequest::new(
                direction,
                self.start_sector,
                self.cursor as u64,
            )),
            _ => Err(IdeDiskError::DmaNotReady { command: 0 }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdeTransferDirection {
    Input,
    Output,
    Dma(IdeDmaDirection),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IdeGeometry {
    cylinders: u16,
    heads: u8,
    sectors: u8,
}

impl IdeGeometry {
    fn from_capacity(capacity: u32) -> Result<Self, IdeDiskError> {
        if capacity == 0 {
            return Err(IdeDiskError::InvalidCapacity {
                sectors: u64::from(capacity),
            });
        }
        if capacity >= 16_383 * 16 * 63 {
            return Ok(Self {
                cylinders: 16_383,
                heads: 16,
                sectors: 63,
            });
        }

        let sectors = if capacity >= 63 { 63 } else { capacity };
        let heads = if capacity / sectors >= 16 {
            16
        } else {
            capacity / sectors
        };
        let cylinders = capacity / (heads * sectors);
        Ok(Self {
            cylinders: cylinders as u16,
            heads: heads as u8,
            sectors: sectors as u8,
        })
    }
}

fn put_word(bytes: &mut [u8], word: usize, value: u16) {
    bytes[word * 2..word * 2 + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_dword(bytes: &mut [u8], word: usize, value: u32) {
    bytes[word * 2..word * 2 + 4].copy_from_slice(&value.to_le_bytes());
}
