pub(super) const RISCV_LINUX_EXIT: u64 = 93;
pub(super) const RISCV_LINUX_EXIT_GROUP: u64 = 94;

pub(super) fn syscall_exit_code(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}
