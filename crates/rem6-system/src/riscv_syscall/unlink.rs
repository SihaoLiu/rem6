use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvGuestRmdirError, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_AT_FDCWD, RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EISDIR, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_ENOTDIR,
    RISCV_LINUX_ENOTEMPTY, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_UNLINKAT: u64 = 35;
pub(super) const RISCV_LINUX_UNLINK: u64 = 1026;

const RISCV_LINUX_AT_REMOVEDIR: u64 = 0x200;

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
    let flags = request.argument(2);
    if flags & !RISCV_LINUX_AT_REMOVEDIR != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    let path = match read_unlink_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if flags & RISCV_LINUX_AT_REMOVEDIR != 0 {
        return syscall_rmdir_registered_path_at(request.argument(0), path, state);
    }

    syscall_unlink_registered_path_at(request.argument(0), path, state)
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
    if state.guest_directory_entries(&path).is_some() {
        return linux_error(RISCV_LINUX_EISDIR);
    }
    if !state.unlink_guest_path(&path) {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    0
}

fn syscall_unlink_registered_path_at(
    dirfd: u64,
    path: Vec<u8>,
    state: &mut RiscvSyscallState,
) -> u64 {
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let path = match resolve_unlink_path_at(dirfd, &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let path = state.existing_guest_path_key(&path).unwrap_or(path);
    if state.guest_directory_entries(&path).is_some() {
        return linux_error(RISCV_LINUX_EISDIR);
    }
    if !state.unlink_guest_path(&path) {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    0
}

fn syscall_rmdir_registered_path_at(
    dirfd: u64,
    path: Vec<u8>,
    state: &mut RiscvSyscallState,
) -> u64 {
    if path.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let path = match resolve_unlink_path_at(dirfd, &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    match state.rmdir_guest_directory(&path) {
        Ok(()) => 0,
        Err(RiscvGuestRmdirError::Missing) => linux_error(RISCV_LINUX_ENOENT),
        Err(RiscvGuestRmdirError::NotDirectory) => linux_error(RISCV_LINUX_ENOTDIR),
        Err(RiscvGuestRmdirError::NotEmpty) => linux_error(RISCV_LINUX_ENOTEMPTY),
    }
}

fn read_unlink_path(address: u64, guest_memory: &RiscvGuestMemoryReader) -> Result<Vec<u8>, u64> {
    read_guest_c_string(guest_memory, address, RISCV_LINUX_PATH_MAX).map_err(|error| {
        linux_error(match error {
            RiscvGuestCStringError::Fault => RISCV_LINUX_EFAULT,
            RiscvGuestCStringError::TooLong => RISCV_LINUX_ENAMETOOLONG,
        })
    })
}

fn resolve_unlink_path_at(
    dirfd: u64,
    path: &[u8],
    state: &RiscvSyscallState,
) -> Result<Vec<u8>, u64> {
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
