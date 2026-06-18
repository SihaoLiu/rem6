use crate::GuestProcessGroupId;

use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
    RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_SETPGID: u64 = 154;
pub(super) const RISCV_LINUX_GETPGID: u64 = 155;
pub(super) const RISCV_LINUX_GETSID: u64 = 156;
pub(super) const RISCV_LINUX_SETSID: u64 = 157;
pub(super) const RISCV_LINUX_PRCTL: u64 = 167;
pub(super) const RISCV_LINUX_PERSONALITY: u64 = 92;

const RISCV_LINUX_PERSONALITY_QUERY: u32 = 0xffff_ffff;
const RISCV_LINUX_PR_SET_NAME: u64 = 15;
const RISCV_LINUX_PR_GET_NAME: u64 = 16;
const RISCV_LINUX_PR_SET_NO_NEW_PRIVS: u64 = 38;
const RISCV_LINUX_PR_GET_NO_NEW_PRIVS: u64 = 39;
const RISCV_LINUX_TASK_COMM_BYTES: usize = 16;

impl RiscvSyscallState {
    pub(super) const fn session_id(&self) -> u64 {
        self.session_id
    }

    pub(super) fn set_session_id(&mut self, session_id: u64) {
        self.session_id = session_id;
    }

    pub(super) const fn process_name(&self) -> [u8; RISCV_LINUX_TASK_COMM_BYTES] {
        self.process_name
    }

    pub(super) fn set_process_name(&mut self, name: [u8; RISCV_LINUX_TASK_COMM_BYTES]) {
        self.process_name = name;
    }

    pub(super) const fn no_new_privs(&self) -> bool {
        self.no_new_privs
    }

    pub(super) fn set_no_new_privs(&mut self) {
        self.no_new_privs = true;
    }

    pub(super) const fn personality(&self) -> u32 {
        self.personality
    }

    pub(super) fn set_personality(&mut self, personality: u32) {
        self.personality = personality;
    }
}

pub(super) fn syscall_personality(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let requested = request.argument(0) as u32;
    let previous = state.personality();
    if requested != RISCV_LINUX_PERSONALITY_QUERY {
        state.set_personality(requested);
    }
    u64::from(previous)
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

pub(super) fn syscall_prctl(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    match request.argument(0) {
        RISCV_LINUX_PR_SET_NAME => guest_memory_reader
            .map(|guest_memory| syscall_prctl_set_name(request.argument(1), state, guest_memory)),
        RISCV_LINUX_PR_GET_NAME => guest_memory_writer
            .map(|guest_memory| syscall_prctl_get_name(request.argument(1), state, guest_memory)),
        RISCV_LINUX_PR_SET_NO_NEW_PRIVS => Some(syscall_prctl_set_no_new_privs(request, state)),
        RISCV_LINUX_PR_GET_NO_NEW_PRIVS => Some(syscall_prctl_get_no_new_privs(request, state)),
        _ => Some(linux_error(RISCV_LINUX_EINVAL)),
    }
}

fn syscall_prctl_set_no_new_privs(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    if request.argument(1) != 1
        || request.argument(2) != 0
        || request.argument(3) != 0
        || request.argument(4) != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    state.set_no_new_privs();
    0
}

fn syscall_prctl_get_no_new_privs(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    if request.argument(1) != 0
        || request.argument(2) != 0
        || request.argument(3) != 0
        || request.argument(4) != 0
    {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    u64::from(state.no_new_privs())
}

fn syscall_prctl_set_name(
    address: u64,
    state: &mut RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    let mut name = [0; RISCV_LINUX_TASK_COMM_BYTES];
    for (index, byte) in name
        .iter_mut()
        .enumerate()
        .take(RISCV_LINUX_TASK_COMM_BYTES - 1)
    {
        let Some(address) = address.checked_add(index as u64) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let Some(bytes) = guest_memory.read(address, 1) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        let Some(value) = bytes.first().copied() else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        if value == 0 {
            break;
        }
        *byte = value;
    }
    state.set_process_name(name);
    0
}

fn syscall_prctl_get_name(
    address: u64,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    if guest_memory.write(address, &state.process_name()) {
        0
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    }
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
