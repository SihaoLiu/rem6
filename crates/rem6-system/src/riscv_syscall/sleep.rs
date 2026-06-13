use super::time::read_timespec64;
use super::{
    linux_error, RiscvGuestMemoryReader, RiscvSyscallRequest, RISCV_LINUX_EFAULT,
    RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_NANOSLEEP: u64 = 101;

pub(super) fn syscall_nanosleep(
    request: RiscvSyscallRequest,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let guest_memory_reader = guest_memory_reader?;
    let requested = match read_timespec64(guest_memory_reader, request.argument(0)) {
        Some(timespec) => timespec,
        None => return Some(linux_error(RISCV_LINUX_EFAULT)),
    };
    if !requested.is_valid() {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if requested.is_zero() {
        return Some(0);
    }

    None
}
