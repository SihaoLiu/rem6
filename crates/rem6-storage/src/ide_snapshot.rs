use crate::ide::IDE_MAX_TRANSFER_SECTORS;
use crate::{
    IdeChannelId, IdeControllerError, IdeDeviceId, IdeDmaDirection, IdeTaskFile,
    IDE_BMI_COMMAND_RW, IDE_BMI_COMMAND_START, IDE_BMI_STATUS_ACTIVE, IDE_BMI_STATUS_DMA_CAP0,
    IDE_BMI_STATUS_DMA_CAP1, IDE_BMI_STATUS_DMA_ERROR, IDE_BMI_STATUS_INTERRUPT, IDE_COMMAND_READ,
    IDE_COMMAND_READ_DMA, IDE_COMMAND_READ_MULTI, IDE_COMMAND_WRITE, IDE_COMMAND_WRITE_DMA,
    IDE_COMMAND_WRITE_MULTI,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdeBmiSnapshot {
    pub(crate) command: u8,
    pub(crate) status: u8,
    pub(crate) prd_table: u32,
}

impl IdeBmiSnapshot {
    pub const fn command(self) -> u8 {
        self.command
    }

    pub const fn status(self) -> u8 {
        self.status
    }

    pub const fn prd_table(self) -> u32 {
        self.prd_table
    }

    pub(crate) fn validate(self, channel: IdeChannelId) -> Result<(), IdeControllerError> {
        let allowed_command = IDE_BMI_COMMAND_START | IDE_BMI_COMMAND_RW;
        if self.command & !allowed_command != 0 {
            return Err(IdeControllerError::InvalidBmiSnapshot {
                channel,
                field: "command",
                value: u32::from(self.command),
            });
        }

        let required_status = IDE_BMI_STATUS_DMA_CAP0 | IDE_BMI_STATUS_DMA_CAP1;
        let allowed_status = IDE_BMI_STATUS_ACTIVE
            | IDE_BMI_STATUS_DMA_ERROR
            | IDE_BMI_STATUS_INTERRUPT
            | IDE_BMI_STATUS_DMA_CAP0
            | IDE_BMI_STATUS_DMA_CAP1;
        if self.status & !allowed_status != 0 || self.status & required_status != required_status {
            return Err(IdeControllerError::InvalidBmiSnapshot {
                channel,
                field: "status",
                value: u32::from(self.status),
            });
        }

        if self.prd_table & 0x3 != 0 {
            return Err(IdeControllerError::InvalidBmiSnapshot {
                channel,
                field: "prd_table",
                value: self.prd_table,
            });
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdeChannelSnapshot {
    pub(crate) id: IdeChannelId,
    pub(crate) selected_device: IdeDeviceId,
    pub(crate) pending_interrupt: bool,
    pub(crate) bmi: IdeBmiSnapshot,
    pub(crate) device0: Option<IdeDiskSnapshot>,
    pub(crate) device1: Option<IdeDiskSnapshot>,
}

impl IdeChannelSnapshot {
    pub const fn id(&self) -> IdeChannelId {
        self.id
    }

    pub const fn selected_device(&self) -> IdeDeviceId {
        self.selected_device
    }

    pub const fn pending_interrupt(&self) -> bool {
        self.pending_interrupt
    }

    pub const fn bmi(&self) -> IdeBmiSnapshot {
        self.bmi
    }

    pub const fn device0(&self) -> Option<&IdeDiskSnapshot> {
        self.device0.as_ref()
    }

    pub const fn device1(&self) -> Option<&IdeDiskSnapshot> {
        self.device1.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdeControllerSnapshot {
    pub(crate) channels: [IdeChannelSnapshot; 2],
}

impl IdeControllerSnapshot {
    pub const fn from_channels(channels: [IdeChannelSnapshot; 2]) -> Self {
        Self { channels }
    }

    pub fn channel(&self, channel: IdeChannelId) -> &IdeChannelSnapshot {
        &self.channels[channel.index()]
    }

    pub const fn channels(&self) -> &[IdeChannelSnapshot; 2] {
        &self.channels
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdeDiskTransferSnapshot {
    Input {
        start_sector: u64,
        cursor: usize,
        payload: Vec<u8>,
    },
    Output {
        start_sector: u64,
        cursor: usize,
        payload: Vec<u8>,
    },
    Dma {
        direction: IdeDmaDirection,
        start_sector: u64,
        sectors: u64,
    },
}

impl IdeDiskTransferSnapshot {
    pub(crate) fn validate(&self) -> Result<(), IdeSnapshotError> {
        let (cursor, payload) = match self {
            Self::Input {
                cursor, payload, ..
            }
            | Self::Output {
                cursor, payload, ..
            } => (*cursor, payload),
            Self::Dma { .. } => return Ok(()),
        };
        if cursor > payload.len() || cursor % 2 != 0 || payload.len() % 2 != 0 {
            return Err(IdeSnapshotError::InvalidTransferSnapshot {
                cursor,
                payload_bytes: payload.len(),
            });
        }
        Ok(())
    }

    pub const fn cursor(&self) -> usize {
        match self {
            Self::Input { cursor, .. } | Self::Output { cursor, .. } => *cursor,
            Self::Dma { .. } => 0,
        }
    }

    pub fn payload(&self) -> &[u8] {
        match self {
            Self::Input { payload, .. } | Self::Output { payload, .. } => payload,
            Self::Dma { .. } => &[],
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IdeDiskSnapshot {
    pub(crate) device_id: IdeDeviceId,
    pub(crate) task_file: IdeTaskFile,
    pub(crate) status: u8,
    pub(crate) control: u8,
    pub(crate) pending_interrupt: bool,
    pub(crate) transfer: Option<IdeDiskTransferSnapshot>,
    pub(crate) pending_command: Option<IdePendingCommandSnapshot>,
}

impl IdeDiskSnapshot {
    pub const fn from_parts(
        device_id: IdeDeviceId,
        task_file: IdeTaskFile,
        status: u8,
        control: u8,
        pending_interrupt: bool,
        transfer: Option<IdeDiskTransferSnapshot>,
        pending_command: Option<IdePendingCommandSnapshot>,
    ) -> Self {
        Self {
            device_id,
            task_file,
            status,
            control,
            pending_interrupt,
            transfer,
            pending_command,
        }
    }

    pub const fn device_id(&self) -> IdeDeviceId {
        self.device_id
    }

    pub const fn task_file(&self) -> &IdeTaskFile {
        &self.task_file
    }

    pub const fn status(&self) -> u8 {
        self.status
    }

    pub const fn control(&self) -> u8 {
        self.control
    }

    pub const fn pending_interrupt(&self) -> bool {
        self.pending_interrupt
    }

    pub const fn transfer(&self) -> Option<&IdeDiskTransferSnapshot> {
        self.transfer.as_ref()
    }

    pub const fn pending_command(&self) -> Option<&IdePendingCommandSnapshot> {
        self.pending_command.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IdePendingCommandSnapshot {
    pub(crate) command: u8,
    pub(crate) start_sector: u64,
    pub(crate) sectors: u64,
}

impl IdePendingCommandSnapshot {
    pub const fn new(command: u8, start_sector: u64, sectors: u64) -> Self {
        Self {
            command,
            start_sector,
            sectors,
        }
    }

    pub const fn command(self) -> u8 {
        self.command
    }

    pub const fn start_sector(self) -> u64 {
        self.start_sector
    }

    pub const fn sectors(self) -> u64 {
        self.sectors
    }

    pub(crate) fn validate(self) -> Result<(), IdeSnapshotError> {
        if !matches!(
            self.command,
            IDE_COMMAND_READ
                | IDE_COMMAND_READ_MULTI
                | IDE_COMMAND_WRITE
                | IDE_COMMAND_WRITE_MULTI
                | IDE_COMMAND_READ_DMA
                | IDE_COMMAND_WRITE_DMA
        ) || self.sectors == 0
            || self.sectors > IDE_MAX_TRANSFER_SECTORS
        {
            return Err(IdeSnapshotError::InvalidPendingCommand {
                command: self.command,
                sectors: self.sectors,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IdeSnapshotError {
    InvalidTransferSnapshot { cursor: usize, payload_bytes: usize },
    InvalidPendingCommand { command: u8, sectors: u64 },
}
