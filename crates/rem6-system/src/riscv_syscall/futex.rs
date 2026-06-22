use rem6_kernel::{PartitionId, Tick};

use super::time::read_timespec64;
use super::{
    linux_error, RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome,
    RiscvSyscallRequest, RiscvSyscallState, RISCV_LINUX_EFAULT, RISCV_LINUX_EINVAL,
    RISCV_LINUX_ENOSYS,
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
const RISCV_LINUX_FUTEX_WAKE_OP: u32 = 5;
const RISCV_LINUX_FUTEX_WAIT_BITSET: u32 = 9;
const RISCV_LINUX_FUTEX_WAKE_BITSET: u32 = 10;
const RISCV_LINUX_FUTEX_PRIVATE_FLAG: u32 = 128;
const RISCV_LINUX_FUTEX_CLOCK_REALTIME_FLAG: u32 = 256;
const RISCV_LINUX_FUTEX_OP_SET: u32 = 0;
const RISCV_LINUX_FUTEX_OP_ADD: u32 = 1;
const RISCV_LINUX_FUTEX_OP_OR: u32 = 2;
const RISCV_LINUX_FUTEX_OP_ANDN: u32 = 3;
const RISCV_LINUX_FUTEX_OP_XOR: u32 = 4;
const RISCV_LINUX_FUTEX_OP_ARG_SHIFT: u32 = 8;
const RISCV_LINUX_FUTEX_OP_CMP_EQ: u32 = 0;
const RISCV_LINUX_FUTEX_OP_CMP_NE: u32 = 1;
const RISCV_LINUX_FUTEX_OP_CMP_LT: u32 = 2;
const RISCV_LINUX_FUTEX_OP_CMP_LE: u32 = 3;
const RISCV_LINUX_FUTEX_OP_CMP_GT: u32 = 4;
const RISCV_LINUX_FUTEX_OP_CMP_GE: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvFutexWaitRequest {
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
    bitset: u32,
    timeout: RiscvFutexWaitTimeout,
}

impl RiscvFutexWaitRequest {
    const fn new(
        address: GuestFutexAddress,
        thread_group: GuestThreadGroupId,
        bitset: u32,
        timeout: RiscvFutexWaitTimeout,
    ) -> Self {
        Self {
            address,
            thread_group,
            bitset,
            timeout,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvFutexWaitTimeout {
    Relative(Option<u64>),
    Absolute(Option<u64>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RiscvFutexWaitTimeoutStatus {
    Pending,
    Expired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvFutexWakeOperation {
    operation: u32,
    operand: i32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RiscvFutexWakeComparison {
    compare: u32,
    operand: i32,
}

pub(super) fn syscall_futex(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    guest_memory: Option<&RiscvGuestMemoryReader>,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
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
                    RiscvFutexWaitTimeout::Relative(futex_timeout_address(request.argument(3))),
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
                    RiscvFutexWaitRequest::new(
                        address,
                        thread_group,
                        bitset,
                        RiscvFutexWaitTimeout::Absolute(futex_timeout_address(request.argument(3))),
                    ),
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
        RISCV_LINUX_FUTEX_WAKE_OP => guest_memory.and_then(|guest_memory| {
            guest_memory_writer.map(|guest_memory_writer| {
                syscall_futex_wake_op(
                    request,
                    state,
                    tick,
                    address,
                    thread_group,
                    guest_memory,
                    guest_memory_writer,
                )
            })
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

fn syscall_futex_wake_op(
    request: RiscvSyscallRequest,
    state: &mut RiscvSyscallState,
    tick: Tick,
    address: GuestFutexAddress,
    thread_group: GuestThreadGroupId,
    guest_memory: &RiscvGuestMemoryReader,
    guest_memory_writer: &RiscvGuestMemoryWriter,
) -> RiscvSyscallOutcome {
    let wake_count = futex_wake_count(request.argument(2));
    let second_wake_count = futex_wake_count(request.argument(3));
    let target_address = GuestFutexAddress::new(request.argument(4));
    let encoded_operation = request.argument(5) as u32;
    let Some(operation) = decode_futex_wake_operation(encoded_operation) else {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS),
        };
    };
    let Some(observed) = read_guest_i32(guest_memory, target_address.get()) else {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        };
    };
    let updated = apply_futex_wake_operation(operation.operation, observed, operation.operand);
    if !write_guest_i32(guest_memory_writer, target_address.get(), updated) {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT),
        };
    }
    let Some(comparison) = decode_futex_wake_comparison(encoded_operation) else {
        return RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS),
        };
    };

    let primary = state
        .guest_futexes
        .wake(address, thread_group, wake_count, tick)
        .expect("guest futex wake-op primary wake cannot fail");
    let secondary_woken =
        if futex_wake_operation_compare(comparison.compare, observed, comparison.operand) {
            state
                .guest_futexes
                .wake(target_address, thread_group, second_wake_count, tick)
                .expect("guest futex wake-op secondary wake cannot fail")
                .woken_count()
        } else {
            0
        };
    RiscvSyscallOutcome::Return {
        value: (primary.woken_count() + secondary_woken) as u64,
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
    let timeout_status = match futex_wait_timeout_status(wait.timeout, tick, guest_memory) {
        Ok(timeout_status) => timeout_status,
        Err(outcome) => return Some(outcome),
    };
    let expected = request.argument(2) as i32;
    if observed != expected {
        return Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN),
        });
    }
    if timeout_status == RiscvFutexWaitTimeoutStatus::Expired {
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
        GuestFutexWaitOutcome::Queued { .. } => Some(RiscvSyscallOutcome::Blocked),
    }
}

fn futex_wait_timeout_status(
    timeout: RiscvFutexWaitTimeout,
    tick: Tick,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<RiscvFutexWaitTimeoutStatus, RiscvSyscallOutcome> {
    match timeout {
        RiscvFutexWaitTimeout::Relative(None) | RiscvFutexWaitTimeout::Absolute(None) => {
            Ok(RiscvFutexWaitTimeoutStatus::Pending)
        }
        RiscvFutexWaitTimeout::Relative(Some(timeout_address)) => {
            let timeout = read_futex_wait_timeout(timeout_address, guest_memory)?;
            if timeout.is_zero() {
                Ok(RiscvFutexWaitTimeoutStatus::Expired)
            } else {
                Ok(RiscvFutexWaitTimeoutStatus::Pending)
            }
        }
        RiscvFutexWaitTimeout::Absolute(Some(timeout_address)) => {
            let timeout = read_futex_wait_timeout(timeout_address, guest_memory)?;
            if timeout.total_nanoseconds() <= u128::from(tick) {
                Ok(RiscvFutexWaitTimeoutStatus::Expired)
            } else {
                Ok(RiscvFutexWaitTimeoutStatus::Pending)
            }
        }
    }
}

fn read_futex_wait_timeout(
    timeout_address: u64,
    guest_memory: &RiscvGuestMemoryReader,
) -> Result<super::time::RiscvLinuxTimespec64, RiscvSyscallOutcome> {
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
    Ok(timeout)
}

const fn futex_timeout_address(address: u64) -> Option<u64> {
    if address == 0 {
        None
    } else {
        Some(address)
    }
}

fn read_guest_i32(guest_memory: &RiscvGuestMemoryReader, address: u64) -> Option<i32> {
    let bytes = guest_memory.read(address, 4)?;
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    Some(i32::from_le_bytes(bytes))
}

fn write_guest_i32(guest_memory_writer: &RiscvGuestMemoryWriter, address: u64, value: i32) -> bool {
    guest_memory_writer.write(address, &value.to_le_bytes())
}

fn decode_futex_wake_operation(encoded: u32) -> Option<RiscvFutexWakeOperation> {
    let raw_operation = (encoded >> 28) & 0xf;
    let operation = raw_operation & !RISCV_LINUX_FUTEX_OP_ARG_SHIFT;
    if !matches!(
        operation,
        RISCV_LINUX_FUTEX_OP_SET
            | RISCV_LINUX_FUTEX_OP_ADD
            | RISCV_LINUX_FUTEX_OP_OR
            | RISCV_LINUX_FUTEX_OP_ANDN
            | RISCV_LINUX_FUTEX_OP_XOR
    ) {
        return None;
    }

    let raw_operand = (encoded >> 12) & 0xfff;
    let mut operand = sign_extend_futex_operand(raw_operand);
    if raw_operation & RISCV_LINUX_FUTEX_OP_ARG_SHIFT != 0 {
        operand = 1_i32.wrapping_shl(raw_operand & 31);
    }
    Some(RiscvFutexWakeOperation { operation, operand })
}

fn decode_futex_wake_comparison(encoded: u32) -> Option<RiscvFutexWakeComparison> {
    let compare = (encoded >> 24) & 0xf;
    if !matches!(
        compare,
        RISCV_LINUX_FUTEX_OP_CMP_EQ
            | RISCV_LINUX_FUTEX_OP_CMP_NE
            | RISCV_LINUX_FUTEX_OP_CMP_LT
            | RISCV_LINUX_FUTEX_OP_CMP_LE
            | RISCV_LINUX_FUTEX_OP_CMP_GT
            | RISCV_LINUX_FUTEX_OP_CMP_GE
    ) {
        return None;
    }

    Some(RiscvFutexWakeComparison {
        compare,
        operand: sign_extend_futex_operand(encoded & 0xfff),
    })
}

fn sign_extend_futex_operand(value: u32) -> i32 {
    ((value << 20) as i32) >> 20
}

fn apply_futex_wake_operation(operation: u32, observed: i32, operand: i32) -> i32 {
    match operation {
        RISCV_LINUX_FUTEX_OP_SET => operand,
        RISCV_LINUX_FUTEX_OP_ADD => observed.wrapping_add(operand),
        RISCV_LINUX_FUTEX_OP_OR => observed | operand,
        RISCV_LINUX_FUTEX_OP_ANDN => observed & !operand,
        RISCV_LINUX_FUTEX_OP_XOR => observed ^ operand,
        _ => unreachable!("futex wake-op decoder rejects unknown operations"),
    }
}

fn futex_wake_operation_compare(compare: u32, observed: i32, operand: i32) -> bool {
    match compare {
        RISCV_LINUX_FUTEX_OP_CMP_EQ => observed == operand,
        RISCV_LINUX_FUTEX_OP_CMP_NE => observed != operand,
        RISCV_LINUX_FUTEX_OP_CMP_LT => observed < operand,
        RISCV_LINUX_FUTEX_OP_CMP_LE => observed <= operand,
        RISCV_LINUX_FUTEX_OP_CMP_GT => observed > operand,
        RISCV_LINUX_FUTEX_OP_CMP_GE => observed >= operand,
        _ => unreachable!("futex wake-op decoder rejects unknown comparisons"),
    }
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
            RISCV_LINUX_FUTEX_REQUEUE | RISCV_LINUX_FUTEX_CMP_REQUEUE | RISCV_LINUX_FUTEX_WAKE_OP
        )
}
