use super::{
    eventfd::{eventfd_read_bytes, RiscvGuestEventFdRead},
    guest_fd_argument,
    inotify::{inotify_read_result, RiscvGuestInotifyRead},
    linux_error,
    signalfd::{signalfd_read_result, signalfd_siginfo_bytes, RiscvGuestSignalFdRead},
    socket::RiscvGuestSocketRead,
    timerfd::{timerfd_read_bytes, timerfd_read_result, RiscvGuestTimerFdRead},
    RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EAGAIN,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ESPIPE,
    RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_WRONLY,
};
use crate::{GuestFd, GuestFdError};

pub(super) const RISCV_LINUX_READ: u64 = 63;
pub(super) const RISCV_LINUX_PREAD64: u64 = 67;

impl RiscvSyscallState {
    pub(super) fn guest_file_fd_is_seekable(&self, fd: GuestFd) -> Result<bool, GuestFdError> {
        let description = self
            .guest_fds
            .entry(fd)
            .ok_or(GuestFdError::BadFd { fd })?
            .description();
        Ok(self.guest_file_descriptions.contains_key(&description))
    }
}

pub(super) fn syscall_read(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> Option<u64> {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }

    let count = request.argument(2);
    match state.guest_eventfd_read(fd, count) {
        Ok(Some(read)) => {
            let value = match read {
                RiscvGuestEventFdRead::Value(value) => value,
                RiscvGuestEventFdRead::Blocked => return None,
                RiscvGuestEventFdRead::WouldBlock => {
                    return Some(linux_error(RISCV_LINUX_EAGAIN));
                }
                RiscvGuestEventFdRead::InvalidSize => {
                    return Some(linux_error(RISCV_LINUX_EINVAL));
                }
            };
            let bytes = eventfd_read_bytes(value);
            if !guest_memory.write(request.argument(1), &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_eventfd_read(fd).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    match state.guest_timerfd_read(fd, count) {
        Ok(Some(read)) => {
            let value = match read {
                RiscvGuestTimerFdRead::Value(value) => value,
                RiscvGuestTimerFdRead::Blocked => return None,
                RiscvGuestTimerFdRead::WouldBlock | RiscvGuestTimerFdRead::InvalidSize => {
                    return timerfd_read_result(read);
                }
            };
            let bytes = timerfd_read_bytes(value);
            if !guest_memory.write(request.argument(1), &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_timerfd_read(fd).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    match state.guest_signalfd_read(fd, count) {
        Ok(Some(read)) => {
            let signals = match read {
                RiscvGuestSignalFdRead::Signals(signals) => signals,
                RiscvGuestSignalFdRead::Blocked => return None,
                RiscvGuestSignalFdRead::WouldBlock | RiscvGuestSignalFdRead::InvalidSize => {
                    return signalfd_read_result(read);
                }
            };
            let mut bytes = Vec::with_capacity(signals.len() * 128);
            for signal in &signals {
                bytes.extend_from_slice(&signalfd_siginfo_bytes(state, *signal));
            }
            if !guest_memory.write(request.argument(1), &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_signalfd_read(fd, &signals).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    match state.guest_inotify_read(fd, count) {
        Ok(Some(read)) => {
            let bytes = match read {
                RiscvGuestInotifyRead::Bytes(bytes) => bytes,
                RiscvGuestInotifyRead::Blocked => return None,
                RiscvGuestInotifyRead::WouldBlock | RiscvGuestInotifyRead::InvalidSize => {
                    return inotify_read_result(read);
                }
            };
            if !guest_memory.write(request.argument(1), &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_inotify_read(fd, bytes.len()).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    if count == 0 {
        return Some(0);
    }

    let Ok(byte_count) = usize::try_from(count) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    match state.guest_socket_read(fd, byte_count) {
        Ok(RiscvGuestSocketRead::Bytes(bytes)) => {
            if bytes.is_empty() {
                return Some(0);
            }
            if !guest_memory.write(request.argument(1), &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_socket_read(fd, bytes.len()).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(RiscvGuestSocketRead::WouldBlock) => return Some(linux_error(RISCV_LINUX_EAGAIN)),
        Ok(RiscvGuestSocketRead::Blocked) => return None,
        Ok(RiscvGuestSocketRead::NotSocket) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    match state.guest_pipe_prefix(fd, byte_count) {
        Ok(Some(bytes)) => {
            if bytes.is_empty() {
                return Some(0);
            }
            if !guest_memory.write(request.argument(1), &bytes) {
                return Some(linux_error(RISCV_LINUX_EFAULT));
            }
            if state.consume_guest_pipe_prefix(fd, bytes.len()).is_err() {
                return Some(linux_error(RISCV_LINUX_EBADF));
            }
            return Some(bytes.len() as u64);
        }
        Ok(None) => {}
        Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
    }
    let read_from_stdin = state.stdin_readable(fd);
    let bytes = if read_from_stdin {
        state.stdin_prefix(byte_count)
    } else {
        match state.guest_file_prefix(fd, byte_count) {
            Ok(Some(bytes)) => bytes,
            Ok(None) | Err(_) => return Some(linux_error(RISCV_LINUX_EBADF)),
        }
    };
    if bytes.is_empty() {
        return Some(0);
    }
    let read_count = bytes.len() as u64;
    let Ok(offset) = state.guest_fds.file_offset(fd) else {
        return Some(linux_error(RISCV_LINUX_EBADF));
    };
    if offset.get().checked_add(read_count).is_none() {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }

    if !guest_memory.write(request.argument(1), &bytes) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    if state.guest_fds.advance_file_offset(fd, read_count).is_err() {
        return Some(linux_error(RISCV_LINUX_EBADF));
    }
    if read_from_stdin {
        state.consume_stdin_prefix(bytes.len());
    }
    Some(read_count)
}

pub(super) fn syscall_pread64(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let offset = request.argument(3);
    if (offset as i64) < 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }
    match state.guest_file_fd_is_seekable(fd) {
        Ok(true) => {}
        Ok(false) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }

    let count = request.argument(2);
    if count == 0 {
        return 0;
    }
    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let bytes = match state.guest_file_slice_at(fd, offset, byte_count) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return linux_error(RISCV_LINUX_ESPIPE),
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    };
    if bytes.is_empty() {
        return 0;
    }
    if !guest_memory.write(request.argument(1), &bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    bytes.len() as u64
}
