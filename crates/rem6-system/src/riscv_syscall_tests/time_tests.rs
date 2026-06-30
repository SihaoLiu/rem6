use super::*;

const RISCV_LINUX_CLOCK_GETRES_FOR_TEST: u64 = 114;
const RISCV_LINUX_GETTIMEOFDAY_FOR_TEST: u64 = 169;
const RISCV_LINUX_SETTIMEOFDAY_FOR_TEST: u64 = 170;
const RISCV_LINUX_ADJTIMEX_FOR_TEST: u64 = 171;
const RISCV_LINUX_GETITIMER_FOR_TEST: u64 = 102;
const RISCV_LINUX_SETITIMER_FOR_TEST: u64 = 103;
const RISCV_NEWLIB_CLOCK_GETTIME64_FOR_TEST: u64 = 403;
const RISCV_NEWLIB_LEGACY_TIME_FOR_TEST: u64 = 1062;
const RISCV_LINUX_ITIMER_REAL_FOR_TEST: u64 = 0;
const RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST: u64 = 1;
const RISCV_LINUX_ITIMER_PROF_FOR_TEST: u64 = 2;
const RISCV_LINUX_TIMEX_BYTES_FOR_TEST: usize = 208;
const RISCV_LINUX_ADJ_OFFSET_FOR_TEST: u32 = 0x0001;

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
fn linux_table_newlib_clock_gettime64_writes_deterministic_timespec() {
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
            RiscvSyscallRequest::new(
                0x7ff2,
                RISCV_NEWLIB_CLOCK_GETTIME64_FOR_TEST,
                [0, 0x8fe0, 0, 0, 0, 0],
            ),
            &mut state,
            6_000_000_789,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x8fe0, 16);
    assert_eq!(read_le_u64(&bytes, 0), 6);
    assert_eq!(read_le_u64(&bytes, 8), 789);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_gettimeofday_writes_timeval_and_timezone_from_tick() {
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
            RiscvSyscallRequest::new(
                0x7ff4,
                RISCV_LINUX_GETTIMEOFDAY_FOR_TEST,
                [0x9000, 0x9020, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let timeval_writes = writes_in_range(&writes, 0x9000, 16);
    let timezone_writes = writes_in_range(&writes, 0x9020, 8);
    assert_eq!(
        write_addresses(&timeval_writes),
        (0x9000..0x9010).collect::<Vec<_>>()
    );
    assert_eq!(
        write_addresses(&timezone_writes),
        (0x9020..0x9028).collect::<Vec<_>>()
    );
    let timeval = collect_guest_writes(&timeval_writes, 0x9000, 16);
    let timezone = collect_guest_writes(&timezone_writes, 0x9020, 8);
    assert_eq!(read_le_u64(&timeval, 0), 2);
    assert_eq!(read_le_u64(&timeval, 8), 3);
    assert_eq!(read_le_u32(&timezone, 0), 0);
    assert_eq!(read_le_u32(&timezone, 4), 0);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_gettimeofday_writes_timezone_when_timeval_is_null() {
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
            RiscvSyscallRequest::new(
                0x7ff6,
                RISCV_LINUX_GETTIMEOFDAY_FOR_TEST,
                [0, 0x9020, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        write_addresses(&writes),
        (0x9020..0x9028).collect::<Vec<_>>()
    );
    let timezone = collect_guest_writes(&writes, 0x9020, 8);
    assert_eq!(read_le_u32(&timezone, 0), 0);
    assert_eq!(read_le_u32(&timezone, 4), 0);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_gettimeofday_accepts_null_timeval_without_writer_when_timezone_is_null() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x7ff8,
                RISCV_LINUX_GETTIMEOFDAY_FOR_TEST,
                [0, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_time_returns_seconds_and_writes_optional_time_t() {
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
            RiscvSyscallRequest::new(
                0x7ffa,
                RISCV_NEWLIB_LEGACY_TIME_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
            ),
            &mut state,
            3_000_000_123,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    assert_eq!(read_le_u64(&bytes, 0), 3);
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x7ffb,
                RISCV_NEWLIB_LEGACY_TIME_FOR_TEST,
                [0, 0, 0, 0, 0, 0],
            ),
            &mut state,
            4_000_000_456,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert!(writes.lock().unwrap().is_empty());

    let faulting_writer = RiscvGuestMemoryWriter::new(|_, _| false);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x7ffc,
                RISCV_NEWLIB_LEGACY_TIME_FOR_TEST,
                [0x9008, 0, 0, 0, 0, 0],
            ),
            &mut state,
            5_000_000_789,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_gettimeofday_reports_guest_write_faults() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, arguments, fault_address) in [
        (0x7ffc, [0x9000, 0, 0, 0, 0, 0], 0x9008),
        (0x8000, [0, 0x9020, 0, 0, 0, 0], 0x9024),
    ] {
        let guest_memory_writer =
            RiscvGuestMemoryWriter::new(move |address, _bytes| address != fault_address);
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_GETTIMEOFDAY_FOR_TEST, arguments),
                &mut state,
                2_000_003_456,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EFAULT)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_settimeofday_accepts_null_timeval_and_timezone_noop() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETTIMEOFDAY_FOR_TEST,
                [0, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_settimeofday_validates_timeval_before_denial() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let valid_timeval_address = 0x9000;
    let invalid_usec_address = 0x9020;
    let negative_seconds_address = 0x9040;
    let timezone_address = 0x9060;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == valid_timeval_address && bytes == 16 {
            Some(timeval_bytes(1, 999_999))
        } else if address == invalid_usec_address && bytes == 16 {
            Some(timeval_bytes(1, 1_000_000))
        } else if address == negative_seconds_address && bytes == 16 {
            Some(timeval_bytes(-1, 0))
        } else if address == timezone_address && bytes == 8 {
            Some(vec![0; 8])
        } else {
            None
        }
    });

    for (pc, arguments, errno) in [
        (
            0x8004,
            [valid_timeval_address, 0, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8006,
            [valid_timeval_address, timezone_address, 0, 0, 0, 0],
            RISCV_LINUX_EPERM,
        ),
        (
            0x8008,
            [invalid_usec_address, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x800c,
            [negative_seconds_address, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (0x8010, [0, timezone_address, 0, 0, 0, 0], RISCV_LINUX_EPERM),
        (0x8014, [0x9080, 0, 0, 0, 0, 0], RISCV_LINUX_EFAULT),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_SETTIMEOFDAY_FOR_TEST, arguments),
                &mut state,
                2_000_003_456,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_settimeofday_reads_timezone_when_timeval_is_present() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let timeval_address = 0x9000;
    let timezone_address = 0x9020;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == timeval_address && bytes == 16 {
            Some(timeval_bytes(1, 999_999))
        } else if address == timezone_address && bytes == 8 {
            None
        } else {
            panic!("unexpected settimeofday read at 0x{address:x} for {bytes} bytes")
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8016,
                RISCV_LINUX_SETTIMEOFDAY_FOR_TEST,
                [timeval_address, timezone_address, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
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
fn linux_table_settimeofday_without_guest_reader_stays_unhandled_for_non_null_pointer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_SETTIMEOFDAY_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
            None,
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_adjtimex_query_writes_tick_derived_snapshot() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let timex_address = 0x9000;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == timex_address && bytes == RISCV_LINUX_TIMEX_BYTES_FOR_TEST {
            Some(timex_bytes(0))
        } else {
            None
        }
    });
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
                0x801a,
                RISCV_LINUX_ADJTIMEX_FOR_TEST,
                [timex_address, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        write_addresses(&writes),
        (timex_address..timex_address + RISCV_LINUX_TIMEX_BYTES_FOR_TEST as u64)
            .collect::<Vec<_>>()
    );
    let timex = collect_guest_writes(&writes, timex_address, RISCV_LINUX_TIMEX_BYTES_FOR_TEST);
    assert_eq!(read_le_u32(&timex, 0), 0);
    assert_eq!(read_le_i64(&timex, 72), 2);
    assert_eq!(read_le_i64(&timex, 80), 3);
    assert_eq!(read_le_i64(&timex, 88), 10_000);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_adjtimex_reports_argument_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let valid_adjustment_address = 0x9000;
    let invalid_modes_address = 0x9100;
    let query_address = 0x9200;
    let fault_address = 0x9300;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != RISCV_LINUX_TIMEX_BYTES_FOR_TEST {
            return None;
        }
        if address == valid_adjustment_address {
            Some(timex_bytes(RISCV_LINUX_ADJ_OFFSET_FOR_TEST))
        } else if address == invalid_modes_address {
            Some(timex_bytes(0x4000_0000))
        } else if address == query_address {
            Some(timex_bytes(0))
        } else {
            None
        }
    });
    let faulting_writer = RiscvGuestMemoryWriter::new(move |address, _bytes| {
        address != fault_address && address != query_address + 8
    });

    for (pc, argument, errno) in [
        (0x801c, 0, RISCV_LINUX_EFAULT),
        (0x8020, valid_adjustment_address, RISCV_LINUX_EPERM),
        (0x8024, invalid_modes_address, RISCV_LINUX_EINVAL),
        (0x8028, fault_address, RISCV_LINUX_EFAULT),
        (0x802c, query_address, RISCV_LINUX_EFAULT),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_ADJTIMEX_FOR_TEST,
                    [argument, 0, 0, 0, 0, 0],
                ),
                &mut state,
                2_000_003_456,
                Some(&guest_memory_reader),
                Some(&faulting_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_adjtimex_query_without_guest_writer_stays_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let timex_address = 0x9000;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == timex_address && bytes == RISCV_LINUX_TIMEX_BYTES_FOR_TEST {
            Some(timex_bytes(0))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8030,
                RISCV_LINUX_ADJTIMEX_FOR_TEST,
                [timex_address, 0, 0, 0, 0, 0],
            ),
            &mut state,
            2_000_003_456,
            Some(&guest_memory_reader),
            None,
        ),
        None
    );
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

#[test]
fn linux_table_interval_timers_roundtrip_state_and_previous_value() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let new_value_address = 0x9000;
    let old_value_address = 0x9100;
    let current_value_address = 0x9200;
    let new_value = itimerval_bytes(2, 300_000, 4, 500_000);
    let new_value_for_reader = new_value.clone();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == new_value_address && bytes == 32 {
            Some(new_value_for_reader.clone())
        } else {
            None
        }
    });
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
                RISCV_LINUX_GETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_REAL_FOR_TEST,
                    current_value_address,
                    0,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), current_value_address, 32),
        itimerval_bytes(0, 0, 0, 0)
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
                    new_value_address,
                    old_value_address,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), old_value_address, 32),
        itimerval_bytes(0, 0, 0, 0)
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
                    current_value_address,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), current_value_address, 32),
        new_value
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setitimer_rejects_invalid_timer_and_timeval_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let invalid_large_usec_address = 0x9000;
    let invalid_interval_seconds_address = 0x9010;
    let invalid_interval_usec_address = 0x9020;
    let invalid_value_seconds_address = 0x9030;
    let invalid_value_usec_address = 0x9040;
    let valid_value_address = 0x9100;
    let current_value_address = 0x9200;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == invalid_large_usec_address && bytes == 32 {
            Some(itimerval_bytes(0, 1_000_000, 0, 0))
        } else if address == invalid_interval_seconds_address && bytes == 32 {
            Some(itimerval_bytes(-1, 0, 0, 0))
        } else if address == invalid_interval_usec_address && bytes == 32 {
            Some(itimerval_bytes(0, -1, 0, 0))
        } else if address == invalid_value_seconds_address && bytes == 32 {
            Some(itimerval_bytes(0, 0, -1, 0))
        } else if address == invalid_value_usec_address && bytes == 32 {
            Some(itimerval_bytes(0, 0, 0, -1))
        } else if address == valid_value_address && bytes == 32 {
            Some(itimerval_bytes(0, 5, 0, 6))
        } else {
            None
        }
    });
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (pc, which, value_address) in [
        (0x8000, 99, valid_value_address),
        (
            0x8004,
            RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
            invalid_large_usec_address,
        ),
        (
            0x8008,
            RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
            invalid_interval_seconds_address,
        ),
        (
            0x800c,
            RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
            invalid_interval_usec_address,
        ),
        (
            0x8010,
            RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
            invalid_value_seconds_address,
        ),
        (
            0x8014,
            RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
            invalid_value_usec_address,
        ),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_SETITIMER_FOR_TEST,
                    [which, value_address, 0, 0, 0, 0],
                ),
                &mut state,
                0,
                Some(&guest_memory_reader),
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST,
                    current_value_address,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), current_value_address, 32),
        itimerval_bytes(0, 0, 0, 0)
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_interval_timers_support_prof_and_preserve_state_on_old_value_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let initial_value_address = 0x9000;
    let replacement_value_address = 0x9100;
    let current_value_address = 0x9200;
    let faulting_old_value_address = 0x9300;
    let prof_value_address = 0x9400;
    let prof_current_value_address = 0x9500;
    let initial_value = itimerval_bytes(1, 2, 3, 4);
    let replacement_value = itimerval_bytes(5, 6, 7, 8);
    let prof_value = itimerval_bytes(9, 10, 11, 12);
    let initial_value_for_reader = initial_value.clone();
    let replacement_value_for_reader = replacement_value.clone();
    let prof_value_for_reader = prof_value.clone();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == initial_value_address && bytes == 32 {
            Some(initial_value_for_reader.clone())
        } else if address == replacement_value_address && bytes == 32 {
            Some(replacement_value_for_reader.clone())
        } else if address == prof_value_address && bytes == 32 {
            Some(prof_value_for_reader.clone())
        } else {
            None
        }
    });
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        address != faulting_old_value_address + 1
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8200,
                RISCV_LINUX_SETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_REAL_FOR_TEST,
                    initial_value_address,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8204,
                RISCV_LINUX_SETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_REAL_FOR_TEST,
                    replacement_value_address,
                    faulting_old_value_address,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    writes.lock().unwrap().clear();
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8208,
                RISCV_LINUX_GETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_REAL_FOR_TEST,
                    current_value_address,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), current_value_address, 32),
        initial_value
    );
    writes.lock().unwrap().clear();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x820c,
                RISCV_LINUX_SETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_PROF_FOR_TEST,
                    prof_value_address,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8210,
                RISCV_LINUX_GETITIMER_FOR_TEST,
                [
                    RISCV_LINUX_ITIMER_PROF_FOR_TEST,
                    prof_current_value_address,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        collect_guest_writes(&writes.lock().unwrap(), prof_current_value_address, 32),
        prof_value
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn itimerval_bytes(
    interval_seconds: i64,
    interval_microseconds: i64,
    value_seconds: i64,
    value_microseconds: i64,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(&interval_seconds.to_le_bytes());
    bytes.extend_from_slice(&interval_microseconds.to_le_bytes());
    bytes.extend_from_slice(&value_seconds.to_le_bytes());
    bytes.extend_from_slice(&value_microseconds.to_le_bytes());
    bytes
}

fn timeval_bytes(seconds: i64, microseconds: i64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&seconds.to_le_bytes());
    bytes.extend_from_slice(&microseconds.to_le_bytes());
    bytes
}

fn timex_bytes(modes: u32) -> Vec<u8> {
    let mut bytes = vec![0; RISCV_LINUX_TIMEX_BYTES_FOR_TEST];
    bytes[..4].copy_from_slice(&modes.to_le_bytes());
    bytes
}

fn read_le_i64(bytes: &[u8], offset: usize) -> i64 {
    i64::from_le_bytes(bytes[offset..offset + 8].try_into().unwrap())
}

fn writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<(u64, Vec<u8>)> {
    let end = base + len as u64;
    writes
        .iter()
        .filter(|(address, _)| (base..end).contains(address))
        .cloned()
        .collect()
}

fn write_addresses(writes: &[(u64, Vec<u8>)]) -> Vec<u64> {
    writes.iter().map(|(address, _)| *address).collect()
}
