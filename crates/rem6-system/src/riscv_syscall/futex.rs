use rem6_kernel::{PartitionId, Tick};

use super::time::read_timespec64;
use super::{
    linux_error, RiscvGuestMemoryReader, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL, RISCV_LINUX_ENOSYS,
};
use crate::{
    GuestFutexAddress, GuestFutexKey, GuestFutexWaitOutcome, GuestFutexWaitRequest,
    GuestThreadGroupId, GuestThreadId,
};

const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_ETIMEDOUT: u64 = 110;
const RISCV_LINUX_FUTEX_WAIT: u32 = 0;
const RISCV_LINUX_FUTEX_WAKE: u32 = 1;
const RISCV_LINUX_FUTEX_REQUEUE: u32 = 3;
const RISCV_LINUX_FUTEX_CMP_REQUEUE: u32 = 4;
const RISCV_LINUX_FUTEX_WAIT_BITSET: u32 = 9;
const RISCV_LINUX_FUTEX_WAKE_BITSET: u32 = 10;
const RISCV_LINUX_FUTEX_PRIVATE_FLAG: u32 = 128;
const RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG: u32 = 256;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvFutexWaitRequest {
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
    bitset: u32,
    timeout_address: Option<u64>,
}

impl RiscvFutexWaitRequest {
    const fn new(
        address: GuestFutexAddress,
        thread_group: GuestThreadGroupId,
        bitset: u32,
        timeout_address: Option<u64>,
    ) -> Self {
        Self {
            address,
            thread_group,
            bitset,
            timeout_address,
        }
    }
}

pub(super) fn syscall_futex(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: Option<&RiscvGuestMemoryReader>,
) -> Option<RiscvSyscallOutcome> {
    let raw_op = request.argument(1) as u32;
    let op = raw_op & !RISCV_LINUX_FUTEX_PRIVATE_FLAG;
    if futex_clock_realtime_is_invalid(op) {
        return Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS),
        });
    }
    let op = op & !RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG;
    let address = GuestFutexAddress::new(request.argument(0));
    let thread_group = GuestThreadGroupId::new(state.identity().thread_group_id());
    match op {
        RISCV_LINUX_FUTEX_WAIT => guest_memory.and_then(|guest_memory| {
            syscall_futex_wait(
                request,
                state,
                tick,
                RiscvFutexWaitRequest::new(
                    address,
                    thread_group,
                    u32::MAX,
                    Some(request.argument(3)),
                ),
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
                    RiscvFutexWaitRequest::new(address, thread_group, bitset, None),
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
        RISCV_LINUX_FUTEX_REQUEUE => Some(syscall_futex_requeue(
            request,
            state,
            tick,
            address,
            thread_group,
        )),
        RISCV_LINUX_FUTEX_CMP_REQUEUE => guest_memory.map(|guest_memory| {
            syscall_futex_cmp_requeue(request, state, tick, address, thread_group, guest_memory)
        }),
        RISCV_LINUX_FUTEX_WAKE_BITSET => {
            let bitset = request.argument(5) as u32;
            if bitset == 0 {
                return Some(RiscvSyscallOutcome::Return {
                    value: linux_error(RISCV_LINUX_EINVAL),
                });
            }
            let count = futex_wake_count(request.argument(2));
            let outcome = state
                .guest_futexes
                .wake_bitset(address, thread_group, count, bitset, tick)
                .expect("guest futex bitset wake cannot fail");
            Some(RiscvSyscallOutcome::Return {
                value: outcome.woken_count() as u64,
            })
        }
        _ => None,
    }
}

fn syscall_futex_requeue(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
) -> RiscvSyscallOutcome {
    let target_address = GuestFutexAddress::new(request.argument(4));
    let Some(wake_count) = futex_requeue_count(request.argument(2)) else {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL),
        };
    };
    let Some(requeue_count) = futex_requeue_count(request.argument(3)) else {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL),
        };
    };
    let outcome = state
        .guest_futexes
        .requeue(
            address,
            target_address,
            thread_group,
            wake_count,
            requeue_count,
            tick,
        )
        .expect("guest futex requeue cannot fail");
    RiscvSyscallOutcome::Return {
        value: (outcome.woken().len() + outcome.requeued().len()) as u64,
    }
}

fn syscall_futex_cmp_requeue(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
    guest_memory: &RiscvGuestMemoryReader,
) -> RiscvSyscallOutcome {
    let Some(observed) = read_guest_i32(guest_memory, request.argument(0)) else {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        };
    };
    let expected = request.argument(5) as i32;
    if observed != expected {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN),
        };
    }
    syscall_futex_requeue(request, state, tick, address, thread_group)
}

fn syscall_futex_wait(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    wait: RiscvFutexWaitRequest,
    guest_memory: &RiscvGuestMemoryReader,
) -> Option<RiscvSyscallOutcome> {
    let Some(observed) = read_guest_i32(guest_memory, request.argument(0)) else {
        return Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        });
    };
    let timeout_is_zero = match wait.timeout_address {
        Some(timeout_address) => match futex_wait_timeout_is_zero(timeout_address, guest_memory) {
            Ok(timeout_is_zero) => timeout_is_zero,
            Err(outcome) => return Some(outcome),
        },
        None => false,
    };
    let expected = request.argument(2) as i32;
    if observed != expected {
        return Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN),
        });
    }
    if timeout_is_zero {
        return Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ETIMEDOUT),
        });
    }

    let wait = GuestFutexWaitRequest::new(
        GuestFutexKey::new(wait.address, wait.thread_group),
        GuestThreadId::new(state.identity().thread_id()),
        PartitionId::new(0),
        tick,
        expected,
        observed,
    )
    .with_bitset(wait.bitset);
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

fn futex_wait_timeout_is_zero(
    timeout_address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<bool, RiscvSyscallOutcome> {
    if timeout_address == 0 {
        return Ok(false);
    }
    let Some(timeout) = read_timespec64(guest_memory, timeout_address) else {
        return Err(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        });
    };
    if !timeout.is_valid() {
        return Err(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL),
        });
    }
    Ok(timeout.is_zero())
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

fn futex_requeue_count(value: u64) -> Option<usize> {
    let count = value as i32;
    if count < 0 {
        None
    } else {
        Some(count as usize)
    }
}

fn futex_clock_realtime_is_invalid(op: u32) -> bool {
    op & RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG != 0
        && matches!(
            op & !RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG,
            RISCV_LINUX_FUTEX_REQUEUE | RISCV_LINUX_FUTEX_CMP_REQUEUE
        )
}
