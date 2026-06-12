use crate::{GuestFdError, GuestFileStatusFlags};

use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestMemoryReader,
    RiscvGuestOpenRequest, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EMFILE,
    RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_O_ACCMODE, RISCV_LINUX_O_CLOEXEC,
    RISCV_LINUX_O_RDONLY, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_OPEN: u64 = 1024;

pub(super) fn syscall_openat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    syscall_open_registered_path(
        request.argument(0),
        request.argument(1),
        request.argument(2),
        request.argument(3),
        state,
        guest_memory,
    )
}

pub(super) fn syscall_open(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    syscall_open_registered_path(
        RISCV_LINUX_AT_FDCWD,
        request.argument(0),
        request.argument(1),
        request.argument(2),
        state,
        guest_memory,
    )
}

fn syscall_open_registered_path(
    dirfd: u64,
    path_address: u64,
    flags: u64,
    mode: u64,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if dirfd != RISCV_LINUX_AT_FDCWD {
        return linux_error(RISCV_LINUX_EBADF);
    }

    if flags & !(RISCV_LINUX_O_ACCMODE | RISCV_LINUX_O_CLOEXEC) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if flags & RISCV_LINUX_O_ACCMODE != RISCV_LINUX_O_RDONLY {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let path = match read_guest_c_string(guest_memory, path_address, RISCV_LINUX_PATH_MAX) {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    if path.is_empty() || !state.guest_path_registered(&path) {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    let file_contents = state.guest_file_contents(&path).map(Vec::from);
    let status_flags = GuestFileStatusFlags::new((flags & !RISCV_LINUX_O_CLOEXEC) as u32);
    let close_on_exec = flags & RISCV_LINUX_O_CLOEXEC != 0;
    match state.open_guest_path(RiscvGuestOpenRequest {
        dirfd,
        path,
        flags,
        mode,
        status_flags,
        close_on_exec,
        file_contents,
    }) {
        Ok(fd) => u64::from(fd.get()),
        Err(GuestFdError::FdSpaceExhausted) => linux_error(RISCV_LINUX_EMFILE),
        Err(_error) => linux_error(RISCV_LINUX_EBADF),
    }
}
