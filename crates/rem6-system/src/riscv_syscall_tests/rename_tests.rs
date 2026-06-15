use super::*;

const RISCV_LINUX_LINK_FOR_RENAME_TEST: u64 = 1025;
const RISCV_LINUX_CHDIR_FOR_RENAME_TEST: u64 = 49;
const RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST: u64 = 34;
const RISCV_LINUX_OPENAT_FOR_RENAME_TEST: u64 = 56;
const RISCV_LINUX_NEWFSTATAT_FOR_RENAME_TEST: u64 = 79;
const RISCV_LINUX_UMASK_FOR_RENAME_TEST: u64 = 166;
const RISCV_LINUX_RENAMEAT_FOR_TEST: u64 = 38;
const RISCV_LINUX_RENAMEAT2_FOR_TEST: u64 = 276;
const RISCV_LINUX_O_WRONLY_FOR_RENAME_TEST: u64 = 1;
const RISCV_LINUX_O_CREAT_FOR_RENAME_TEST: u64 = 0o100;
const RISCV_LINUX_O_DIRECTORY_FOR_RENAME_TEST: u64 = 0o200000;
const RISCV_LINUX_ENOTEMPTY_FOR_RENAME_TEST: u64 = 39;

#[test]
fn linux_table_renameat2_moves_registered_guest_file_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"renamed.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
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
    assert!(state.guest_path_stat(b"guest.txt").is_none());
    assert_eq!(
        state.guest_file_contents(b"renamed.txt"),
        Some(&b"file-backed input\n"[..])
    );
}

#[test]
fn linux_table_renameat_moves_registered_guest_file_path_and_ignores_stale_flags_register() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"renamed.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    1_u64 << 63,
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
    assert!(state.guest_path_stat(b"guest.txt").is_none());
    assert_eq!(
        state.guest_file_contents(b"renamed.txt"),
        Some(&b"file-backed input\n"[..])
    );
}

#[test]
fn linux_table_renameat2_replaces_existing_destination() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    state.register_guest_file(b"renamed.txt", b"other input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"renamed.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
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
    assert!(state.guest_path_stat(b"guest.txt").is_none());
    assert_eq!(
        state.guest_file_contents(b"renamed.txt"),
        Some(&b"file-backed input\n"[..])
    );
}

#[test]
fn linux_table_renameat2_drops_replaced_destination_mode_for_reregistration() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"source.txt", b"source input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"target.txt"), (0x9010, b"source.txt")]);
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
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_UMASK_FOR_RENAME_TEST,
                [0o027, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_OPENAT_FOR_RENAME_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY_FOR_RENAME_TEST | RISCV_LINUX_O_CREAT_FOR_RENAME_TEST,
                    0o666,
                    0,
                    0,
                ],
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
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9010,
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
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
        state.guest_file_contents(b"target.txt"),
        Some(&b"source input\n"[..])
    );
    assert!(state.unlink_guest_path(b"target.txt"));
    state.register_guest_file(b"target.txt", b"new input\n");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_NEWFSTATAT_FOR_RENAME_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0xa000, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let stat = collect_guest_writes(&writes.lock().unwrap(), 0xa000, 128);
    assert_eq!(read_le_u32(&stat, 16), 0o100444);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_renameat2_replaces_existing_absolute_destination() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    state.register_guest_file(b"/tmp/existing.txt", b"absolute existing\n");
    let guest_memory_reader =
        c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"/tmp/existing.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
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
    assert!(state.guest_path_stat(b"guest.txt").is_none());
    assert!(state.guest_file_contents(b"tmp/existing.txt").is_none());
    assert_eq!(
        state.guest_file_contents(b"/tmp/existing.txt"),
        Some(&b"file-backed input\n"[..])
    );
}

#[test]
fn linux_table_renameat2_same_file_hard_links_preserves_both_names() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"alias.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LINK_FOR_RENAME_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
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
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
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
        state.guest_path_stat(b"guest.txt"),
        state.guest_path_stat(b"alias.txt")
    );
    assert_eq!(
        state.guest_file_contents(b"guest.txt"),
        Some(&b"file-backed input\n"[..])
    );
    assert_eq!(
        state.guest_file_contents(b"alias.txt"),
        Some(&b"file-backed input\n"[..])
    );
}

#[test]
fn linux_table_renameat2_moves_guest_directory_subtree() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"a"),
        (0x9010, b"a/b"),
        (0x9020, b"c"),
        (0x9030, b"c/b"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
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
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9010, 0o700, 0, 0, 0],
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
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9020,
                    0,
                    0,
                ],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_path_stat(b"a").is_none());
    assert!(state.guest_path_stat(b"a/b").is_none());
    assert!(state.guest_path_stat(b"c").is_some());
    assert!(state.guest_path_stat(b"c/b").is_some());
}

#[test]
fn linux_table_renameat2_rebases_open_guest_directory_fd_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader =
        c_string_reader(&[(0x9000, b"a"), (0x9010, b"c"), (0x9020, b"child")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
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
                RISCV_LINUX_OPENAT_FOR_RENAME_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_RENAME_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9010,
                    0,
                    0,
                ],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
                [3, 0x9020, 0o700, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_path_stat(b"/a/child").is_none());
    assert!(state.guest_path_stat(b"/c/child").is_some());
}

#[test]
fn linux_table_renameat2_rebases_current_guest_directory_path() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"a"),
        (0x9010, b"/a"),
        (0x9020, b"/c"),
        (0x9030, b"child"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
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
                RISCV_LINUX_CHDIR_FOR_RENAME_TEST,
                [0x9000, 0, 0, 0, 0, 0],
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
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9010,
                    RISCV_LINUX_AT_FDCWD,
                    0x9020,
                    0,
                    0,
                ],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.current_directory(), b"/c");
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9030, 0o700, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_path_stat(b"/a/child").is_none());
    assert!(state.guest_path_stat(b"/c/child").is_some());
}

#[test]
fn linux_table_renameat2_rejects_empty_directory_over_nonempty_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"empty"),
        (0x9010, b"target"),
        (0x9020, b"target/child"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
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
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
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
                RISCV_LINUX_MKDIRAT_FOR_RENAME_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9020, 0o755, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9010,
                    0,
                    0,
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTEMPTY_FOR_RENAME_TEST)
        })
    );
    assert!(state.guest_path_stat(b"empty").is_some());
    assert!(state.guest_path_stat(b"target").is_some());
    assert!(state.guest_path_stat(b"target/child").is_some());
}

#[test]
fn linux_table_renameat2_rejects_relative_path_with_unknown_directory_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"renamed.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [3, 0x9000, RISCV_LINUX_AT_FDCWD, 0x9100, 0, 0],
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
    assert!(state.guest_path_stat(b"renamed.txt").is_none());
}

#[test]
fn linux_table_renameat2_rejects_relative_destination_with_unknown_directory_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"renamed.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 4, 0x9100, 0, 0],
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
    assert!(state.guest_path_stat(b"renamed.txt").is_none());
}

#[test]
fn linux_table_renameat2_rejects_unsupported_flags_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"renamed.txt")]);
    let unsupported_flag = 1_u64 << 63;

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    unsupported_flag,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.guest_path_stat(b"guest.txt").is_some());
    assert!(state.guest_path_stat(b"renamed.txt").is_none());
}

#[test]
fn linux_table_renameat2_rejects_missing_source_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader =
        c_string_reader(&[(0x9000, b"missing.txt"), (0x9100, b"renamed.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.guest_path_stat(b"missing.txt").is_none());
    assert!(state.guest_path_stat(b"renamed.txt").is_none());
}

#[test]
fn linux_table_renameat2_rejects_empty_source_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b""), (0x9100, b"renamed.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.guest_path_stat(b"guest.txt").is_some());
    assert!(state.guest_path_stat(b"renamed.txt").is_none());
}

#[test]
fn linux_table_renameat2_rejects_empty_destination_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RENAMEAT2_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_AT_FDCWD,
                    0x9100,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.guest_path_stat(b"guest.txt").is_some());
}

fn c_string_reader(entries: &[(u64, &'static [u8])]) -> RiscvGuestMemoryReader {
    let entries = entries
        .iter()
        .map(|(base, bytes)| (*base, nul_terminated(bytes)))
        .collect::<Vec<_>>();
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 {
            return None;
        }
        entries.iter().find_map(|(base, value)| {
            value
                .get(usize::try_from(address.checked_sub(*base)?).ok()?)
                .copied()
                .map(|byte| vec![byte])
        })
    })
}

fn nul_terminated(bytes: &'static [u8]) -> Vec<u8> {
    let mut value = bytes.to_vec();
    value.push(0);
    value
}
