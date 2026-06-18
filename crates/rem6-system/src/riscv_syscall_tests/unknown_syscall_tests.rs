use super::*;

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
