use super::time::read_timespec64;
use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EAGAIN, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

const RISCV_LINUX_SIG_BLOCK: u64 = 0;
const RISCV_LINUX_SIG_UNBLOCK: u64 = 1;
const RISCV_LINUX_SIG_SETMASK: u64 = 2;
const RISCV_LINUX_SIGSET_BYTES: u64 = 8;
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
