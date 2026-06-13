use super::*;

#[test]
fn linux_table_nanosleep_zero_duration_returns_without_guest_write() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9000;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_address, _bytes| panic!("nanosleep zero must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_NANOSLEEP,
                [request_address, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            17,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_nanosleep_positive_duration_is_unhandled_after_validation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9200;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(0, 1))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_NANOSLEEP,
                [request_address, 0, 0, 0, 0, 0],
            ),
            &mut state,
            18,
            Some(&guest_memory_reader),
            None,
        ),
        None
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_nanosleep_reports_request_read_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9400;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        assert_eq!(address, request_address);
        assert_eq!(bytes, 16);
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_NANOSLEEP,
                [request_address, 0, 0, 0, 0, 0],
            ),
            &mut state,
            19,
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
fn linux_table_nanosleep_reports_short_request_read_as_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9500;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        assert_eq!(address, request_address);
        assert_eq!(bytes, 16);
        Some(timespec64_bytes(0, 0)[..8].to_vec())
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_NANOSLEEP,
                [request_address, 0, 0, 0, 0, 0],
            ),
            &mut state,
            20,
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
fn linux_table_nanosleep_reports_invalid_timespec() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9600;

    for (pc, seconds, nanoseconds) in [(0x8000, -1, 0), (0x8004, 0, -1), (0x8008, 0, 1_000_000_000)]
    {
        let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
            if address == request_address && bytes == 16 {
                Some(timespec64_bytes(seconds, nanoseconds))
            } else {
                None
            }
        });

        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_NANOSLEEP,
                    [request_address, 0, 0, 0, 0, 0],
                ),
                &mut state,
                20,
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
fn linux_table_clock_nanosleep_zero_relative_duration_returns_without_guest_write() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9800;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("clock_nanosleep zero must not write")
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_CLOCK_NANOSLEEP,
                [0, 0, request_address, 0x9900, 0, 0],
            ),
            &mut state,
            21,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_nanosleep_reports_invalid_clock_id() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9a00;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_CLOCK_NANOSLEEP,
                [99, 0, request_address, 0, 0, 0],
            ),
            &mut state,
            22,
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
fn linux_table_clock_nanosleep_zero_tai_duration_returns_without_guest_write() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9a80;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|_address, _bytes| panic!("clock_tai zero must not write"));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8016,
                RISCV_LINUX_CLOCK_NANOSLEEP,
                [11, 0, request_address, 0, 0, 0],
            ),
            &mut state,
            23,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_nanosleep_zero_process_cpu_duration_returns_without_guest_write() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9aa0;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("process cpu clock zero must not write")
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8017,
                RISCV_LINUX_CLOCK_NANOSLEEP,
                [2, 0, request_address, 0, 0, 0],
            ),
            &mut state,
            24,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_nanosleep_reports_unsupported_thread_cpu_clock_id() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9ac0;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_CLOCK_NANOSLEEP,
                [3, 0, request_address, 0, 0, 0],
            ),
            &mut state,
            25,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTSUP)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_clock_nanosleep_elapsed_absolute_deadline_returns_without_guest_write() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let request_address = 0x9b00;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == request_address && bytes == 16 {
            Some(timespec64_bytes(1, 25))
        } else {
            None
        }
    });
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("elapsed absolute clock_nanosleep must not write")
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8019,
                RISCV_LINUX_CLOCK_NANOSLEEP,
                [1, 1, request_address, 0x9c00, 0, 0],
            ),
            &mut state,
            1_000_000_025,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn timespec64_bytes(seconds: i64, nanoseconds: i64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&seconds.to_le_bytes());
    bytes.extend_from_slice(&nanoseconds.to_le_bytes());
    bytes
}
