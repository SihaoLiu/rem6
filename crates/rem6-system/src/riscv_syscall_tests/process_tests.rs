use super::super::*;
use crate::{GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestWaitStatus};

fn child(pid: u32, process_group: u32, status: GuestWaitStatus) -> GuestChildStatus {
    GuestChildStatus::new(
        GuestProcessId::new(pid).unwrap(),
        GuestProcessGroupId::new(process_group).unwrap(),
        status,
    )
}

#[test]
fn linux_table_getpgid_and_getsid_report_current_process_scope() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETPGID, [41, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_GETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_GETSID, [41, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_process_group_queries_validate_pid_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETPGID, [99, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETSID,
                [0x0000_0000_ffff_ffff, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setpgid_rejects_current_session_leader_and_preserves_wait_group() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let original_group_child = child(51, 41, GuestWaitStatus::exited(1));
    let new_group_child = child(52, 77, GuestWaitStatus::exited(2));
    state.push_wait_child(original_group_child);
    state.push_wait_child(new_group_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETPGID, [0, 77, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 51 })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[new_group_child]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setpgid_can_create_group_for_current_nonleader() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::with_identity_process_group_and_session(
        0,
        RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10),
        GuestProcessGroupId::new(77).unwrap(),
        77,
    );
    let old_group_child = child(51, 77, GuestWaitStatus::exited(1));
    let new_group_child = child(52, 41, GuestWaitStatus::exited(2));
    state.push_wait_child(old_group_child);
    state.push_wait_child(new_group_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 52 })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[old_group_child]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setpgid_validates_target_and_group_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, arguments, error) in [
        (0x8000, [99, 99, 0, 0, 0, 0], RISCV_LINUX_ESRCH),
        (
            0x8004,
            [0x0000_0000_ffff_ffff, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8008,
            [0, 0x0000_0000_ffff_ffff, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x800c,
            [99, 0x0000_0000_ffff_ffff, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_SETPGID, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(error)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setsid_rejects_current_process_group_leader() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setsid_creates_session_for_current_nonleader() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::with_identity_process_group_and_session(
        0,
        RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10),
        GuestProcessGroupId::new(77).unwrap(),
        77,
    );

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_wait4_after_rejected_setpgid_still_uses_current_process_group() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let original_group_child = child(51, 41, GuestWaitStatus::exited(1));
    state.push_wait_child(original_group_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETPGID, [0, 77, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 51 })
    );
    assert!(state.guest_wait_queue().is_empty());
}
