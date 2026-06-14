use super::*;

const RISCV_LINUX_ENOMEM_FOR_TEST: u64 = 12;
const RISCV_LINUX_MSYNC_FOR_TEST: u64 = 227;
const RISCV_LINUX_MS_ASYNC_FOR_TEST: u64 = 1;
const RISCV_LINUX_MS_INVALIDATE_FOR_TEST: u64 = 2;
const RISCV_LINUX_MS_SYNC_FOR_TEST: u64 = 4;

#[test]
fn linux_table_msync_accepts_supported_flags_for_tracked_mappings() {
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

    for flags in [
        0,
        RISCV_LINUX_MS_ASYNC_FOR_TEST,
        RISCV_LINUX_MS_SYNC_FOR_TEST,
        RISCV_LINUX_MS_INVALIDATE_FOR_TEST,
        RISCV_LINUX_MS_ASYNC_FOR_TEST | RISCV_LINUX_MS_INVALIDATE_FOR_TEST,
        RISCV_LINUX_MS_SYNC_FOR_TEST | RISCV_LINUX_MS_INVALIDATE_FOR_TEST,
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_MSYNC_FOR_TEST,
                    [
                        RISCV64_LINUX_MMAP_BASE,
                        RISCV_PAGE_BYTES + 3,
                        flags,
                        0,
                        0,
                        0
                    ],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_msync_accepts_zero_length_probe() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MSYNC_FOR_TEST,
                [0, 0, RISCV_LINUX_MS_SYNC_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_msync_rejects_unmapped_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MSYNC_FOR_TEST,
                [
                    RISCV64_LINUX_MMAP_BASE,
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MS_SYNC_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOMEM_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_msync_rejects_invalid_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for arguments in [
        [
            RISCV64_LINUX_MMAP_BASE + 1,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MS_SYNC_FOR_TEST,
            0,
            0,
            0,
        ],
        [
            RISCV64_LINUX_MMAP_BASE,
            RISCV_PAGE_BYTES,
            RISCV_LINUX_MS_ASYNC_FOR_TEST | RISCV_LINUX_MS_SYNC_FOR_TEST,
            0,
            0,
            0,
        ],
        [RISCV64_LINUX_MMAP_BASE, RISCV_PAGE_BYTES, 8, 0, 0, 0],
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(0x8000, RISCV_LINUX_MSYNC_FOR_TEST, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
}

#[test]
fn linux_table_msync_reports_enomem_for_overflowing_address_range() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MSYNC_FOR_TEST,
                [
                    u64::MAX - (RISCV_PAGE_BYTES - 1),
                    RISCV_PAGE_BYTES,
                    RISCV_LINUX_MS_SYNC_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOMEM_FOR_TEST)
        })
    );
}
