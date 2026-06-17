use super::{
    linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAGAIN, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDWR,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_EVENTFD2: u64 = 19;

const RISCV_LINUX_EFD_SEMAPHORE: u64 = 1;
const RISCV_LINUX_EVENTFD_VALID_FLAGS: u64 =
    RISCV_LINUX_EFD_SEMAPHORE | RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_EVENTFD_COUNTER_BYTES: u64 = 8;
const RISCV_LINUX_EVENTFD_MAX_COUNTER: u64 = u64::MAX - 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestEventFd {
    counter: u64,
    semaphore: bool,
}

impl RiscvGuestEventFd {
    const fn new(counter: u64, semaphore: bool) -> Self {
        Self { counter, semaphore }
    }

    const fn read_value(self) -> Option<u64> {
        if self.counter == 0 {
            None
        } else if self.semaphore {
            Some(1)
        } else {
            Some(self.counter)
        }
    }

    const fn can_write(self, value: u64) -> bool {
        match self.counter.checked_add(value) {
            Some(next) => next <= RISCV_LINUX_EVENTFD_MAX_COUNTER,
            None => false,
        }
    }

    fn consume_read(&mut self) {
        if self.semaphore {
            self.counter = self.counter.saturating_sub(1);
        } else {
            self.counter = 0;
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestEventFdRead {
    Value(u64),
    Blocked,
    WouldBlock,
    InvalidSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestEventFdWrite {
    Accepted,
    Blocked,
    WouldBlock,
    InvalidSize,
    InvalidValue,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestEventFdReady {
    readable: bool,
    writable: bool,
}

impl RiscvGuestEventFdReady {
    pub(super) const fn readable(self) -> bool {
        self.readable
    }

    pub(super) const fn writable(self) -> bool {
        self.writable
    }
}

impl RiscvSyscallState {
    pub(super) fn guest_eventfd_read(
        &self,
        fd: GuestFd,
        count: u64,
    ) -> Result<Option<RiscvGuestEventFdRead>, GuestFdError> {
        let Some(eventfd) = self.eventfd_for_fd(fd)? else {
            return Ok(None);
        };
        if count < RISCV_LINUX_EVENTFD_COUNTER_BYTES {
            return Ok(Some(RiscvGuestEventFdRead::InvalidSize));
        }
        Ok(Some(match eventfd.read_value() {
            Some(value) => RiscvGuestEventFdRead::Value(value),
            None if self.eventfd_nonblocking(fd)? => RiscvGuestEventFdRead::WouldBlock,
            None => RiscvGuestEventFdRead::Blocked,
        }))
    }

    pub(super) fn consume_guest_eventfd_read(&mut self, fd: GuestFd) -> Result<(), GuestFdError> {
        let Some(description) = self.eventfd_description_for_fd(fd)? else {
            return Err(GuestFdError::BadFd { fd });
        };
        let eventfd = self
            .guest_eventfds
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        eventfd.consume_read();
        Ok(())
    }

    pub(super) fn write_guest_eventfd_from_fd(
        &mut self,
        fd: GuestFd,
        bytes: &[u8],
    ) -> Result<Option<RiscvGuestEventFdWrite>, GuestFdError> {
        let Some(description) = self.eventfd_description_for_fd(fd)? else {
            return Ok(None);
        };
        if bytes.len() < RISCV_LINUX_EVENTFD_COUNTER_BYTES as usize {
            return Ok(Some(RiscvGuestEventFdWrite::InvalidSize));
        }
        let mut raw = [0_u8; 8];
        raw.copy_from_slice(&bytes[..8]);
        let value = u64::from_le_bytes(raw);
        if value == u64::MAX {
            return Ok(Some(RiscvGuestEventFdWrite::InvalidValue));
        }
        let nonblocking = self.eventfd_nonblocking(fd)?;
        let eventfd = self
            .guest_eventfds
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        if !eventfd.can_write(value) {
            return Ok(Some(if nonblocking {
                RiscvGuestEventFdWrite::WouldBlock
            } else {
                RiscvGuestEventFdWrite::Blocked
            }));
        }
        eventfd.counter += value;
        Ok(Some(RiscvGuestEventFdWrite::Accepted))
    }

    pub(super) fn guest_eventfd_ready(
        &self,
        fd: GuestFd,
    ) -> Result<Option<RiscvGuestEventFdReady>, GuestFdError> {
        let Some(eventfd) = self.eventfd_for_fd(fd)? else {
            return Ok(None);
        };
        Ok(Some(RiscvGuestEventFdReady {
            readable: eventfd.counter > 0,
            writable: eventfd.counter < RISCV_LINUX_EVENTFD_MAX_COUNTER,
        }))
    }

    pub(super) fn remove_guest_eventfd_description(&mut self, description: GuestFileDescriptionId) {
        self.guest_eventfds.remove(&description);
    }

    fn open_guest_eventfd(&mut self, initial: u64, flags: u64) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_eventfd_fd()?;
        let description = self.next_open_description()?;
        let close_on_exec = flags & RISCV_LINUX_O_CLOEXEC != 0;
        let status_flags = RISCV_LINUX_O_RDWR | (flags & RISCV_LINUX_O_NONBLOCK);
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(status_flags as u32),
            ))?;
        self.guest_fds.insert(
            fd,
            GuestFdEntry::new(description).with_close_on_exec(close_on_exec),
        )?;
        self.guest_eventfds.insert(
            description,
            RiscvGuestEventFd::new(initial, flags & RISCV_LINUX_EFD_SEMAPHORE != 0),
        );
        Ok(fd)
    }

    fn eventfd_description_for_fd(
        &self,
        fd: GuestFd,
    ) -> Result<Option<GuestFileDescriptionId>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self
            .guest_eventfds
            .contains_key(&description)
            .then_some(description))
    }

    fn eventfd_for_fd(&self, fd: GuestFd) -> Result<Option<RiscvGuestEventFd>, GuestFdError> {
        let Some(description) = self.eventfd_description_for_fd(fd)? else {
            return Ok(None);
        };
        self.guest_eventfds
            .get(&description)
            .copied()
            .map(Some)
            .ok_or(GuestFdError::MissingFileDescription { description })
    }

    fn eventfd_nonblocking(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        Ok(self.guest_fds.status_flags(fd)?.bits() & RISCV_LINUX_O_NONBLOCK as u32 != 0)
    }

    fn next_eventfd_fd(&self) -> Result<GuestFd, GuestFdError> {
        let snapshot = self.guest_fds.snapshot();
        let mut candidate = 0_i32;
        loop {
            let fd = GuestFd::new(candidate)?;
            if snapshot.entries().iter().all(|entry| entry.fd() != fd) {
                return Ok(fd);
            }
            candidate = candidate
                .checked_add(1)
                .ok_or(GuestFdError::FdSpaceExhausted)?;
        }
    }
}

pub(super) fn syscall_eventfd2(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let initial = request.argument(0) & u64::from(u32::MAX);
    let flags = u64::from(request.argument(1) as u32);
    if flags & !RISCV_LINUX_EVENTFD_VALID_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.open_guest_eventfd(initial, flags) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn eventfd_read_bytes(value: u64) -> [u8; 8] {
    value.to_le_bytes()
}

pub(super) fn eventfd_write_result(write: RiscvGuestEventFdWrite) -> Option<u64> {
    match write {
        RiscvGuestEventFdWrite::Accepted => Some(RISCV_LINUX_EVENTFD_COUNTER_BYTES),
        RiscvGuestEventFdWrite::Blocked => None,
        RiscvGuestEventFdWrite::WouldBlock => Some(linux_error(RISCV_LINUX_EAGAIN)),
        RiscvGuestEventFdWrite::InvalidSize | RiscvGuestEventFdWrite::InvalidValue => {
            Some(linux_error(RISCV_LINUX_EINVAL))
        }
    }
}

pub(super) fn eventfd_write_bytes_written() -> u64 {
    RISCV_LINUX_EVENTFD_COUNTER_BYTES
}
