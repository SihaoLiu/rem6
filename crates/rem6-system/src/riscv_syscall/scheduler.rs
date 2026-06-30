use std::{cmp, mem};

use super::{
    clock::write_riscv_linux_time_pair, linux_error, RiscvGuestMemoryReader,
    RiscvGuestMemoryWriter, RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_E2BIG,
    RISCV_LINUX_EACCES, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_EPERM,
    RISCV_LINUX_ESRCH, RISCV_PAGE_BYTES,
};

pub(super) const RISCV_LINUX_SCHED_GETSCHEDULER: u64 = 120;
pub(super) const RISCV_LINUX_SCHED_GETPARAM: u64 = 121;
pub(super) const RISCV_LINUX_SCHED_SETPARAM: u64 = 118;
pub(super) const RISCV_LINUX_SCHED_SETSCHEDULER: u64 = 119;
pub(super) const RISCV_LINUX_SCHED_SETAFFINITY: u64 = 122;
pub(super) const RISCV_LINUX_SCHED_GETAFFINITY: u64 = 123;
pub(super) const RISCV_LINUX_SCHED_GET_PRIORITY_MAX: u64 = 125;
pub(super) const RISCV_LINUX_SCHED_GET_PRIORITY_MIN: u64 = 126;
pub(super) const RISCV_LINUX_SCHED_RR_GET_INTERVAL: u64 = 127;
pub(super) const RISCV_LINUX_SCHED_SETATTR: u64 = 274;
pub(super) const RISCV_LINUX_SCHED_GETATTR: u64 = 275;
pub(super) const RISCV_LINUX_IOPRIO_SET: u64 = 30;
pub(super) const RISCV_LINUX_IOPRIO_GET: u64 = 31;
pub(super) const RISCV_LINUX_SETPRIORITY: u64 = 140;
pub(super) const RISCV_LINUX_GETPRIORITY: u64 = 141;

pub(super) const RISCV_LINUX_DEFAULT_SCHED_POLICY: i32 = RISCV_LINUX_SCHED_OTHER;
const RISCV_LINUX_DEFAULT_SCHED_PRIORITY: i32 = 0;
const RISCV_LINUX_SCHED_PARAM_BYTES: usize = mem::size_of::<i32>();
const RISCV_LINUX_NICE_MIN: i32 = -20;
const RISCV_LINUX_NICE_MAX: i32 = 19;
const RISCV_LINUX_IOPRIO_WHO_PROCESS: i32 = 1;
const RISCV_LINUX_IOPRIO_CLASS_SHIFT: u64 = 13;
const RISCV_LINUX_IOPRIO_DATA_MASK: u64 = (1 << RISCV_LINUX_IOPRIO_CLASS_SHIFT) - 1;
const RISCV_LINUX_IOPRIO_CLASS_MASK: u64 = 0x7;
const RISCV_LINUX_IOPRIO_ENCODING_MASK: u64 = 0xffff;
const RISCV_LINUX_IOPRIO_CLASS_NONE: u64 = 0;
const RISCV_LINUX_IOPRIO_CLASS_RT: u64 = 1;
const RISCV_LINUX_IOPRIO_CLASS_BE: u64 = 2;
const RISCV_LINUX_IOPRIO_CLASS_IDLE: u64 = 3;
const RISCV_LINUX_PRIO_PROCESS: i32 = 0;
const RISCV_LINUX_PRIO_PGRP: i32 = 1;
const RISCV_LINUX_PRIO_USER: i32 = 2;
const RISCV_LINUX_RAW_PRIORITY_BASE: i32 = 20;
const RISCV_LINUX_SCHED_RR_INTERVAL_NANOSECONDS: u64 = 2_000_000;
const RISCV_LINUX_SCHED_ATTR_BYTES: u64 = 56;
const RISCV_LINUX_SCHED_ATTR_BYTES_VER0: u64 = 48;
const RISCV_LINUX_SCHED_ATTR_BYTES_USIZE: usize = RISCV_LINUX_SCHED_ATTR_BYTES as usize;
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

    state.sched_policy() as u64
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

pub(super) fn syscall_getpriority(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    match priority_target(request, state) {
        Ok(()) => state.raw_linux_priority(),
        Err(errno) => linux_error(errno),
    }
}

pub(super) fn syscall_ioprio_get(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> u64 {
    match ioprio_target(request.argument(0), request.argument(1), state) {
        Ok(()) => state.process_ioprio(),
        Err(errno) => linux_error(errno),
    }
}

pub(super) fn syscall_ioprio_set(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    let ioprio = normalize_ioprio(request.argument(2));
    if let Err(errno) = validate_ioprio(ioprio) {
        return linux_error(errno);
    }
    if let Err(errno) = ioprio_target(request.argument(0), request.argument(1), state) {
        return linux_error(errno);
    }

    state.set_process_ioprio(ioprio);
    0
}

pub(super) fn syscall_setpriority(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
) -> u64 {
    if let Err(errno) = priority_target(request, state) {
        return linux_error(errno);
    }

    let requested_nice =
        linux_int_argument(request.argument(2)).clamp(RISCV_LINUX_NICE_MIN, RISCV_LINUX_NICE_MAX);
    if requested_nice < state.process_nice() {
        return linux_error(RISCV_LINUX_EACCES);
    }

    state.set_process_nice(requested_nice);
    0
}

pub(super) fn syscall_sched_rr_get_interval(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let requested_pid = linux_int_argument(request.argument(0));
    if requested_pid < 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if !matches_current_process(requested_pid as u64, state) {
        return Some(linux_error(RISCV_LINUX_ESRCH));
    }
    let interval_address = request.argument(1);
    if interval_address == 0 {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    guest_memory_writer.map(|guest_memory_writer| {
        write_riscv_linux_time_pair(
            interval_address,
            0,
            RISCV_LINUX_SCHED_RR_INTERVAL_NANOSECONDS,
            guest_memory_writer,
        )
    })
}

pub(super) fn syscall_sched_getparam(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let parameter_address = request.argument(1);
    if parameter_address == 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if let Err(errno) = scheduler_target(request.argument(0), state) {
        return Some(linux_error(errno));
    }

    let guest_memory_writer = guest_memory_writer?;
    if !guest_memory_writer.write(
        parameter_address,
        &RISCV_LINUX_DEFAULT_SCHED_PRIORITY.to_le_bytes(),
    ) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    Some(0)
}

pub(super) fn syscall_sched_setparam(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let parameter_address = request.argument(1);
    if parameter_address == 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    let guest_memory_reader = guest_memory_reader?;
    let Some(priority) = read_sched_priority(guest_memory_reader, parameter_address) else {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    };
    if let Err(errno) = scheduler_target(request.argument(0), state) {
        return Some(linux_error(errno));
    }
    if priority != RISCV_LINUX_DEFAULT_SCHED_PRIORITY {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    Some(0)
}

pub(super) fn syscall_sched_setscheduler(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let parameter_address = request.argument(2);
    if parameter_address == 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    let guest_memory_reader = guest_memory_reader?;
    let Some(priority) = read_sched_priority(guest_memory_reader, parameter_address) else {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    };
    if let Err(errno) = scheduler_target(request.argument(0), state) {
        return Some(linux_error(errno));
    }
    let policy = linux_int_argument(request.argument(1));
    if !settable_scheduler_policy(policy) {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if priority != RISCV_LINUX_DEFAULT_SCHED_PRIORITY {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    state.set_sched_policy(policy);
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

pub(super) fn syscall_sched_setattr(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let requested_pid = linux_int_argument(request.argument(0));
    let attr_address = request.argument(1);
    if requested_pid < 0 || attr_address == 0 || request.argument(2) != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let guest_memory_reader = guest_memory_reader?;
    let Some(size_bytes) = read_guest_exact(guest_memory_reader, attr_address, 4) else {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    };
    let requested_bytes = u32::from_le_bytes(size_bytes.as_slice().try_into().ok()?) as u64;
    let copied_bytes = if requested_bytes == 0 {
        RISCV_LINUX_SCHED_ATTR_BYTES_VER0
    } else {
        requested_bytes
    };
    if copied_bytes < RISCV_LINUX_SCHED_ATTR_BYTES_VER0 || copied_bytes > RISCV_PAGE_BYTES {
        write_modeled_sched_attr_size(attr_address, guest_memory_writer);
        return Some(linux_error(RISCV_LINUX_E2BIG));
    }

    let Some(attr) = read_guest_exact(guest_memory_reader, attr_address, copied_bytes as usize)
    else {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    };
    if copied_bytes > RISCV_LINUX_SCHED_ATTR_BYTES
        && attr[RISCV_LINUX_SCHED_ATTR_BYTES_USIZE..]
            .iter()
            .any(|byte| *byte != 0)
    {
        write_modeled_sched_attr_size(attr_address, guest_memory_writer);
        return Some(linux_error(RISCV_LINUX_E2BIG));
    }
    if let Err(errno) = scheduler_target(request.argument(0), state) {
        return Some(linux_error(errno));
    }

    let policy = read_sched_attr_policy(&attr);
    let flags = read_sched_attr_flags(&attr);
    let nice = read_sched_attr_nice(&attr).clamp(RISCV_LINUX_NICE_MIN, RISCV_LINUX_NICE_MAX);
    let priority = read_sched_attr_priority(&attr);
    if flags != 0 || !settable_scheduler_policy(policy) || priority != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if nice < state.process_nice() {
        return Some(linux_error(RISCV_LINUX_EPERM));
    }

    state.set_sched_policy(policy);
    state.set_process_nice(nice);
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

pub(super) fn syscall_sched_getattr(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let requested_pid = linux_int_argument(request.argument(0));
    let attr_address = request.argument(1);
    let requested_bytes = request.argument(2);
    if requested_pid < 0
        || attr_address == 0
        || requested_bytes < RISCV_LINUX_SCHED_ATTR_BYTES_VER0
        || requested_bytes > RISCV_PAGE_BYTES
        || request.argument(3) != 0
    {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if !matches_current_process(requested_pid as u64, state) {
        return Some(linux_error(RISCV_LINUX_ESRCH));
    }

    let guest_memory_writer = guest_memory_writer?;
    let written_bytes = cmp::min(requested_bytes, RISCV_LINUX_SCHED_ATTR_BYTES) as usize;
    let bytes = riscv_linux_sched_attr_bytes(state, written_bytes);
    if !guest_memory_writer.write(attr_address, &bytes[..written_bytes]) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    Some(0)
}

fn write_modeled_sched_attr_size(
    attr_address: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) {
    if let Some(guest_memory_writer) = guest_memory_writer {
        let _ = guest_memory_writer.write(
            attr_address,
            &(RISCV_LINUX_SCHED_ATTR_BYTES as u32).to_le_bytes(),
        );
    }
}

fn riscv_linux_sched_attr_bytes(
    state: &RiscvSyscallState,
    written_bytes: usize,
) -> [u8; RISCV_LINUX_SCHED_ATTR_BYTES_USIZE] {
    let mut bytes = [0; RISCV_LINUX_SCHED_ATTR_BYTES_USIZE];
    bytes[0..4].copy_from_slice(&(written_bytes as u32).to_le_bytes());
    bytes[4..8].copy_from_slice(&(state.sched_policy() as u32).to_le_bytes());
    bytes[16..20].copy_from_slice(&state.process_nice().to_le_bytes());
    bytes[20..24].copy_from_slice(&(RISCV_LINUX_DEFAULT_SCHED_PRIORITY as u32).to_le_bytes());
    bytes
}

fn read_sched_attr_policy(bytes: &[u8]) -> i32 {
    u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as i32
}

fn read_sched_attr_flags(bytes: &[u8]) -> u64 {
    u64::from_le_bytes(bytes[8..16].try_into().unwrap())
}

fn read_sched_attr_nice(bytes: &[u8]) -> i32 {
    i32::from_le_bytes(bytes[16..20].try_into().unwrap())
}

fn read_sched_attr_priority(bytes: &[u8]) -> u32 {
    u32::from_le_bytes(bytes[20..24].try_into().unwrap())
}

impl RiscvSyscallState {
    fn sched_policy(&self) -> i32 {
        self.sched_policy
    }

    fn set_sched_policy(&mut self, policy: i32) {
        self.sched_policy = policy;
    }

    fn process_nice(&self) -> i32 {
        self.process_nice
    }

    fn process_ioprio(&self) -> u64 {
        self.process_ioprio
    }

    fn raw_linux_priority(&self) -> u64 {
        (RISCV_LINUX_RAW_PRIORITY_BASE - self.process_nice) as u64
    }

    fn set_process_nice(&mut self, process_nice: i32) {
        self.process_nice = process_nice;
    }

    fn set_process_ioprio(&mut self, process_ioprio: u64) {
        self.process_ioprio = process_ioprio;
    }
}

fn ioprio_target(
    which_argument: u64,
    who_argument: u64,
    state: &RiscvSyscallState,
) -> Result<(), u64> {
    match linux_int_argument(which_argument) {
        RISCV_LINUX_IOPRIO_WHO_PROCESS => {
            let requested_pid = linux_int_argument(who_argument);
            if requested_pid < 0 {
                return Err(RISCV_LINUX_ESRCH);
            }
            if matches_current_process(requested_pid as u64, state) {
                Ok(())
            } else {
                Err(RISCV_LINUX_ESRCH)
            }
        }
        _ => Err(RISCV_LINUX_EINVAL),
    }
}

fn priority_target(request: RiscvSyscallRequest, state: &RiscvSyscallState) -> Result<(), u64> {
    let requested = linux_int_argument(request.argument(1));
    match linux_int_argument(request.argument(0)) {
        RISCV_LINUX_PRIO_PROCESS => {
            priority_scope_target(requested, |target| matches_current_process(target, state))
        }
        RISCV_LINUX_PRIO_PGRP => priority_scope_target(requested, |target| {
            matches_current_process_group(target, state)
        }),
        RISCV_LINUX_PRIO_USER => {
            priority_scope_target(requested, |target| matches_current_user(target, state))
        }
        _ => Err(RISCV_LINUX_EINVAL),
    }
}

fn scheduler_target(pid_argument: u64, state: &RiscvSyscallState) -> Result<(), u64> {
    let requested_pid = linux_int_argument(pid_argument);
    if requested_pid < 0 {
        return Err(RISCV_LINUX_EINVAL);
    }
    if matches_current_process(requested_pid as u64, state) {
        Ok(())
    } else {
        Err(RISCV_LINUX_ESRCH)
    }
}

fn matches_current_process(requested_pid: u64, state: &RiscvSyscallState) -> bool {
    requested_pid == 0
        || requested_pid == state.identity().thread_id()
        || requested_pid == state.identity().thread_group_id()
}

fn matches_current_process_group(requested_process_group: u64, state: &RiscvSyscallState) -> bool {
    requested_process_group == 0
        || requested_process_group == u64::from(state.guest_wait.current_process_group().get())
}

fn matches_current_user(requested_user: u64, state: &RiscvSyscallState) -> bool {
    requested_user == 0 || requested_user == state.identity().user_id()
}

fn priority_scope_target(
    requested: i32,
    matches_scope: impl FnOnce(u64) -> bool,
) -> Result<(), u64> {
    if requested < 0 {
        return Err(RISCV_LINUX_ESRCH);
    }
    if matches_scope(requested as u64) {
        Ok(())
    } else {
        Err(RISCV_LINUX_ESRCH)
    }
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

const fn settable_scheduler_policy(policy: i32) -> bool {
    matches!(
        policy,
        RISCV_LINUX_SCHED_OTHER | RISCV_LINUX_SCHED_BATCH | RISCV_LINUX_SCHED_IDLE
    )
}

const fn validate_ioprio(ioprio: u64) -> Result<(), u64> {
    let class = (ioprio >> RISCV_LINUX_IOPRIO_CLASS_SHIFT) & RISCV_LINUX_IOPRIO_CLASS_MASK;
    let data = ioprio & RISCV_LINUX_IOPRIO_DATA_MASK;
    match class {
        RISCV_LINUX_IOPRIO_CLASS_NONE if data == 0 => Ok(()),
        RISCV_LINUX_IOPRIO_CLASS_NONE => Err(RISCV_LINUX_EINVAL),
        RISCV_LINUX_IOPRIO_CLASS_RT => Err(RISCV_LINUX_EPERM),
        RISCV_LINUX_IOPRIO_CLASS_BE | RISCV_LINUX_IOPRIO_CLASS_IDLE => Ok(()),
        _ => Err(RISCV_LINUX_EINVAL),
    }
}

const fn normalize_ioprio(ioprio: u64) -> u64 {
    ioprio & RISCV_LINUX_IOPRIO_ENCODING_MASK
}

fn read_sched_priority(guest_memory_reader: &RiscvGuestMemoryReader, address: u64) -> Option<i32> {
    let bytes = read_guest_exact(guest_memory_reader, address, RISCV_LINUX_SCHED_PARAM_BYTES)?;
    Some(i32::from_le_bytes(bytes.as_slice().try_into().ok()?))
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
