use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryReader, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EAGAIN, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_O_CLOEXEC, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDWR,
};
use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

pub(super) const RISCV_LINUX_SIGNALFD4: u64 = 74;

const RISCV_LINUX_SIGNALFD_SIGINFO_BYTES: u64 = 128;
const RISCV_LINUX_SIGNALFD_SIGSET_BYTES: u64 = 8;
const RISCV_LINUX_SIGNALFD_VALID_FLAGS: u64 = RISCV_LINUX_O_CLOEXEC | RISCV_LINUX_O_NONBLOCK;
const RISCV_LINUX_FIRST_SIGNAL: u64 = 1;
const RISCV_LINUX_LAST_SIGNAL: u64 = 64;
const RISCV_LINUX_SIGKILL_MASK: u64 = 1 << (9 - 1);
const RISCV_LINUX_SIGSTOP_MASK: u64 = 1 << (19 - 1);
const RISCV_LINUX_UNBLOCKABLE_SIGNALS: u64 = RISCV_LINUX_SIGKILL_MASK | RISCV_LINUX_SIGSTOP_MASK;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestSignalFd {
    mask: u64,
}

impl RiscvGuestSignalFd {
    const fn new(mask: u64) -> Self {
        Self { mask }
    }

    const fn pending_match(self, pending_mask: u64) -> u64 {
        self.mask & pending_mask
    }

    fn pending_signals(self, pending_mask: u64, limit: usize) -> Vec<u64> {
        let pending = self.pending_match(pending_mask);
        (RISCV_LINUX_FIRST_SIGNAL..=RISCV_LINUX_LAST_SIGNAL)
            .filter(|signal| pending & signal_bit(*signal) != 0)
            .take(limit)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum RiscvGuestSignalFdRead {
    Signals(Vec<u64>),
    Blocked,
    WouldBlock,
    InvalidSize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestSignalFdReady {
    readable: bool,
}

impl RiscvGuestSignalFdReady {
    pub(super) const fn readable(self) -> bool {
        self.readable
    }
}

impl RiscvSyscallState {
    pub(super) fn guest_signalfd_read(
        &self,
        fd: GuestFd,
        count: u64,
    ) -> Result<Option<RiscvGuestSignalFdRead>, GuestFdError> {
        let Some(signalfd) = self.signalfd_for_fd(fd)? else {
            return Ok(None);
        };
        if count < RISCV_LINUX_SIGNALFD_SIGINFO_BYTES {
            return Ok(Some(RiscvGuestSignalFdRead::InvalidSize));
        }
        let limit = usize::try_from(count / RISCV_LINUX_SIGNALFD_SIGINFO_BYTES)
            .unwrap_or(usize::MAX)
            .min(RISCV_LINUX_LAST_SIGNAL as usize);
        let signals = signalfd.pending_signals(self.pending_signal_mask(), limit);
        Ok(Some(if signals.is_empty() {
            if self.signalfd_nonblocking(fd)? {
                RiscvGuestSignalFdRead::WouldBlock
            } else {
                RiscvGuestSignalFdRead::Blocked
            }
        } else {
            RiscvGuestSignalFdRead::Signals(signals)
        }))
    }

    pub(super) fn consume_guest_signalfd_read(
        &mut self,
        fd: GuestFd,
        signals: &[u64],
    ) -> Result<(), GuestFdError> {
        let Some(_description) = self.signalfd_description_for_fd(fd)? else {
            return Err(GuestFdError::BadFd { fd });
        };
        let clear_mask = signals
            .iter()
            .fold(0_u64, |mask, signal| mask | signal_bit(*signal));
        self.clear_pending_signal_mask(clear_mask);
        Ok(())
    }

    pub(super) fn guest_signalfd_ready(
        &self,
        fd: GuestFd,
    ) -> Result<Option<RiscvGuestSignalFdReady>, GuestFdError> {
        let Some(signalfd) = self.signalfd_for_fd(fd)? else {
            return Ok(None);
        };
        Ok(Some(RiscvGuestSignalFdReady {
            readable: signalfd.pending_match(self.pending_signal_mask()) != 0,
        }))
    }

    pub(super) fn remove_guest_signalfd_description(
        &mut self,
        description: GuestFileDescriptionId,
    ) {
        self.guest_signalfds.remove(&description);
    }

    fn open_guest_signalfd(&mut self, mask: u64, flags: u64) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_guest_fd_excluding(&[])?;
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
        self.guest_signalfds
            .insert(description, RiscvGuestSignalFd::new(mask));
        Ok(fd)
    }

    fn update_guest_signalfd(&mut self, fd: GuestFd, mask: u64) -> Result<bool, GuestFdError> {
        let Some(description) = self.signalfd_description_for_fd(fd)? else {
            return Ok(false);
        };
        let signalfd = self
            .guest_signalfds
            .get_mut(&description)
            .ok_or(GuestFdError::MissingFileDescription { description })?;
        signalfd.mask = mask;
        Ok(true)
    }

    fn signalfd_description_for_fd(
        &self,
        fd: GuestFd,
    ) -> Result<Option<GuestFileDescriptionId>, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self
            .guest_signalfds
            .contains_key(&description)
            .then_some(description))
    }

    fn signalfd_for_fd(&self, fd: GuestFd) -> Result<Option<RiscvGuestSignalFd>, GuestFdError> {
        let Some(description) = self.signalfd_description_for_fd(fd)? else {
            return Ok(None);
        };
        self.guest_signalfds
            .get(&description)
            .copied()
            .map(Some)
            .ok_or(GuestFdError::MissingFileDescription { description })
    }

    fn signalfd_nonblocking(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        Ok(self.guest_fds.status_flags(fd)?.bits() & RISCV_LINUX_O_NONBLOCK as u32 != 0)
    }
}

pub(super) fn syscall_signalfd4(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    if request.argument(2) != RISCV_LINUX_SIGNALFD_SIGSET_BYTES {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    let flags = u64::from(request.argument(3) as u32);
    if flags & !RISCV_LINUX_SIGNALFD_VALID_FLAGS != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    let guest_memory_reader = guest_memory_reader?;
    let mask = match read_signalfd_mask(guest_memory_reader, request.argument(1)) {
        Some(mask) => mask & !RISCV_LINUX_UNBLOCKABLE_SIGNALS,
        None => return Some(linux_error(RISCV_LINUX_EFAULT)),
    };

    let fd_argument = request.argument(0) as u32 as i32;
    if fd_argument == -1 {
        return Some(match state.open_guest_signalfd(mask, flags) {
            Ok(fd) => u64::from(fd.get()),
            Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
            Err(_error) => linux_error(RISCV_LINUX_EINVAL),
        });
    }
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    Some(match state.update_guest_signalfd(fd, mask) {
        Ok(true) => request.argument(0),
        Ok(false) => linux_error(RISCV_LINUX_EINVAL),
        Err(GuestFdError::BadFd { .. }) => linux_error(RISCV_LINUX_EBADF),
        Err(_error) => linux_error(RISCV_LINUX_EINVAL),
    })
}

pub(super) fn signalfd_siginfo_bytes(
    state: &RiscvSyscallState,
    signal: u64,
) -> [u8; RISCV_LINUX_SIGNALFD_SIGINFO_BYTES as usize] {
    let mut bytes = [0_u8; RISCV_LINUX_SIGNALFD_SIGINFO_BYTES as usize];
    bytes[0..4].copy_from_slice(&(signal as u32).to_le_bytes());
    bytes[12..16].copy_from_slice(&(state.identity().thread_group_id() as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&(state.identity().user_id() as u32).to_le_bytes());
    bytes
}

pub(super) fn signalfd_read_result(read: RiscvGuestSignalFdRead) -> Option<u64> {
    match read {
        RiscvGuestSignalFdRead::Signals(signals) => Some(
            u64::try_from(signals.len())
                .unwrap_or(u64::MAX)
                .saturating_mul(RISCV_LINUX_SIGNALFD_SIGINFO_BYTES),
        ),
        RiscvGuestSignalFdRead::Blocked => None,
        RiscvGuestSignalFdRead::WouldBlock => Some(linux_error(RISCV_LINUX_EAGAIN)),
        RiscvGuestSignalFdRead::InvalidSize => Some(linux_error(RISCV_LINUX_EINVAL)),
    }
}

pub(super) fn signalfd_write_result() -> u64 {
    linux_error(RISCV_LINUX_EINVAL)
}

fn read_signalfd_mask(guest_memory_reader: &RiscvGuestMemoryReader, address: u64) -> Option<u64> {
    let bytes = guest_memory_reader.read(address, RISCV_LINUX_SIGNALFD_SIGSET_BYTES as usize)?;
    let bytes: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn signal_bit(signal: u64) -> u64 {
    1_u64 << (signal - 1)
}
