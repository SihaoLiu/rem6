use crate::{IdeChannelId, IdeDeviceId, IdeDmaDirection, IdeTaskFile};

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
}

impl IdeDiskSnapshot {
    pub const fn from_parts(
        device_id: IdeDeviceId,
        task_file: IdeTaskFile,
        status: u8,
        control: u8,
        pending_interrupt: bool,
        transfer: Option<IdeDiskTransferSnapshot>,
    ) -> Self {
        Self {
            device_id,
            task_file,
            status,
            control,
            pending_interrupt,
            transfer,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum IdeSnapshotError {
    InvalidTransferSnapshot { cursor: usize, payload_bytes: usize },
}
