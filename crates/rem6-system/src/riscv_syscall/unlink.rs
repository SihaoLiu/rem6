use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_UNLINK: u64 = 1026;

pub(super) fn syscall_unlink(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_guest_c_string(guest_memory, request.argument(0), RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() || !state.unlink_guest_path(&path) {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    0
}
