use super::*;

const RISCV_LINUX_SYSLOG_FOR_TEST: u64 = 116;
const RISCV_LINUX_EPERM_FOR_TEST: u64 = 1;

#[test]
fn linux_table_syslog_reports_deterministic_errors_without_unknown_records() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, command, errno) in [
        (0x8000, 0, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8004, 1, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8008, 2, RISCV_LINUX_EPERM_FOR_TEST),
        (0x800c, 3, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8010, 4, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8014, 5, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8018, 6, RISCV_LINUX_EPERM_FOR_TEST),
        (0x801c, 7, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8020, 8, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8024, 9, RISCV_LINUX_EPERM_FOR_TEST),
        (0x8028, 10, RISCV_LINUX_EPERM_FOR_TEST),
        (0x802c, 99, RISCV_LINUX_EINVAL),
        (0x8030, u64::MAX, RISCV_LINUX_EINVAL),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SYSLOG_FOR_TEST,
                    [command, 0x9000, 4096, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}
