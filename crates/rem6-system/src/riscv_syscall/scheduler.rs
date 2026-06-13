use std::{cmp, mem};

use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ESRCH,
};

pub(super) const RISCV_LINUX_SCHED_GETSCHEDULER: u64 = 120;
pub(super) const RISCV_LINUX_SCHED_GETPARAM: u64 = 121;
pub(super) const RISCV_LINUX_SCHED_SETAFFINITY: u64 = 122;
pub(super) const RISCV_LINUX_SCHED_GETAFFINITY: u64 = 123;
pub(super) const RISCV_LINUX_SCHED_GET_PRIORITY_MAX: u64 = 125;
pub(super) const RISCV_LINUX_SCHED_GET_PRIORITY_MIN: u64 = 126;

const RISCV_LINUX_DEFAULT_SCHED_PRIORITY: i32 = 0;
const RISCV_LINUX_SCHED_OTHER: i32 = 0;
const RISCV_LINUX_SCHED_FIFO: i32 = 1;
const RISCV_LINUX_SCHED_RR: i32 = 2;
const RISCV_LINUX_SCHED_BATCH: i32 = 3;
const RISCV_LINUX_SCHED_IDLE: i32 = 5;
const RISCV_LINUX_SCHED_DEADLINE: i32 = 6;
const RISCV_LINUX_GUEST_CPU_IDS: u64 = 1;
const RISCV_LINUX_GUEST_AFFINITY_BYTES: u64 = mem::size_of::<u64>() as u64;
const RISCV_LINUX_GUEST_AFFINITY_BYTES_USIZE: usize = mem::size_of::<u64>();
const RISCV_LINUX_GUEST_AFFINITY_MASK: u64 = 1;
const RISCV_LINUX_BITS_PER_BYTE: u64 = u8::BITS as u64;

pub(super) fn syscall_sched_getscheduler(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
) -> u64 {
    let requested_pid = linux_int_argument(request.argument(0));
    if requested_pid < 0 {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    if !matches_current_process(requested_pid as u64, state) {
        return linux_error(RISCV_LINUX_ESRCH);
    }

    RISCV_LINUX_SCHED_OTHER as u64
}

pub(super) fn syscall_sched_get_priority_max(request: RiscvSyscallRequest) -> u64 {
    match scheduler_priority_range(request.argument(0)) {
        Some((_, maximum)) => maximum,
        None => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn syscall_sched_get_priority_min(request: RiscvSyscallRequest) -> u64 {
    match scheduler_priority_range(request.argument(0)) {
        Some((minimum, _)) => minimum,
        None => linux_error(RISCV_LINUX_EINVAL),
    }
}

pub(super) fn syscall_sched_getparam(
    request: RiscvSyscallRequest,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    if linux_int_argument(request.argument(0)) < 0 || request.argument(1) == 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let guest_memory_writer = guest_memory_writer?;
    if !guest_memory_writer.write(
        request.argument(1),
        &RISCV_LINUX_DEFAULT_SCHED_PRIORITY.to_le_bytes(),
    ) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    Some(0)
}

pub(super) fn syscall_sched_setaffinity(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let requested_size = request.argument(1);
    let read_bytes = cmp::min(requested_size, RISCV_LINUX_GUEST_AFFINITY_BYTES) as usize;
    let mut mask_bytes = [0; RISCV_LINUX_GUEST_AFFINITY_BYTES_USIZE];
    if read_bytes > 0 {
        let guest_memory_reader = guest_memory_reader?;
        let Some(bytes) = read_guest_exact(guest_memory_reader, request.argument(2), read_bytes)
        else {
            return Some(linux_error(RISCV_LINUX_EFAULT));
        };
        mask_bytes[..read_bytes].copy_from_slice(&bytes);
    }

    let requested_pid = request.argument(0);
    if !matches_current_process(requested_pid, state) {
        return Some(linux_error(RISCV_LINUX_ESRCH));
    }
    if requested_size < RISCV_LINUX_GUEST_AFFINITY_BYTES {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let requested_mask = u64::from_le_bytes(mask_bytes);
    if requested_mask & RISCV_LINUX_GUEST_AFFINITY_MASK == 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    Some(0)
}

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

fn linux_int_argument(argument: u64) -> i32 {
    argument as u32 as i32
}

fn scheduler_priority_range(policy_argument: u64) -> Option<(u64, u64)> {
    match linux_int_argument(policy_argument) {
        RISCV_LINUX_SCHED_OTHER
        | RISCV_LINUX_SCHED_BATCH
        | RISCV_LINUX_SCHED_IDLE
        | RISCV_LINUX_SCHED_DEADLINE => Some((0, 0)),
        RISCV_LINUX_SCHED_FIFO | RISCV_LINUX_SCHED_RR => Some((1, 99)),
        _ => None,
    }
}

fn read_guest_exact(
    guest_memory_reader: &RiscvGuestMemoryReader,
    address: u64,
    bytes: usize,
) -> Option<Vec<u8>> {
    guest_memory_reader
        .read(address, bytes)
        .filter(|read| read.len() == bytes)
}
