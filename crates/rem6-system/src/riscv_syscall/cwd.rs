use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_ERANGE,
};

pub(super) fn syscall_getcwd(
    address: u64,
    size: u64,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let cwd = state.current_directory();
    if cwd.len() as u64 >= size {
        return linux_error(RISCV_LINUX_ERANGE);
    }

    for offset in 0..size {
        let Some(byte_address) = address.checked_add(offset) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let byte = usize::try_from(offset)
            .ok()
            .and_then(|index| cwd.get(index))
            .copied()
            .unwrap_or(0);
        if !guest_memory.write(byte_address, std::slice::from_ref(&byte)) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    cwd.len() as u64
}
