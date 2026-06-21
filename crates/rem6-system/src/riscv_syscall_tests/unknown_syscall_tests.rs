use super::*;

const RISCV_LINUX_LOOKUP_DCOOKIE_FOR_TEST: u64 = 18;
const RISCV_LINUX_NFSSERVCTL_FOR_TEST: u64 = 42;

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
