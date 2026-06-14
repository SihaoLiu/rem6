use super::{
    guest_fd_argument, linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
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
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds.status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let count = request.argument(2);
    if count == 0 {
        return 0;
    }

    let Ok(byte_count) = usize::try_from(count) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    match state.guest_pipe_prefix(fd, byte_count) {
        Ok(Some(bytes)) => {
            if bytes.is_empty() {
                return 0;
            }
            if !guest_memory.write(request.argument(1), &bytes) {
                return linux_error(RISCV_LINUX_EFAULT);
            }
            if state.consume_guest_pipe_prefix(fd, bytes.len()).is_err() {
                return linux_error(RISCV_LINUX_EBADF);
            }
            return bytes.len() as u64;
        }
        Ok(None) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    let read_from_stdin = state.stdin_readable(fd);
    let bytes = if read_from_stdin {
        state.stdin_prefix(byte_count)
    } else {
        match state.guest_file_prefix(fd, byte_count) {
            Ok(Some(bytes)) => bytes,
            Ok(None) | Err(_) => return linux_error(RISCV_LINUX_EBADF),
        }
    };
    if bytes.is_empty() {
        return 0;
    }
    let read_count = bytes.len() as u64;
    let Ok(offset) = state.guest_fds.file_offset(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if offset.get().checked_add(read_count).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    if !guest_memory.write(request.argument(1), &bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    if state.guest_fds.advance_file_offset(fd, read_count).is_err() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if read_from_stdin {
        state.consume_stdin_prefix(bytes.len());
    }
    read_count
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
