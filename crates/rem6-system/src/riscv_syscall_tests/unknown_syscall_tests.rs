use super::*;

const RISCV_LINUX_LOOKUP_DCOOKIE_FOR_TEST: u64 = 18;
const RISCV_LINUX_NFSSERVCTL_FOR_TEST: u64 = 42;
const RISCV_LINUX_IO_URING_SETUP_FOR_TEST: u64 = 425;
const RISCV_LINUX_IO_URING_ENTER_FOR_TEST: u64 = 426;
const RISCV_LINUX_IO_URING_REGISTER_FOR_TEST: u64 = 427;
const RISCV_LINUX_CLONE3_FOR_TEST: u64 = 435;
const RISCV_LINUX_PROCESS_MADVISE_FOR_TEST: u64 = 440;
const RISCV_LINUX_LANDLOCK_CREATE_RULESET_FOR_TEST: u64 = 444;
const RISCV_LINUX_LANDLOCK_ADD_RULE_FOR_TEST: u64 = 445;
const RISCV_LINUX_LANDLOCK_RESTRICT_SELF_FOR_TEST: u64 = 446;
const RISCV_LINUX_PROCESS_MRELEASE_FOR_TEST: u64 = 448;
const RISCV_LINUX_FUTEX_WAITV_FOR_TEST: u64 = 449;
const RISCV_LINUX_SET_MEMPOLICY_HOME_NODE_FOR_TEST: u64 = 450;
const RISCV_LINUX_CACHESTAT_FOR_TEST: u64 = 451;

#[test]
fn linux_table_unknown_numbers_return_enosys_and_record_request() {
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 9999, [0; 6]),
            &mut state,
            43
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(state.program_break(), 0);
    assert!(state.guest_writes().is_empty());
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(0x8000, 9999, [0; 6], 43)]
    );
}

#[test]
fn linux_table_known_ni_syscalls_return_enosys_without_unknown_records() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, number) in [
        (0x8000, RISCV_LINUX_LOOKUP_DCOOKIE_FOR_TEST),
        (0x8004, RISCV_LINUX_NFSSERVCTL_FOR_TEST),
        (0x8008, RISCV_LINUX_IO_URING_SETUP_FOR_TEST),
        (0x800c, RISCV_LINUX_IO_URING_ENTER_FOR_TEST),
        (0x8010, RISCV_LINUX_IO_URING_REGISTER_FOR_TEST),
        (0x8014, RISCV_LINUX_CLONE3_FOR_TEST),
        (0x8018, RISCV_LINUX_PROCESS_MADVISE_FOR_TEST),
        (0x801c, RISCV_LINUX_LANDLOCK_CREATE_RULESET_FOR_TEST),
        (0x8020, RISCV_LINUX_LANDLOCK_ADD_RULE_FOR_TEST),
        (0x8024, RISCV_LINUX_LANDLOCK_RESTRICT_SELF_FOR_TEST),
        (0x8028, RISCV_LINUX_PROCESS_MRELEASE_FOR_TEST),
        (0x802c, RISCV_LINUX_FUTEX_WAITV_FOR_TEST),
        (0x8030, RISCV_LINUX_SET_MEMPOLICY_HOME_NODE_FOR_TEST),
        (0x8034, RISCV_LINUX_CACHESTAT_FOR_TEST),
    ] {
        assert_eq!(
            table.handle_at_tick(RiscvSyscallRequest::new(pc, number, [0; 6]), &mut state, 37),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ENOSYS)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}
