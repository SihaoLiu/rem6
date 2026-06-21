use super::*;

const RISCV_LINUX_SETNS_FOR_TEST: u64 = 268;
const RISCV_LINUX_CLONE_NEWNS_FOR_TEST: u64 = 0x0002_0000;
const RISCV_LINUX_CLONE_THREAD_FOR_TEST: u64 = 0x0001_0000;

#[test]
fn linux_table_setns_rejects_invalid_fd_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETNS_FOR_TEST,
                [u64::MAX, 0, 0, 0, 0, 0],
            ),
            &mut state,
            17,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setns_rejects_non_namespace_fd_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, flags) in [(0x8000, 0), (0x8004, RISCV_LINUX_CLONE_NEWNS_FOR_TEST)] {
        assert_eq!(
            table.handle_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_SETNS_FOR_TEST, [1, flags, 0, 0, 0, 0],),
                &mut state,
                19,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setns_rejects_invalid_namespace_type_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETNS_FOR_TEST,
                [1, RISCV_LINUX_CLONE_THREAD_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            23,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}
