use super::{
    linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EINVAL, RISCV_LINUX_ENOSYS,
    RISCV_LINUX_EPERM,
};

pub(super) const RISCV_LINUX_SET_TID_ADDRESS: u64 = 96;
pub(super) const RISCV_LINUX_MEMBARRIER: u64 = 283;
pub(super) const RISCV_LINUX_RSEQ: u64 = 293;

const RISCV_LINUX_MEMBARRIER_CMD_QUERY: u64 = 0;
const RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED: u64 = 1 << 3;
const RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED: u64 = 1 << 4;
const RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS: u64 = 1 << 9;
const RISCV_LINUX_MEMBARRIER_SUPPORTED_COMMANDS: u64 = RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED
    | RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED
    | RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS;

impl RiscvSyscallState {
    pub(super) const fn membarrier_registrations(&self) -> u64 {
        self.membarrier_registrations
    }

    pub(super) fn register_membarrier_command(&mut self, command: u64) {
        self.membarrier_registrations |= command;
    }
}

pub(super) fn syscall_thread(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    match request.number() {
        RISCV_LINUX_SET_TID_ADDRESS => syscall_set_tid_address(request.argument(0), state),
        RISCV_LINUX_MEMBARRIER => syscall_membarrier(request, state),
        RISCV_LINUX_RSEQ => linux_error(RISCV_LINUX_ENOSYS),
        _ => unreachable!("RISC-V Linux thread syscall is handled by caller"),
    }
}

pub(super) fn syscall_set_tid_address(
    clear_tid_address: u64,
    state: &mut RiscvSyscallState,
) -> u64 {
    state.set_child_clear_tid(clear_tid_address);
    state.identity().thread_id()
}

fn syscall_membarrier(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let command = request.argument(0);
    let flags = request.argument(1);
    if flags != 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    match command {
        RISCV_LINUX_MEMBARRIER_CMD_QUERY => RISCV_LINUX_MEMBARRIER_SUPPORTED_COMMANDS,
        RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED => {
            state.register_membarrier_command(command);
            0
        }
        RISCV_LINUX_MEMBARRIER_CMD_PRIVATE_EXPEDITED => {
            if state.membarrier_registrations()
                & RISCV_LINUX_MEMBARRIER_CMD_REGISTER_PRIVATE_EXPEDITED
                == 0
            {
                linux_error(RISCV_LINUX_EPERM)
            } else {
                0
            }
        }
        RISCV_LINUX_MEMBARRIER_CMD_GET_REGISTRATIONS => state.membarrier_registrations(),
        _ => linux_error(RISCV_LINUX_EINVAL),
    }
}
