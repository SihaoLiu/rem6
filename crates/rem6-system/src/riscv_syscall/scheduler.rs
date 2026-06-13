use std::{cmp, mem};

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_SCHED_GETAFFINITY: u64 = 123;

const RISCV_LINUX_GUEST_CPU_IDS: u64 = 1;
const RISCV_LINUX_GUEST_AFFINITY_BYTES: u64 = mem::size_of::<u64>() as u64;
const RISCV_LINUX_GUEST_AFFINITY_MASK: u64 = 1;
const RISCV_LINUX_BITS_PER_BYTE: u64 = u8::BITS as u64;

pub(super) fn syscall_sched_getaffinity(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let requested_size = request.argument(1);
    if requested_size
        .checked_mul(RISCV_LINUX_BITS_PER_BYTE)
        .is_none_or(|bits| bits < RISCV_LINUX_GUEST_CPU_IDS)
        || !requested_size.is_multiple_of(RISCV_LINUX_GUEST_AFFINITY_BYTES)
    {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let requested_pid = request.argument(0);
    if !matches_current_process(requested_pid, state) {
        return Some(linux_error(RISCV_LINUX_ESRCH));
    }

    let guest_memory_writer = guest_memory_writer?;
    let written_bytes = cmp::min(requested_size, RISCV_LINUX_GUEST_AFFINITY_BYTES);
    if !guest_memory_writer.write(
        request.argument(2),
        &RISCV_LINUX_GUEST_AFFINITY_MASK.to_le_bytes()[..written_bytes as usize],
    ) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    Some(written_bytes)
}

fn matches_current_process(requested_pid: u64, state: &RiscvSyscallState) -> bool {
    requested_pid == 0
        || requested_pid == state.identity().thread_id()
        || requested_pid == state.identity().thread_group_id()
}
