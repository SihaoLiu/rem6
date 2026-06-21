use super::permissions::apply_file_creation_mask;
use super::{
    guest_fd_argument, linux_error, read_guest_c_string, RiscvGuestCStringError,
    RiscvGuestMemoryReader, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT, RISCV_LINUX_ENAMETOOLONG,
    RISCV_LINUX_ENOENT, RISCV_LINUX_ENOTDIR, RISCV_LINUX_EPERM, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_MKNODAT: u64 = 33;
const RISCV_LINUX_S_IFMT: u64 = 0o170000;
const RISCV_LINUX_S_IFREG: u64 = 0o100000;

pub(super) fn syscall_mknodat(
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

    let mode = request.argument(2);
    let node_type = mode & RISCV_LINUX_S_IFMT;
    if node_type != 0 && node_type != RISCV_LINUX_S_IFREG {
        return linux_error(RISCV_LINUX_EPERM);
    }

    let path = match resolve_mknod_path(request.argument(0), &path, state) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if state.existing_guest_path_key(&path).is_some()
        || state.guest_directory_entries(&path).is_some()
    {
        return linux_error(RISCV_LINUX_EEXIST);
    }

    state.replace_guest_file_contents(&path, Vec::new());
    state.set_guest_file_permissions(&path, apply_file_creation_mask(mode, state));
    state.notify_guest_file_created(&path);
    0
}

fn resolve_mknod_path(dirfd: u64, path: &[u8], state: &RiscvSyscallState) -> Result<Vec<u8>, u64> {
    if dirfd == RISCV_LINUX_AT_FDCWD || path.starts_with(b"/") {
        return state
            .resolve_guest_path_following_intermediate_symlinks(path)
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
        .resolve_guest_path_from_directory_following_intermediate_symlinks(&directory, path)
        .map_err(|error| linux_error(error.linux_error_code()))
}
