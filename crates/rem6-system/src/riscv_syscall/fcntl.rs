use crate::GuestFileStatusFlags;

use super::{
    guest_fd_argument, linux_error, RiscvSyscallOutcome, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EBADF, RISCV_LINUX_O_APPEND, RISCV_LINUX_O_NONBLOCK,
};

pub(super) const RISCV_LINUX_FCNTL: u64 = 25;
pub(super) const RISCV_LINUX_F_GETFD: u64 = 1;
pub(super) const RISCV_LINUX_F_SETFD: u64 = 2;
pub(super) const RISCV_LINUX_F_GETFL: u64 = 3;
pub(super) const RISCV_LINUX_F_SETFL: u64 = 4;
pub(super) const RISCV_LINUX_FD_CLOEXEC: u64 = 1;

pub(super) fn syscall_fcntl(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> Option<RiscvSyscallOutcome> {
    let command = request.argument(1);
    if !matches!(
        command,
        RISCV_LINUX_F_GETFD | RISCV_LINUX_F_SETFD | RISCV_LINUX_F_GETFL | RISCV_LINUX_F_SETFL
    ) {
        return None;
    }

    let fd = match guest_fd_argument(request.argument(0)) {
        Some(fd) => fd,
        None => return Some(guest_fd_error_return()),
    };

    let outcome = match command {
        RISCV_LINUX_F_GETFD => state
            .guest_fds
            .close_on_exec(fd)
            .map(|close| u64::from(close) * RISCV_LINUX_FD_CLOEXEC),
        RISCV_LINUX_F_SETFD => state
            .guest_fds
            .set_close_on_exec(fd, request.argument(2) & RISCV_LINUX_FD_CLOEXEC != 0)
            .map(|()| 0),
        RISCV_LINUX_F_GETFL => state
            .guest_fds
            .status_flags(fd)
            .map(|flags| u64::from(flags.bits())),
        RISCV_LINUX_F_SETFL => {
            let current = match state.guest_fds.status_flags(fd) {
                Ok(flags) => flags,
                Err(_error) => return Some(guest_fd_error_return()),
            };
            let requested = request.argument(2) as u32;
            let mutable_flags = (RISCV_LINUX_O_APPEND | RISCV_LINUX_O_NONBLOCK) as u32;
            state
                .guest_fds
                .set_status_flags(
                    fd,
                    GuestFileStatusFlags::new(
                        (current.bits() & !mutable_flags) | (requested & mutable_flags),
                    ),
                )
                .map(|()| 0)
        }
        _ => return None,
    };

    Some(match outcome {
        Ok(value) => RiscvSyscallOutcome::Return { value },
        Err(_error) => guest_fd_error_return(),
    })
}

fn guest_fd_error_return() -> RiscvSyscallOutcome {
    RiscvSyscallOutcome::Return {
        value: linux_error(RISCV_LINUX_EBADF),
    }
}
