use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::sync::Arc;

use crate::{StorageError, StorageImageLayer, StorageSectorId, STORAGE_SECTOR_BYTES};

pub trait SimpleDiskGuestMemory {
    fn read_bytes(&mut self, address: u64, bytes: u64) -> Result<Vec<u8>, SimpleDiskError>;
    fn write_bytes(&mut self, address: u64, data: &[u8]) -> Result<(), SimpleDiskError>;
}

#[derive(Clone, Debug)]
pub struct SimpleDisk {
    image: Arc<dyn StorageImageLayer>,
}

impl SimpleDisk {
    pub fn new(image: Arc<dyn StorageImageLayer>) -> Self {
        Self { image }
    }

    pub fn capacity_sectors(&self) -> u64 {
        self.image.capacity_sectors()
    }

    pub fn read_to_guest(
        &self,
        guest: &mut impl SimpleDiskGuestMemory,
        guest_address: u64,
        start_sector: StorageSectorId,
        byte_count: u64,
    ) -> Result<SimpleDiskTransfer, SimpleDiskError> {
        let transfer = self.validate_transfer(guest_address, start_sector, byte_count)?;
        let mut payload = Vec::with_capacity(transfer.bytes_usize()?);
        for sector_offset in 0..transfer.sectors() {
            let sector = transfer.sector_at(sector_offset)?;
            payload.extend_from_slice(&self.image.read_sector(sector)?);
        }
        guest.write_bytes(guest_address, &payload)?;
        Ok(transfer)
    }

    pub fn write_from_guest(
        &self,
        guest: &mut impl SimpleDiskGuestMemory,
        guest_address: u64,
        start_sector: StorageSectorId,
        byte_count: u64,
    ) -> Result<SimpleDiskTransfer, SimpleDiskError> {
        let transfer = self.validate_transfer(guest_address, start_sector, byte_count)?;
        let payload = guest.read_bytes(guest_address, byte_count)?;
        let expected = transfer.bytes_usize()?;
        if payload.len() != expected {
            return Err(SimpleDiskError::GuestTransferSizeMismatch {
                expected_bytes: byte_count,
                actual_bytes: payload.len() as u64,
            });
        }

        for (sector_offset, chunk) in payload
            .chunks_exact(STORAGE_SECTOR_BYTES as usize)
            .enumerate()
        {
            let sector = transfer.sector_at(sector_offset as u64)?;
            let mut data = [0_u8; STORAGE_SECTOR_BYTES as usize];
            data.copy_from_slice(chunk);
            self.image.write_sector(sector, data)?;
        }
        Ok(transfer)
    }

    pub fn flush(&self) -> Result<(), SimpleDiskError> {
        self.image.flush().map_err(SimpleDiskError::Storage)
    }

    fn validate_transfer(
        &self,
        guest_address: u64,
        start_sector: StorageSectorId,
        byte_count: u64,
    ) -> Result<SimpleDiskTransfer, SimpleDiskError> {
        if byte_count == 0 || !byte_count.is_multiple_of(STORAGE_SECTOR_BYTES) {
            return Err(SimpleDiskError::InvalidTransferByteCount { bytes: byte_count });
        }
        guest_address
            .checked_add(byte_count)
            .ok_or(SimpleDiskError::GuestAddressOverflow {
                address: guest_address,
                bytes: byte_count,
            })?;
        let sectors = byte_count / STORAGE_SECTOR_BYTES;
        let end_sector = start_sector.get().checked_add(sectors).ok_or(
            StorageError::RequestAddressOverflow {
                sector: start_sector,
                sectors,
            },
        )?;
        let capacity_sectors = self.image.capacity_sectors();
        if end_sector > capacity_sectors {
            return Err(StorageError::OutOfRange {
                sector: start_sector,
                sectors,
                capacity_sectors,
            }
            .into());
        }

        let transfer = SimpleDiskTransfer::new(guest_address, start_sector, sectors, byte_count);
        transfer.bytes_usize()?;
        Ok(transfer)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SimpleDiskTransfer {
    guest_address: u64,
    start_sector: StorageSectorId,
    sectors: u64,
    bytes: u64,
}

impl SimpleDiskTransfer {
    pub const fn new(
        guest_address: u64,
        start_sector: StorageSectorId,
        sectors: u64,
        bytes: u64,
    ) -> Self {
        Self {
            guest_address,
            start_sector,
            sectors,
            bytes,
        }
    }

    pub const fn guest_address(self) -> u64 {
        self.guest_address
    }

    pub const fn start_sector(self) -> StorageSectorId {
        self.start_sector
    }

    pub const fn sectors(self) -> u64 {
        self.sectors
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    fn bytes_usize(self) -> Result<usize, SimpleDiskError> {
        usize::try_from(self.bytes)
            .map_err(|_| SimpleDiskError::TransferTooLarge { bytes: self.bytes })
    }

    fn sector_at(self, offset: u64) -> Result<StorageSectorId, SimpleDiskError> {
        self.start_sector
            .get()
            .checked_add(offset)
            .map(StorageSectorId::new)
            .ok_or(SimpleDiskError::Storage(
                StorageError::RequestAddressOverflow {
                    sector: self.start_sector,
                    sectors: self.sectors,
                },
            ))
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SimpleDiskError {
    InvalidTransferByteCount {
        bytes: u64,
    },
    TransferTooLarge {
        bytes: u64,
    },
    GuestAddressOverflow {
        address: u64,
        bytes: u64,
    },
    GuestMemory {
        operation: &'static str,
        address: u64,
        bytes: u64,
        capacity_bytes: u64,
    },
    GuestTransferSizeMismatch {
        expected_bytes: u64,
        actual_bytes: u64,
    },
    Storage(StorageError),
}

impl From<StorageError> for SimpleDiskError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl Display for SimpleDiskError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidTransferByteCount { bytes } => write!(
                formatter,
                "simple disk transfer size {bytes} is not a nonzero sector multiple"
            ),
            Self::TransferTooLarge { bytes } => {
                write!(formatter, "simple disk transfer size {bytes} is too large")
            }
            Self::GuestAddressOverflow { address, bytes } => write!(
                formatter,
                "simple disk guest address {address:#x} with {bytes} bytes overflows"
            ),
            Self::GuestMemory {
                operation,
                address,
                bytes,
                capacity_bytes,
            } => write!(
                formatter,
                "simple disk guest memory {operation} at {address:#x} for {bytes} bytes exceeds capacity {capacity_bytes}"
            ),
            Self::GuestTransferSizeMismatch {
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "simple disk guest memory returned {actual_bytes} bytes but expected {expected_bytes}"
            ),
            Self::Storage(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for SimpleDiskError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Storage(error) => Some(error),
            _ => None,
        }
    }
}
