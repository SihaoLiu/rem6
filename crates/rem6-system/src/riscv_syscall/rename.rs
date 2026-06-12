use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvGuestRenameError, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_RENAMEAT2: u64 = 276;

pub(super) fn syscall_renameat2(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if request.argument(4) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let source = match read_rename_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let destination = match read_rename_path(request.argument(3), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if !dirfd_supports_path(request.argument(0), &source)
        || !dirfd_supports_path(request.argument(2), &destination)
    {
        return linux_error(RISCV_LINUX_EBADF);
    }
    if source.is_empty() || destination.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let source = match state.resolve_existing_guest_path(&source) {
        Ok(Some(path)) => path,
        Ok(None) => return linux_error(RISCV_LINUX_ENOENT),
        Err(error) => return linux_error(error.linux_error_code()),
    };
    let destination = state.resolve_guest_path_for_create(&destination);

    match state.rename_guest_path(&source, &destination) {
        Ok(()) => 0,
        Err(RiscvGuestRenameError::SourceMissing) => linux_error(RISCV_LINUX_ENOENT),
    }
}

fn read_rename_path(address: u64, guest_memory: &RiscvGuestMemoryReader) -> Result<Vec<u8>, u64> {
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
