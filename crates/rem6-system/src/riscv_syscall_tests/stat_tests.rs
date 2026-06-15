use super::*;

const RISCV_LINUX_FACCESSAT_FOR_TEST: u64 = 48;
const RISCV_LINUX_FACCESSAT2_FOR_TEST: u64 = 439;
const RISCV_LINUX_OPENAT_FOR_STAT_TEST: u64 = 56;
const RISCV_LINUX_STATX_FOR_TEST: u64 = 291;
const RISCV_LINUX_ACCESS_FOR_TEST: u64 = 1033;
const RISCV_LINUX_LSTAT_FOR_TEST: u64 = 1039;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;
const RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST: u32 = 0x0000_07ff;
const RISCV_LINUX_STATX_RESERVED_FOR_TEST: u64 = 0x8000_0000;
const RISCV_LINUX_AT_STATX_SYNC_TYPE_FOR_TEST: u64 = 0x6000;
const RISCV_LINUX_R_OK_FOR_TEST: u64 = 4;
const RISCV_LINUX_W_OK_FOR_TEST: u64 = 2;
const RISCV_LINUX_X_OK_FOR_TEST: u64 = 1;
const RISCV_LINUX_EACCES_FOR_TEST: u64 = 13;
const RISCV_LINUX_AT_EACCESS_FOR_TEST: u64 = 0x200;

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
fn linux_table_lstat_writes_registered_guest_file_stat() {
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
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LSTAT_FOR_TEST,
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

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(read_le_u64(&stat, 48), 18);
    assert_eq!(read_le_u32(&stat, 16), 0o100444);
}

#[test]
fn linux_table_lstat_writes_registered_guest_symlink_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let guest_memory_reader = c_string_reader(0x9000, b"/proc/self/exe");
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
                RISCV_LINUX_LSTAT_FOR_TEST,
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

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 128);
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    assert_eq!(read_le_u64(&stat, 48), 9);
    assert_eq!(read_le_u32(&stat, 16), 0o120777);
    assert_eq!(read_le_u32(&stat, 20), 1);
}

#[test]
fn linux_table_statx_writes_registered_guest_file_statx() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/input.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"/input.txt");
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
                RISCV_LINUX_STATX_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_NO_AUTOMOUNT,
                    u64::from(RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST),
                    0x9200,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 256);
    let statx = collect_guest_writes(&writes, 0x9200, 256);
    assert_eq!(
        read_le_u32(&statx, 0),
        RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST
    );
    assert_eq!(read_le_u32(&statx, 4), 8192);
    assert_eq!(read_le_u32(&statx, 16), 1);
    assert_eq!(read_le_u32(&statx, 20), 100);
    assert_eq!(read_le_u32(&statx, 24), 100);
    assert_eq!(read_le_u32(&statx, 28) & 0xffff, 0o100444);
    assert_ne!(read_le_u64(&statx, 32), 0);
    assert_eq!(read_le_u64(&statx, 40), 18);
    assert_eq!(read_le_u64(&statx, 48), 1);
    assert_eq!(read_le_u64(&statx, 56), 0);
}

#[test]
fn linux_table_statx_empty_path_stats_guest_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(0x9000, b"");
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
                RISCV_LINUX_STATX_FOR_TEST,
                [
                    1,
                    0x9000,
                    RISCV_LINUX_AT_EMPTY_PATH,
                    u64::from(RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST),
                    0x9100,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 256);
    let statx = collect_guest_writes(&writes, 0x9100, 256);
    assert_eq!(read_le_u32(&statx, 16), 1);
    assert_eq!(read_le_u32(&statx, 28) & 0xffff, 0o020666);
    assert_eq!(read_le_u64(&statx, 40), 0);
}

#[test]
fn linux_table_statx_rejects_reserved_mask_and_bad_sync_flags_without_writing() {
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
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_STATX_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    0,
                    RISCV_LINUX_STATX_RESERVED_FOR_TEST,
                    0x9100,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_STATX_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_STATX_SYNC_TYPE_FOR_TEST,
                    u64::from(RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST),
                    0x9200,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_statx_empty_path_at_cwd_stats_current_guest_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"");
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
                RISCV_LINUX_STATX_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_EMPTY_PATH,
                    u64::from(RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST),
                    0x9100,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 256);
    let statx = collect_guest_writes(&writes, 0x9100, 256);
    assert_eq!(read_le_u32(&statx, 28) & 0xffff, 0o040555);
}

#[test]
fn linux_table_statx_resolves_relative_path_against_guest_directory_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"sub/guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader_with(&[(0x9000, b"sub"), (0x9010, b"guest.txt")]);
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
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_STAT_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_STATX_FOR_TEST,
                [
                    3,
                    0x9010,
                    0,
                    u64::from(RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST),
                    0x9200,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let writes = writes.lock().unwrap();
    assert_eq!(writes.len(), 256);
    let statx = collect_guest_writes(&writes, 0x9200, 256);
    assert_eq!(read_le_u64(&statx, 40), 18);
    assert_eq!(read_le_u32(&statx, 28) & 0xffff, 0o100444);
}

#[test]
fn linux_table_statx_missing_path_returns_enoent_without_writing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(0x9000, b"missing.txt");
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
                RISCV_LINUX_STATX_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    0,
                    u64::from(RISCV_LINUX_STATX_BASIC_STATS_FOR_TEST),
                    0x9100,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
    assert!(writes.lock().unwrap().is_empty());
}

#[test]
fn linux_table_legacy_stat_and_lstat_reject_empty_paths() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"");
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
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_LSTAT_FOR_TEST,
                [0x9000, 0x9200, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(writes.lock().unwrap().is_empty());
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

#[test]
fn linux_table_faccessat2_checks_registered_guest_file_paths() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader =
        c_string_reader_with(&[(0x9000, b"guest.txt"), (0x9020, b"missing.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
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
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_R_OK_FOR_TEST,
                    RISCV_LINUX_AT_EACCESS_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9020, 0, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat2_resolves_relative_path_against_guest_directory_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"sub/guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader_with(&[(0x9000, b"sub"), (0x9010, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_STAT_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [3, 0x9010, RISCV_LINUX_R_OK_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat2_returns_enotdir_for_regular_file_dirfd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"plain.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader_with(&[(0x9000, b"plain.txt"), (0x9020, b"child")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_STAT_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [3, 0x9020, RISCV_LINUX_R_OK_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTDIR)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_faccessat2_nofollow_checks_guest_symlink_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let guest_memory_reader = c_string_reader(0x9000, b"/proc/self/exe");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_X_OK_FOR_TEST,
                    RISCV_LINUX_AT_SYMLINK_NOFOLLOW,
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
fn linux_table_faccessat2_rejects_unknown_flags_before_dirfd_validation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [3, 0x9000, RISCV_LINUX_R_OK_FOR_TEST, 0x8000_0000, 0, 0],
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
fn linux_table_faccessat2_empty_path_checks_guest_fd_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(0x9000, b"");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [
                    1,
                    0x9000,
                    RISCV_LINUX_R_OK_FOR_TEST,
                    RISCV_LINUX_AT_EMPTY_PATH,
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
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FACCESSAT2_FOR_TEST,
                [
                    1,
                    0x9000,
                    RISCV_LINUX_X_OK_FOR_TEST,
                    RISCV_LINUX_AT_EMPTY_PATH,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EACCES_FOR_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn c_string_reader(base: u64, bytes: &'static [u8]) -> RiscvGuestMemoryReader {
    c_string_reader_with(&[(base, bytes)])
}

fn c_string_reader_with(strings: &[(u64, &'static [u8])]) -> RiscvGuestMemoryReader {
    let strings = strings
        .iter()
        .map(|(base, bytes)| (*base, [*bytes, b"\0"].concat()))
        .collect::<Vec<_>>();
    RiscvGuestMemoryReader::new(move |address, count| {
        if count != 1 {
            return None;
        }
        strings.iter().find_map(|(base, path)| {
            if address < *base {
                return None;
            }
            path.get((address - *base) as usize)
                .copied()
                .map(|byte| vec![byte])
        })
    })
}
