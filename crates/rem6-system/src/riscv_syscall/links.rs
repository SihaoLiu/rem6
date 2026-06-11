use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_PATH_MAX,
};

pub(super) fn syscall_readlinkat(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> u64 {
    if request.argument(0) != RISCV_LINUX_AT_FDCWD {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if request.argument(3) == 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_guest_c_string(
        guest_memory_reader,
        request.argument(1),
        RISCV_LINUX_PATH_MAX,
    ) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    let Ok(buffer_bytes) = usize::try_from(request.argument(3)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let Some(target) = state.guest_link_target(&path) else {
        return linux_error(RISCV_LINUX_ENOENT);
    };
    let bytes = &target[..target.len().min(buffer_bytes)];
    if bytes.is_empty() {
        return 0;
    }
    if !guest_memory_writer.write(request.argument(2), bytes) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    bytes.len() as u64
}
