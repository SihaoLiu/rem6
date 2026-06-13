use super::*;

const RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST: u64 = 122;
const RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST: u64 = 123;
const RISCV_LINUX_ESRCH_FOR_TEST: u64 = 3;
const RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST: u64 = 8;
const RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST: u64 = 1;

#[test]
fn linux_table_sched_setaffinity_accepts_single_cpu_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let mask_address = 0x9000;
    let guest_memory_reader =
        affinity_reader_at(mask_address, RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    mask_address,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_accepts_current_process_and_thread_ids() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, pid, mask_address) in [(0x8000, 41, 0x9000), (0x8004, 42, 0x9010)] {
        let guest_memory_reader =
            affinity_reader_at(mask_address, RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST);
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                    [
                        pid,
                        RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                        mask_address,
                        0,
                        0,
                        0,
                    ],
                ),
                &mut state,
                10,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_masks_without_guest_cpu_zero() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, mask) in [(0x8000, 0), (0x8004, 2)] {
        let guest_memory_reader = affinity_reader_at(0x9000, mask);
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                    [
                        0,
                        RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                        0x9000,
                        0,
                        0,
                        0,
                    ],
                ),
                &mut state,
                10,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_short_sizes_after_reading_guest_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, 4);
        Some(RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST.to_le_bytes()[..4].to_vec())
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [0, 4, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_zero_size_without_guest_read() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [0, 0, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_truncates_oversized_guest_mask_to_single_word() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize);
        Some(
            RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST
                .to_le_bytes()
                .to_vec(),
        )
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [0, 16, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_rejects_unknown_pid_after_reading_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader =
        affinity_reader_at(0x9000, RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    999,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_reports_guest_read_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(bytes, RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize);
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_setaffinity_without_guest_reader_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_SETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            10,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_writes_current_thread_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let mask_address = 0x9000;
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    mask_address,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        written_mask_at(&writes.lock().unwrap(), mask_address),
        RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST.to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_accepts_current_process_and_thread_ids() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (pc, pid, mask_address) in [(0x8000, 41, 0x9000), (0x8004, 42, 0x9010)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                    [
                        pid,
                        RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                        mask_address,
                        0,
                        0,
                        0,
                    ],
                ),
                &mut state,
                11,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST
            })
        );
        assert_eq!(
            written_mask_at(&writes.lock().unwrap(), mask_address),
            RISCV_LINUX_DEFAULT_AFFINITY_MASK_FOR_TEST.to_le_bytes()
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_rejects_short_and_unaligned_sizes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("invalid size must not write"));

    for (pc, size) in [(0x8000, 0), (0x8004, 4), (0x8008, 12)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                    [0, size, 0x9000, 0, 0, 0],
                ),
                &mut state,
                12,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_rejects_unknown_pid() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_, _| panic!("unknown pid must not write affinity"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    999,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            13,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_reports_guest_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, bytes| {
        assert_eq!(address, 0x9000);
        assert_eq!(
            bytes.len(),
            RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize
        );
        false
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            14,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sched_getaffinity_without_guest_writer_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SCHED_GETAFFINITY_FOR_TEST,
                [
                    0,
                    RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST,
                    0x9000,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            15,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn written_mask_at(writes: &[(u64, Vec<u8>)], address: u64) -> [u8; 8] {
    writes
        .iter()
        .find_map(|(written_address, bytes)| {
            (*written_address == address).then(|| bytes.as_slice().try_into().unwrap())
        })
        .unwrap_or_else(|| panic!("missing affinity write at {address:#x}"))
}

fn affinity_reader_at(address: u64, mask: u64) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |read_address, bytes| {
        assert_eq!(read_address, address);
        assert_eq!(bytes, RISCV_LINUX_DEFAULT_AFFINITY_BYTES_FOR_TEST as usize);
        Some(mask.to_le_bytes().to_vec())
    })
}
