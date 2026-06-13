use crate::GuestProcessGroupId;

use super::{
    linux_error, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
    RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_SETPGID: u64 = 154;
pub(super) const RISCV_LINUX_GETPGID: u64 = 155;
pub(super) const RISCV_LINUX_GETSID: u64 = 156;
pub(super) const RISCV_LINUX_SETSID: u64 = 157;

impl RiscvSyscallState {
    pub(super) const fn session_id(&self) -> u64 {
        self.session_id
    }

    pub(super) fn set_session_id(&mut self, session_id: u64) {
        self.session_id = session_id;
    }
}

pub(super) fn syscall_setpgid(request: RiscvSyscallRequest, state: &mut RiscvSyscallState) -> u64 {
    let requested_group = match requested_process_group_argument(request.argument(1)) {
        Ok(process_group) => process_group,
        Err(error) => return linux_error(error),
    };
    let target_pid = match current_process_target(request.argument(0), state) {
        Ok(pid) => pid,
        Err(error) => return linux_error(error),
    };
    if target_pid == state.session_id() {
        return linux_error(RISCV_LINUX_EPERM);
    }

    let process_group = match requested_process_group(requested_group, target_pid, state) {
        Ok(process_group) => process_group,
        Err(error) => return linux_error(error),
    };

    state.guest_wait.set_current_process_group(process_group);
    0
}

pub(super) fn syscall_getpgid(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    if let Err(error) = current_process_query(request.argument(0), state) {
        return linux_error(error);
    };
    u64::from(state.guest_wait.current_process_group().get())
}

pub(super) fn syscall_getsid(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    if let Err(error) = current_process_query(request.argument(0), state) {
        return linux_error(error);
    };
    state.session_id()
}

pub(super) fn syscall_setsid(state: &mut RiscvSyscallState) -> u64 {
    let process_id = state.identity().thread_group_id();
    if u64::from(state.guest_wait.current_process_group().get()) == process_id {
        return linux_error(RISCV_LINUX_EPERM);
    }

    let Ok(process_group) = guest_process_group_id(process_id) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    state.guest_wait.set_current_process_group(process_group);
    state.set_session_id(process_id);
    process_id
}

fn current_process_query(argument: u64, state: &RiscvSyscallState) -> Result<(), u64> {
    let pid = linux_pid_argument(argument);
    if pid < 0 {
        return Err(RISCV_LINUX_ESRCH);
    }
    if pid == 0 || u64::try_from(pid).ok() == Some(state.identity().thread_group_id()) {
        return Ok(());
    }
    Err(RISCV_LINUX_ESRCH)
}

fn current_process_target(argument: u64, state: &RiscvSyscallState) -> Result<u64, u64> {
    let pid = linux_pid_argument(argument);
    if pid < 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    if pid == 0 {
        return Ok(state.identity().thread_group_id());
    }
    if u64::try_from(pid).ok() == Some(state.identity().thread_group_id()) {
        return Ok(pid as u64);
    }
    Err(RISCV_LINUX_ESRCH)
}

fn requested_process_group_argument(argument: u64) -> Result<u64, u64> {
    let process_group = linux_pid_argument(argument);
    if process_group < 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(process_group as u64)
}

fn requested_process_group(
    process_group: u64,
    target_pid: u64,
    state: &RiscvSyscallState,
) -> Result<GuestProcessGroupId, u64> {
    let process_group = if process_group == 0 {
        target_pid
    } else {
        process_group
    };
    if process_group != target_pid
        && process_group != u64::from(state.guest_wait.current_process_group().get())
    {
        return Err(RISCV_LINUX_EPERM);
    }
    guest_process_group_id(process_group).map_err(|_| RISCV_LINUX_EINVAL)
}

fn guest_process_group_id(process_group: u64) -> Result<GuestProcessGroupId, ()> {
    let process_group = u32::try_from(process_group).map_err(|_| ())?;
    GuestProcessGroupId::new(process_group).map_err(|_| ())
}

fn linux_pid_argument(argument: u64) -> i32 {
    argument as u32 as i32
}
