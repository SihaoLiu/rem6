use super::RiscvSyscallState;

const RISCV_LINUX_MODE_PERMISSION_BITS: u32 = 0o777;

pub(super) const RISCV_LINUX_UMASK: u64 = 166;

pub(super) fn syscall_umask(mask: u64, state: &mut RiscvSyscallState) -> u64 {
    let next_mask = (mask as u32) & RISCV_LINUX_MODE_PERMISSION_BITS;
    u64::from(state.replace_file_creation_mask(next_mask))
}

pub(super) fn apply_file_creation_mask(mode: u64, state: &RiscvSyscallState) -> u32 {
    (mode as u32) & RISCV_LINUX_MODE_PERMISSION_BITS & !state.file_creation_mask()
}
