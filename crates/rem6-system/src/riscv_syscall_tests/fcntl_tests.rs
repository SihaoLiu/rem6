use super::*;

const RISCV_LINUX_F_DUPFD_FOR_TEST: u64 = 0;
const RISCV_LINUX_F_GETLK_FOR_TEST: u64 = 5;
const RISCV_LINUX_F_SETLK_FOR_TEST: u64 = 6;
const RISCV_LINUX_F_SETLKW_FOR_TEST: u64 = 7;
const RISCV_LINUX_F_DUPFD_CLOEXEC_FOR_TEST: u64 = 1030;
const RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST: u64 = 1031;
const RISCV_LINUX_F_GETPIPE_SZ_FOR_TEST: u64 = 1032;
const RISCV_LINUX_F_UNKNOWN_FOR_TEST: u64 = 9999;
const RISCV_LINUX_F_RDLCK_FOR_TEST: u16 = 0;
const RISCV_LINUX_F_WRLCK_FOR_TEST: u16 = 1;
const RISCV_LINUX_F_UNLCK_FOR_TEST: u16 = 2;
const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;
const RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST: u64 = 4096;
const RISCV_LINUX_DEFAULT_PIPE_CAPACITY_FOR_TEST: u64 = 64 * 1024;
const RISCV_LINUX_PIPE_MAX_CAPACITY_FOR_TEST: u64 = 1024 * 1024;

#[test]
fn linux_table_fcntl_dupfd_respects_minimum_and_close_on_exec() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let low_duplicate = GuestFd::new(3).unwrap();
    let high_duplicate = GuestFd::new(5).unwrap();

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_DUPFD_FOR_TEST, 5, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        state
            .guest_fds()
            .entry(high_duplicate)
            .unwrap()
            .description(),
        GuestFileDescriptionId::new(1)
    );
    assert!(!state.guest_fds().close_on_exec(high_duplicate).unwrap());

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_DUPFD_CLOEXEC_FOR_TEST, 3, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state
            .guest_fds()
            .entry(low_duplicate)
            .unwrap()
            .description(),
        GuestFileDescriptionId::new(1)
    );
    assert!(state.guest_fds().close_on_exec(low_duplicate).unwrap());
}

#[test]
fn linux_table_fcntl_dupfd_preserves_stdin_read_source() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.push_stdin_bytes(b"Z");
    let guest_memory_writer =
        RiscvGuestMemoryWriter::new(|address, bytes| address == 0x9100 && bytes == b"Z");

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [0, RISCV_LINUX_F_DUPFD_FOR_TEST, 7, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [7, 0x9100, 1, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(state.stdin_byte_count(), 0);
}

#[test]
fn linux_table_fcntl_dupfd_reports_bad_source_before_invalid_minimum() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [99, RISCV_LINUX_F_DUPFD_FOR_TEST, u64::MAX, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_fcntl_dupfd_rejects_invalid_minimum_after_valid_source() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_DUPFD_FOR_TEST, u64::MAX, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_fcntl_rejects_unknown_command_without_unknown_syscall_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_UNKNOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_getpipe_sz_reports_guest_pipe_capacity() {
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    let read_fd = u64::from(read_le_u32(&fds, 0));
    let write_fd = u64::from(read_le_u32(&fds, 4));

    for fd in [read_fd, write_fd] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(
                    0x8004,
                    RISCV_LINUX_FCNTL,
                    [fd, RISCV_LINUX_F_GETPIPE_SZ_FOR_TEST, 0, 0, 0, 0],
                ),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: RISCV_LINUX_DEFAULT_PIPE_CAPACITY_FOR_TEST
            })
        );
    }
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETPIPE_SZ_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_setpipe_sz_resizes_guest_pipe_and_enforces_nonblock_capacity() {
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
    let payload = vec![b'x'; RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize];
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if (0xa000..0xa000 + payload.len() as u64).contains(&address) {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PIPE2,
                [0x9000, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    let read_fd = u64::from(read_le_u32(&fds, 0));
    let write_fd = u64::from(read_le_u32(&fds, 4));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [
                    write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [read_fd, RISCV_LINUX_F_GETPIPE_SZ_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_WRITE,
                [
                    write_fd,
                    0xa000,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_WRITE, [write_fd, 0xa000, 1, 0, 0, 0]),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FCNTL,
                [
                    write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST - 1,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_setpipe_sz_reports_busy_and_permission_errors() {
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
    let unread_payload = vec![b'u'; (RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST * 2) as usize];
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0xa000 && bytes == unread_payload.len() {
            return Some(unread_payload.clone());
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    let read_fd = u64::from(read_le_u32(&fds, 0));
    let write_fd = u64::from(read_le_u32(&fds, 4));
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_WRITE,
                [
                    write_fd,
                    0xa000,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST * 2,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST * 2
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [
                    read_fd,
                    RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBUSY)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCNTL,
                [
                    write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST,
                    RISCV_LINUX_PIPE_MAX_CAPACITY_FOR_TEST + 1,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FCNTL,
                [read_fd, RISCV_LINUX_F_GETPIPE_SZ_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_DEFAULT_PIPE_CAPACITY_FOR_TEST
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pipe_blocking_large_write_blocks_until_request_capacity_is_available() {
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
    let first_payload = vec![b'a'; 2048];
    let large_payload = vec![b'b'; RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize + 1];
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0xa000 && bytes == first_payload.len() {
            return Some(first_payload.clone());
        }
        if address == 0xb000 && bytes == large_payload.len() {
            return Some(large_payload.clone());
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    let write_fd = u64::from(read_le_u32(&fds, 4));
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [
                    write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITE, [write_fd, 0xa000, 2048, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 2048 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_WRITE,
                [
                    write_fd,
                    0xb000,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST + 1,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pipe_nonblocking_large_partial_write_reads_only_accepted_bytes() {
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
    let fill_payload = vec![b'a'; RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize - 1];
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0xa000 && bytes == fill_payload.len() {
            return Some(fill_payload.clone());
        }
        if address == 0xb000 && bytes == 1 {
            return Some(vec![b'b']);
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PIPE2,
                [0x9000, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    let read_fd = u64::from(read_le_u32(&fds, 0));
    let write_fd = u64::from(read_le_u32(&fds, 4));
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [
                    write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_WRITE,
                [
                    write_fd,
                    0xa000,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST - 1,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST - 1
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_WRITE,
                [
                    write_fd,
                    0xb000,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST + 1,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FCNTL,
                [read_fd, RISCV_LINUX_F_GETPIPE_SZ_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_pipe_nonblocking_large_writev_reads_only_accepted_prefix() {
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
    let fill_payload = vec![b'a'; RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize - 1];
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == 0xa000 && bytes == fill_payload.len() {
            return Some(fill_payload.clone());
        }
        if address == 0xc000 && bytes == 16 {
            let mut iovec = Vec::new();
            iovec.extend_from_slice(&0xb000_u64.to_le_bytes());
            iovec.extend_from_slice(&(RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST + 1).to_le_bytes());
            return Some(iovec);
        }
        if address == 0xb000 && bytes == 1 {
            return Some(vec![b'b']);
        }
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PIPE2,
                [0x9000, RISCV_LINUX_O_NONBLOCK, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let fds = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 8);
    let write_fd = u64::from(read_le_u32(&fds, 4));
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [
                    write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_WRITE,
                [
                    write_fd,
                    0xa000,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST - 1,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST - 1
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_WRITEV, [write_fd, 0xc000, 1, 0, 0, 0]),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_reports_bad_fd_before_unknown_command() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [99, RISCV_LINUX_F_UNKNOWN_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_setfl_enables_append_guest_file_writes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let path = b"guest.txt\0".to_vec();
    let first = b"front".to_vec();
    let second = b"tail\n".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 && address < 0xa000 + first.len() as u64 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return first.get(start..end).map(Vec::from);
        }
        if address >= 0xa100 && address < 0xa100 + second.len() as u64 {
            let start = usize::try_from(address - 0xa100).ok()?;
            let end = start.checked_add(bytes)?;
            return second.get(start..end).map(Vec::from);
        }
        None
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_RDWR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [3, 0xa000, 5, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_SETFL, RISCV_LINUX_O_APPEND, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_LSEEK, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_WRITE, [3, 0xa100, 5, 0, 0, 0]),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_RDWR_FOR_TEST | RISCV_LINUX_O_APPEND
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8018, RISCV_LINUX_LSEEK, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x801c, RISCV_LINUX_READ, [3, 0xb000, 32, 0, 0, 0]),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0xb000, b"fronttail\n".to_vec())]
    );
    assert_eq!(state.guest_path_stat(b"guest.txt").unwrap().size(), 10);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_getlk_reports_no_conflict_for_open_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let flock = linux_flock_bytes(RISCV_LINUX_F_WRLCK_FOR_TEST, 0, 0, 0, 41);
    let guest_memory_reader = linux_flock_reader(0x9000, flock);
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
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETLK_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, 0x9000);
    assert_eq!(
        read_linux_flock_type(&writes[0].1),
        RISCV_LINUX_F_UNLCK_FOR_TEST
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_setlk_and_setlkw_accept_valid_advisory_lock_for_open_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let flock = linux_flock_bytes(RISCV_LINUX_F_WRLCK_FOR_TEST, 0, 0, 0, 0);

    for (index, command) in [RISCV_LINUX_F_SETLK_FOR_TEST, RISCV_LINUX_F_SETLKW_FOR_TEST]
        .into_iter()
        .enumerate()
    {
        let guest_memory_reader = linux_flock_reader(0x9000, flock.clone());
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    0x8000 + index as u64 * 4,
                    RISCV_LINUX_FCNTL,
                    [1, command, 0x9000, 0, 0, 0],
                ),
                &mut state,
                7 + index as u64,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_lock_commands_report_pointer_and_type_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let bad_type = linux_flock_bytes(99, 0, 0, 0, 0);
    let bad_type_reader = linux_flock_reader(0x9000, bad_type);
    let unlock_probe = linux_flock_bytes(RISCV_LINUX_F_UNLCK_FOR_TEST, 0, 0, 0, 0);
    let unlock_probe_reader = linux_flock_reader(0x9100, unlock_probe);
    let sparse_flock = linux_flock_bytes(RISCV_LINUX_F_WRLCK_FOR_TEST, 0, 0, 0, 0);
    let sparse_reader = sparse_linux_flock_reader(0x9200, sparse_flock);
    let faulting_reader = RiscvGuestMemoryReader::new(|_, _| None);
    let writer = RiscvGuestMemoryWriter::new(|_, _| true);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETLK_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&bad_type_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETLK_FOR_TEST, 0x9100, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&unlock_probe_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETLK_FOR_TEST, 0x9200, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&sparse_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_GETLK_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&faulting_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FCNTL,
                [99, RISCV_LINUX_F_GETLK_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&faulting_reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fcntl_setlk_enforces_descriptor_access_mode() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let write_lock = linux_flock_bytes(RISCV_LINUX_F_WRLCK_FOR_TEST, 0, 0, 0, 0);
    let write_lock_reader = linux_flock_reader(0x9000, write_lock);
    let read_lock = linux_flock_bytes(RISCV_LINUX_F_RDLCK_FOR_TEST, 0, 0, 0, 0);
    let read_lock_reader = linux_flock_reader(0x9100, read_lock);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCNTL,
                [0, RISCV_LINUX_F_SETLK_FOR_TEST, 0x9000, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&write_lock_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [1, RISCV_LINUX_F_SETLK_FOR_TEST, 0x9100, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&read_lock_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn linux_flock_bytes(lock_type: u16, whence: u16, start: i64, len: i64, pid: i32) -> Vec<u8> {
    let mut bytes = vec![0; 32];
    bytes[0..2].copy_from_slice(&lock_type.to_le_bytes());
    bytes[2..4].copy_from_slice(&whence.to_le_bytes());
    bytes[8..16].copy_from_slice(&start.to_le_bytes());
    bytes[16..24].copy_from_slice(&len.to_le_bytes());
    bytes[24..28].copy_from_slice(&pid.to_le_bytes());
    bytes
}

fn read_linux_flock_type(bytes: &[u8]) -> u16 {
    u16::from_le_bytes(bytes[0..2].try_into().unwrap())
}

fn linux_flock_reader(base: u64, flock: Vec<u8>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        let start = usize::try_from(address.checked_sub(base)?).ok()?;
        let end = start.checked_add(bytes)?;
        flock.get(start..end).map(Vec::from)
    })
}

fn sparse_linux_flock_reader(base: u64, flock: Vec<u8>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 2 && bytes != 4 && bytes != 8 {
            return None;
        }
        let start = usize::try_from(address.checked_sub(base)?).ok()?;
        let end = start.checked_add(bytes)?;
        flock.get(start..end).map(Vec::from)
    })
}
