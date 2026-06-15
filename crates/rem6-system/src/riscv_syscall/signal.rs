use super::time::read_timespec64;
use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EAGAIN, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_ENOMEM, RISCV_LINUX_ENOSYS, RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_SIGALTSTACK: u64 = 132;

const RISCV_LINUX_SIG_BLOCK: u64 = 0;
const RISCV_LINUX_SIG_UNBLOCK: u64 = 1;
const RISCV_LINUX_SIG_SETMASK: u64 = 2;
const RISCV_LINUX_SIGSET_BYTES: u64 = 8;
const RISCV_LINUX_STACK_T_BYTES: usize = 24;
const RISCV_LINUX_MINSIGSTKSZ: u64 = 2048;
const RISCV_LINUX_SS_DISABLE: u64 = 2;
const RISCV_LINUX_SIGKILL_MASK: u64 = 1 << (9 - 1);
const RISCV_LINUX_SIGSTOP_MASK: u64 = 1 << (19 - 1);
const RISCV_LINUX_UNBLOCKABLE_SIGNALS: u64 = RISCV_LINUX_SIGKILL_MASK | RISCV_LINUX_SIGSTOP_MASK;
const RISCV_LINUX_SIGACTION_BYTES: usize = 24;
const RISCV_LINUX_FIRST_SIGNAL: u64 = 1;
const RISCV_LINUX_LAST_SIGNAL: u64 = 64;
const RISCV_LINUX_SIGKILL: u64 = 9;
const RISCV_LINUX_SIGSTOP: u64 = 19;
const RISCV_LINUX_SA_NOCLDSTOP: u64 = 0x0000_0001;
const RISCV_LINUX_SA_NOCLDWAIT: u64 = 0x0000_0002;
const RISCV_LINUX_SA_SIGINFO: u64 = 0x0000_0004;
const RISCV_LINUX_SA_EXPOSE_TAGBITS: u64 = 0x0000_0800;
const RISCV_LINUX_SA_ONSTACK: u64 = 0x0800_0000;
const RISCV_LINUX_SA_RESTART: u64 = 0x1000_0000;
const RISCV_LINUX_SA_NODEFER: u64 = 0x4000_0000;
const RISCV_LINUX_SA_RESETHAND: u64 = 0x8000_0000;
const RISCV_LINUX_UAPI_SA_FLAGS: u64 = RISCV_LINUX_SA_NOCLDSTOP
    | RISCV_LINUX_SA_NOCLDWAIT
    | RISCV_LINUX_SA_SIGINFO
    | RISCV_LINUX_SA_EXPOSE_TAGBITS
    | RISCV_LINUX_SA_ONSTACK
    | RISCV_LINUX_SA_RESTART
    | RISCV_LINUX_SA_NODEFER
    | RISCV_LINUX_SA_RESETHAND;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvSignalAction {
    handler: u64,
    flags: u64,
    mask: u64,
}

impl RiscvSignalAction {
    const fn new(handler: u64, flags: u64, mask: u64) -> Self {
        Self {
            handler,
            flags,
            mask,
        }
    }

    fn sanitized(self) -> Self {
        Self {
            handler: self.handler,
            flags: self.flags & RISCV_LINUX_UAPI_SA_FLAGS,
            mask: blockable_signal_mask(self.mask),
        }
    }

    fn to_guest_bytes(self) -> [u8; RISCV_LINUX_SIGACTION_BYTES] {
        let mut bytes = [0; RISCV_LINUX_SIGACTION_BYTES];
        bytes[0..8].copy_from_slice(&self.handler.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.flags.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.mask.to_le_bytes());
        bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvSignalAltStack {
    sp: u64,
    flags: u64,
    size: u64,
}

impl RiscvSignalAltStack {
    pub(super) const fn disabled() -> Self {
        Self {
            sp: 0,
            flags: RISCV_LINUX_SS_DISABLE,
            size: 0,
        }
    }

    const fn enabled(sp: u64, size: u64) -> Self {
        Self { sp, flags: 0, size }
    }

    fn from_guest_bytes(bytes: Vec<u8>) -> Option<Self> {
        if bytes.len() != RISCV_LINUX_STACK_T_BYTES {
            return None;
        }
        let sp = u64::from_le_bytes(bytes[0..8].try_into().ok()?);
        let flags = u64::from(u32::from_le_bytes(bytes[8..12].try_into().ok()?));
        let size = u64::from_le_bytes(bytes[16..24].try_into().ok()?);
        Some(match flags {
            0 => Self::enabled(sp, size),
            RISCV_LINUX_SS_DISABLE => Self::disabled(),
            _ => Self { sp, flags, size },
        })
    }

    fn validate(self) -> Result<Self, u64> {
        match self.flags {
            0 if self.size < RISCV_LINUX_MINSIGSTKSZ => Err(RISCV_LINUX_ENOMEM),
            0 => Ok(self),
            RISCV_LINUX_SS_DISABLE => Ok(Self::disabled()),
            _ => Err(RISCV_LINUX_EINVAL),
        }
    }

    fn to_guest_bytes(self) -> [u8; RISCV_LINUX_STACK_T_BYTES] {
        let mut bytes = [0; RISCV_LINUX_STACK_T_BYTES];
        bytes[0..8].copy_from_slice(&self.sp.to_le_bytes());
        bytes[8..12].copy_from_slice(&(self.flags as u32).to_le_bytes());
        bytes[16..24].copy_from_slice(&self.size.to_le_bytes());
        bytes
    }
}

impl RiscvSyscallState {
    pub(super) const fn signal_alt_stack(&self) -> RiscvSignalAltStack {
        self.signal_alt_stack
    }

    pub(super) fn set_signal_alt_stack(&mut self, alt_stack: RiscvSignalAltStack) {
        self.signal_alt_stack = alt_stack;
    }
}

pub(super) fn syscall_kill(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: rem6_kernel::Tick,
) -> u64 {
    let signal = linux_int_argument(request.argument(1));
    if signal != 0 && !valid_signal_i32(signal) {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let pid = linux_pid_argument(request.argument(0));
    if !kill_target_exists(pid, state) {
        return linux_error(RISCV_LINUX_ESRCH);
    }

    signal_probe_or_unimplemented_delivery(request, state, tick, signal)
}

pub(super) fn syscall_tkill(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: rem6_kernel::Tick,
) -> u64 {
    let signal = linux_int_argument(request.argument(1));
    let Some(tid) = positive_linux_pid_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    if tid != state.identity().thread_id() {
        return linux_error(RISCV_LINUX_ESRCH);
    }
    if signal != 0 && !valid_signal_i32(signal) {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    signal_probe_or_unimplemented_delivery(request, state, tick, signal)
}

pub(super) fn syscall_tgkill(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: rem6_kernel::Tick,
) -> u64 {
    let signal = linux_int_argument(request.argument(2));
    let Some(thread_group_id) = positive_linux_pid_argument(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let Some(thread_id) = positive_linux_pid_argument(request.argument(1)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    let identity = state.identity();
    if thread_group_id != identity.thread_group_id() || thread_id != identity.thread_id() {
        return linux_error(RISCV_LINUX_ESRCH);
    }
    if signal != 0 && !valid_signal_i32(signal) {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    signal_probe_or_unimplemented_delivery(request, state, tick, signal)
}

pub(super) fn syscall_rt_sigaction(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    if request.argument(3) != RISCV_LINUX_SIGSET_BYTES {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let signal = request.argument(0);
    let action_address = request.argument(1);
    let old_action_address = request.argument(2);
    let requested_action = if action_address == 0 {
        None
    } else {
        let guest_memory_reader = guest_memory_reader?;
        match read_signal_action(guest_memory_reader, action_address) {
            Some(action) => Some(action.sanitized()),
            None => return Some(linux_error(RISCV_LINUX_EFAULT)),
        }
    };

    if !valid_signal(signal)
        || requested_action.is_some() && matches!(signal, RISCV_LINUX_SIGKILL | RISCV_LINUX_SIGSTOP)
    {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let old_action = state.signal_action(signal);
    if let Some(action) = requested_action {
        state.set_signal_action(signal, action);
    }

    if old_action_address != 0 {
        let guest_memory_writer = guest_memory_writer?;
        if !guest_memory_writer.write(old_action_address, &old_action.to_guest_bytes()) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }
    Some(0)
}

pub(super) fn syscall_rt_sigprocmask(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    if request.argument(3) != RISCV_LINUX_SIGSET_BYTES {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let set_address = request.argument(1);
    let oldset_address = request.argument(2);
    let requested_mask = if set_address == 0 {
        None
    } else {
        let guest_memory_reader = guest_memory_reader?;
        match read_signal_mask(guest_memory_reader, set_address) {
            Some(mask) => Some(mask),
            None => return Some(linux_error(RISCV_LINUX_EFAULT)),
        }
    };

    let next_mask = match requested_mask {
        Some(mask) => match request.argument(0) {
            RISCV_LINUX_SIG_BLOCK => Some(state.signal_mask() | blockable_signal_mask(mask)),
            RISCV_LINUX_SIG_UNBLOCK => Some(state.signal_mask() & !blockable_signal_mask(mask)),
            RISCV_LINUX_SIG_SETMASK => Some(blockable_signal_mask(mask)),
            _ => return Some(linux_error(RISCV_LINUX_EINVAL)),
        },
        None => None,
    };

    let old_mask = state.signal_mask();
    if let Some(mask) = next_mask {
        state.set_signal_mask(mask);
    }

    if oldset_address != 0 {
        let guest_memory_writer = guest_memory_writer?;
        if !guest_memory_writer.write(oldset_address, &old_mask.to_le_bytes()) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }
    Some(0)
}

pub(super) fn syscall_rt_sigpending(
    request: RiscvSyscallRequest,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let sigsetsize = request.argument(1);
    if sigsetsize > RISCV_LINUX_SIGSET_BYTES {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let guest_memory_writer = guest_memory_writer?;
    let pending_mask = 0_u64.to_le_bytes();
    if !guest_memory_writer.write(request.argument(0), &pending_mask[..sigsetsize as usize]) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }
    Some(0)
}

pub(super) fn syscall_sigaltstack(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let stack_address = request.argument(0);
    let old_stack_address = request.argument(1);
    let requested_stack = if stack_address == 0 {
        None
    } else {
        let guest_memory_reader = guest_memory_reader?;
        let bytes = match guest_memory_reader.read(stack_address, RISCV_LINUX_STACK_T_BYTES) {
            Some(bytes) => bytes,
            None => return Some(linux_error(RISCV_LINUX_EFAULT)),
        };
        let stack = match RiscvSignalAltStack::from_guest_bytes(bytes) {
            Some(stack) => stack,
            None => return Some(linux_error(RISCV_LINUX_EFAULT)),
        };
        match stack.validate() {
            Ok(stack) => Some(stack),
            Err(error) => return Some(linux_error(error)),
        }
    };

    let old_stack = state.signal_alt_stack();
    if old_stack_address != 0 {
        let guest_memory_writer = guest_memory_writer?;
        if !guest_memory_writer.write(old_stack_address, &old_stack.to_guest_bytes()) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }
    if let Some(stack) = requested_stack {
        state.set_signal_alt_stack(stack);
    }
    Some(0)
}

pub(super) fn syscall_rt_sigtimedwait(
    request: RiscvSyscallRequest,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    if request.argument(3) != RISCV_LINUX_SIGSET_BYTES {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let guest_memory_reader = guest_memory_reader?;
    if read_signal_mask(guest_memory_reader, request.argument(0)).is_none() {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    let timeout_address = request.argument(2);
    if timeout_address == 0 {
        return None;
    }
    match read_timespec64(guest_memory_reader, timeout_address) {
        Some(timeout) if timeout.is_zero() => {}
        Some(timeout) if timeout.is_valid() => {
            return None;
        }
        Some(_) => return Some(linux_error(RISCV_LINUX_EINVAL)),
        None => return Some(linux_error(RISCV_LINUX_EFAULT)),
    }

    Some(linux_error(RISCV_LINUX_EAGAIN))
}

fn read_signal_mask(guest_memory_reader: &RiscvGuestMemoryReader, address: u64) -> Option<u64> {
    let bytes = guest_memory_reader.read(address, RISCV_LINUX_SIGSET_BYTES as usize)?;
    let bytes: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn blockable_signal_mask(mask: u64) -> u64 {
    mask & !RISCV_LINUX_UNBLOCKABLE_SIGNALS
}

fn valid_signal(signal: u64) -> bool {
    (RISCV_LINUX_FIRST_SIGNAL..=RISCV_LINUX_LAST_SIGNAL).contains(&signal)
}

fn valid_signal_i32(signal: i32) -> bool {
    signal > 0 && (signal as u64) <= RISCV_LINUX_LAST_SIGNAL
}

fn linux_pid_argument(argument: u64) -> i32 {
    argument as u32 as i32
}

fn linux_int_argument(argument: u64) -> i32 {
    argument as u32 as i32
}

fn positive_linux_pid_argument(argument: u64) -> Option<u64> {
    u64::try_from(linux_pid_argument(argument))
        .ok()
        .filter(|pid| *pid > 0)
}

fn kill_target_exists(pid: i32, state: &RiscvSyscallState) -> bool {
    let current_process = state.identity().thread_group_id();
    if pid > 0 {
        return u64::try_from(pid).ok() == Some(current_process);
    }
    if pid == 0 || pid == -1 {
        return true;
    }
    pid.checked_abs()
        .and_then(|process_group| u64::try_from(process_group).ok())
        == Some(current_process)
}

fn signal_probe_or_unimplemented_delivery(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: rem6_kernel::Tick,
    signal: i32,
) -> u64 {
    if signal == 0 {
        return 0;
    }

    state.push_unknown_syscall(super::RiscvUnknownSyscallRecord::new(
        request.pc(),
        request.number(),
        request.arguments(),
        tick,
    ));
    linux_error(RISCV_LINUX_ENOSYS)
}

fn read_signal_action(
    guest_memory_reader: &RiscvGuestMemoryReader,
    address: u64,
) -> Option<RiscvSignalAction> {
    let bytes = guest_memory_reader.read(address, RISCV_LINUX_SIGACTION_BYTES)?;
    if bytes.len() != RISCV_LINUX_SIGACTION_BYTES {
        return None;
    }
    Some(RiscvSignalAction::new(
        u64::from_le_bytes(bytes[0..8].try_into().ok()?),
        u64::from_le_bytes(bytes[8..16].try_into().ok()?),
        u64::from_le_bytes(bytes[16..24].try_into().ok()?),
    ))
}
