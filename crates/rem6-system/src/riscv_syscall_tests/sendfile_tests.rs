use super::*;
use std::sync::{Arc, Mutex};

const RISCV_LINUX_SENDFILE_FOR_TEST: u64 = 71;
const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;
const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;
const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;
const RISCV_LINUX_O_APPEND_FOR_TEST: u64 = 0o2000;
const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;

type RecordedWrites = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_sendfile_copies_guest_file_bytes_and_advances_offsets() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdef");
    state.register_guest_file(b"/out.bin", b"old");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0,],
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    RISCV_LINUX_O_RDWR_FOR_TEST
                        | RISCV_LINUX_O_CREAT_FOR_TEST
                        | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0o600,
                    0,
                    0,
                ],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SENDFILE_FOR_TEST, [4, 3, 0, 4, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_LSEEK,
                [4, 0, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [4, 0x9200, 8, 0, 0, 0]),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_READ, [3, 0x9300, 8, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9200, 4), 0x9200, 4),
        b"abcd"
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 2), 0x9300, 2),
        b"ef"
    );
}

#[test]
fn linux_table_sendfile_uses_and_updates_explicit_offset_pointer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdef");
    state.register_guest_file(b"/out.bin", b"");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0xa000, 2_u64.to_le_bytes().to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0,],
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    RISCV_LINUX_O_RDWR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SENDFILE_FOR_TEST,
                [4, 3, 0xa000, 3, 0, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0x9300, 2, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa000, 8), 0xa000, 8),
        5_u64.to_le_bytes()
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 2), 0x9300, 2),
        b"ab"
    );
}

#[test]
fn linux_table_sendfile_with_aliased_file_description_advances_shared_offset_once() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/data.bin", b"abcdef");
    let reader = memory_reader(vec![(0x9000, b"/data.bin\0".to_vec())]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
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
                    0,
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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_DUP, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SENDFILE_FOR_TEST, [4, 3, 0, 2, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 2 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0x9100, 4, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9100, 4), 0x9100, 4),
        b"cdef"
    );
}

#[test]
fn linux_table_sendfile_reports_fd_offset_and_fault_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdef");
    state.register_guest_file(b"/out.bin", b"");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0xa000, u64::MAX.to_le_bytes().to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0,],
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    RISCV_LINUX_O_RDWR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SENDFILE_FOR_TEST, [99, 3, 0, 1, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_SENDFILE_FOR_TEST, [4, 99, 0, 1, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_SENDFILE_FOR_TEST,
                [4, 3, 0xb000, 1, 0, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_SENDFILE_FOR_TEST,
                [4, 3, 0xa000, 2, 0, 0],
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_sendfile_validates_explicit_offset_on_zero_count() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdef");
    state.register_guest_file(b"/out.bin", b"");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0xa000, u64::MAX.to_le_bytes().to_vec()),
        (0xa100, 2_u64.to_le_bytes().to_vec()),
    ]);
    let rejecting_writer = rejecting_writer();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0,],
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    RISCV_LINUX_O_RDWR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SENDFILE_FOR_TEST,
                [4, 3, 0xb000, 0, 0, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&rejecting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_SENDFILE_FOR_TEST,
                [4, 3, 0xa000, 0, 0, 0],
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&rejecting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_SENDFILE_FOR_TEST,
                [4, 3, 0xa100, 0, 0, 0],
            ),
            &mut state,
            5,
            Some(&reader),
            Some(&rejecting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_sendfile_writes_back_explicit_offset_on_zero_byte_success() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdef");
    state.register_guest_file(b"/out.bin", b"");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0xa000, 6_u64.to_le_bytes().to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0,],
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    RISCV_LINUX_O_RDWR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SENDFILE_FOR_TEST,
                [4, 3, 0xa000, 4, 0, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa000, 8), 0xa000, 8),
        6_u64.to_le_bytes()
    );
}

#[test]
fn linux_table_sendfile_reports_fd_mode_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdef");
    state.register_guest_file(b"/out.bin", b"");
    state.register_guest_file(b"/append.bin", b"");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0x9200, b"/append.bin\0".to_vec()),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_WRONLY, 0, 0, 0,],
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
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9100, RISCV_LINUX_O_RDONLY, 0, 0, 0,],
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9200,
                    RISCV_LINUX_O_RDWR_FOR_TEST | RISCV_LINUX_O_APPEND_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_SENDFILE_FOR_TEST, [5, 3, 0, 1, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_SENDFILE_FOR_TEST, [4, 5, 0, 1, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_SENDFILE_FOR_TEST, [5, 4, 0, 1, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

fn memory_reader(regions: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, len| {
        regions.iter().find_map(|(base, bytes)| {
            let offset = address.checked_sub(*base)?;
            let offset = usize::try_from(offset).ok()?;
            let end = offset.checked_add(len)?;
            (end <= bytes.len()).then(|| bytes[offset..end].to_vec())
        })
    })
}

fn recording_writer(writes: RecordedWrites) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes.lock().unwrap().push((address, bytes.to_vec()));
        true
    })
}

fn rejecting_writer() -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(|_, _| false)
}

fn writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<(u64, Vec<u8>)> {
    let end = base.saturating_add(len as u64);
    writes
        .iter()
        .filter(|(address, bytes)| {
            let write_end = address.saturating_add(bytes.len() as u64);
            *address >= base && write_end <= end
        })
        .cloned()
        .collect()
}
