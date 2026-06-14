use super::*;

const RISCV_LINUX_MLOCK_FOR_TEST: u64 = 228;
const RISCV_LINUX_MUNLOCK_FOR_TEST: u64 = 229;
const RISCV_LINUX_ENOMEM_FOR_TEST: u64 = 12;

#[test]
fn linux_table_mlock_and_munlock_accept_mmap_backed_unaligned_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, 2 * RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );

    for number in [RISCV_LINUX_MLOCK_FOR_TEST, RISCV_LINUX_MUNLOCK_FOR_TEST] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    number,
                    [RISCV64_LINUX_MMAP_BASE + 17, RISCV_PAGE_BYTES, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_mlock_and_munlock_accept_brk_backed_heap_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0x8000);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_BRK, [0xa001, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0xa001 })
    );

    for number in [RISCV_LINUX_MLOCK_FOR_TEST, RISCV_LINUX_MUNLOCK_FOR_TEST] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8004, number, [0x8017, 0x1000, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
}

#[test]
fn linux_table_mlock_and_munlock_accept_zero_length_probe() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [RISCV_LINUX_MLOCK_FOR_TEST, RISCV_LINUX_MUNLOCK_FOR_TEST] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, number, [u64::MAX, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
}

#[test]
fn linux_table_mlock_and_munlock_reject_unmapped_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [RISCV_LINUX_MLOCK_FOR_TEST, RISCV_LINUX_MUNLOCK_FOR_TEST] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8000,
                    number,
                    [RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ENOMEM_FOR_TEST)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_mlock_and_munlock_reject_partly_unmapped_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MMAP,
                [0, RISCV_PAGE_BYTES, 3, 34, u64::MAX, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );

    for number in [RISCV_LINUX_MLOCK_FOR_TEST, RISCV_LINUX_MUNLOCK_FOR_TEST] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    number,
                    [
                        RISCV64_LINUX_MMAP_BASE + RISCV_PAGE_BYTES - 1,
                        2,
                        0,
                        0,
                        0,
                        0
                    ],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_ENOMEM_FOR_TEST)
            })
        );
    }
}

#[test]
fn linux_table_mlock_and_munlock_report_einval_for_overflowing_address_range() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [RISCV_LINUX_MLOCK_FOR_TEST, RISCV_LINUX_MUNLOCK_FOR_TEST] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, number, [u64::MAX, 1, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
}
