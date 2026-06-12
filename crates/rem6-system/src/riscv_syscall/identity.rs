const RISCV_LINUX_SINGLE_PROCESS_ID: u64 = 100;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RiscvSyscallIdentity {
    thread_group_id: u64,
    thread_id: u64,
    parent_process_id: u64,
    user_id: u64,
    effective_user_id: u64,
    group_id: u64,
    effective_group_id: u64,
}

impl RiscvSyscallIdentity {
    pub(crate) const fn new(
        thread_group_id: u64,
        thread_id: u64,
        parent_process_id: u64,
        user_id: u64,
        effective_user_id: u64,
        group_id: u64,
        effective_group_id: u64,
    ) -> Self {
        Self {
            thread_group_id,
            thread_id,
            parent_process_id,
            user_id,
            effective_user_id,
            group_id,
            effective_group_id,
        }
    }

    pub(crate) const fn linux_single_process() -> Self {
        Self::new(
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            0,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
            RISCV_LINUX_SINGLE_PROCESS_ID,
        )
    }

    pub(crate) const fn thread_group_id(self) -> u64 {
        self.thread_group_id
    }

    pub(crate) const fn thread_id(self) -> u64 {
        self.thread_id
    }

    pub(crate) const fn parent_process_id(self) -> u64 {
        self.parent_process_id
    }

    pub(crate) const fn user_id(self) -> u64 {
        self.user_id
    }

    pub(crate) const fn effective_user_id(self) -> u64 {
        self.effective_user_id
    }

    pub(crate) const fn group_id(self) -> u64 {
        self.group_id
    }

    pub(crate) const fn effective_group_id(self) -> u64 {
        self.effective_group_id
    }
}
