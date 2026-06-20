use crate::{
    GuestFd, GuestFdEntry, GuestFdError, GuestFileDescription, GuestFileDescriptionId,
    GuestFileStatusFlags,
};

use super::{
    guest_fd_argument, linux_error,
    signal::{signal_probe_or_unimplemented_delivery, valid_signal_i32},
    RiscvSyscallIdentity, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE, RISCV_LINUX_ESRCH, RISCV_LINUX_O_NONBLOCK,
    RISCV_LINUX_O_RDWR,
};

pub(super) const RISCV_LINUX_PIDFD_SEND_SIGNAL: u64 = 424;
pub(super) const RISCV_LINUX_PIDFD_OPEN: u64 = 434;
pub(super) const RISCV_LINUX_PIDFD_GETFD: u64 = 438;

const RISCV_LINUX_PIDFD_THREAD: u32 = 0o200;
const RISCV_LINUX_PIDFD_SUPPORTED_FLAGS: u32 =
    RISCV_LINUX_O_NONBLOCK as u32 | RISCV_LINUX_PIDFD_THREAD;
const RISCV_LINUX_O_LARGEFILE: u32 = 0x8000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvGuestPidFd {
    target: RiscvGuestPidFdTarget,
}

impl RiscvGuestPidFd {
    const fn new(target: RiscvGuestPidFdTarget) -> Self {
        Self { target }
    }

    const fn target(self) -> RiscvGuestPidFdTarget {
        self.target
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvGuestPidFdTarget {
    ThreadGroup(u64),
    Thread(u64),
}

impl RiscvGuestPidFdTarget {
    const fn matches_identity(self, identity: RiscvSyscallIdentity) -> bool {
        match self {
            Self::ThreadGroup(process_id) => process_id == identity.thread_group_id(),
            Self::Thread(thread_id) => thread_id == identity.thread_id(),
        }
    }

    const fn process_group_id(self, identity: RiscvSyscallIdentity) -> Option<u64> {
        match self {
            Self::ThreadGroup(process_id) if process_id == identity.thread_group_id() => {
                Some(process_id)
            }
            Self::ThreadGroup(_) | Self::Thread(_) => None,
        }
    }
}

impl RiscvSyscallState {
    fn open_guest_pidfd(
        &mut self,
        target: RiscvGuestPidFdTarget,
        flags: u32,
    ) -> Result<GuestFd, GuestFdError> {
        let fd = self.next_open_fd()?;
        let description = self.next_open_description()?;
        self.guest_fds
            .insert_description(GuestFileDescription::guest_backed(
                description,
                GuestFileStatusFlags::new(
                    RISCV_LINUX_O_RDWR as u32
                        | RISCV_LINUX_O_LARGEFILE
                        | (flags & RISCV_LINUX_O_NONBLOCK as u32),
                ),
            ))?;
        self.guest_fds.insert(fd, GuestFdEntry::new(description))?;
        self.guest_fds.set_close_on_exec(fd, true)?;
        self.guest_pidfds
            .insert(description, RiscvGuestPidFd::new(target));
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
    let flags = linux_uint_argument(request.argument(1));
    if flags & !RISCV_LINUX_PIDFD_SUPPORTED_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let process_id = linux_pid_argument(request.argument(0));
    if process_id <= 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Ok(process_id) = u64::try_from(process_id) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let target = match pidfd_open_target(process_id, flags, state.identity()) {
        Ok(target) => target,
        Err(error) => return linux_error(error),
    };

    match state.open_guest_pidfd(target, flags) {
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
    let flags = linux_uint_argument(request.argument(3));
    if !matches!(flags, 0 | 1 | 2 | 4) {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let Some(fd) = linux_fd_argument(request.argument(0)) else {
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
    if request.argument(2) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let identity = state.identity();
    if !pidfd.target().matches_identity(identity) {
        return linux_error(RISCV_LINUX_ESRCH);
    }
    if flags == 4
        && Some(u64::from(state.guest_wait.current_process_group().get()))
            != pidfd.target().process_group_id(identity)
    {
        return linux_error(RISCV_LINUX_ESRCH);
    }

    signal_probe_or_unimplemented_delivery(request, state, tick, signal)
}

pub(super) fn syscall_pidfd_getfd(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    if linux_uint_argument(request.argument(2)) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let Some(pidfd_fd) = linux_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let pidfd = match state.pidfd_for_fd(pidfd_fd) {
        Ok(Some(pidfd)) => pidfd,
        Ok(None) | Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    if !pidfd.target().matches_identity(state.identity()) {
        return linux_error(RISCV_LINUX_ESRCH);
    }

    let Some(target_fd) = linux_fd_argument(request.argument(1)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    match state.guest_fds.dup(target_fd) {
        Ok(new_fd) => {
            state.duplicate_fd_source(target_fd, new_fd);
            if state.guest_fds.set_close_on_exec(new_fd, true).is_err() {
                return linux_error(RISCV_LINUX_EBADF);
            }
            u64::from(new_fd.get())
        }
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}

fn linux_pid_argument(argument: u64) -> i32 {
    argument as u32 as i32
}

fn pidfd_open_target(
    process_id: u64,
    flags: u32,
    identity: RiscvSyscallIdentity,
) -> Result<RiscvGuestPidFdTarget, u64> {
    if flags & RISCV_LINUX_PIDFD_THREAD != 0 {
        if process_id == identity.thread_id() {
            Ok(RiscvGuestPidFdTarget::Thread(process_id))
        } else {
            Err(RISCV_LINUX_ESRCH)
        }
    } else if process_id == identity.thread_group_id() {
        Ok(RiscvGuestPidFdTarget::ThreadGroup(process_id))
    } else {
        Err(RISCV_LINUX_ESRCH)
    }
}

fn linux_int_argument(argument: u64) -> i32 {
    argument as u32 as i32
}

fn linux_uint_argument(argument: u64) -> u32 {
    argument as u32
}

fn linux_fd_argument(argument: u64) -> Option<GuestFd> {
    guest_fd_argument(u64::from(linux_uint_argument(argument)))
}
