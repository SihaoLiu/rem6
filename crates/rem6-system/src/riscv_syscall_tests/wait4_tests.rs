use super::super::*;
use crate::{GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestWaitStatus};

const RISCV_LINUX_ECHILD_FOR_TEST: u64 = 10;
const RISCV_LINUX_WNOHANG_FOR_TEST: u64 = 1;
const RISCV64_LINUX_RUSAGE_BYTES: usize = 144;

fn child(pid: u32, process_group: u32, status: GuestWaitStatus) -> GuestChildStatus {
    GuestChildStatus::new(
        GuestProcessId::new(pid).unwrap(),
        GuestProcessGroupId::new(process_group).unwrap(),
        status,
    )
}

#[test]
fn linux_table_wait4_reaps_child_when_status_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_wait_child(child(123, 100, GuestWaitStatus::exited(7)));
    let faulting_guest_memory_writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, &((7_i32) << 8).to_le_bytes());
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&faulting_guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.guest_wait_queue().is_empty());
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WAIT4, [(-1_i64) as u64, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
        })
    );
}

#[test]
fn linux_table_wait4_wnohang_empty_queue_returns_echild() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0, RISCV_LINUX_WNOHANG_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
        })
    );
}

#[test]
fn linux_table_wait4_wnohang_unmatched_selector_returns_echild() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let other_child = child(123, 100, GuestWaitStatus::exited(7));
    state.push_wait_child(other_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [456, 0, RISCV_LINUX_WNOHANG_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ECHILD_FOR_TEST)
        })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[other_child]
    );
}

#[test]
fn linux_table_wait4_pid_zero_uses_process_group_not_credential_group() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let credential_group_child = child(91, 9, GuestWaitStatus::exited(1));
    let current_group_child = child(52, 41, GuestWaitStatus::exited(5));
    state.push_wait_child(credential_group_child);
    state.push_wait_child(current_group_child);

    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 52 })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[credential_group_child]
    );
}

#[test]
fn linux_table_wait4_writes_zero_rusage_for_reaped_child() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_wait_child(child(123, 100, GuestWaitStatus::exited(7)));
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_WAIT4,
                [(-1_i64) as u64, 0x9000, 0, 0xa000, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 123 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        writes.as_slice(),
        &[
            (0x9000, ((7_i32) << 8).to_le_bytes().to_vec()),
            (0xa000, vec![0; RISCV64_LINUX_RUSAGE_BYTES]),
        ]
    );
    assert!(state.guest_wait_queue().is_empty());
}
