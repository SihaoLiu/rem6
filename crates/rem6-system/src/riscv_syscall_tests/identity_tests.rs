use super::super::*;

const RISCV_LINUX_SETFSUID_FOR_TEST: u64 = 151;
const RISCV_LINUX_SETFSGID_FOR_TEST: u64 = 152;
const RISCV_LINUX_SETUID_FOR_TEST: u64 = 146;
const RISCV_LINUX_SETGID_FOR_TEST: u64 = 144;
const RISCV_LINUX_SETREUID_FOR_TEST: u64 = 145;
const RISCV_LINUX_SETREGID_FOR_TEST: u64 = 143;
const RISCV_LINUX_SETRESUID_FOR_TEST: u64 = 147;
const RISCV_LINUX_SETRESGID_FOR_TEST: u64 = 149;

#[test]
fn linux_table_setfsuid_and_setfsgid_return_previous_and_update_allowed_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETFSUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETFSUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SETFSGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_SETFSGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_SETFSUID_FOR_TEST, [11, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_SETFSUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8018, RISCV_LINUX_SETFSGID_FOR_TEST, [12, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x801c, RISCV_LINUX_SETFSGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_effective_root_setfsuid_and_setfsgid_accept_new_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 0, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETFSGID_FOR_TEST, [55, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETFSGID_FOR_TEST, [55, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 55 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SETFSUID_FOR_TEST, [44, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_SETFSUID_FOR_TEST, [44, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 44 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_file_system_identity_tracks_effective_identity_updates() {
    let table = RiscvSyscallTable::new();
    let mut setid_state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut setid_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETFSUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut setid_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SETGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut setid_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_SETFSGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut setid_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert!(setid_state.unknown_syscalls().is_empty());

    let mut setre_state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_SETREUID_FOR_TEST,
                [u64::MAX, 7, 0, 0, 0, 0],
            ),
            &mut setre_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_SETFSUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut setre_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_SETREGID_FOR_TEST,
                [u64::MAX, 9, 0, 0, 0, 0],
            ),
            &mut setre_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x801c, RISCV_LINUX_SETFSGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut setre_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert!(setre_state.unknown_syscalls().is_empty());

    let mut setres_state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8020,
                RISCV_LINUX_SETRESUID_FOR_TEST,
                [u64::MAX, 7, u64::MAX, 0, 0, 0],
            ),
            &mut setres_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8024, RISCV_LINUX_SETFSUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut setres_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8028,
                RISCV_LINUX_SETRESGID_FOR_TEST,
                [u64::MAX, 9, u64::MAX, 0, 0, 0],
            ),
            &mut setres_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x802c, RISCV_LINUX_SETFSGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut setres_state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert!(setres_state.unknown_syscalls().is_empty());
}
