use super::*;

const RISCV_LINUX_UNLINK_FOR_TEST: u64 = 1026;
const RISCV_LINUX_UNLINKAT_FOR_TEST: u64 = 35;
const RISCV_LINUX_MKDIRAT_FOR_TEST: u64 = 34;
const RISCV_LINUX_OPENAT_FOR_UNLINK_TEST: u64 = 56;
const RISCV_LINUX_AT_REMOVEDIR_FOR_TEST: u64 = 0x200;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;
const RISCV_LINUX_ENOTDIR_FOR_TEST: u64 = 20;
const RISCV_LINUX_EISDIR_FOR_TEST: u64 = 21;
const RISCV_LINUX_ENOTEMPTY_FOR_TEST: u64 = 39;

#[test]
fn linux_table_unlinkat_removes_registered_guest_file_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let path = b"guest.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
    assert!(state.guest_path_stat(b"guest.txt").is_none());
}

#[test]
fn linux_table_unlinkat_rejects_relative_path_with_unknown_directory_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let path = b"guest.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [3, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.guest_path_stat(b"guest.txt").is_some());
}

#[test]
fn linux_table_unlinkat_removedir_removes_empty_guest_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"made\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o755, 0, 0, 0],
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
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_REMOVEDIR_FOR_TEST,
                    0,
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
                RISCV_LINUX_OPENAT_FOR_UNLINK_TEST,
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
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
}

#[test]
fn linux_table_unlinkat_removedir_uses_directory_fd_for_relative_child() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"parent"),
        (0x9010, b"parent/child"),
        (0x9020, b"child"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o755, 0, 0, 0],
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
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9010, 0o755, 0, 0, 0],
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
                RISCV_LINUX_OPENAT_FOR_UNLINK_TEST,
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
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [3, 0x9020, RISCV_LINUX_AT_REMOVEDIR_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_path_stat(b"parent/child").is_none());
    assert!(state.guest_path_stat(b"parent").is_some());
}

#[test]
fn linux_table_unlinkat_without_removedir_rejects_guest_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"made\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o755, 0, 0, 0],
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
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EISDIR_FOR_TEST)
        })
    );
    assert!(state.guest_path_stat(b"made").is_some());
}

#[test]
fn linux_table_unlinkat_removedir_rejects_non_directory_and_nonempty_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    state.register_guest_file(b"sub/child.txt", b"nested input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9010, b"sub")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_REMOVEDIR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTDIR_FOR_TEST)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9010,
                    RISCV_LINUX_AT_REMOVEDIR_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTEMPTY_FOR_TEST)
        })
    );
    assert!(state.guest_path_stat(b"sub").is_some());
}

#[test]
fn linux_table_unlinkat_removedir_handles_absolute_registered_guest_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_directory(b"/abs");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"/abs")]);
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
                RISCV_LINUX_NEWFSTATAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0xa000, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let stat = collect_guest_writes(&writes.lock().unwrap(), 0xa000, 128);
    assert_eq!(read_le_u32(&stat, 16), 0o040555);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_UNLINKAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_REMOVEDIR_FOR_TEST,
                    0,
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
    assert!(state.guest_path_stat(b"/abs").is_none());
}

#[test]
fn linux_table_unlink_removes_registered_guest_file_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let path = b"guest.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_UNLINK_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
    assert!(state.guest_path_stat(b"guest.txt").is_none());
}

fn c_string_reader(entries: &[(u64, &'static [u8])]) -> RiscvGuestMemoryReader {
    let entries = entries
        .iter()
        .map(|(base, bytes)| {
            let mut c_string = bytes.to_vec();
            c_string.push(0);
            (*base, c_string)
        })
        .collect::<Vec<_>>();
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 {
            return None;
        }
        entries.iter().find_map(|(base, contents)| {
            address
                .checked_sub(*base)
                .and_then(|offset| contents.get(offset as usize))
                .copied()
                .map(|byte| vec![byte])
        })
    })
}

#[test]
fn linux_table_unlink_missing_guest_path_returns_enoent() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"missing.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_UNLINK_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
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
fn linux_table_unlink_preserves_open_guest_file_description() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let path = b"guest.txt\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 || address < 0x9000 {
            return None;
        }
        path.get((address - 0x9000) as usize)
            .copied()
            .map(|byte| vec![byte])
    });
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
                RISCV_LINUX_OPEN,
                [0x9000, RISCV_LINUX_O_RDONLY, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_UNLINK_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_READ, [3, 0x9100, 18, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 18 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes, 0x9100, 18),
        b"file-backed input\n"
    );
}
