use super::{guest_fd_argument, linux_error, RiscvSyscallState, RISCV_LINUX_EBADF};

pub(super) const RISCV_LINUX_SYNC: u64 = 81;
pub(super) const RISCV_LINUX_FSYNC: u64 = 82;
pub(super) const RISCV_LINUX_FDATASYNC: u64 = 83;
pub(super) const RISCV_LINUX_SYNCFS: u64 = 267;

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
