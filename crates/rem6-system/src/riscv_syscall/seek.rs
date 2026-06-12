use crate::GuestFileOffset;

use super::{
    guest_fd_argument, linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EBADF,
    RISCV_LINUX_EINVAL, RISCV_LINUX_ESPIPE,
};

pub(super) const RISCV_LINUX_LSEEK: u64 = 62;

const RISCV_LINUX_SEEK_SET: u64 = 0;
const RISCV_LINUX_SEEK_CUR: u64 = 1;
const RISCV_LINUX_SEEK_END: u64 = 2;

pub(super) fn syscall_lseek(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let Some(fd) = guest_fd_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(current_offset) = state.guest_fds.file_offset(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(stat) = state.guest_fd_stat(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    let Ok(is_directory) = state.guest_fd_is_directory(fd) else {
        return linux_error(RISCV_LINUX_EBADF);
    };
    if !stat.is_regular_file() && !is_directory {
        return linux_error(RISCV_LINUX_ESPIPE);
    }

    let requested_offset = request.argument(1) as i64 as i128;
    let base = match request.argument(2) {
        RISCV_LINUX_SEEK_SET => 0,
        RISCV_LINUX_SEEK_CUR => i128::from(current_offset.get()),
        RISCV_LINUX_SEEK_END => {
            if is_directory {
                let Ok(Some(length)) = state.guest_directory_description_len(fd) else {
                    return linux_error(RISCV_LINUX_EBADF);
                };
                i128::from(length)
            } else {
                i128::from(stat.size())
            }
        }
        _ => return linux_error(RISCV_LINUX_EINVAL),
    };

    let Some(next_offset) = base.checked_add(requested_offset) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if next_offset < 0 || next_offset > i128::from(u64::MAX) {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let next_offset = next_offset as u64;
    if state
        .guest_fds
        .set_file_offset(fd, GuestFileOffset::new(next_offset))
        .is_err()
    {
        return linux_error(RISCV_LINUX_EBADF);
    }
    next_offset
}
