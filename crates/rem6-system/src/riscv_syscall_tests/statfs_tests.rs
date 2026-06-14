use super::*;

const RISCV_LINUX_STATFS_FOR_TEST: u64 = 43;
const RISCV_LINUX_FSTATFS_FOR_TEST: u64 = 44;
const RISCV_LINUX_OPENAT_FOR_STATFS_TEST: u64 = 56;
const RISCV_LINUX_STATFS_BYTES_FOR_TEST: usize = 120;
const RISCV_LINUX_STATFS_MAGIC_FOR_TEST: u64 = 0x5245_4d36;
const RISCV_LINUX_STATFS_BLOCK_SIZE_FOR_TEST: u64 = 4096;
const RISCV_LINUX_STATFS_NAME_MAX_FOR_TEST: u64 = 255;

type RecordedGuestWrites = std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_statfs_writes_registered_guest_namespace_statfs() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0).with_linux_se_memory_capacity(64 * 1024 * 1024);
    state.register_guest_file(b"guest.txt", b"hello");
    state.register_guest_directory(b"subdir");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");
    let (guest_memory_writer, writes) = recording_writer();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_STATFS_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let statfs = collect_guest_writes(
        &writes.lock().unwrap(),
        0x9100,
        RISCV_LINUX_STATFS_BYTES_FOR_TEST,
    );
    assert_statfs_header(&statfs);
    assert_eq!(read_le_u64(&statfs, 16), 16 * 1024);
    assert_eq!(read_le_u64(&statfs, 24), 16 * 1024 - 1);
    assert_eq!(read_le_u64(&statfs, 32), 16 * 1024 - 1);
    assert_eq!(read_le_u64(&statfs, 40), 3);
    assert_eq!(read_le_u64(&statfs, 48), 16 * 1024 - 3);
}

#[test]
fn linux_table_fstatfs_writes_open_guest_file_statfs() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0).with_linux_se_memory_capacity(64 * 1024 * 1024);
    state.register_guest_file(b"/input.txt", b"hello");
    let guest_memory_reader = c_string_reader(0x9000, b"/input.txt");
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_STATFS_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    let (guest_memory_writer, writes) = recording_writer();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FSTATFS_FOR_TEST,
                [3, 0x9200, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let statfs = collect_guest_writes(
        &writes.lock().unwrap(),
        0x9200,
        RISCV_LINUX_STATFS_BYTES_FOR_TEST,
    );
    assert_statfs_header(&statfs);
    assert_eq!(read_le_u64(&statfs, 40), 2);
}

#[test]
fn linux_table_statfs_counts_hard_linked_file_identity_once() {
    let mut state = RiscvSyscallState::new(0).with_linux_se_memory_capacity(64 * 1024 * 1024);
    state.register_guest_file(b"source.txt", b"hello");
    state
        .link_guest_path(b"source.txt", b"alias.txt")
        .expect("registered source can be linked");

    let statfs = statfs_bytes_for_state(state, b"source.txt");

    assert_eq!(read_le_u64(&statfs, 24), 16 * 1024 - 1);
    assert_eq!(read_le_u64(&statfs, 40), 2);
    assert_eq!(read_le_u64(&statfs, 48), 16 * 1024 - 2);
}

#[test]
fn linux_table_statfs_counts_implicit_parent_directories() {
    let mut state = RiscvSyscallState::new(0).with_linux_se_memory_capacity(64 * 1024 * 1024);
    state.register_guest_file(b"sub/guest.txt", b"hello");

    let statfs = statfs_bytes_for_state(state, b"sub");

    assert_eq!(read_le_u64(&statfs, 40), 3);
    assert_eq!(read_le_u64(&statfs, 48), 16 * 1024 - 3);
}

#[test]
fn linux_table_statfs_does_not_charge_empty_regular_file_blocks() {
    let mut state = RiscvSyscallState::new(0).with_linux_se_memory_capacity(64 * 1024 * 1024);
    state.register_guest_file(b"empty.txt", b"");

    let statfs = statfs_bytes_for_state(state, b"empty.txt");

    assert_eq!(read_le_u64(&statfs, 24), 16 * 1024);
    assert_eq!(read_le_u64(&statfs, 32), 16 * 1024);
    assert_eq!(read_le_u64(&statfs, 40), 2);
}

#[test]
fn linux_table_statfs_counts_hard_linked_symlink_identity_once() {
    let mut state = RiscvSyscallState::new(0).with_linux_se_memory_capacity(64 * 1024 * 1024);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    state
        .link_guest_path(b"/proc/self/exe", b"/proc/self/alias")
        .expect("registered symlink can be linked");

    let statfs = statfs_bytes_for_state(state, b"/");

    assert_eq!(read_le_u64(&statfs, 24), 16 * 1024 - 1);
    assert_eq!(read_le_u64(&statfs, 40), 4);
    assert_eq!(read_le_u64(&statfs, 48), 16 * 1024 - 4);
}

#[test]
fn linux_table_statfs_reports_path_errors_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let missing_path = c_string_reader(0x9000, b"missing.txt");
    let (guest_memory_writer, writes) = recording_writer();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_STATFS_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&missing_path),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(writes.lock().unwrap().is_empty());

    let faulting_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_STATFS_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&faulting_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_fstatfs_reports_bad_fd_and_write_fault() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let good_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FSTATFS_FOR_TEST,
                [99, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&good_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );

    let faulting_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| false);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FSTATFS_FOR_TEST,
                [1, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

fn assert_statfs_header(statfs: &[u8]) {
    assert_eq!(read_le_u64(statfs, 0), RISCV_LINUX_STATFS_MAGIC_FOR_TEST);
    assert_eq!(
        read_le_u64(statfs, 8),
        RISCV_LINUX_STATFS_BLOCK_SIZE_FOR_TEST
    );
    assert_eq!(read_le_u32(statfs, 56), 0x7265_6d36);
    assert_eq!(read_le_u32(statfs, 60), 0);
    assert_eq!(
        read_le_u64(statfs, 64),
        RISCV_LINUX_STATFS_NAME_MAX_FOR_TEST
    );
    assert_eq!(
        read_le_u64(statfs, 72),
        RISCV_LINUX_STATFS_BLOCK_SIZE_FOR_TEST
    );
    assert_eq!(read_le_u64(statfs, 80), 0);
}

fn statfs_bytes_for_state(mut state: RiscvSyscallState, path: &'static [u8]) -> Vec<u8> {
    let table = RiscvSyscallTable::new();
    let guest_memory_reader = c_string_reader(0x9000, path);
    let (guest_memory_writer, writes) = recording_writer();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_STATFS_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
    let bytes = collect_guest_writes(
        &writes.lock().unwrap(),
        0x9100,
        RISCV_LINUX_STATFS_BYTES_FOR_TEST,
    );
    bytes
}

fn recording_writer() -> (RiscvGuestMemoryWriter, RecordedGuestWrites) {
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });
    (guest_memory_writer, writes)
}

fn c_string_reader(base: u64, bytes: &'static [u8]) -> RiscvGuestMemoryReader {
    let bytes = [bytes, b"\0"].concat();
    RiscvGuestMemoryReader::new(move |address, count| {
        if count != 1 || address < base {
            return None;
        }
        bytes
            .get((address - base) as usize)
            .copied()
            .map(|byte| vec![byte])
    })
}
