#[allow(dead_code, unused_imports)]
#[path = "riscv_syscall_emulation/support.rs"]
mod support;

use rem6_system::{
    RiscvGuestMemoryReader, RiscvGuestMemoryWriter, RiscvSyscallOutcome, RiscvSyscallRequest,
    RiscvSyscallState, RiscvSyscallTable,
};
use support::*;

const RISCV_LINUX_PIPE2: u64 = 59;
const RISCV_LINUX_READ: u64 = 63;
const RISCV_LINUX_WRITE: u64 = 64;
const RISCV_LINUX_VMSPLICE: u64 = 75;
const RISCV_LINUX_TEE: u64 = 77;
const RISCV_LINUX_FCNTL: u64 = 25;
const RISCV_LINUX_CLOSE: u64 = 57;
const RISCV_LINUX_OPEN: u64 = 1024;
const RISCV_LINUX_F_SETPIPE_SZ: u64 = 1031;
const RISCV_LINUX_SPLICE_F_NONBLOCK: u64 = 0x02;
const RISCV_LINUX_IOV_MAX: u64 = 1024;
const RISCV_LINUX_PIPE_PAGE_BYTES: usize = 4096;
const RISCV_LINUX_EAGAIN: u64 = 11;
const RISCV_LINUX_EBADF: u64 = 9;
const RISCV_LINUX_EFAULT: u64 = 14;
const RISCV_LINUX_EINVAL: u64 = 22;

fn linux_error(errno: u64) -> u64 {
    0_u64.wrapping_sub(errno)
}

fn pipe_store() -> Arc<Mutex<PartitionedMemoryStore>> {
    let fd_area = [0; 8];
    let read_area = [0; 9];
    loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &fd_area),
            (0x9000, b"pipe-data"),
            (0x9100, &read_area),
        ],
    )
}

fn fds_from_memory(store: &Arc<Mutex<PartitionedMemoryStore>>) -> (u64, u64) {
    fds_from_memory_at(store, 0x8800)
}

fn fds_from_memory_at(store: &Arc<Mutex<PartitionedMemoryStore>>, address: u64) -> (u64, u64) {
    let fds = guest_memory_reader(Arc::clone(store))(address, 8).unwrap();
    let read_fd = i32::from_le_bytes(fds[..4].try_into().unwrap());
    let write_fd = i32::from_le_bytes(fds[4..].try_into().unwrap());
    (read_fd as u64, write_fd as u64)
}

fn iovec(address: u64, len: u64) -> [u8; 16] {
    let mut bytes = [0; 16];
    bytes[..8].copy_from_slice(&address.to_le_bytes());
    bytes[8..].copy_from_slice(&len.to_le_bytes());
    bytes
}

fn create_pipe_at(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    state: &mut RiscvSyscallState,
    writer: &RiscvGuestMemoryWriter,
    address: u64,
    tick: u64,
) -> (u64, u64) {
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000 + tick * 4,
                RISCV_LINUX_PIPE2,
                [address, 0, 0, 0, 0, 0]
            ),
            state,
            tick,
            None,
            Some(writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    fds_from_memory_at(store, address)
}

fn fill_pipe_with_test_bytes(
    state: &mut RiscvSyscallState,
    reader: &RiscvGuestMemoryReader,
    write_fd: u64,
    start_tick: u64,
) {
    const PIPE_FILL_CHUNK_BYTES: usize = 16;

    for index in 0..(RISCV_LINUX_PIPE_PAGE_BYTES / PIPE_FILL_CHUNK_BYTES) {
        assert_eq!(
            RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    0x8400 + index as u64 * 4,
                    RISCV_LINUX_WRITE,
                    [write_fd, 0x9000, PIPE_FILL_CHUNK_BYTES as u64, 0, 0, 0],
                ),
                state,
                start_tick + index as u64,
                Some(reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: PIPE_FILL_CHUNK_BYTES as u64
            })
        );
    }
}

#[test]
fn linux_table_pipe2_roundtrips_bytes_and_close_releases_endpoints() {
    let store = pipe_store();
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let (read_fd, write_fd) = fds_from_memory(&store);
    assert_ne!(read_fd, write_fd);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [write_fd, 0x9000, 9, 0, 0, 0]),
            &mut state,
            11,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert!(state.guest_writes().is_empty());

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [read_fd, 0x9100, 9, 0, 0, 0]),
            &mut state,
            12,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 9),
        Some(b"pipe-data".to_vec())
    );
    assert!(state.guest_writes().is_empty());

    for fd in [read_fd, write_fd] {
        assert_eq!(
            RiscvSyscallTable::new().handle(
                RiscvSyscallRequest::new(0x800c, RISCV_LINUX_CLOSE, [fd, 0, 0, 0, 0, 0]),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
    }
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [read_fd, 0x9100, 1, 0, 0, 0]),
            &mut state,
            13,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_WRITE, [write_fd, 0x9000, 1, 0, 0, 0]),
            &mut state,
            14,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_vmsplice_writes_guest_iovec_bytes_to_pipe() {
    let iovec_a = iovec(0x9000, 4);
    let iovec_b = iovec(0x9004, 5);
    let read_area = [0; 9];
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &[0; 8]),
            (0x9000, b"pipe-data"),
            (0x9100, &read_area),
            (0x9200, &iovec_a),
            (0x9210, &iovec_b),
        ],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let (read_fd, write_fd) = fds_from_memory(&store);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_VMSPLICE, [write_fd, 0x9200, 2, 0, 0, 0],),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [read_fd, 0x9100, 9, 0, 0, 0]),
            &mut state,
            2,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 9),
        Some(b"pipe-data".to_vec())
    );
}

#[test]
fn linux_table_vmsplice_rejects_regular_and_unusable_fds() {
    let iovec_a = iovec(0x9000, 4);
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &[0; 8]),
            (0x9000, b"pipe"),
            (0x9200, &iovec_a),
            (0x9300, b"/input.txt\0"),
        ],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"file");

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_OPEN, [0x9300, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_VMSPLICE, [3, 0x9200, 1, 0, 0, 0]),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_VMSPLICE, [99, 0x9200, 1, 0, 0, 0]),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let (read_fd, _) = fds_from_memory(&store);
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_VMSPLICE, [read_fd, 0x9200, 1, 0, 0, 0],),
            &mut state,
            4,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}

#[test]
fn linux_table_vmsplice_rejects_faulting_iovecs_and_invalid_lengths() {
    let faulting_data_iovec = iovec(0xa000, 1);
    let overflow_a = iovec(0x9000, u64::MAX);
    let overflow_b = iovec(0x9000, 1);
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &[0; 8]),
            (0x9000, b"x"),
            (0x9200, &faulting_data_iovec),
            (0x9300, &overflow_a),
            (0x9310, &overflow_b),
        ],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let (_, write_fd) = fds_from_memory(&store);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_VMSPLICE,
                [write_fd, 0x9200, RISCV_LINUX_IOV_MAX + 1, 0, 0, 0],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_VMSPLICE, [write_fd, 0xa000, 1, 0, 0, 0]),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_VMSPLICE, [write_fd, 0x9200, 1, 0, 0, 0]),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_VMSPLICE, [write_fd, 0x9300, 2, 0, 0, 0]),
            &mut state,
            4,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_vmsplice_reports_full_pipe_write_availability() {
    const PIPE_FILL_CHUNK_BYTES: usize = 16;

    let fill = [b'x'; PIPE_FILL_CHUNK_BYTES];
    let iovec_a = iovec(0x9000, 1);
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[(0x8800, &[0; 8]), (0x8f00, &iovec_a), (0x9000, &fill)],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            0,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let (_, write_fd) = fds_from_memory(&store);
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [
                    write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ,
                    RISCV_LINUX_PIPE_PAGE_BYTES as u64,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            1,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES as u64
        })
    );
    for tick in 2..(2 + RISCV_LINUX_PIPE_PAGE_BYTES / PIPE_FILL_CHUNK_BYTES) {
        assert_eq!(
            RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    0x8008,
                    RISCV_LINUX_WRITE,
                    [write_fd, 0x9000, PIPE_FILL_CHUNK_BYTES as u64, 0, 0, 0],
                ),
                &mut state,
                tick as u64,
                Some(&reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: PIPE_FILL_CHUNK_BYTES as u64
            })
        );
    }

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_VMSPLICE,
                [write_fd, 0x8f00, 1, RISCV_LINUX_SPLICE_F_NONBLOCK, 0, 0],
            ),
            &mut state,
            260,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_VMSPLICE, [write_fd, 0x8f00, 1, 0, 0, 0]),
            &mut state,
            261,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
}

#[test]
fn linux_table_tee_duplicates_pipe_bytes_without_consuming_input() {
    let source_read_area = [0; 9];
    let target_read_area = [0; 9];
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &[0; 8]),
            (0x8810, &[0; 8]),
            (0x9000, b"pipe-data"),
            (0x9100, &source_read_area),
            (0x9120, &target_read_area),
        ],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let (source_read_fd, source_write_fd) = create_pipe_at(&store, &mut state, &writer, 0x8800, 0);
    let (target_read_fd, target_write_fd) = create_pipe_at(&store, &mut state, &writer, 0x8810, 1);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_WRITE,
                [source_write_fd, 0x9000, 9, 0, 0, 0]
            ),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_TEE,
                [source_read_fd, target_write_fd, 9, 0, 0, 0],
            ),
            &mut state,
            3,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_READ,
                [target_read_fd, 0x9120, 9, 0, 0, 0]
            ),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9120, 9),
        Some(b"pipe-data".to_vec())
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_READ,
                [source_read_fd, 0x9100, 9, 0, 0, 0]
            ),
            &mut state,
            5,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        guest_memory_reader(Arc::clone(&store))(0x9100, 9),
        Some(b"pipe-data".to_vec())
    );
}

#[test]
fn linux_table_tee_rejects_bad_fds_modes_and_non_pipe_pairs() {
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[
            (0x8800, &[0; 8]),
            (0x8810, &[0; 8]),
            (0x9300, b"/input.txt\0"),
        ],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"file");
    let (source_read_fd, source_write_fd) = create_pipe_at(&store, &mut state, &writer, 0x8800, 0);
    let (target_read_fd, target_write_fd) = create_pipe_at(&store, &mut state, &writer, 0x8810, 1);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_OPEN, [0x9300, 0, 0, 0, 0, 0]),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_OPEN, [0x9300, 1, 0, 0, 0, 0]),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );

    for (pc, args, errno) in [
        (0x8010, [99, target_write_fd, 1, 0, 0, 0], RISCV_LINUX_EBADF),
        (
            0x8014,
            [source_write_fd, target_write_fd, 1, 0, 0, 0],
            RISCV_LINUX_EBADF,
        ),
        (
            0x8018,
            [source_read_fd, target_read_fd, 1, 0, 0, 0],
            RISCV_LINUX_EBADF,
        ),
        (
            0x801c,
            [source_read_fd, source_write_fd, 1, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (0x8020, [7, target_write_fd, 1, 0, 0, 0], RISCV_LINUX_EINVAL),
        (0x8024, [source_read_fd, 8, 1, 0, 0, 0], RISCV_LINUX_EINVAL),
        (
            0x8028,
            [source_read_fd, target_write_fd, 1, 0x100, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
    ] {
        assert_eq!(
            RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_TEE, args),
                &mut state,
                pc,
                None,
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(errno)
            })
        );
    }

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x802c, RISCV_LINUX_TEE, [99, 98, 0, 0, 0, 0]),
            &mut state,
            9,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_tee_reports_empty_input_and_full_output_availability() {
    let fill = [b'x'; 16];
    let store = loaded_program_store_with_data(
        &[(0x8000, 0)],
        &[(0x8800, &[0; 8]), (0x8810, &[0; 8]), (0x9000, &fill)],
    );
    let reader = RiscvGuestMemoryReader::new(guest_memory_reader(Arc::clone(&store)));
    let writer = RiscvGuestMemoryWriter::new(guest_memory_writer(Arc::clone(&store)));
    let mut state = RiscvSyscallState::new(0);
    let (source_read_fd, source_write_fd) = create_pipe_at(&store, &mut state, &writer, 0x8800, 0);
    let (_, target_write_fd) = create_pipe_at(&store, &mut state, &writer, 0x8810, 1);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_TEE,
                [
                    source_read_fd,
                    target_write_fd,
                    1,
                    RISCV_LINUX_SPLICE_F_NONBLOCK,
                    0,
                    0,
                ],
            ),
            &mut state,
            2,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_TEE,
                [source_read_fd, target_write_fd, 1, 0, 0, 0],
            ),
            &mut state,
            3,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_WRITE,
                [source_write_fd, 0x9000, 1, 0, 0, 0]
            ),
            &mut state,
            4,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FCNTL,
                [
                    target_write_fd,
                    RISCV_LINUX_F_SETPIPE_SZ,
                    RISCV_LINUX_PIPE_PAGE_BYTES as u64,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            5,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_PIPE_PAGE_BYTES as u64
        })
    );
    fill_pipe_with_test_bytes(&mut state, &reader, target_write_fd, 6);

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8800,
                RISCV_LINUX_TEE,
                [
                    source_read_fd,
                    target_write_fd,
                    1,
                    RISCV_LINUX_SPLICE_F_NONBLOCK,
                    0,
                    0,
                ],
            ),
            &mut state,
            300,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EAGAIN)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8804,
                RISCV_LINUX_TEE,
                [source_read_fd, target_write_fd, 1, 0, 0, 0],
            ),
            &mut state,
            301,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
}

#[test]
fn linux_table_pipe2_rejects_invalid_flags_and_faulting_fd_array() {
    let mut state = RiscvSyscallState::new(0);
    let invalid_flags = 1_u64 << 40;
    let panic_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| {
        panic!("invalid pipe2 flags should not write the fd array")
    });

    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PIPE2,
                [0x8800, invalid_flags, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&panic_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    assert_eq!(
        RiscvSyscallTable::new().handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_PIPE2, [0x8800, 0, 0, 0, 0, 0]),
            &mut state,
            1,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        RiscvSyscallTable::new().handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CLOSE, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
}
