use super::time::read_timespec64;
use super::{
    linux_error, RiscvGuestMemoryReader, RiscvSyscallRequest, RISCV_LINUX_EFAULT,
    RISCV_LINUX_EINVAL, RISCV_LINUX_ENOTSUP,
};
use rem6_kernel::Tick;

pub(super) const RISCV_LINUX_NANOSLEEP: u64 = 101;
pub(super) const RISCV_LINUX_CLOCK_NANOSLEEP: u64 = 115;
const RISCV_LINUX_CLOCK_REALTIME: u64 = 0;
const RISCV_LINUX_CLOCK_MONOTONIC: u64 = 1;
const RISCV_LINUX_CLOCK_PROCESS_CPUTIME_ID: u64 = 2;
const RISCV_LINUX_CLOCK_THREAD_CPUTIME_ID: u64 = 3;
const RISCV_LINUX_CLOCK_MONOTONIC_RAW: u64 = 4;
const RISCV_LINUX_CLOCK_REALTIME_COARSE: u64 = 5;
const RISCV_LINUX_CLOCK_MONOTONIC_COARSE: u64 = 6;
const RISCV_LINUX_CLOCK_BOOTTIME: u64 = 7;
const RISCV_LINUX_CLOCK_TAI: u64 = 11;
const RISCV_LINUX_TIMER_ABSTIME: u64 = 1;

pub(super) fn syscall_nanosleep(
    request: RiscvSyscallRequest,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let guest_memory_reader = guest_memory_reader?;
    let requested = match read_timespec64(guest_memory_reader, request.argument(0)) {
        Some(timespec) => timespec,
        None => return Some(linux_error(RISCV_LINUX_EFAULT)),
    };
    if !requested.is_valid() {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if requested.is_zero() {
        return Some(0);
    }

    None
}

pub(super) fn syscall_clock_nanosleep(
    request: RiscvSyscallRequest,
    tick: Tick,
    guest_memory_reader: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    if let Err(error) = clock_nanosleep_clock_error(request.argument(0)) {
        return Some(linux_error(error));
    }
    let flags = request.argument(1);
    if flags & !RISCV_LINUX_TIMER_ABSTIME != 0 {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }

    let guest_memory_reader = guest_memory_reader?;
    let requested = match read_timespec64(guest_memory_reader, request.argument(2)) {
        Some(timespec) => timespec,
        None => return Some(linux_error(RISCV_LINUX_EFAULT)),
    };
    if !requested.is_valid() {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    }
    if flags & RISCV_LINUX_TIMER_ABSTIME != 0 {
        return if requested.total_nanoseconds() <= u128::from(tick) {
            Some(0)
        } else {
            None
        };
    }
    if requested.is_zero() {
        return Some(0);
    }

    None
}

const fn clock_nanosleep_clock_error(clock_id: u64) -> Result<(), u64> {
    match clock_id {
        RISCV_LINUX_CLOCK_REALTIME
        | RISCV_LINUX_CLOCK_MONOTONIC
        | RISCV_LINUX_CLOCK_PROCESS_CPUTIME_ID
        | RISCV_LINUX_CLOCK_BOOTTIME
        | RISCV_LINUX_CLOCK_TAI => Ok(()),
        RISCV_LINUX_CLOCK_THREAD_CPUTIME_ID
        | RISCV_LINUX_CLOCK_MONOTONIC_RAW
        | RISCV_LINUX_CLOCK_REALTIME_COARSE
        | RISCV_LINUX_CLOCK_MONOTONIC_COARSE => Err(RISCV_LINUX_ENOTSUP),
        _ => Err(RISCV_LINUX_EINVAL),
    }
}
