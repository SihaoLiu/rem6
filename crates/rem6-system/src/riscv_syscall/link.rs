use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestLinkError,
    RiscvGuestMemoryReader, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_AT_FDCWD,
    RISCV_LINUX_EBADF, RISCV_LINUX_EEXIST, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_EPERM, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_LINKAT: u64 = 37;
pub(super) const RISCV_LINUX_LINK: u64 = 1025;

pub(super) fn syscall_link_operation(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    match request.number() {
        RISCV_LINUX_LINK => syscall_link(request, state, guest_memory),
        RISCV_LINUX_LINKAT => syscall_linkat(request, state, guest_memory),
        _ => unreachable!("link operation only handles link and linkat"),
    }
}

pub(super) fn syscall_link(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let source = match read_link_path(request.argument(0), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let destination = match read_link_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    syscall_link_registered_paths(source, destination, state)
}

pub(super) fn syscall_linkat(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if request.argument(4) != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let source = match read_link_path(request.argument(1), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    let destination = match read_link_path(request.argument(3), guest_memory) {
        Ok(path) => path,
        Err(error) => return error,
    };
    if !dirfd_supports_path(request.argument(0), &source)
        || !dirfd_supports_path(request.argument(2), &destination)
    {
        return linux_error(RISCV_LINUX_EBADF);
    }

    syscall_link_registered_paths(source, destination, state)
}

fn syscall_link_registered_paths(
    source: Vec<u8>,
    destination: Vec<u8>,
    state: &mut RiscvSyscallState,
) -> u64 {
    if source.is_empty() || destination.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }
    let source = match state.resolve_guest_path(&source) {
        Ok(path) => path,
        Err(error) => return linux_error(error.linux_error_code()),
    };
    if state.guest_directory_entries(&source).is_some() {
        return linux_error(RISCV_LINUX_EPERM);
    }
    let Some(source) = state.existing_guest_path_key(&source) else {
        return linux_error(RISCV_LINUX_ENOENT);
    };
    let destination = state.resolve_guest_path_for_create(&destination);

    match state.link_guest_path(&source, &destination) {
        Ok(()) => 0,
        Err(RiscvGuestLinkError::SourceMissing) => linux_error(RISCV_LINUX_ENOENT),
        Err(RiscvGuestLinkError::SourceIsDirectory) => linux_error(RISCV_LINUX_EPERM),
        Err(RiscvGuestLinkError::DestinationExists) => linux_error(RISCV_LINUX_EEXIST),
    }
}

fn read_link_path(address: u64, guest_memory: &RiscvGuestMemoryReader) -> Result<Vec<u8>, u64> {
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
