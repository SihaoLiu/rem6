use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD, RISCV_LINUX_EBADF,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT,
    RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_UNLINKAT: u64 = 35;
pub(super) const RISCV_LINUX_UNLINK: u64 = 1026;

pub(super) fn syscall_unlink_operation(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    match request.number() {
        RISCV_LINUX_UNLINK => syscall_unlink(request, state, guest_memory),
        RISCV_LINUX_UNLINKAT => syscall_unlinkat(request, state, guest_memory),
        _ => unreachable!("unlink operation only handles unlink and unlinkat"),
    }
}

pub(super) fn syscall_unlink(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_unlink_path(request.argument(0), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    syscall_unlink_registered_path(path, state)
}

pub(super) fn syscall_unlinkat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if request.argument(2) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let path = match read_unlink_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if !dirfd_supports_path(request.argument(0), &path) {
        return linux_error(RISCV_LINUX_EBADF);
    }

    syscall_unlink_registered_path(path, state)
}

fn syscall_unlink_registered_path(path: Vec<u8>, state: &mut RiscvSyscallState) -> u64 {
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let path = match state.resolve_existing_guest_path(&path) {
        Ok(Some(path)) => path,
        Ok(None) => match state.resolve_guest_path(&path) {
            Ok(path) => path,
            Err(error) => return linux_error(error.linux_error_code()),
        },
        Err(error) => return linux_error(error.linux_error_code()),
    };
    if !state.unlink_guest_path(&path) {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    0
}

fn read_unlink_path(address: u64, guest_memory: &RiscvGuestMemoryReader) -> Result<Vec<u8>, u64> {
    read_guest_c_string(guest_memory, address, RISCV_LINUX_PATH_MAX).map_err(|error| {
        linux_error(match error {
            RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
            RiscvGuestCStringError::TooLong => RISCV_LINUX_ENAMETOOLONG,
        })
    })
}

fn dirfd_supports_path(dirfd: u64, path: &[u8]) -> bool {
    dirfd == RISCV_LINUX_AT_FDCWD || path.starts_with(b"/")
}
