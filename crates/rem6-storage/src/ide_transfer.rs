use crate::{
    IdeDiskError, IdeDiskTransferSnapshot, IdeDmaDirection, IdeDmaRequest,
    IdePendingCommandSnapshot, STORAGE_SECTOR_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdePendingCommand {
    pub(crate) command: u8,
    pub(crate) start_sector: u64,
    pub(crate) sectors: u64,
}

impl IdePendingCommand {
    pub(crate) const fn new(command: u8, start_sector: u64, sectors: u64) -> Self {
        Self {
            command,
            start_sector,
            sectors,
        }
    }

    pub(crate) const fn snapshot(self) -> IdePendingCommandSnapshot {
        IdePendingCommandSnapshot::new(self.command, self.start_sector, self.sectors)
    }

    pub(crate) const fn from_snapshot(snapshot: IdePendingCommandSnapshot) -> Self {
        Self {
            command: snapshot.command(),
            start_sector: snapshot.start_sector(),
            sectors: snapshot.sectors(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdeTransfer {
    pub(crate) direction: IdeTransferDirection,
    pub(crate) start_sector: u64,
    pub(crate) payload: Vec<u8>,
    pub(crate) cursor: usize,
}

impl IdeTransfer {
    pub(crate) fn new_input(payload: Vec<u8>) -> Self {
        Self {
            direction: IdeTransferDirection::Input,
            start_sector: 0,
            payload,
            cursor: 0,
        }
    }

    pub(crate) fn new_output(start_sector: u64, sectors: u64) -> Result<Self, IdeDiskError> {
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

    pub(crate) fn new_dma(
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

    pub(crate) fn read_word(&mut self) -> Result<u16, IdeDiskError> {
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

    pub(crate) fn write_word(&mut self, value: u16) -> Result<(), IdeDiskError> {
        if self.cursor + 2 > self.payload.len() {
            return Err(IdeDiskError::DataWriteNotReady);
        }
        self.payload[self.cursor..self.cursor + 2].copy_from_slice(&value.to_le_bytes());
        self.cursor += 2;
        Ok(())
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.cursor == self.payload.len()
    }

    pub(crate) fn sector_boundary(&self) -> bool {
        self.cursor > 0
            && self.cursor.is_multiple_of(STORAGE_SECTOR_BYTES as usize)
            && !self.is_complete()
    }

    pub(crate) fn snapshot(&self) -> IdeDiskTransferSnapshot {
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

    pub(crate) fn from_snapshot(snapshot: &IdeDiskTransferSnapshot) -> Result<Self, IdeDiskError> {
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

    pub(crate) fn dma_request(&self) -> Result<IdeDmaRequest, IdeDiskError> {
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
pub(crate) enum IdeTransferDirection {
    Input,
    Output,
    Dma(IdeDmaDirection),
}
