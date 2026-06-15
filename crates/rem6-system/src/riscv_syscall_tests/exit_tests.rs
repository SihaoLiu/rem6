use super::*;

#[test]
fn linux_table_maps_exit_numbers_to_stop_codes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT, [17; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Exit { code: 17 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT_GROUP, [19; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Exit { code: 19 })
    );
}
