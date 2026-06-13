const RISCV_LINUX_SINGLE_PROCESS_ID: u64 = 100;

pub(super) const RISCV_LINUX_GETPID: u64 = 172;
pub(super) const RISCV_LINUX_GETPPID: u64 = 173;
pub(super) const RISCV_LINUX_GETUID: u64 = 174;
pub(super) const RISCV_LINUX_GETEUID: u64 = 175;
pub(super) const RISCV_LINUX_GETGID: u64 = 176;
pub(super) const RISCV_LINUX_GETEGID: u64 = 177;
pub(super) const RISCV_LINUX_GETTID: u64 = 178;

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

pub(super) fn syscall_identity(number: u64, identity: RiscvSyscallIdentity) -> Option<u64> {
    match number {
        RISCV_LINUX_GETPID => Some(identity.thread_group_id()),
        RISCV_LINUX_GETPPID => Some(identity.parent_process_id()),
        RISCV_LINUX_GETTID => Some(identity.thread_id()),
        RISCV_LINUX_GETUID => Some(identity.user_id()),
        RISCV_LINUX_GETEUID => Some(identity.effective_user_id()),
        RISCV_LINUX_GETGID => Some(identity.group_id()),
        RISCV_LINUX_GETEGID => Some(identity.effective_group_id()),
        _ => None,
    }
}
