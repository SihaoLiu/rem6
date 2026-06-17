use super::*;

const RISCV_LINUX_PREAD64_FOR_TEST: u64 = 67;
const RISCV_LINUX_PWRITE64_FOR_TEST: u64 = 68;
const RISCV_LINUX_PREADV_FOR_TEST: u64 = 69;
const RISCV_LINUX_PWRITEV_FOR_TEST: u64 = 70;
const RISCV_LINUX_O_APPEND_FOR_TEST: u64 = 0o2000;
const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;

type RecordedWrites = std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_pread64_reads_at_offset_without_advancing_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let reader = memory_reader(vec![(0x9000, b"/data.bin\0".to_vec())]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PREAD64_FOR_TEST,
                [3, 0x9100, 3, 2, 0, 0],
            ),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [3, 0x9200, 2, 0, 0, 0]),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9100, 3), 0x9100, 3),
        b"cde"
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9200, 2), 0x9200, 2),
        b"ab"
    );
}

#[test]
fn linux_table_pwrite64_writes_at_offset_without_advancing_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let reader = memory_reader(vec![
        (0x9000, b"/data.bin\0".to_vec()),
        (0x9100, b"XY".to_vec()),
    ]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(std::sync::Arc::clone(&writes));

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
                    0
                ],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [3, 0x9200, 1, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [3, 0x9100, 2, 3, 0, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0x9300, 5, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9200, 1), 0x9200, 1),
        b"a"
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 5), 0x9300, 5),
        b"bcXYf"
    );
    assert_eq!(state.guest_writes().len(), 1);
    assert_eq!(state.guest_writes()[0].fd(), GuestFd::new(3).unwrap());
}

#[test]
fn linux_table_positioned_io_reports_linux_fd_and_offset_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let reader = memory_reader(vec![
        (0x9000, b"/data.bin\0".to_vec()),
        (0x9100, b"XY".to_vec()),
    ]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PREAD64_FOR_TEST,
                [99, 0x9200, 1, u64::MAX, 0, 0],
            ),
            &mut state,
            2,
            None,
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
                RISCV_LINUX_PREAD64_FOR_TEST,
                [99, 0x9200, 1, 0, 0, 0],
            ),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_PREAD64_FOR_TEST,
                [0, 0x9200, 1, 0, 0, 0],
            ),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESPIPE)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [99, 0x9100, 2, u64::MAX, 0, 0],
            ),
            &mut state,
            5,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [99, 0x9100, 2, 0, 0, 0],
            ),
            &mut state,
            6,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [3, 0x9100, 2, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x801c,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [1, 0x9100, 2, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESPIPE)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8020,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_WRONLY, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8024,
                RISCV_LINUX_PREAD64_FOR_TEST,
                [4, 0x9200, 1, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_pwrite64_appends_without_advancing_offset_for_append_fds() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abc");
    let reader = memory_reader(vec![
        (0x9000, b"/data.bin\0".to_vec()),
        (0x9100, b"X".to_vec()),
    ]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(std::sync::Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_RDWR_FOR_TEST | RISCV_LINUX_O_APPEND_FOR_TEST,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [3, 0x9100, 1, 0, 0, 0],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [3, 0x9200, 4, 0, 0, 0]),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_guest_writes(&writes, 0x9200, 4), b"abcX");
}

#[test]
fn linux_table_pwrite64_zero_fills_gaps_and_rejects_dense_limit() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"ab");
    let reader = memory_reader(vec![
        (0x9000, b"/data.bin\0".to_vec()),
        (0x9100, b"Z".to_vec()),
    ]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(std::sync::Arc::clone(&writes));

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
                    0
                ],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [3, 0x9100, 1, 5, 0, 0],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PWRITE64_FOR_TEST,
                [3, 0x9100, 1, 64 * 1024 * 1024, 0, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFBIG)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0x9200, 6, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 6 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9200, 6), 0x9200, 6),
        b"ab\0\0\0Z"
    );
}

#[test]
fn linux_table_positional_vector_io_uses_split_offset_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let read_iov = rv64_iovec(0x9200, 2);
    let write_iov = rv64_iovec(0x9300, 1);
    let reader = memory_reader(vec![
        (0x9000, b"/data.bin\0".to_vec()),
        (0x9100, read_iov.to_vec()),
        (0x9110, write_iov.to_vec()),
        (0x9300, b"Z".to_vec()),
    ]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(std::sync::Arc::clone(&writes));

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
                    0
                ],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PREADV_FOR_TEST, [3, 0x9100, 1, 0, 1, 0],),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PWRITEV_FOR_TEST,
                [3, 0x9110, 1, 0, 1, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFBIG)
        })
    );
    assert!(state.guest_writes().is_empty());
}

fn rv64_iovec(base: u64, len: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&base.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    bytes
}

fn memory_reader(regions: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 0 {
            return Some(Vec::new());
        }
        let end = address.checked_add(bytes as u64)?;
        regions.iter().find_map(|(base, contents)| {
            let region_end = base.checked_add(contents.len() as u64)?;
            if address < *base || end > region_end {
                return None;
            }
            let start = usize::try_from(address - *base).ok()?;
            let stop = start.checked_add(bytes)?;
            Some(contents[start..stop].to_vec())
        })
    })
}

fn recording_writer(writes: RecordedWrites) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes.lock().unwrap().push((address, bytes.to_vec()));
        true
    })
}

fn writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<(u64, Vec<u8>)> {
    let end = base + len as u64;
    writes
        .iter()
        .filter(|(address, _bytes)| *address >= base && *address < end)
        .cloned()
        .collect()
}
