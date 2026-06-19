use super::{
    guest_fd_argument, linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EINVAL, RISCV_LINUX_ESPIPE, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_WRONLY,
};

pub(super) const RISCV_LINUX_SYNC: u64 = 81;
pub(super) const RISCV_LINUX_FSYNC: u64 = 82;
pub(super) const RISCV_LINUX_FDATASYNC: u64 = 83;
pub(super) const RISCV_LINUX_SYNC_FILE_RANGE: u64 = 84;
pub(super) const RISCV_LINUX_READAHEAD: u64 = 213;
pub(super) const RISCV_LINUX_SYNCFS: u64 = 267;

const RISCV_LINUX_SYNC_FILE_RANGE_VALID_FLAGS: u64 = 0x7;

pub(super) fn syscall_sync() -> u64 {
    0
}

pub(super) fn syscall_fd_sync(fd_argument: u64, state: &RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(fd_argument) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds().entry(fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    0
}

pub(super) fn syscall_sync_file_range(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds().entry(fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if request.argument(1) > i64::MAX as u64 || request.argument(2) > i64::MAX as u64 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if request.argument(3) & !RISCV_LINUX_SYNC_FILE_RANGE_VALID_FLAGS != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.guest_fd_is_pipe(fd) {
        Ok(true) => return linux_error(RISCV_LINUX_ESPIPE),
        Ok(false) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }
    0
}

pub(super) fn syscall_readahead(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(status_flags) = state.guest_fds().status_flags(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if status_flags.bits() & (RISCV_LINUX_O_ACCMODE as u32) == RISCV_LINUX_O_WRONLY as u32 {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if request.argument(1) > i64::MAX as u64 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.guest_file_fd_is_seekable(fd) {
        Ok(true) => 0,
        Ok(false) => linux_error(RISCV_LINUX_EINVAL),
        Err(_) => linux_error(RISCV_LINUX_EBADF),
    }
}
