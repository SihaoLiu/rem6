use rem6_kernel::Tick;

use super::{linux_error, RiscvGuestMemoryWriter, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL};

const RISCV_LINUX_CLOCK_REALTIME: u64 = 0;
const RISCV_LINUX_CLOCK_MONOTONIC: u64 = 1;
const RISCV_LINUX_CLOCK_PROCESS_CPUTIME_ID: u64 = 2;
const RISCV_LINUX_CLOCK_THREAD_CPUTIME_ID: u64 = 3;
const RISCV_LINUX_CLOCK_MONOTONIC_RAW: u64 = 4;
const RISCV_LINUX_CLOCK_REALTIME_COARSE: u64 = 5;
const RISCV_LINUX_CLOCK_MONOTONIC_COARSE: u64 = 6;
const RISCV_LINUX_CLOCK_BOOTTIME: u64 = 7;
const RISCV_LINUX_NANOSECONDS_PER_SECOND: u64 = 1_000_000_000;
const RISCV_LINUX_TIMESPEC_BYTES: usize = 16;

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
    let mut bytes = [0; RISCV_LINUX_TIMESPEC_BYTES];
    bytes[..8].copy_from_slice(&seconds.to_le_bytes());
    bytes[8..].copy_from_slice(&nanoseconds.to_le_bytes());
    for (offset, byte) in bytes.iter().enumerate() {
        let Some(address) = timespec_address.checked_add(offset as u64) else {
            return linux_error(RISCV_LINUX_EFAULT);
        };
        if !guest_memory.write(address, std::slice::from_ref(byte)) {
            return linux_error(RISCV_LINUX_EFAULT);
        }
    }
    0
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
