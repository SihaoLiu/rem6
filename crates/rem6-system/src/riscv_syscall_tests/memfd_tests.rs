use super::*;
use std::sync::{Arc, Mutex};

const RISCV_LINUX_MEMFD_CREATE_FOR_TEST: u64 = 279;
const RISCV_LINUX_MFD_CLOEXEC_FOR_TEST: u64 = 0x0001;
const RISCV_LINUX_MFD_ALLOW_SEALING_FOR_TEST: u64 = 0x0002;
const RISCV_LINUX_MFD_UNKNOWN_FOR_TEST: u64 = 0x0020;
const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;
const RISCV_LINUX_F_ADD_SEALS_FOR_TEST: u64 = 1033;
const RISCV_LINUX_F_GET_SEALS_FOR_TEST: u64 = 1034;
const RISCV_LINUX_F_SEAL_SEAL_FOR_TEST: u64 = 0x0001;
const RISCV_LINUX_F_SEAL_SHRINK_FOR_TEST: u64 = 0x0002;
const RISCV_LINUX_F_SEAL_GROW_FOR_TEST: u64 = 0x0004;
const RISCV_LINUX_F_SEAL_WRITE_FOR_TEST: u64 = 0x0008;

type RecordedWrites = Arc<Mutex<Vec<(u64, Vec<u8>)>>>;

#[test]
fn linux_table_memfd_create_returns_anonymous_regular_file_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![
        (0x9000, b"scratch\0".to_vec()),
        (0x9100, b"abc".to_vec()),
    ]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writer = recording_writer(Arc::clone(&writes));

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMFD_CREATE_FOR_TEST,
                [0x9000, RISCV_LINUX_MFD_CLOEXEC_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert!(state.unknown_syscalls().is_empty());
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFD, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_FD_CLOEXEC
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITE, [3, 0x9100, 3, 0, 0, 0]),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_LSEEK,
                [3, 0, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0xa000, 3, 0, 0, 0]),
            &mut state,
            3,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_FTRUNCATE, [3, 5, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8018, RISCV_LINUX_FSTAT, [3, 0xb000, 0, 0, 0, 0]),
            &mut state,
            4,
            None,
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa000, 3), 0xa000, 3),
        b"abc"
    );
    let stat = collect_guest_writes(&writes_in_range(&writes, 0xb000, 128), 0xb000, 128);
    assert_eq!(read_le_u32(&stat, 16), 0o100777);
    assert_eq!(read_le_u32(&stat, 20), 0);
    assert_eq!(read_le_u64(&stat, 48), 5);
}

#[test]
fn linux_table_memfd_create_rejects_unknown_flags_without_allocating_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![(0x9000, b"scratch\0".to_vec())]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMFD_CREATE_FOR_TEST,
                [0x9000, RISCV_LINUX_MFD_UNKNOWN_FOR_TEST, 0, 0, 0, 0],
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
    assert!(state.guest_fds().entry(GuestFd::new(3).unwrap()).is_none());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_memfd_create_truncates_flags_to_linux_unsigned_int() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![(0x9000, b"scratch\0".to_vec())]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMFD_CREATE_FOR_TEST,
                [0x9000, 1 << 63, 0, 0, 0, 0],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_memfd_create_accepts_linux_maximum_name_length() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let mut name = vec![b'a'; 249];
    name.push(0);
    let reader = memory_reader(vec![(0x9000, name)]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMFD_CREATE_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
            ),
            &mut state,
            1,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_memfd_create_supports_basic_file_seals() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![
        (0x9000, b"sealed\0".to_vec()),
        (0x9100, b"abc".to_vec()),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMFD_CREATE_FOR_TEST,
                [0x9000, RISCV_LINUX_MFD_ALLOW_SEALING_FOR_TEST, 0, 0, 0, 0],
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
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GET_SEALS_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITE, [3, 0x9100, 3, 0, 0, 0]),
            &mut state,
            2,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCNTL,
                [
                    3,
                    RISCV_LINUX_F_ADD_SEALS_FOR_TEST,
                    RISCV_LINUX_F_SEAL_SHRINK_FOR_TEST,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GET_SEALS_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_F_SEAL_SHRINK_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_FTRUNCATE, [3, 2, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_FCNTL,
                [
                    3,
                    RISCV_LINUX_F_ADD_SEALS_FOR_TEST,
                    RISCV_LINUX_F_SEAL_WRITE_FOR_TEST,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x801c, RISCV_LINUX_WRITE, [3, 0x9100, 1, 0, 0, 0]),
            &mut state,
            3,
            Some(&reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
}

#[test]
fn linux_table_memfd_create_without_allow_sealing_starts_seal_locked() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = memory_reader(vec![(0x9000, b"sealed\0".to_vec())]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MEMFD_CREATE_FOR_TEST,
                [0x9000, 0, 0, 0, 0, 0],
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
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GET_SEALS_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_F_SEAL_SEAL_FOR_TEST
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [
                    3,
                    RISCV_LINUX_F_ADD_SEALS_FOR_TEST,
                    RISCV_LINUX_F_SEAL_GROW_FOR_TEST,
                    0,
                    0,
                    0
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
}

fn memory_reader(chunks: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        let end = address.checked_add(bytes as u64)?;
        chunks.iter().find_map(|(base, data)| {
            let data_end = base.checked_add(data.len() as u64)?;
            if address < *base || end > data_end {
                return None;
            }
            let start = usize::try_from(address - *base).ok()?;
            Some(data[start..start + bytes].to_vec())
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
        .filter(|(address, bytes)| {
            let write_end = *address + bytes.len() as u64;
            *address >= base && write_end <= end
        })
        .cloned()
        .collect()
}
