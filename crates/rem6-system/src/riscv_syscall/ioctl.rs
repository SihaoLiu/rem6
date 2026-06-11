use super::{
    guest_fd_argument, linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_ENOTTY,
};

pub(super) const RISCV_LINUX_IOCTL: u64 = 29;

pub(super) fn syscall_ioctl(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds().entry(fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    linux_error(RISCV_LINUX_ENOTTY)
}
