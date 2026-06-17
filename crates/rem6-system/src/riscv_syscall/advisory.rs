use super::{
    guest_fd_argument, linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EINVAL, RISCV_LINUX_ESPIPE,
};

pub(super) const RISCV_LINUX_FADVISE64: u64 = 223;

const RISCV_LINUX_POSIX_FADV_NORMAL: u64 = 0;
const RISCV_LINUX_POSIX_FADV_RANDOM: u64 = 1;
const RISCV_LINUX_POSIX_FADV_SEQUENTIAL: u64 = 2;
const RISCV_LINUX_POSIX_FADV_WILLNEED: u64 = 3;
const RISCV_LINUX_POSIX_FADV_DONTNEED: u64 = 4;
const RISCV_LINUX_POSIX_FADV_NOREUSE: u64 = 5;

pub(super) fn syscall_fadvise64(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds().entry(fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if signed_argument_is_negative(request.argument(1))
        || signed_argument_is_negative(request.argument(2))
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if !valid_fadvise64_advice(request.argument(3)) {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match state.guest_fd_is_pipe(fd) {
        Ok(true) => return linux_error(RISCV_LINUX_ESPIPE),
        Ok(false) => {}
        Err(_) => return linux_error(RISCV_LINUX_EBADF),
    }

    0
}

const fn signed_argument_is_negative(value: u64) -> bool {
    value > i64::MAX as u64
}

const fn valid_fadvise64_advice(advice: u64) -> bool {
    matches!(
        advice,
        RISCV_LINUX_POSIX_FADV_NORMAL
            | RISCV_LINUX_POSIX_FADV_RANDOM
            | RISCV_LINUX_POSIX_FADV_SEQUENTIAL
            | RISCV_LINUX_POSIX_FADV_WILLNEED
            | RISCV_LINUX_POSIX_FADV_DONTNEED
            | RISCV_LINUX_POSIX_FADV_NOREUSE
    )
}
