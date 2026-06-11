use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EPERM,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvRobustList {
    head: u64,
    length: u64,
}

impl RiscvRobustList {
    pub(super) const fn new(head: u64, length: u64) -> Self {
        Self { head, length }
    }
}

impl RiscvSyscallState {
    fn set_robust_list(&mut self, head: u64, length: u64) {
        self.robust_list = RiscvRobustList::new(head, length);
    }

    fn robust_list(&self) -> RiscvRobustList {
        self.robust_list
    }
}

pub(super) fn syscall_set_robust_list(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    state.set_robust_list(request.argument(0), request.argument(1));
    0
}

pub(super) fn syscall_get_robust_list(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let pid = request.argument(0);
    if pid != 0 && pid != state.identity().thread_id() {
        return linux_error(RISCV_LINUX_EPERM);
    }

    let robust_list = state.robust_list();
    if !guest_memory.write(request.argument(1), &robust_list.head.to_le_bytes()) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    if !guest_memory.write(request.argument(2), &robust_list.length.to_le_bytes()) {
        return linux_error(RISCV_LINUX_EFAULT);
    }
    0
}
