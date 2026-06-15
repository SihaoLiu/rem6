use super::{
    guest_fd_argument, linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_FLOCK: u64 = 32;

const RISCV_LINUX_LOCK_SH: u64 = 1;
const RISCV_LINUX_LOCK_EX: u64 = 2;
const RISCV_LINUX_LOCK_NB: u64 = 4;
const RISCV_LINUX_LOCK_UN: u64 = 8;
const RISCV_LINUX_LOCK_BASE_MASK: u64 =
    RISCV_LINUX_LOCK_SH | RISCV_LINUX_LOCK_EX | RISCV_LINUX_LOCK_UN;
const RISCV_LINUX_LOCK_VALID_MASK: u64 = RISCV_LINUX_LOCK_BASE_MASK | RISCV_LINUX_LOCK_NB;

pub(super) fn syscall_flock(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if state.guest_fds.entry(fd).is_none() {
        return linux_error(RISCV_LINUX_EBADF);
    }

    let operation = request.argument(1);
    if !valid_flock_operation(operation) {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    0
}

fn valid_flock_operation(operation: u64) -> bool {
    operation & !RISCV_LINUX_LOCK_VALID_MASK == 0
        && matches!(
            operation & RISCV_LINUX_LOCK_BASE_MASK,
            RISCV_LINUX_LOCK_SH | RISCV_LINUX_LOCK_EX | RISCV_LINUX_LOCK_UN
        )
}
