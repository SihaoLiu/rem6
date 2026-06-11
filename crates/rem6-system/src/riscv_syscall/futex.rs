use rem6_kernel::{PartitionId, Tick};

use super::{
    linux_error, RiscvGuestMemoryReader, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
};
use crate::{
    GuestFutexAddress, GuestFutexKey, GuestFutexWaitOutcome, GuestFutexWaitRequest,
    GuestThreadGroupId, GuestThreadId,
};

const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_FUTEX_WAIT: u32 = 0;
const RISCV_LINUX_FUTEX_WAKE: u32 = 1;
const RISCV_LINUX_FUTEX_WAIT_BITSET: u32 = 9;
const RISCV_LINUX_FUTEX_WAKE_BITSET: u32 = 10;
const RISCV_LINUX_FUTEX_PRIVATE_FLAG: u32 = 128;
const RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG: u32 = 256;

pub(super) fn syscall_futex(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<RiscvSyscallOutcome> {
    let op = (request.argument(1) as u32)
        & !(RISCV_LINUX_FUTEX_PRIVATE_FLAG | RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG);
    let address = GuestFutexAddress::new(request.argument(0));
    let thread_group = GuestThreadGroupId::new(state.identity().thread_group_id());
    match op {
        RISCV_LINUX_FUTEX_WAIT => guest_memory.and_then(|guest_memory| {
            syscall_futex_wait(
                request,
                state,
                tick,
                address,
                thread_group,
                u32::MAX,
                guest_memory,
            )
        }),
        RISCV_LINUX_FUTEX_WAIT_BITSET => {
            let bitset = request.argument(5) as u32;
            if bitset == 0 {
                return Some(RiscvSyscallOutcome::Return {
                    value: linux_error(RISCV_LINUX_EINVAL),
                });
            }
            guest_memory.and_then(|guest_memory| {
                syscall_futex_wait(
                    request,
                    state,
                    tick,
                    address,
                    thread_group,
                    bitset,
                    guest_memory,
                )
            })
        }
        RISCV_LINUX_FUTEX_WAKE => {
            let count = futex_wake_count(request.argument(2));
            let outcome = state
                .guest_futexes
                .wake(address, thread_group, count, tick)
                .expect("guest futex wake cannot fail");
            Some(RiscvSyscallOutcome::Return {
                value: outcome.woken_count() as u64,
            })
        }
        RISCV_LINUX_FUTEX_WAKE_BITSET => {
            let bitset = request.argument(5) as u32;
            if bitset == 0 {
                return Some(RiscvSyscallOutcome::Return {
                    value: linux_error(RISCV_LINUX_EINVAL),
                });
            }
            let outcome = state
                .guest_futexes
                .wake_bitset(address, thread_group, usize::MAX, bitset, tick)
                .expect("guest futex bitset wake cannot fail");
            Some(RiscvSyscallOutcome::Return {
                value: outcome.woken_count() as u64,
            })
        }
        _ => None,
    }
}

fn syscall_futex_wait(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
    bitset: u32,
    guest_memory: &RiscvGuestMemoryReader,
) -> Option<RiscvSyscallOutcome> {
    let Some(observed) = read_guest_i32(guest_memory, request.argument(0)) else {
        return Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        });
    };
    let expected = request.argument(2) as i32;
    if observed == expected {
        return None;
    }

    let wait = GuestFutexWaitRequest::new(
        GuestFutexKey::new(address, thread_group),
        GuestThreadId::new(state.identity().thread_id()),
        PartitionId::new(0),
        tick,
        expected,
        observed,
    )
    .with_bitset(bitset);
    match state
        .guest_futexes
        .wait(wait)
        .expect("mismatched guest futex wait cannot enqueue")
    {
        GuestFutexWaitOutcome::WouldBlock { .. } => Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN),
        }),
        GuestFutexWaitOutcome::Queued { .. } => None,
    }
}

fn read_guest_i32(guest_memory: &RiscvGuestMemoryReader, address: u64) -> Option<i32> {
    let bytes = guest_memory.read(address, 4)?;
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    Some(i32::from_le_bytes(bytes))
}

fn futex_wake_count(value: u64) -> usize {
    let count = value as i32;
    if count <= 0 {
        0
    } else {
        count as usize
    }
}
