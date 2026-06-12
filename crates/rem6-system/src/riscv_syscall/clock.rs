use rem6_kernel::Tick;

use super::{
    linux_error, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};

pub(super) const RISCV_LINUX_CLOCK_GETTIME: u64 = 113;
pub(super) const RISCV_LINUX_TIMES: u64 = 153;
pub(super) const RISCV_LINUX_GETTIMEOFDAY: u64 = 169;
const RISCV_LINUX_CLOCK_REALTIME: u64 = 0;
const RISCV_LINUX_CLOCK_MONOTONIC: u64 = 1;
const RISCV_LINUX_CLOCK_PROCESS_CPUTIME_ID: u64 = 2;
const RISCV_LINUX_CLOCK_THREAD_CPUTIME_ID: u64 = 3;
const RISCV_LINUX_CLOCK_MONOTONIC_RAW: u64 = 4;
const RISCV_LINUX_CLOCK_REALTIME_COARSE: u64 = 5;
const RISCV_LINUX_CLOCK_MONOTONIC_COARSE: u64 = 6;
const RISCV_LINUX_CLOCK_BOOTTIME: u64 = 7;
const RISCV_LINUX_NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const RISCV_LINUX_NANOSECONDS_PER_MICROSECOND: u64 = 1_000;
const RISCV_LINUX_CLOCK_TICKS_PER_SECOND: u64 = 100;
const RISCV_LINUX_NANOSECONDS_PER_CLOCK_TICK: u64 =
    RISCV_LINUX_NANOSECONDS_PER_SECOND / RISCV_LINUX_CLOCK_TICKS_PER_SECOND;

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

pub(super) fn syscall_gettimeofday(
    timeval_address: u64,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryWriter,
) -> u64 {
    let seconds = tick / RISCV_LINUX_NANOSECONDS_PER_SECOND;
    let microseconds =
        (tick % RISCV_LINUX_NANOSECONDS_PER_SECOND) / RISCV_LINUX_NANOSECONDS_PER_MICROSECOND;
    write_riscv_linux_time_pair(timeval_address, seconds, microseconds, guest_memory)
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
        RISCV_LINUX_TIMES => syscall_times(request, tick, guest_memory),
        RISCV_LINUX_GETTIMEOFDAY => guest_memory.map(|guest_memory| RiscvSyscallOutcome::Return {
            value: syscall_gettimeofday(request.argument(0), tick, guest_memory),
        }),
        RISCV_LINUX_CLOCK_GETTIME => guest_memory.map(|guest_memory| RiscvSyscallOutcome::Return {
            value: syscall_clock_gettime(
                request.argument(0),
                request.argument(1),
                tick,
                guest_memory,
            ),
        }),
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

fn write_riscv_linux_time_pair(
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
    )
}
