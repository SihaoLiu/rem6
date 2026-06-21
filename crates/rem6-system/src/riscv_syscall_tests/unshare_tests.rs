use super::*;

const RISCV_LINUX_UNSHARE_FOR_TEST: u64 = 97;
const RISCV_LINUX_CLONE_FS_FOR_TEST: u64 = 0x0000_0200;
const RISCV_LINUX_CLONE_FILES_FOR_TEST: u64 = 0x0000_0400;
const RISCV_LINUX_CLONE_SYSVSEM_FOR_TEST: u64 = 0x0004_0000;
const RISCV_LINUX_CLONE_NEWNS_FOR_TEST: u64 = 0x0002_0000;
const RISCV_LINUX_CLONE_THREAD_FOR_TEST: u64 = 0x0001_0000;

#[test]
fn linux_table_unshare_accepts_single_process_resource_flags() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, flags) in [
        (0x8000, 0),
        (0x8004, RISCV_LINUX_CLONE_FS_FOR_TEST),
        (0x8008, RISCV_LINUX_CLONE_FILES_FOR_TEST),
        (0x800c, RISCV_LINUX_CLONE_SYSVSEM_FOR_TEST),
        (
            0x8010,
            RISCV_LINUX_CLONE_FS_FOR_TEST
                | RISCV_LINUX_CLONE_FILES_FOR_TEST
                | RISCV_LINUX_CLONE_SYSVSEM_FOR_TEST,
        ),
    ] {
        assert_eq!(
            table.handle_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_UNSHARE_FOR_TEST, [flags, 0, 0, 0, 0, 0],),
                &mut state,
                9,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_unshare_rejects_namespace_flags_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_UNSHARE_FOR_TEST,
                [RISCV_LINUX_CLONE_NEWNS_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
            11,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_unshare_rejects_invalid_flags_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_UNSHARE_FOR_TEST,
                [RISCV_LINUX_CLONE_THREAD_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
            13,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}
