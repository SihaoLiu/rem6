use super::RiscvGuestMemoryReader;

const RISCV_LINUX_TIMESPEC64_BYTES: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvLinuxTimespec64 {
    seconds: i64,
    nanoseconds: i64,
}

impl RiscvLinuxTimespec64 {
    pub(super) const fn new(seconds: i64, nanoseconds: i64) -> Self {
        Self {
            seconds,
            nanoseconds,
        }
    }

    pub(super) const fn is_zero(self) -> bool {
        self.seconds == 0 && self.nanoseconds == 0
    }

    pub(super) fn is_valid(self) -> bool {
        self.seconds >= 0 && (0..1_000_000_000).contains(&self.nanoseconds)
    }

    pub(super) fn total_nanoseconds(self) -> u128 {
        debug_assert!(self.is_valid());
        self.seconds as u128 * 1_000_000_000 + self.nanoseconds as u128
    }
}

pub(super) fn read_timespec64(
    guest_memory_reader: &RiscvGuestMemoryReader,
    address: u64,
) -> Option<RiscvLinuxTimespec64> {
    let bytes = guest_memory_reader.read(address, RISCV_LINUX_TIMESPEC64_BYTES)?;
    if bytes.len() != RISCV_LINUX_TIMESPEC64_BYTES {
        return None;
    }
    Some(RiscvLinuxTimespec64::new(
        i64::from_le_bytes(bytes[0..8].try_into().ok()?),
        i64::from_le_bytes(bytes[8..16].try_into().ok()?),
    ))
}
