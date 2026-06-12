use super::*;

const RISCV_LINUX_FACCESSAT_FOR_TEST: u64 = 48;
const RISCV_LINUX_ACCESS_FOR_TEST: u64 = 1033;
const RISCV_LINUX_R_OK_FOR_TEST: u64 = 4;
const RISCV_LINUX_W_OK_FOR_TEST: u64 = 2;
const RISCV_LINUX_EACCES_FOR_TEST: u64 = 13;

#[test]
fn linux_table_stat_writes_registered_guest_file_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_STAT, [0x9000, 0x9100, 0, 0, 0, 0]),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(read_le_u64(&stat, 48), 18);
    assert_eq!(read_le_u32(&stat, 16), 0o100444);
}

#[test]
fn linux_table_access_checks_registered_guest_file_paths() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_ACCESS_FOR_TEST,
                [0x9000, RISCV_LINUX_R_OK_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_access_missing_guest_path_returns_enoent() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(0x9000, b"missing.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_ACCESS_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_access_write_mode_returns_eacces_for_read_only_guest_file() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_ACCESS_FOR_TEST,
                [0x9000, RISCV_LINUX_W_OK_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EACCES_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat_checks_registered_guest_file_paths() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_R_OK_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat_rejects_non_cwd_dirfd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT_FOR_TEST,
                [3, 0x9000, RISCV_LINUX_R_OK_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat_ignores_dirfd_for_absolute_guest_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"/input.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT_FOR_TEST,
                [3, 0x9000, RISCV_LINUX_R_OK_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat_empty_path_precedes_non_cwd_dirfd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(0x9000, b"");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT_FOR_TEST,
                [3, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat_invalid_mode_precedes_non_cwd_dirfd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT_FOR_TEST,
                [3, 0x9000, 8, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat_faulting_path_precedes_non_cwd_dirfd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT_FOR_TEST,
                [3, 0x8000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn c_string_reader(base: u64, bytes: &'static [u8]) -> RiscvGuestMemoryReader {
    let path = [bytes, b"\0"].concat();
    RiscvGuestMemoryReader::new(move |address, count| {
        if count != 1 || address < base {
            return None;
        }
        path.get((address - base) as usize)
            .copied()
            .map(|byte| vec![byte])
    })
}
