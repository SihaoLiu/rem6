use super::*;

const RISCV_LINUX_FTRUNCATE_FOR_TEST: u64 = 46;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;
const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;
const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;

type RecordedWrites = std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_ftruncate_shrinks_guest_file_and_preserves_offset() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let path = b"/data.bin\0".to_vec();
    let reader = path_reader(path, 0x9000);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_READ, [3, 0x9100, 2, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_FTRUNCATE_FOR_TEST, [3, 4, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_FSTAT, [3, 0x9200, 0, 0, 0, 0]),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0x9300, 8, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );

    let writes = writes.lock().unwrap();
    let stat = collect_guest_writes(&writes_in_range(&writes, 0x9200, 128), 0x9200, 128);
    assert_eq!(read_le_u64(&stat, 48), 4);
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 2), 0x9300, 2),
        b"cd"
    );
}

#[test]
fn linux_table_ftruncate_extends_guest_file_with_zeroes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"ab");
    let path = b"/data.bin\0".to_vec();
    let reader = path_reader(path, 0x9000);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

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
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_FTRUNCATE_FOR_TEST, [3, 5, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_LSEEK,
                [3, 0, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0x9100, 5, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_guest_writes(&writes, 0x9100, 5), b"ab\0\0\0");
}

#[test]
fn linux_table_ftruncate_rejects_bad_read_only_and_oversized_files() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let path = b"/data.bin\0".to_vec();
    let reader = path_reader(path, 0x9000);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_FTRUNCATE_FOR_TEST, [99, 4, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
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
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_FTRUNCATE_FOR_TEST, [3, 4, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FTRUNCATE_FOR_TEST,
                [99, 64 * 1024 * 1024 + 1, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FTRUNCATE_FOR_TEST,
                [3, 64 * 1024 * 1024 + 1, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_ftruncate_rejects_negative_and_oversized_lengths() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let path = b"/data.bin\0".to_vec();
    let reader = path_reader(path, 0x9000);

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
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FTRUNCATE_FOR_TEST,
                [1, 64 * 1024 * 1024 + 1, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FTRUNCATE_FOR_TEST,
                [3, 64 * 1024 * 1024 + 1, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFBIG)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FTRUNCATE_FOR_TEST,
                [3, u64::MAX, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_ftruncate_rejects_non_regular_and_non_guest_backed_fds() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_directory(b"/dir");
    let reader = path_reader(b"/dir\0".to_vec(), 0x9000);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_FTRUNCATE_FOR_TEST, [1, 4, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_TEST,
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
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_FTRUNCATE_FOR_TEST, [3, 4, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

fn path_reader(path: Vec<u8>, base: u64) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < base {
            return None;
        }
        path.get((address - base) as usize)
            .copied()
            .map(|byte| vec![byte])
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
