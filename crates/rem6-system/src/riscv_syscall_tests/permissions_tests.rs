use super::*;

const RISCV_LINUX_FCHMOD_FOR_TEST: u64 = 52;
const RISCV_LINUX_FCHMODAT_FOR_TEST: u64 = 53;
const RISCV_LINUX_FCHMODAT2_FOR_TEST: u64 = 452;
const RISCV_LINUX_FCHOWNAT_FOR_TEST: u64 = 54;
const RISCV_LINUX_FCHOWN_FOR_TEST: u64 = 55;
const RISCV_LINUX_OPENAT_FOR_PERMISSIONS_TEST: u64 = 56;
const RISCV_LINUX_FSTAT_FOR_PERMISSIONS_TEST: u64 = 80;
const RISCV_NEWLIB_LEGACY_CHMOD_FOR_TEST: u64 = 1028;
const RISCV_LINUX_NEWFSTATAT_FOR_PERMISSIONS_TEST: u64 = 79;
const RISCV_LINUX_ACCESS_FOR_PERMISSIONS_TEST: u64 = 1033;
const RISCV_LINUX_EPERM_FOR_PERMISSIONS_TEST: u64 = 1;
const RISCV_LINUX_EACCES_FOR_PERMISSIONS_TEST: u64 = 13;
const RISCV_LINUX_W_OK_FOR_PERMISSIONS_TEST: u64 = 2;
const RISCV_LINUX_X_OK_FOR_PERMISSIONS_TEST: u64 = 1;
const RISCV_LINUX_O_DIRECTORY_FOR_PERMISSIONS_TEST: u64 = 0o200000;
const RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST: u64 = u64::MAX;
const RISCV_LINUX_TRUNCATED_NO_OWNER_CHANGE_FOR_TEST: u64 = u32::MAX as u64;

#[test]
fn linux_table_chmod_updates_registered_file_permissions() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_NEWLIB_LEGACY_CHMOD_FOR_TEST,
                [0x9000, 0o600, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        stat_mode(&table, &mut state, &guest_memory_reader, 0x8004),
        0o100600
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_ACCESS_FOR_PERMISSIONS_TEST,
                [0x9000, RISCV_LINUX_W_OK_FOR_PERMISSIONS_TEST, 0, 0, 0, 0],
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
                0x800c,
                RISCV_LINUX_ACCESS_FOR_PERMISSIONS_TEST,
                [0x9000, RISCV_LINUX_X_OK_FOR_PERMISSIONS_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EACCES_FOR_PERMISSIONS_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_chmod_prefers_exact_file_over_implicit_child_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"node", b"exact file\n");
    state.register_guest_file(b"node/child.txt", b"nested file\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"node"), (0x9010, b"node/child.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_NEWLIB_LEGACY_CHMOD_FOR_TEST,
                [0x9000, 0o600, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        stat_mode_at(&table, &mut state, &guest_memory_reader, 0x9000, 0x8004),
        0o100600
    );
    assert_eq!(
        stat_mode_at(&table, &mut state, &guest_memory_reader, 0x9010, 0x8008),
        0o100444
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fchmod_updates_open_guest_file_permissions() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_PERMISSIONS_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_FCHMOD_FOR_TEST, [3, 0o700, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(fd_stat_mode(&table, &mut state, 3, 0x8008), 0o100700);
    assert_eq!(
        stat_mode(&table, &mut state, &guest_memory_reader, 0x8008),
        0o100700
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fchmodat_resolves_relative_path_against_guest_directory_fd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"sub/guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"sub"),
        (0x9010, b"guest.txt"),
        (0x9020, b"sub/guest.txt"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_PERMISSIONS_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_PERMISSIONS_TEST,
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
                RISCV_LINUX_FCHMODAT_FOR_TEST,
                [3, 0x9010, 0o640, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        stat_mode_at(&table, &mut state, &guest_memory_reader, 0x9020, 0x8008),
        0o100640
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fchmodat_ignores_fourth_argument_for_syscall_53() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCHMODAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o640, 0xdead_beef, 0, 0,],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        stat_mode(&table, &mut state, &guest_memory_reader, 0x8004),
        0o100640
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fchmodat2_updates_registered_file_permissions() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCHMODAT2_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o640, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        stat_mode(&table, &mut state, &guest_memory_reader, 0x8004),
        0o100640
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_fchmodat2_rejects_unknown_flags() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCHMODAT2_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o640, 0x8000, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        stat_mode(&table, &mut state, &guest_memory_reader, 0x8004),
        0o100444
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_chmod_family_preserves_special_mode_bits() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    state.register_guest_directory(b"sub");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9010, b"sub")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_NEWLIB_LEGACY_CHMOD_FOR_TEST,
                [0x9000, 0o4755, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        stat_mode(&table, &mut state, &guest_memory_reader, 0x8004),
        0o104755
    );

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCHMODAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9010, 0o1755, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        stat_mode_at(&table, &mut state, &guest_memory_reader, 0x9010, 0x800c),
        0o041755
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_chown_family_validates_registered_paths_fds_and_flags() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    state.register_guest_symlink(b"link.txt", b"guest.txt");
    state.register_guest_file(b"sub/target.txt", b"nested input\n");
    state.register_guest_symlink(b"abs-link.txt", b"/guest.txt");
    state.register_guest_symlink(b"sub/link.txt", b"../guest.txt");
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"guest.txt"),
        (0x9010, b"missing.txt"),
        (0x9020, b""),
        (0x9030, b"link.txt"),
        (0x9040, b"abs-link.txt"),
        (0x9050, b"sub/link.txt"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
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
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_TRUNCATED_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_TRUNCATED_NO_OWNER_CHANGE_FOR_TEST,
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
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9010,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
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
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    0x8000,
                    0,
                ],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_OPENAT_FOR_PERMISSIONS_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_FCHOWN_FOR_TEST,
                [
                    3,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8018,
                RISCV_LINUX_FCHOWN_FOR_TEST,
                [
                    99,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x801c,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    3,
                    0x9020,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_AT_EMPTY_PATH,
                    0,
                ],
            ),
            &mut state,
            12,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8020,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    99,
                    0x9020,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_AT_EMPTY_PATH,
                    0,
                ],
            ),
            &mut state,
            13,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8024,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9030,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
            14,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8028,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9040,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
            15,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x802c,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9050,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    0,
                    0,
                ],
            ),
            &mut state,
            16,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8030,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9030,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_NO_OWNER_CHANGE_FOR_TEST,
                    RISCV_LINUX_AT_SYMLINK_NOFOLLOW,
                    0,
                ],
            ),
            &mut state,
            17,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8034,
                RISCV_LINUX_FCHOWNAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0,],
            ),
            &mut state,
            18,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM_FOR_PERMISSIONS_TEST)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8030, RISCV_LINUX_FCHOWN_FOR_TEST, [3, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM_FOR_PERMISSIONS_TEST)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_chmod_reports_missing_path_and_bad_fd_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[(0x9000, b"missing.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_NEWLIB_LEGACY_CHMOD_FOR_TEST,
                [0x9000, 0o600, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_FCHMOD_FOR_TEST, [99, 0o600, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EBADF)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn stat_mode(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    pc: u64,
) -> u32 {
    stat_mode_at(table, state, guest_memory_reader, 0x9000, pc)
}

fn stat_mode_at(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    guest_memory_reader: &RiscvGuestMemoryReader,
    path_address: u64,
    pc: u64,
) -> u32 {
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
                pc,
                RISCV_LINUX_NEWFSTATAT_FOR_PERMISSIONS_TEST,
                [RISCV_LINUX_AT_FDCWD, path_address, 0x9100, 0, 0, 0],
            ),
            state,
            11,
            Some(guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    read_le_u32(&stat, 16)
}

fn fd_stat_mode(table: &RiscvSyscallTable, state: &mut RiscvSyscallState, fd: u64, pc: u64) -> u32 {
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
                pc,
                RISCV_LINUX_FSTAT_FOR_PERMISSIONS_TEST,
                [fd, 0x9100, 0, 0, 0, 0],
            ),
            state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let stat = collect_guest_writes(&writes, 0x9100, 128);
    read_le_u32(&stat, 16)
}

fn c_string_reader(strings: &[(u64, &'static [u8])]) -> RiscvGuestMemoryReader {
    let strings = strings
        .iter()
        .map(|(base, bytes)| (*base, [*bytes, b"\0"].concat()))
        .collect::<Vec<_>>();
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 {
            return None;
        }
        strings.iter().find_map(|(base, contents)| {
            let offset = usize::try_from(address.checked_sub(*base)?).ok()?;
            contents.get(offset).copied().map(|byte| vec![byte])
        })
    })
}
