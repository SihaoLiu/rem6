use super::{RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallState};
use crate::GuestFutexAddress;

pub(super) const RISCV_LINUX_EXIT: u64 = 93;
pub(super) const RISCV_LINUX_EXIT_GROUP: u64 = 94;

pub(super) fn syscall_exit_code(value: u64) -> i32 {
    value.min(i32::MAX as u64) as i32
}

pub(super) fn syscall_exit(
    value: u64,
    state: &mut RiscvSyscallState,
    tick: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) -> RiscvSyscallOutcome {
    consume_child_clear_tid(state, tick, guest_memory_writer);
    RiscvSyscallOutcome::Exit {
        code: syscall_exit_code(value),
    }
}

fn consume_child_clear_tid(
    state: &mut RiscvSyscallState,
    tick: u64,
    guest_memory_writer: Option<&RiscvGuestMemoryWriter>,
) {
    let Some(clear_tid) = state.child_clear_tid() else {
        return;
    };
    let Some(guest_memory_writer) = guest_memory_writer else {
        return;
    };
    if !guest_memory_writer.write(clear_tid, &0_i32.to_le_bytes()) {
        return;
    }

    state.set_child_clear_tid(0);
    let thread_group = state.identity().thread_group_id();
    let _ = state.guest_futexes.wake(
        GuestFutexAddress::new(clear_tid),
        crate::GuestThreadGroupId::new(thread_group),
        1,
        tick,
    );
}
