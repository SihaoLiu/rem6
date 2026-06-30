use super::*;

const RISCV_LINUX_SETPRIORITY_FOR_TEST: u64 = 140;
const RISCV_LINUX_GETPRIORITY_FOR_TEST: u64 = 141;
const RISCV_LINUX_PRIO_PROCESS_FOR_TEST: u64 = 0;
const RISCV_LINUX_PRIO_PGRP_FOR_TEST: u64 = 1;
const RISCV_LINUX_PRIO_USER_FOR_TEST: u64 = 2;

#[test]
fn linux_table_process_priority_accepts_current_group_and_user_scopes() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, number, arguments, tick, value) in [
        (
            0x8100,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_PGRP_FOR_TEST, 0, 0, 0, 0, 0],
            16,
            20,
        ),
        (
            0x8104,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_PGRP_FOR_TEST, 41, 0, 0, 0, 0],
            17,
            20,
        ),
        (
            0x8108,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_USER_FOR_TEST, 0, 0, 0, 0, 0],
            18,
            20,
        ),
        (
            0x810c,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_USER_FOR_TEST, 7, 0, 0, 0, 0],
            19,
            20,
        ),
        (
            0x8110,
            RISCV_LINUX_SETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_PGRP_FOR_TEST, 41, 9, 0, 0, 0],
            20,
            0,
        ),
        (
            0x8114,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_PROCESS_FOR_TEST, 0, 0, 0, 0, 0],
            21,
            11,
        ),
        (
            0x8118,
            RISCV_LINUX_SETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_USER_FOR_TEST, 7, 12, 0, 0, 0],
            22,
            0,
        ),
        (
            0x811c,
            RISCV_LINUX_GETPRIORITY_FOR_TEST,
            [RISCV_LINUX_PRIO_PROCESS_FOR_TEST, 0, 0, 0, 0, 0],
            23,
            8,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, number, arguments),
                &mut state,
                tick,
                None,
                None,
            ),
            Some(RiscvSyscallOutcome::Return { value }),
            "priority syscall at {pc:#x}"
        );
    }

    assert!(state.unknown_syscalls().is_empty());
}
