use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvGuestMkdirError, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_AT_FDCWD, RISCV_LINUX_EBADF, RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT,
    RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_ENOTDIR, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_MKDIRAT: u64 = 34;

pub(super) fn syscall_mkdirat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let path = match read_guest_c_string(guest_memory, request.argument(1), RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let path = match resolve_mkdir_path(request.argument(0), &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    match state.mkdir_guest_directory(&path, request.argument(2)) {
        Ok(()) => 0,
        Err(RiscvGuestMkdirError::Exists) => linux_error(RISCV_LINUX_EEXIST),
    }
}

fn resolve_mkdir_path(dirfd: u64, path: &[u8], state: &RiscvSyscallState) -> Result<Vec<u8>, u64> {
    if dirfd == RISCV_LINUX_AT_FDCWD || path.starts_with(b"/") {
        return state
            .resolve_guest_path(path)
            .map_err(|error| linux_error(error.linux_error_code()));
    }
    let Some(fd) = guest_fd_argument(dirfd) else {
        return Err(linux_error(RISCV_LINUX_EBADF));
    };
    let directory = match state.guest_directory_path_for_fd(fd) {
        Ok(Some(path)) => path,
        Ok(None) => return Err(linux_error(RISCV_LINUX_ENOTDIR)),
        Err(_error) => return Err(linux_error(RISCV_LINUX_EBADF)),
    };
    state
        .resolve_guest_path_from_directory(&directory, path)
        .map_err(|error| linux_error(error.linux_error_code()))
}
