use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
};

pub(super) const RISCV_LINUX_PRLIMIT64: u64 = 261;

const RISCV_LINUX_RLIMIT_DATA: u64 = 2;
const RISCV_LINUX_RLIMIT_STACK: u64 = 3;
const RISCV_LINUX_RLIMIT_BYTES: usize = 16;
const RISCV_LINUX_STACK_LIMIT_BYTES: u64 = 8 * 1024 * 1024;
const RISCV_LINUX_DATA_LIMIT_BYTES: u64 = 256 * 1024 * 1024;

pub(super) fn syscall_prlimit64(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let pid = request.argument(0);
    if pid != 0 && pid != state.identity().thread_group_id() {
        return Some(linux_error(RISCV_LINUX_EPERM));
    }

    let Some((current, maximum)) = resource_limit(request.argument(1)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };

    let old_limit_address = request.argument(3);
    if old_limit_address == 0 {
        return Some(0);
    }

    let guest_memory = guest_memory?;
    let bytes = rlimit_bytes(current, maximum);
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(address) = old_limit_address.checked_add(offset as u64) else {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        };
        if !guest_memory.write(address, &[*byte]) {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        }
    }

    Some(0)
}

fn resource_limit(resource: u64) -> Option<(u64, u64)> {
    match resource {
        RISCV_LINUX_RLIMIT_STACK => {
            Some((RISCV_LINUX_STACK_LIMIT_BYTES, RISCV_LINUX_STACK_LIMIT_BYTES))
        }
        RISCV_LINUX_RLIMIT_DATA => {
            Some((RISCV_LINUX_DATA_LIMIT_BYTES, RISCV_LINUX_DATA_LIMIT_BYTES))
        }
        _ => None,
    }
}

fn rlimit_bytes(current: u64, maximum: u64) -> [u8; RISCV_LINUX_RLIMIT_BYTES] {
    let mut bytes = [0; RISCV_LINUX_RLIMIT_BYTES];
    bytes[0..8].copy_from_slice(&current.to_le_bytes());
    bytes[8..16].copy_from_slice(&maximum.to_le_bytes());
    bytes
}
