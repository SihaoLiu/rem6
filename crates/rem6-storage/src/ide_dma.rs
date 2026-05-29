use crate::{IdeChannelId, IdeControllerError, STORAGE_SECTOR_BYTES};

const IDE_PRD_ENTRY_BYTES: u64 = 8;
const IDE_PRD_BASE_MASK: u32 = 0xffff_fffe;
const IDE_PRD_COUNT_MASK: u16 = 0xfffe;
const IDE_PRD_EOT_MASK: u16 = 0x8000;
const IDE_PRD_ZERO_COUNT_BYTES: u64 = 0x1_0000;
const IDE_MAX_PRD_ENTRIES: usize = 4096;

pub trait IdeControllerGuestMemory {
    fn validate_read(&self, address: u64, bytes: u64) -> Result<(), IdeControllerError>;
    fn validate_write(&self, address: u64, bytes: u64) -> Result<(), IdeControllerError>;
    fn read_bytes(&mut self, address: u64, bytes: u64) -> Result<Vec<u8>, IdeControllerError>;
    fn write_bytes(&mut self, address: u64, data: &[u8]) -> Result<(), IdeControllerError>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IdeDmaDirection {
    ToGuest,
    FromGuest,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct IdeDmaRequest {
    direction: IdeDmaDirection,
    start_sector: u64,
    sectors: u64,
}

impl IdeDmaRequest {
    pub(crate) const fn new(direction: IdeDmaDirection, start_sector: u64, sectors: u64) -> Self {
        Self {
            direction,
            start_sector,
            sectors,
        }
    }

    pub(crate) const fn direction(self) -> IdeDmaDirection {
        self.direction
    }

    pub(crate) const fn start_sector(self) -> u64 {
        self.start_sector
    }

    pub(crate) const fn sectors(self) -> u64 {
        self.sectors
    }

    pub(crate) fn bytes(self) -> Option<u64> {
        self.sectors.checked_mul(STORAGE_SECTOR_BYTES)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IdeDmaPlan {
    segments: Vec<IdeDmaSegment>,
    bytes: u64,
}

impl IdeDmaPlan {
    pub(crate) fn decode(
        channel: IdeChannelId,
        guest: &mut impl IdeControllerGuestMemory,
        table_address: u32,
        expected_bytes: u64,
    ) -> Result<Self, IdeControllerError> {
        let mut address = u64::from(table_address & !0x3);
        let mut segments = Vec::new();
        let mut bytes = 0_u64;
        for _ in 0..IDE_MAX_PRD_ENTRIES {
            guest.validate_read(address, IDE_PRD_ENTRY_BYTES)?;
            let entry = guest.read_bytes(address, IDE_PRD_ENTRY_BYTES)?;
            if entry.len() != IDE_PRD_ENTRY_BYTES as usize {
                return Err(IdeControllerError::GuestTransferSizeMismatch {
                    expected_bytes: IDE_PRD_ENTRY_BYTES,
                    actual_bytes: entry.len() as u64,
                });
            }

            let base = u32::from_le_bytes(entry[0..4].try_into().unwrap()) & IDE_PRD_BASE_MASK;
            let raw_count = u16::from_le_bytes(entry[4..6].try_into().unwrap());
            let end = u16::from_le_bytes(entry[6..8].try_into().unwrap()) & IDE_PRD_EOT_MASK != 0;
            let segment_bytes = prd_byte_count(channel, raw_count)?;
            bytes = bytes
                .checked_add(segment_bytes)
                .ok_or(IdeControllerError::DmaByteCountOverflow { channel })?;
            if bytes > expected_bytes {
                return Err(IdeControllerError::DmaByteCountMismatch {
                    channel,
                    expected_bytes,
                    prd_bytes: bytes,
                });
            }
            segments.push(IdeDmaSegment {
                address: u64::from(base),
                bytes: segment_bytes,
            });
            if end {
                if bytes != expected_bytes {
                    return Err(IdeControllerError::DmaByteCountMismatch {
                        channel,
                        expected_bytes,
                        prd_bytes: bytes,
                    });
                }
                return Ok(Self { segments, bytes });
            }
            address = address.checked_add(IDE_PRD_ENTRY_BYTES).ok_or(
                IdeControllerError::GuestAddressOverflow {
                    address,
                    bytes: IDE_PRD_ENTRY_BYTES,
                },
            )?;
        }
        Err(IdeControllerError::PrdTableMissingEnd { channel })
    }

    pub(crate) fn validate_access(
        &self,
        direction: IdeDmaDirection,
        guest: &impl IdeControllerGuestMemory,
    ) -> Result<(), IdeControllerError> {
        for segment in &self.segments {
            match direction {
                IdeDmaDirection::ToGuest => guest.validate_write(segment.address, segment.bytes)?,
                IdeDmaDirection::FromGuest => {
                    guest.validate_read(segment.address, segment.bytes)?
                }
            }
        }
        Ok(())
    }

    pub(crate) fn write_payload(
        &self,
        guest: &mut impl IdeControllerGuestMemory,
        payload: &[u8],
    ) -> Result<(), IdeControllerError> {
        if payload.len() as u64 != self.bytes {
            return Err(IdeControllerError::GuestTransferSizeMismatch {
                expected_bytes: self.bytes,
                actual_bytes: payload.len() as u64,
            });
        }
        let mut offset = 0_usize;
        for segment in &self.segments {
            let end = offset + segment.bytes as usize;
            guest.write_bytes(segment.address, &payload[offset..end])?;
            offset = end;
        }
        Ok(())
    }

    pub(crate) fn read_payload(
        &self,
        guest: &mut impl IdeControllerGuestMemory,
    ) -> Result<Vec<u8>, IdeControllerError> {
        let mut payload = Vec::with_capacity(self.bytes as usize);
        for segment in &self.segments {
            let bytes = guest.read_bytes(segment.address, segment.bytes)?;
            if bytes.len() as u64 != segment.bytes {
                return Err(IdeControllerError::GuestTransferSizeMismatch {
                    expected_bytes: segment.bytes,
                    actual_bytes: bytes.len() as u64,
                });
            }
            payload.extend_from_slice(&bytes);
        }
        Ok(payload)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct IdeDmaSegment {
    address: u64,
    bytes: u64,
}

fn prd_byte_count(channel: IdeChannelId, raw_count: u16) -> Result<u64, IdeControllerError> {
    let bytes = if raw_count == 0 {
        IDE_PRD_ZERO_COUNT_BYTES
    } else {
        u64::from(raw_count & IDE_PRD_COUNT_MASK)
    };
    if bytes == 0 || !bytes.is_multiple_of(STORAGE_SECTOR_BYTES) {
        return Err(IdeControllerError::InvalidPrdByteCount { channel, bytes });
    }
    Ok(bytes)
}
