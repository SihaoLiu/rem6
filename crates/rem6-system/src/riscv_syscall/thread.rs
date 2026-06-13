use super::RiscvSyscallState;

pub(super) const RISCV_LINUX_SET_TID_ADDRESS: u64 = 96;

pub(super) fn syscall_set_tid_address(
    clear_tid_address: u64,
    state: &mut RiscvSyscallState,
) -> u64 {
    state.set_child_clear_tid(clear_tid_address);
    state.identity().thread_id()
}
