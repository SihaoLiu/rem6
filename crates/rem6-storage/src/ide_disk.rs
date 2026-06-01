use std::sync::Arc;

use crate::ide::{
    IdeDeviceId, IDE_ALTSTAT_OFFSET, IDE_COMMAND_ATAPI_IDENTIFY_DEVICE, IDE_COMMAND_IDENTIFY,
    IDE_COMMAND_OFFSET, IDE_COMMAND_READ, IDE_COMMAND_READ_DMA, IDE_COMMAND_READ_MULTI,
    IDE_COMMAND_READ_NATIVE_MAX, IDE_COMMAND_WRITE, IDE_COMMAND_WRITE_DMA, IDE_COMMAND_WRITE_MULTI,
    IDE_CONTROL_IEN, IDE_CONTROL_OFFSET, IDE_CONTROL_RST, IDE_DRIVE_LBA, IDE_DRIVE_OFFSET,
    IDE_ERROR_ABORT, IDE_ERROR_OFFSET, IDE_FEATURES_OFFSET, IDE_HCYL_OFFSET, IDE_LCYL_OFFSET,
    IDE_NSECTOR_OFFSET, IDE_SECTOR_OFFSET, IDE_STATUS_BSY, IDE_STATUS_DF, IDE_STATUS_DRDY,
    IDE_STATUS_DRQ, IDE_STATUS_ERR, IDE_STATUS_OFFSET, IDE_STATUS_SEEK,
};
use crate::{
    ide_identify_payload, IdeDiskError, IdeDiskSnapshot, IdeDmaDirection, IdeDmaRequest,
    IdePendingCommand, IdeTaskFile, IdeTransfer, IdeTransferDirection, StorageError,
    StorageImageLayer, StorageSectorId, STORAGE_SECTOR_BYTES,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdeCommandIssue {
    Completed,
    Delayed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdeDataReadIssue {
    word: u16,
    sector_delay: bool,
}

impl IdeDataReadIssue {
    const fn ready(word: u16) -> Self {
        Self {
            word,
            sector_delay: false,
        }
    }

    const fn delayed(word: u16) -> Self {
        Self {
            word,
            sector_delay: true,
        }
    }

    pub(crate) const fn word(self) -> u16 {
        self.word
    }

    pub(crate) const fn sector_delay(self) -> bool {
        self.sector_delay
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdeDataWriteIssue {
    sector_delay: bool,
}

impl IdeDataWriteIssue {
    const fn ready() -> Self {
        Self {
            sector_delay: false,
        }
    }

    const fn delayed() -> Self {
        Self { sector_delay: true }
    }

    pub(crate) const fn sector_delay(self) -> bool {
        self.sector_delay
    }
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
    pending_command: Option<IdePendingCommand>,
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
            pending_command: None,
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
            pending_command: self.pending_command.map(IdePendingCommand::snapshot),
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
        self.pending_command = snapshot
            .pending_command
            .map(IdePendingCommand::from_snapshot);
        Ok(())
    }

    pub(crate) fn validate_snapshot(&self, snapshot: &IdeDiskSnapshot) -> Result<(), IdeDiskError> {
        if snapshot.device_id != self.device_id {
            return Err(IdeDiskError::SnapshotDeviceMismatch {
                expected: self.device_id,
                actual: snapshot.device_id,
            });
        }
        if let Some(transfer) = &snapshot.transfer {
            transfer.validate()?;
        }
        if let Some(pending) = snapshot.pending_command {
            pending.validate()?;
            self.validate_snapshot_sector_range(pending.start_sector(), pending.sectors())?;
        }
        Ok(())
    }

    fn validate_snapshot_sector_range(
        &self,
        start_sector: u64,
        sectors: u64,
    ) -> Result<(), IdeDiskError> {
        let end =
            start_sector
                .checked_add(sectors)
                .ok_or(StorageError::RequestAddressOverflow {
                    sector: StorageSectorId::new(start_sector),
                    sectors,
                })?;
        let capacity = self.image.capacity_sectors();
        if end > capacity {
            return Err(StorageError::OutOfRange {
                sector: StorageSectorId::new(start_sector),
                sectors,
                capacity_sectors: capacity,
            }
            .into());
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

    pub fn write_command_u8_timed(
        &mut self,
        offset: u8,
        value: u8,
    ) -> Result<IdeCommandIssue, IdeDiskError> {
        match offset {
            IDE_COMMAND_OFFSET => self.start_timed_command(value),
            _ => {
                self.write_command_u8(offset, value)?;
                Ok(IdeCommandIssue::Completed)
            }
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
            self.pending_command = None;
        } else if was_reset {
            self.registers = IdeTaskFile::reset();
            self.status = IDE_STATUS_DRDY;
            self.pending_interrupt = false;
            self.transfer = None;
            self.pending_command = None;
        }
        Ok(())
    }

    pub fn read_data_u16(&mut self) -> Result<u16, IdeDiskError> {
        if self.status & IDE_STATUS_DRQ == 0 {
            return Err(IdeDiskError::DataReadNotReady);
        }
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

    pub(crate) fn read_data_u16_timed(&mut self) -> Result<IdeDataReadIssue, IdeDiskError> {
        if self.status & IDE_STATUS_DRQ == 0 {
            return Err(IdeDiskError::DataReadNotReady);
        }
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
            return Ok(IdeDataReadIssue::ready(word));
        }
        if transfer.sector_boundary() {
            self.status = IDE_STATUS_DRDY | IDE_STATUS_BSY;
            self.pending_interrupt = false;
            return Ok(IdeDataReadIssue::delayed(word));
        }
        Ok(IdeDataReadIssue::ready(word))
    }

    pub fn write_data_u16(&mut self, value: u16) -> Result<(), IdeDiskError> {
        if self.status & IDE_STATUS_DRQ == 0 {
            return Err(IdeDiskError::DataWriteNotReady);
        }
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

    pub(crate) fn write_data_u16_timed(
        &mut self,
        value: u16,
    ) -> Result<IdeDataWriteIssue, IdeDiskError> {
        if self.status & IDE_STATUS_DRQ == 0 {
            return Err(IdeDiskError::DataWriteNotReady);
        }
        let write = {
            let transfer = self
                .transfer
                .as_mut()
                .ok_or(IdeDiskError::DataWriteNotReady)?;
            if transfer.direction != IdeTransferDirection::Output {
                return Err(IdeDiskError::DataWriteNotReady);
            }
            transfer.write_word(value)?;
            let needs_sector_commit = transfer.is_complete() || transfer.sector_boundary();
            let sector = if needs_sector_commit {
                transfer.completed_sector_data()
            } else {
                None
            };
            (sector, transfer.is_complete(), transfer.sector_boundary())
        };

        if let Some((sector, data)) = write.0 {
            self.image
                .write_sector(StorageSectorId::new(sector), data)?;
        }
        if write.1 {
            self.complete_data_transfer();
            return Ok(IdeDataWriteIssue::ready());
        }
        if write.2 {
            self.status = IDE_STATUS_DRDY | IDE_STATUS_BSY;
            self.pending_interrupt = false;
            return Ok(IdeDataWriteIssue::delayed());
        }
        Ok(IdeDataWriteIssue::ready())
    }

    pub(crate) fn complete_timed_data_read(&mut self) -> Result<(), IdeDiskError> {
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(IdeDiskError::DataReadNotReady)?;
        if transfer.direction != IdeTransferDirection::Input {
            return Err(IdeDiskError::DataReadNotReady);
        }
        self.status = IDE_STATUS_DRDY | IDE_STATUS_DRQ;
        self.pending_interrupt = !self.interrupts_disabled();
        Ok(())
    }

    pub(crate) fn complete_timed_data_write(&mut self) -> Result<(), IdeDiskError> {
        let transfer = self
            .transfer
            .as_ref()
            .ok_or(IdeDiskError::DataWriteNotReady)?;
        if transfer.direction != IdeTransferDirection::Output {
            return Err(IdeDiskError::DataWriteNotReady);
        }
        self.status = IDE_STATUS_DRDY | IDE_STATUS_DRQ;
        self.pending_interrupt = !self.interrupts_disabled();
        Ok(())
    }

    fn start_command(&mut self, command: u8) -> Result<(), IdeDiskError> {
        self.registers.command = command;
        self.registers.error &= !IDE_ERROR_ABORT;
        self.status &= !IDE_STATUS_ERR;
        self.pending_interrupt = false;
        self.transfer = None;
        self.pending_command = None;

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
                let mut payload =
                    Vec::with_capacity(sectors as usize * STORAGE_SECTOR_BYTES as usize);
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

    fn start_timed_command(&mut self, command: u8) -> Result<IdeCommandIssue, IdeDiskError> {
        if !matches!(
            command,
            IDE_COMMAND_READ
                | IDE_COMMAND_READ_MULTI
                | IDE_COMMAND_WRITE
                | IDE_COMMAND_WRITE_MULTI
                | IDE_COMMAND_READ_DMA
                | IDE_COMMAND_WRITE_DMA
        ) {
            self.start_command(command)?;
            return Ok(IdeCommandIssue::Completed);
        }

        self.registers.command = command;
        self.registers.error &= !IDE_ERROR_ABORT;
        self.status &= !IDE_STATUS_ERR;
        self.pending_interrupt = false;
        self.transfer = None;
        self.pending_command = None;
        let (start_sector, sectors) = self.lba_transfer(command)?;
        self.start_busy();
        self.pending_command = Some(IdePendingCommand::new(command, start_sector, sectors));
        Ok(IdeCommandIssue::Delayed)
    }

    pub(crate) fn complete_timed_command(&mut self) -> Result<(), IdeDiskError> {
        let pending = self
            .pending_command
            .take()
            .ok_or(IdeDiskError::NoPendingTimedCommand {
                device: self.device_id,
            })?;
        match pending.command {
            IDE_COMMAND_READ | IDE_COMMAND_READ_MULTI => {
                let mut payload =
                    Vec::with_capacity(pending.sectors as usize * STORAGE_SECTOR_BYTES as usize);
                for offset in 0..pending.sectors {
                    payload.extend_from_slice(
                        &self
                            .image
                            .read_sector(StorageSectorId::new(pending.start_sector + offset))?,
                    );
                }
                self.prepare_input(payload);
            }
            IDE_COMMAND_WRITE | IDE_COMMAND_WRITE_MULTI => {
                self.transfer = Some(IdeTransfer::new_output(
                    pending.start_sector,
                    pending.sectors,
                )?);
                self.status = IDE_STATUS_DRDY | IDE_STATUS_DRQ;
            }
            IDE_COMMAND_READ_DMA | IDE_COMMAND_WRITE_DMA => {
                let direction = if pending.command == IDE_COMMAND_READ_DMA {
                    IdeDmaDirection::ToGuest
                } else {
                    IdeDmaDirection::FromGuest
                };
                self.transfer = Some(IdeTransfer::new_dma(
                    direction,
                    pending.start_sector,
                    pending.sectors,
                )?);
                self.status = IDE_STATUS_DRDY | IDE_STATUS_DRQ;
            }
            command => return Err(IdeDiskError::InvalidPendingTimedCommand { command }),
        }
        Ok(())
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

    pub(crate) fn dma_request(&self) -> Result<IdeDmaRequest, IdeDiskError> {
        match self.transfer.as_ref() {
            Some(transfer) => transfer.dma_request(),
            None => Err(IdeDiskError::DmaNotReady {
                command: self.registers.command,
            }),
        }
    }

    pub(crate) fn read_dma_payload(&self, request: IdeDmaRequest) -> Result<Vec<u8>, IdeDiskError> {
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

    pub(crate) fn commit_dma_payload(
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

    pub(crate) fn complete_dma(&mut self) -> Result<(), IdeDiskError> {
        self.dma_request()?;
        self.transfer = None;
        self.status = IDE_STATUS_DRDY | IDE_STATUS_SEEK;
        self.pending_interrupt = !self.interrupts_disabled();
        Ok(())
    }

    fn commit_write_transfer(&self, transfer: IdeTransfer) -> Result<(), IdeDiskError> {
        for (offset, chunk) in transfer
            .payload
            .chunks_exact(STORAGE_SECTOR_BYTES as usize)
            .enumerate()
        {
            let mut sector = [0_u8; STORAGE_SECTOR_BYTES as usize];
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
        ide_identify_payload(capacity)
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
