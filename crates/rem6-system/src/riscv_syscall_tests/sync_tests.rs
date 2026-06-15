use super::*;

const RISCV_LINUX_SYNC_FOR_TEST: u64 = 81;
const RISCV_LINUX_FSYNC_FOR_TEST: u64 = 82;
const RISCV_LINUX_FDATASYNC_FOR_TEST: u64 = 83;
const RISCV_LINUX_SYNCFS_FOR_TEST: u64 = 267;

#[test]
fn linux_table_handles_sync_family_without_unknown_records() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SYNC_FOR_TEST, [0; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    for (pc, syscall) in [
        (0x8004, RISCV_LINUX_FSYNC_FOR_TEST),
        (0x8008, RISCV_LINUX_FDATASYNC_FOR_TEST),
        (0x800c, RISCV_LINUX_SYNCFS_FOR_TEST),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc, syscall, [1, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc + 0x100, syscall, [99, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc + 0x200, syscall, [(1_u64 << 32) | 1, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EBADF)
            })
        );
    }

    assert!(state.unknown_syscalls().is_empty());
}
