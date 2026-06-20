use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

use super::{
    guest_fd_argument, linux_error,
    signal::{signal_probe_or_unimplemented_delivery, valid_signal_i32},
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EMFILE, RISCV_LINUX_ESRCH, RISCV_LINUX_O_NONBLOCK, RISCV_LINUX_O_RDWR,
};

pub(super) const RISCV_LINUX_PIDFD_SEND_SIGNAL: u64 = 424;
pub(super) const RISCV_LINUX_PIDFD_OPEN: u64 = 434;

const RISCV_LINUX_PIDFD_SUPPORTED_FLAGS: u32 = RISCV_LINUX_O_NONBLOCK as u32;
const RISCV_LINUX_O_LARGEFILE: u32 = 0x8000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestPidFd {
    process_id: u64,
}

impl RiscvGuestPidFd {
    const fn new(process_id: u64) -> Self {
        Self { process_id }
    }

    const fn process_id(self) -> u64 {
        self.process_id
    }
}

impl RiscvSyscallState {
    fn open_guest_pidfd(&mut self, process_id: u64, flags: u32) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_open_fd()?;
        let description = self.next_open_description()?;
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(
                    RISCV_LINUX_O_RDWR as u32
                        | RISCV_LINUX_O_LARGEFILE
                        | (flags & RISCV_LINUX_PIDFD_SUPPORTED_FLAGS),
                ),
            ))?;
        self.guest_fds.insert(fd, GuestFdEntry::new(description))?;
        self.guest_fds.set_close_on_exec(fd, true)?;
        self.guest_pidfds
            .insert(description, RiscvGuestPidFd::new(process_id));
        Ok(fd)
    }

    fn pidfd_for_fd(&self, fd: GuestFd) -> Result<Option<RiscvGuestPidFd>, GuestFdError> {
        let description = self.guest_fds.description_for_fd(fd)?.id();
        Ok(self.guest_pidfds.get(&description).copied())
    }

    pub(super) fn remove_guest_pidfd_description(&mut self, description: GuestFileDescriptionId) {
        self.guest_pidfds.remove(&description);
    }
}

pub(super) fn syscall_pidfd_open(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let process_id = linux_pid_argument(request.argument(0));
    if process_id <= 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Ok(process_id) = u64::try_from(process_id) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if process_id != state.identity().thread_group_id() {
        return linux_error(RISCV_LINUX_ESRCH);
    }

    let flags = request.argument(1) as u32;
    if flags & !RISCV_LINUX_PIDFD_SUPPORTED_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    match state.open_guest_pidfd(process_id, flags) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn syscall_pidfd_send_signal(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: rem6_kernel::Tick,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let pidfd = match state.pidfd_for_fd(fd) {
        Ok(Some(pidfd)) => pidfd,
        Ok(None) | Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };

    let signal = linux_int_argument(request.argument(1));
    if signal != 0 && !valid_signal_i32(signal) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if request.argument(2) != 0 || request.argument(3) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    if pidfd.process_id() != state.identity().thread_group_id() {
        return linux_error(RISCV_LINUX_ESRCH);
    }

    signal_probe_or_unimplemented_delivery(request, state, tick, signal)
}

fn linux_pid_argument(argument: u64) -> i32 {
    argument as u32 as i32
}

fn linux_int_argument(argument: u64) -> i32 {
    argument as u32 as i32
}
