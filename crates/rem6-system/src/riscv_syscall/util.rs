use crate::GuestFd;

use super::RiscvGuestMemoryReader;

pub(super) fn guest_fd_argument(value: u64) -> Option<GuestFd> {
    i32::try_from(value)
        .ok()
        .and_then(|fd| GuestFd::new(fd).ok())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestCStringError {
    Fault,
    TooLong,
}

pub(super) fn read_guest_c_string(
    guest_memory: &RiscvGuestMemoryReader,
    address: u64,
    limit: usize,
) -> Result<Vec<u8>, RiscvGuestCStringError> {
    let mut bytes = Vec::new();
    for offset in 0..limit {
        let address = address
            .checked_add(offset as u64)
            .ok_or(RiscvGuestCStringError::Fault)?;
        let byte = guest_memory
            .read(address, 1)
            .filter(|bytes| bytes.len() == 1)
            .and_then(|bytes| bytes.first().copied())
            .ok_or(RiscvGuestCStringError::Fault)?;
        if byte == 0 {
            return Ok(bytes);
        }
        bytes.push(byte);
    }
    Err(RiscvGuestCStringError::TooLong)
}

pub(super) const fn linux_error(errno: u64) -> u64 {
    0u64.wrapping_sub(errno)
}
