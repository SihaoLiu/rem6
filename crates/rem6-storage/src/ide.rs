use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use crate::{StorageError, StorageImageLayer, StorageSectorId, STORAGE_SECTOR_BYTES};

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
pub const IDE_COMMAND_IDENTIFY: u8 = 0xec;
pub const IDE_COMMAND_ATAPI_IDENTIFY_DEVICE: u8 = 0xa1;
pub const IDE_COMMAND_READ_NATIVE_MAX: u8 = 0xf8;

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
struct IdeTaskFile {
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
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum IdeTransferDirection {
    Input,
    Output,
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdeDiskError {
    InvalidCapacity { sectors: u64 },
    InvalidCommandOffset { offset: u8 },
    InvalidControlOffset { offset: u8 },
    ChsAccessUnsupported { command: u8, drive: u8 },
    UnsupportedCommand { command: u8 },
    DataReadNotReady,
    DataWriteNotReady,
    TransferTooLarge { sectors: u64 },
    Storage(StorageError),
}

impl From<StorageError> for IdeDiskError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl Display for IdeDiskError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidCapacity { sectors } => {
                write!(
                    formatter,
                    "IDE disk capacity {sectors} sectors is not supported"
                )
            }
            Self::InvalidCommandOffset { offset } => {
                write!(formatter, "invalid IDE command register offset {offset:#x}")
            }
            Self::InvalidControlOffset { offset } => {
                write!(formatter, "invalid IDE control register offset {offset:#x}")
            }
            Self::ChsAccessUnsupported { command, drive } => write!(
                formatter,
                "IDE command {command:#x} requested CHS access with drive register {drive:#x}"
            ),
            Self::UnsupportedCommand { command } => {
                write!(formatter, "unsupported IDE command {command:#x}")
            }
            Self::DataReadNotReady => {
                write!(formatter, "IDE data register is not ready for read")
            }
            Self::DataWriteNotReady => {
                write!(formatter, "IDE data register is not ready for write")
            }
            Self::TransferTooLarge { sectors } => {
                write!(formatter, "IDE transfer of {sectors} sectors is too large")
            }
            Self::Storage(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for IdeDiskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}

fn put_word(bytes: &mut [u8], word: usize, value: u16) {
    bytes[word * 2..word * 2 + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_dword(bytes: &mut [u8], word: usize, value: u32) {
    bytes[word * 2..word * 2 + 4].copy_from_slice(&value.to_le_bytes());
}
