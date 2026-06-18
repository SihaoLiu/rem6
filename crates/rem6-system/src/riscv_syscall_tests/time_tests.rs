use super::*;

const RISCV_LINUX_CLOCK_GETRES_FOR_TEST: u64 = 114;
const RISCV_LINUX_GETITIMER_FOR_TEST: u64 = 102;
const RISCV_LINUX_SETITIMER_FOR_TEST: u64 = 103;
const RISCV_LINUX_ITIMER_REAL_FOR_TEST: u64 = 0;
const RISCV_LINUX_ITIMER_VIRTUAL_FOR_TEST: u64 = 1;
const RISCV_LINUX_ITIMER_PROF_FOR_TEST: u64 = 2;

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
