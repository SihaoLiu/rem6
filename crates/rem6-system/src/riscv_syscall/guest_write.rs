use rem6_kernel::Tick;

use crate::GuestFd;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvGuestWriteRecord {
    fd: GuestFd,
    address: u64,
    tick: Tick,
    bytes: Vec<u8>,
}

impl RiscvGuestWriteRecord {
    pub fn new(fd: GuestFd, address: u64, tick: Tick, bytes: Vec<u8>) -> Self {
        Self {
            fd,
            address,
            tick,
            bytes,
        }
    }

    pub const fn fd(&self) -> GuestFd {
        self.fd
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
