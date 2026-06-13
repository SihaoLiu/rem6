use super::*;

const RISCV_LINUX_CLOCK_GETRES_FOR_TEST: u64 = 114;

#[test]
fn linux_table_clock_gettime_accepts_tai_clock_id() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
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
            RiscvSyscallRequest::new(0x7ff0, RISCV_LINUX_CLOCK_GETTIME, [11, 0x8ff0, 0, 0, 0, 0],),
            &mut state,
            2_000_000_123,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x8ff0, 16);
    assert_eq!(read_le_u64(&bytes, 0), 2);
    assert_eq!(read_le_u64(&bytes, 8), 123);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_getres_writes_qemu_observed_resolutions() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (clock_id, address, expected_nanoseconds) in [
        (0, 0x9000, 1),
        (1, 0x9020, 1),
        (2, 0x9040, 1),
        (3, 0x9060, 1),
        (4, 0x9080, 1),
        (5, 0x90a0, 1_000_000),
        (6, 0x90c0, 1_000_000),
        (7, 0x90e0, 1),
        (11, 0x9100, 1),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    0x8000 + clock_id,
                    RISCV_LINUX_CLOCK_GETRES_FOR_TEST,
                    [clock_id, address, 0, 0, 0, 0],
                ),
                &mut state,
                31,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        let bytes = collect_guest_writes(&writes.lock().unwrap(), address, 16);
        assert_eq!(read_le_u64(&bytes, 0), 0);
        assert_eq!(read_le_u64(&bytes, 8), expected_nanoseconds);
        writes.lock().unwrap().clear();
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_getres_accepts_null_timespec_for_valid_clock() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_address, _bytes| panic!("NULL clock_getres must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8020,
                RISCV_LINUX_CLOCK_GETRES_FOR_TEST,
                [0, 0, 0, 0, 0, 0],
            ),
            &mut state,
            32,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_getres_reports_invalid_clock_id() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("invalid clock_getres clock must not write")
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8024,
                RISCV_LINUX_CLOCK_GETRES_FOR_TEST,
                [99, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            33,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_getres_reports_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, _bytes| address != 0x9008);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8028,
                RISCV_LINUX_CLOCK_GETRES_FOR_TEST,
                [0, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            34,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}
