use super::*;
use std::sync::{Arc, Mutex};

const RISCV_LINUX_COPY_FILE_RANGE_FOR_TEST: u64 = 285;
const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;
const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;
const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;
const RISCV_LINUX_O_APPEND_FOR_TEST: u64 = 0o2000;
const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;
const RISCV_LINUX_SEEK_CUR_FOR_TEST: u64 = 1;

type RecordedWrites = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_copy_file_range_copies_guest_file_bytes_and_updates_explicit_offsets() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdefgh");
    state.register_guest_file(b"/out.bin", b"XYZ");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0xa000, 2_u64.to_le_bytes().to_vec()),
        (0xa008, 1_u64.to_le_bytes().to_vec()),
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
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITE, [4, 0x9200, 3, 0, 0, 0],),
            &mut state,
            3,
            Some(&memory_reader(vec![(0x9200, b"XYZ".to_vec())])),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_COPY_FILE_RANGE_FOR_TEST,
                [3, 0xa000, 4, 0xa008, 3, 0],
            ),
            &mut state,
            4,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_LSEEK,
                [4, 0, RISCV_LINUX_SEEK_CUR_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_LSEEK,
                [4, 0, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8018, RISCV_LINUX_READ, [4, 0x9300, 8, 0, 0, 0]),
            &mut state,
            5,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x801c, RISCV_LINUX_READ, [3, 0x9400, 8, 0, 0, 0]),
            &mut state,
            6,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa000, 8), 0xa000, 8),
        5_u64.to_le_bytes()
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa008, 8), 0xa008, 8),
        4_u64.to_le_bytes()
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 4), 0x9300, 4),
        b"Xcde"
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9400, 8), 0x9400, 8),
        b"abcdefgh"
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_copy_file_range_reports_linux_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdefgh");
    state.register_guest_file(b"/out.bin", b"XYZ");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0xa000, 0_u64.wrapping_sub(1).to_le_bytes().to_vec()),
    ]);

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

    for (pc, arguments, errno) in [
        (0x8008, [3, 0, 4, 0, 1, 1], RISCV_LINUX_EINVAL),
        (0x800c, [99, 0, 4, 0, 1, 0], RISCV_LINUX_EBADF),
        (0x8010, [3, 0xa000, 4, 0, 1, 0], RISCV_LINUX_EINVAL),
        (0x8014, [99, 0xb000, 4, 0, 1, 1], RISCV_LINUX_EBADF),
        (0x8018, [3, 0xb000, 4, 0, 1, 1], RISCV_LINUX_EFAULT),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_COPY_FILE_RANGE_FOR_TEST, arguments),
                &mut state,
                3,
                Some(&reader),
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
fn linux_table_copy_file_range_reports_append_output_as_bad_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdefgh");
    state.register_guest_file(b"/out.bin", b"XYZ");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
    ]);

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
                    RISCV_LINUX_O_RDWR_FOR_TEST | RISCV_LINUX_O_APPEND_FOR_TEST,
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
                RISCV_LINUX_COPY_FILE_RANGE_FOR_TEST,
                [3, 0, 4, 0, 1, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(state.guest_file_contents(b"/out.bin"), Some(&b"XYZ"[..]));
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_copy_file_range_rejects_same_file_overlapping_ranges() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/same.bin", b"abcdef");
    let reader = memory_reader(vec![
        (0x9000, b"/same.bin\0".to_vec()),
        (0xa000, 1_u64.to_le_bytes().to_vec()),
        (0xa008, 2_u64.to_le_bytes().to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    for (pc, expected_fd) in [(0x8000, 3), (0x8004, 4)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
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
            Some(RiscvSyscallOutcome::Return { value: expected_fd })
        );
    }
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_COPY_FILE_RANGE_FOR_TEST,
                [3, 0xa000, 4, 0xa008, 3, 0],
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        state.guest_file_contents(b"/same.bin"),
        Some(&b"abcdef"[..])
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_copy_file_range_allows_same_file_eof_limited_non_overlap() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/same.bin", b"abc");
    let reader = memory_reader(vec![
        (0x9000, b"/same.bin\0".to_vec()),
        (0xa000, 0_u64.to_le_bytes().to_vec()),
        (0xa008, 4_u64.to_le_bytes().to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    for (pc, expected_fd) in [(0x8000, 3), (0x8004, 4)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
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
            Some(RiscvSyscallOutcome::Return { value: expected_fd })
        );
    }
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_COPY_FILE_RANGE_FOR_TEST,
                [3, 0xa000, 4, 0xa008, 10, 0],
            ),
            &mut state,
            2,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state.guest_file_contents(b"/same.bin"),
        Some(&b"abc\0abc"[..])
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa000, 8), 0xa000, 8),
        3_u64.to_le_bytes()
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa008, 8), 0xa008, 8),
        7_u64.to_le_bytes()
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn memory_reader(regions: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, len| {
        regions.iter().find_map(|(base, bytes)| {
            let offset = usize::try_from(address.checked_sub(*base)?).ok()?;
            let end = offset.checked_add(len)?;
            bytes.get(offset..end).map(Vec::from)
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
        .filter(|(address, bytes)| *address >= base && *address + bytes.len() as u64 <= end)
        .cloned()
        .collect()
}
