use rem6_kernel::Tick;

use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_EPERM,
};

pub(super) const RISCV_LINUX_GETITIMER: u64 = 102;
pub(super) const RISCV_LINUX_SETITIMER: u64 = 103;
pub(super) const RISCV_LINUX_CLOCK_SETTIME: u64 = 112;
pub(super) const RISCV_LINUX_CLOCK_GETTIME: u64 = 113;
pub(super) const RISCV_LINUX_CLOCK_GETRES: u64 = 114;
pub(super) const RISCV_LINUX_TIMES: u64 = 153;
pub(super) const RISCV_LINUX_GETTIMEOFDAY: u64 = 169;
pub(super) const RISCV_LINUX_SETTIMEOFDAY: u64 = 170;
pub(super) const RISCV_NEWLIB_CLOCK_GETTIME64: u64 = 403;
pub(super) const RISCV_NEWLIB_LEGACY_TIME: u64 = 1062;
const RISCV_LINUX_ITIMER_REAL: u64 = 0;
const RISCV_LINUX_ITIMER_VIRTUAL: u64 = 1;
const RISCV_LINUX_ITIMER_PROF: u64 = 2;
const RISCV_LINUX_ITIMERVAL_BYTES: usize = 32;
const RISCV_LINUX_CLOCK_REALTIME: u64 = 0;
const RISCV_LINUX_CLOCK_MONOTONIC: u64 = 1;
const RISCV_LINUX_CLOCK_PROCESS_CPUTIME_ID: u64 = 2;
const RISCV_LINUX_CLOCK_THREAD_CPUTIME_ID: u64 = 3;
const RISCV_LINUX_CLOCK_MONOTONIC_RAW: u64 = 4;
const RISCV_LINUX_CLOCK_REALTIME_COARSE: u64 = 5;
const RISCV_LINUX_CLOCK_MONOTONIC_COARSE: u64 = 6;
const RISCV_LINUX_CLOCK_BOOTTIME: u64 = 7;
const RISCV_LINUX_CLOCK_TAI: u64 = 11;
const RISCV_LINUX_NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const RISCV_LINUX_NANOSECONDS_PER_MICROSECOND: u64 = 1_000;
const RISCV_LINUX_MICROSECONDS_PER_SECOND: u64 = 1_000_000;
const RISCV_LINUX_CLOCK_TICKS_PER_SECOND: u64 = 100;
const RISCV_LINUX_TIMEZONE_BYTES: usize = 8;
const RISCV_LINUX_NANOSECONDS_PER_CLOCK_TICK: u64 =
    RISCV_LINUX_NANOSECONDS_PER_SECOND / RISCV_LINUX_CLOCK_TICKS_PER_SECOND;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvLinuxItimerval {
    interval_seconds: u64,
    interval_microseconds: u64,
    value_seconds: u64,
    value_microseconds: u64,
}

impl RiscvLinuxItimerval {
    pub(super) const fn zero() -> Self {
        Self {
            interval_seconds: 0,
            interval_microseconds: 0,
            value_seconds: 0,
            value_microseconds: 0,
        }
    }

    fn encode(self) -> [u8; RISCV_LINUX_ITIMERVAL_BYTES] {
        let mut bytes = [0; RISCV_LINUX_ITIMERVAL_BYTES];
        bytes[0..8].copy_from_slice(&self.interval_seconds.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.interval_microseconds.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.value_seconds.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.value_microseconds.to_le_bytes());
        bytes
    }
}

impl RiscvSyscallState {
    pub(super) const fn initial_interval_timers() -> [RiscvLinuxItimerval; 3] {
        [
            RiscvLinuxItimerval::zero(),
            RiscvLinuxItimerval::zero(),
            RiscvLinuxItimerval::zero(),
        ]
    }
}

pub(super) fn syscall_getitimer(
    request: RiscvSyscallRequest,
    state: &RiscvSyscallState,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let Some(index) = interval_timer_index(request.argument(0)) else {
        return linux_error(RISCV_LINUX_EINVAL);
    };
    write_riscv_linux_itimerval(
        request.argument(1),
        state.interval_timers[index],
        guest_memory,
    )
}

pub(super) fn syscall_setitimer(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    guest_memory_reader: &super::RiscvGuestMemoryReader,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let Some(index) = interval_timer_index(request.argument(0)) else {
        return Some(linux_error(RISCV_LINUX_EINVAL));
    };
    let new_value = match read_riscv_linux_itimerval(request.argument(1), guest_memory_reader) {
        Ok(value) => value,
        Err(error) => return Some(linux_error(error)),
    };
    let old_value_address = request.argument(2);
    if old_value_address != 0 {
        let guest_memory_writer = guest_memory_writer?;
        let old_write = write_riscv_linux_itimerval(
            old_value_address,
            state.interval_timers[index],
            guest_memory_writer,
        );
        if old_write != 0 {
            return Some(old_write);
        }
    }
    state.interval_timers[index] = new_value;
    Some(0)
}

pub(super) fn syscall_clock_gettime(
    clock_id: u64,
    timespec_address: u64,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    if !valid_clock_id(clock_id) {
        return linux_error(RISCV_LINUX_EINVAL);
    }

    let seconds = tick / RISCV_LINUX_NANOSECONDS_PER_SECOND;
    let nanoseconds = tick % RISCV_LINUX_NANOSECONDS_PER_SECOND;
    write_riscv_linux_time_pair(timespec_address, seconds, nanoseconds, guest_memory)
}

pub(super) fn syscall_clock_getres(
    clock_id: u64,
    timespec_address: u64,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    let resolution = match clock_resolution_nanoseconds(clock_id) {
        Some(resolution) => resolution,
        None => return Some(linux_error(RISCV_LINUX_EINVAL)),
    };
    if timespec_address == 0 {
        return Some(0);
    }
    guest_memory.map(|guest_memory| {
        write_riscv_linux_time_pair(timespec_address, 0, resolution, guest_memory)
    })
}

pub(super) fn syscall_clock_settime(
    request: RiscvSyscallRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> u64 {
    if request.argument(0) != RISCV_LINUX_CLOCK_REALTIME {
        return linux_error(RISCV_LINUX_EINVAL);
    }
    match read_riscv_linux_timespec(request.argument(1), guest_memory) {
        Ok(()) => linux_error(RISCV_LINUX_EPERM),
        Err(error) => linux_error(error),
    }
}

pub(super) fn syscall_gettimeofday(
    timeval_address: u64,
    timezone_address: u64,
    tick: Tick,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<u64> {
    if timeval_address == 0 && timezone_address == 0 {
        return Some(0);
    }

    let guest_memory = guest_memory?;
    if timeval_address != 0 {
        let seconds = tick / RISCV_LINUX_NANOSECONDS_PER_SECOND;
        let microseconds =
            (tick % RISCV_LINUX_NANOSECONDS_PER_SECOND) / RISCV_LINUX_NANOSECONDS_PER_MICROSECOND;
        let timeval_result =
            write_riscv_linux_time_pair(timeval_address, seconds, microseconds, guest_memory);
        if timeval_result != 0 {
            return Some(timeval_result);
        }
    }

    if timezone_address != 0 && !write_riscv_linux_timezone(timezone_address, guest_memory) {
        return Some(linux_error(RISCV_LINUX_EFAULT));
    }

    Some(0)
}

pub(super) fn syscall_settimeofday(
    request: RiscvSyscallRequest,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<u64> {
    let timeval_address = request.argument(0);
    let timezone_address = request.argument(1);
    if timeval_address == 0 && timezone_address == 0 {
        return Some(0);
    }

    let guest_memory = guest_memory?;
    if timeval_address != 0 {
        if let Err(error) = read_riscv_linux_timeval(timeval_address, guest_memory) {
            return Some(linux_error(error));
        }
    }
    if timezone_address != 0 {
        if let Err(error) = read_riscv_linux_timezone(timezone_address, guest_memory) {
            return Some(linux_error(error));
        }
    }

    Some(linux_error(RISCV_LINUX_EPERM))
}

pub(super) fn syscall_legacy_time(
    request: RiscvSyscallRequest,
    tick: Tick,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let seconds = tick / RISCV_LINUX_NANOSECONDS_PER_SECOND;
    let time_address = request.argument(0);
    if time_address == 0 {
        return Some(RiscvSyscallOutcome::Return { value: seconds });
    }
    let guest_memory = guest_memory?;
    let value = if write_riscv_linux_bytes(time_address, &seconds.to_le_bytes(), guest_memory) {
        seconds
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    };
    Some(RiscvSyscallOutcome::Return { value })
}

const fn riscv_linux_clock_ticks(tick: Tick) -> u64 {
    tick / RISCV_LINUX_NANOSECONDS_PER_CLOCK_TICK
}

pub(super) fn syscall_times(
    request: RiscvSyscallRequest,
    tick: Tick,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    let tms_address = request.argument(0);
    let elapsed = riscv_linux_clock_ticks(tick);
    if tms_address == 0 {
        return Some(RiscvSyscallOutcome::Return { value: elapsed });
    }
    Some(RiscvSyscallOutcome::Return {
        value: write_riscv_linux_tms(tms_address, elapsed, guest_memory?),
    })
}

pub(super) fn syscall_clock(
    request: RiscvSyscallRequest,
    tick: Tick,
    guest_memory: Option<&RiscvGuestMemoryWriter>,
) -> Option<RiscvSyscallOutcome> {
    match request.number() {
        RISCV_NEWLIB_LEGACY_TIME => syscall_legacy_time(request, tick, guest_memory),
        RISCV_LINUX_TIMES => syscall_times(request, tick, guest_memory),
        RISCV_LINUX_GETTIMEOFDAY => {
            syscall_gettimeofday(request.argument(0), request.argument(1), tick, guest_memory)
                .map(|value| RiscvSyscallOutcome::Return { value })
        }
        RISCV_LINUX_CLOCK_GETTIME | RISCV_NEWLIB_CLOCK_GETTIME64 => {
            guest_memory.map(|guest_memory| RiscvSyscallOutcome::Return {
                value: syscall_clock_gettime(
                    request.argument(0),
                    request.argument(1),
                    tick,
                    guest_memory,
                ),
            })
        }
        RISCV_LINUX_CLOCK_GETRES => {
            syscall_clock_getres(request.argument(0), request.argument(1), guest_memory)
                .map(|value| RiscvSyscallOutcome::Return { value })
        }
        _ => None,
    }
}

fn write_riscv_linux_tms(
    tms_address: u64,
    elapsed: u64,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let mut bytes = [0; 32];
    bytes[..8].copy_from_slice(&elapsed.to_le_bytes());
    if write_riscv_linux_bytes(tms_address, &bytes, guest_memory) {
        elapsed
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    }
}

pub(super) fn write_riscv_linux_time_pair(
    address: u64,
    seconds: u64,
    fraction: u64,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&seconds.to_le_bytes());
    bytes[8..].copy_from_slice(&fraction.to_le_bytes());
    if write_riscv_linux_bytes(address, &bytes, guest_memory) {
        0
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    }
}

fn write_riscv_linux_timezone(address: u64, guest_memory: &RiscvGuestMemoryWriter) -> bool {
    write_riscv_linux_bytes(address, &[0; RISCV_LINUX_TIMEZONE_BYTES], guest_memory)
}

fn write_riscv_linux_bytes(
    address: u64,
    bytes: &[u8],
    guest_memory: &RiscvGuestMemoryWriter,
) -> bool {
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(address) = address.checked_add(offset as u64) else {
            return false;
        };
        if !guest_memory.write(address, std::slice::from_ref(byte)) {
            return false;
        }
    }
    true
}

fn interval_timer_index(which: u64) -> Option<usize> {
    match which {
        RISCV_LINUX_ITIMER_REAL => Some(0),
        RISCV_LINUX_ITIMER_VIRTUAL => Some(1),
        RISCV_LINUX_ITIMER_PROF => Some(2),
        _ => None,
    }
}

fn read_riscv_linux_itimerval(
    address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<RiscvLinuxItimerval, u64> {
    let bytes = guest_memory
        .read(address, RISCV_LINUX_ITIMERVAL_BYTES)
        .filter(|bytes| bytes.len() == RISCV_LINUX_ITIMERVAL_BYTES)
        .ok_or(RISCV_LINUX_EFAULT)?;
    Ok(RiscvLinuxItimerval {
        interval_seconds: read_nonnegative_timeval_field(&bytes, 0, None)?,
        interval_microseconds: read_nonnegative_timeval_field(
            &bytes,
            8,
            Some(RISCV_LINUX_MICROSECONDS_PER_SECOND),
        )?,
        value_seconds: read_nonnegative_timeval_field(&bytes, 16, None)?,
        value_microseconds: read_nonnegative_timeval_field(
            &bytes,
            24,
            Some(RISCV_LINUX_MICROSECONDS_PER_SECOND),
        )?,
    })
}

fn read_riscv_linux_timespec(
    address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<(), u64> {
    read_riscv_linux_time_pair(address, guest_memory, RISCV_LINUX_NANOSECONDS_PER_SECOND)
}

fn read_riscv_linux_timeval(
    address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<(), u64> {
    read_riscv_linux_time_pair(address, guest_memory, RISCV_LINUX_MICROSECONDS_PER_SECOND)
}

fn read_riscv_linux_time_pair(
    address: u64,
    guest_memory: &RiscvGuestMemoryReader,
    fraction_exclusive_limit: u64,
) -> Result<(), u64> {
    let bytes = guest_memory
        .read(address, 16)
        .filter(|bytes| bytes.len() == 16)
        .ok_or(RISCV_LINUX_EFAULT)?;
    read_nonnegative_timeval_field(&bytes, 0, None)?;
    read_nonnegative_timeval_field(&bytes, 8, Some(fraction_exclusive_limit))?;
    Ok(())
}

fn read_riscv_linux_timezone(
    address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<(), u64> {
    guest_memory
        .read(address, RISCV_LINUX_TIMEZONE_BYTES)
        .filter(|bytes| bytes.len() == RISCV_LINUX_TIMEZONE_BYTES)
        .map(|_| ())
        .ok_or(RISCV_LINUX_EFAULT)
}

fn read_nonnegative_timeval_field(
    bytes: &[u8],
    offset: usize,
    exclusive_limit: Option<u64>,
) -> Result<u64, u64> {
    let value = i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap());
    let Ok(value) = u64::try_from(value) else {
        return Err(RISCV_LINUX_EINVAL);
    };
    if exclusive_limit.is_some_and(|limit| value >= limit) {
        return Err(RISCV_LINUX_EINVAL);
    }
    Ok(value)
}

fn write_riscv_linux_itimerval(
    address: u64,
    value: RiscvLinuxItimerval,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    if write_riscv_linux_bytes(address, &value.encode(), guest_memory) {
        0
    } else {
        linux_error(RISCV_LINUX_EFAULT)
    }
}

fn valid_clock_id(clock_id: u64) -> bool {
    matches!(
        clock_id,
        RISCV_LINUX_CLOCK_REALTIME
            | RISCV_LINUX_CLOCK_MONOTONIC
            | RISCV_LINUX_CLOCK_PROCESS_CPUTIME_ID
            | RISCV_LINUX_CLOCK_THREAD_CPUTIME_ID
            | RISCV_LINUX_CLOCK_MONOTONIC_RAW
            | RISCV_LINUX_CLOCK_REALTIME_COARSE
            | RISCV_LINUX_CLOCK_MONOTONIC_COARSE
            | RISCV_LINUX_CLOCK_BOOTTIME
            | RISCV_LINUX_CLOCK_TAI
    )
}

const fn clock_resolution_nanoseconds(clock_id: u64) -> Option<u64> {
    match clock_id {
        RISCV_LINUX_CLOCK_REALTIME
        | RISCV_LINUX_CLOCK_MONOTONIC
        | RISCV_LINUX_CLOCK_PROCESS_CPUTIME_ID
        | RISCV_LINUX_CLOCK_THREAD_CPUTIME_ID
        | RISCV_LINUX_CLOCK_MONOTONIC_RAW
        | RISCV_LINUX_CLOCK_BOOTTIME
        | RISCV_LINUX_CLOCK_TAI => Some(1),
        RISCV_LINUX_CLOCK_REALTIME_COARSE | RISCV_LINUX_CLOCK_MONOTONIC_COARSE => Some(1_000_000),
        _ => None,
    }
}
