use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

const RISCV_LINUX_SIG_BLOCK: u64 = 0;
const RISCV_LINUX_SIG_UNBLOCK: u64 = 1;
const RISCV_LINUX_SIG_SETMASK: u64 = 2;
const RISCV_LINUX_SIGSET_BYTES: u64 = 8;
const RISCV_LINUX_SIGKILL_MASK: u64 = 1 << (9 - 1);
const RISCV_LINUX_SIGSTOP_MASK: u64 = 1 << (19 - 1);
const RISCV_LINUX_UNBLOCKABLE_SIGNALS: u64 = RISCV_LINUX_SIGKILL_MASK | RISCV_LINUX_SIGSTOP_MASK;

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

fn read_signal_mask(guest_memory_reader: &RiscvGuestMemoryReader, address: u64) -> Option<u64> {
    let bytes = guest_memory_reader.read(address, RISCV_LINUX_SIGSET_BYTES as usize)?;
    let bytes: [u8; 8] = bytes.try_into().ok()?;
    Some(u64::from_le_bytes(bytes))
}

fn blockable_signal_mask(mask: u64) -> u64 {
    mask & !RISCV_LINUX_UNBLOCKABLE_SIGNALS
}
