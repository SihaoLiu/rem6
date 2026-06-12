use super::{
    linux_error, read_guest_c_string, RiscvGuestCStringError, RiscvGuestLinkError,
    RiscvGuestMemoryReader, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EEXIST,
    RISCV_LINUX_EFAULT, RISCV_LINUX_ENAMETOOLONG, RISCV_LINUX_ENOENT, RISCV_LINUX_PATH_MAX,
};

pub(super) const RISCV_LINUX_LINK: u64 = 1025;

pub(super) fn syscall_link(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let source = match read_guest_c_string(guest_memory, request.argument(0), RISCV_LINUX_PATH_MAX)
    {
        Ok(path) => path,
        Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
        Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
    };
    let destination =
        match read_guest_c_string(guest_memory, request.argument(1), RISCV_LINUX_PATH_MAX) {
            Ok(path) => path,
            Err(RiscvGuestCStringError::Fault) => return linux_error(RISCV_LINUX_EFAULT),
            Err(RiscvGuestCStringError::TooLong) => return linux_error(RISCV_LINUX_ENAMETOOLONG),
        };
    if source.is_empty() || destination.is_empty() {
        return linux_error(RISCV_LINUX_ENOENT);
    }

    match state.link_guest_path(&source, &destination) {
        Ok(()) => 0,
        Err(RiscvGuestLinkError::SourceMissing) => linux_error(RISCV_LINUX_ENOENT),
        Err(RiscvGuestLinkError::DestinationExists) => linux_error(RISCV_LINUX_EEXIST),
    }
}
