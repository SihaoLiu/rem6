use super::*;
use std::sync::{Arc, Mutex};

const RISCV_LINUX_SPLICE_FOR_TEST: u64 = 76;
const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;
const RISCV_LINUX_O_TRUNC_FOR_TEST: u64 = 0o1000;
const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;
const RISCV_LINUX_F_SETPIPE_SZ_FOR_TEST: u64 = 1031;
const RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST: u64 = 4096;
const RISCV_LINUX_SPLICE_F_NONBLOCK_FOR_TEST: u64 = 0x02;
const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;

type RecordedWrites = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_splice_moves_guest_file_bytes_to_pipe_and_updates_explicit_offset() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdefgh");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0xa000, 2_u64.to_le_bytes().to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PIPE2, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SPLICE_FOR_TEST, [3, 0xa000, 5, 0, 3, 0],),
            &mut state,
            3,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(5).unwrap())
            .unwrap()
            .get(),
        0
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [4, 0x9200, 8, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0x9300, 8, 0, 0, 0]),
            &mut state,
            5,
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
        collect_guest_writes(&writes_in_range(&writes, 0x9200, 3), 0x9200, 3),
        b"cde"
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 8), 0x9300, 8),
        b"abcdefgh"
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_splice_moves_guest_pipe_bytes_to_file_and_advances_offsets() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/out.bin", b"old");
    let reader = memory_reader(vec![
        (0x9000, b"/out.bin\0".to_vec()),
        (0x9200, b"pipe-data".to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            1,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [4, 0x9200, 9, 0, 0, 0]),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_RDWR_FOR_TEST
                        | RISCV_LINUX_O_CREAT_FOR_TEST
                        | RISCV_LINUX_O_TRUNC_FOR_TEST,
                    0o600,
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
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_SPLICE_FOR_TEST, [3, 0, 5, 0, 4, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(3).unwrap())
            .unwrap()
            .get(),
        0
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(5).unwrap())
            .unwrap()
            .get(),
        4
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_LSEEK,
                [5, 0, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_READ, [5, 0x9300, 8, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8018, RISCV_LINUX_READ, [3, 0x9400, 8, 0, 0, 0]),
            &mut state,
            5,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 4), 0x9300, 4),
        b"pipe"
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0x9400, 5), 0x9400, 5),
        b"-data"
    );
    assert_eq!(state.guest_file_contents(b"/out.bin"), Some(&b"pipe"[..]));
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_splice_reports_linux_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdefgh");
    state.register_guest_file(b"/out.bin", b"old");
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0x9100, b"/out.bin\0".to_vec()),
        (0xa000, 0_u64.wrapping_sub(1).to_le_bytes().to_vec()),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

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
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_PIPE2, [0x9200, 0, 0, 0, 0, 0]),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    for (pc, arguments, errno) in [
        (0x800c, [99, 0, 5, 0, 1, 0], RISCV_LINUX_EBADF),
        (0x8010, [3, 0, 4, 0, 1, 0], RISCV_LINUX_EINVAL),
        (0x8014, [3, 0xa000, 5, 0, 1, 0], RISCV_LINUX_EINVAL),
        (0x8018, [3, 0xb000, 5, 0, 1, 0], RISCV_LINUX_EFAULT),
        (0x801c, [3, 0, 5, 0, 1, 0x8000], RISCV_LINUX_EINVAL),
    ] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_SPLICE_FOR_TEST, arguments),
                &mut state,
                4,
                Some(&reader),
                Some(&writer),
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_splice_to_full_blocking_pipe_reports_blocked() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/in.bin", b"abcdefgh");
    let fill_payload = vec![b'x'; RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize];
    let reader = memory_reader(vec![
        (0x9000, b"/in.bin\0".to_vec()),
        (0xa000, fill_payload),
    ]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PIPE2, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [
                    5,
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
                0x800c,
                RISCV_LINUX_WRITE,
                [5, 0xa000, RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_SPLICE_FOR_TEST, [3, 0, 5, 0, 1, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_SPLICE_FOR_TEST,
                [3, 0, 5, 0, 1, RISCV_LINUX_SPLICE_F_NONBLOCK_FOR_TEST],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN)
        })
    );
}

#[test]
fn linux_table_splice_nonblock_flag_allows_partial_pipe_progress() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let file_payload = vec![b'a'; (RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST + 1) as usize];
    state.register_guest_file(b"/in.bin", &file_payload);
    let reader = memory_reader(vec![(0x9000, b"/in.bin\0".to_vec())]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

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
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PIPE2, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [
                    5,
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
                0x800c,
                RISCV_LINUX_SPLICE_FOR_TEST,
                [
                    3,
                    0,
                    5,
                    0,
                    RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST + 1,
                    RISCV_LINUX_SPLICE_F_NONBLOCK_FOR_TEST,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );
    assert_eq!(
        state
            .guest_fds()
            .file_offset(GuestFd::new(3).unwrap())
            .unwrap()
            .get(),
        RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_READ,
                [4, 0x9200, RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST + 1, 0, 0, 0],
            ),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST
        })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(
            &writes_in_range(
                &writes,
                0x9200,
                RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize
            ),
            0x9200,
            RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize,
        ),
        vec![b'a'; RISCV_LINUX_PIPE_PAGE_BYTES_FOR_TEST as usize]
    );
}

#[test]
fn linux_table_splice_from_empty_pipe_reports_block_or_eagain() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/out.bin", b"");
    let reader = memory_reader(vec![(0x9000, b"/out.bin\0".to_vec())]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            1,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
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
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 5 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SPLICE_FOR_TEST, [3, 0, 5, 0, 1, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_SPLICE_FOR_TEST,
                [3, 0, 5, 0, 1, RISCV_LINUX_SPLICE_F_NONBLOCK_FOR_TEST],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_CLOSE, [4, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_SPLICE_FOR_TEST, [3, 0, 5, 0, 1, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_splice_rejects_same_pipe() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![(0x9200, b"x".to_vec())]);
    let writes: RecordedWrites = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            1,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [4, 0x9200, 1, 0, 0, 0]),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SPLICE_FOR_TEST, [3, 0, 4, 0, 1, 0]),
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
