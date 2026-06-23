use super::*;

const RISCV_LINUX_O_RDWR_FOR_TEST: u64 = 2;
const RISCV_LINUX_O_WRONLY_FOR_TEST: u64 = 1;
const RISCV_LINUX_O_CREAT_FOR_TEST: u64 = 0o100;
const RISCV_LINUX_O_APPEND_FOR_TEST: u64 = 0o2000;
const RISCV_LINUX_O_DSYNC_FOR_TEST: u64 = 0o10000;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;
const RISCV_LINUX_O_NOCTTY_FOR_TEST: u64 = 0o400;
const RISCV_LINUX_O_NOFOLLOW_FOR_TEST: u64 = 0o400000;
const RISCV_LINUX_O_CLOEXEC_FOR_TEST: u64 = 0o2000000;
const RISCV_LINUX_O_SYNC_INTERNAL_FOR_TEST: u64 = 0o4000000;
const RISCV_LINUX_O_SYNC_FOR_TEST: u64 =
    RISCV_LINUX_O_SYNC_INTERNAL_FOR_TEST | RISCV_LINUX_O_DSYNC_FOR_TEST;
const RISCV_NEWLIB_O_CREAT_FOR_TEST: u64 = 0x0200;
const RISCV_NEWLIB_O_TRUNC_FOR_TEST: u64 = 0x0400;
const RISCV_NEWLIB_O_EXCL_FOR_TEST: u64 = 0x0800;
const RISCV_NEWLIB_O_SYNC_FOR_TEST: u64 = 0x2000;
const RISCV_NEWLIB_O_NOCTTY_FOR_TEST: u64 = 0x8000;
const RISCV_NEWLIB_O_DIRECT_FOR_TEST: u64 = 0x80000;
const RISCV_NEWLIB_O_NOFOLLOW_FOR_TEST: u64 = 0x100000;
const RISCV_NEWLIB_O_CLOEXEC_FOR_TEST: u64 = 0x40000;
const RISCV_NEWLIB_O_DIRECTORY_FOR_TEST: u64 = 0x200000;
const RISCV_LINUX_UMASK_FOR_OPEN_TEST: u64 = 166;
const RISCV_LINUX_LINK_FOR_OPEN_TEST: u64 = 1025;
const RISCV_LINUX_RENAMEAT2_FOR_OPEN_TEST: u64 = 276;
const RISCV_LINUX_X_OK_FOR_OPEN_TEST: u64 = 1;
const RISCV_LINUX_EACCES_FOR_OPEN_TEST: u64 = 13;
const RISCV_LINUX_EEXIST_FOR_OPEN_TEST: u64 = 17;
const RISCV_LINUX_EINVAL_FOR_OPEN_TEST: u64 = 22;
const RISCV_LINUX_ELOOP_FOR_OPEN_TEST: u64 = 40;
const RISCV_LINUX_MAP_ANONYMOUS_FOR_OPEN_TEST: u64 = 0x20;

#[test]
fn linux_table_openat_reads_virtual_proc_self_maps() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0x20000);
    let guest_memory_reader = c_string_reader(0x9000, b"/proc/self/maps");
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_BRK, [0x24000, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0x24000 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_MMAP,
                [
                    0,
                    RISCV_PAGE_BYTES + 7,
                    3,
                    RISCV_LINUX_MAP_PRIVATE | RISCV_LINUX_MAP_ANONYMOUS_FOR_OPEN_TEST,
                    u64::MAX,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV64_LINUX_MMAP_BASE
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );

    let expected_maps = concat!(
        "0000000000020000-0000000000024000 rw-p 00000000 00:00 0 [heap]\n",
        "4000000000000000-4000000000002000 rw-p 00000000 00:00 0 [anon]\n",
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_READ,
                [3, 0xa000, expected_maps.len() as u64 + 64, 0, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: expected_maps.len() as u64
        })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0xa000, expected_maps.as_bytes().to_vec())]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_rejects_virtual_proc_maps_through_missing_parent() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0x20000);
    let guest_memory_reader = c_string_reader(0x9000, b"missing/../proc/self/maps");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.guest_opens().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_rejects_virtual_proc_maps_through_proc_fd_parent() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0x20000);
    let guest_memory_reader = c_string_reader(0x9000, b"/proc/self/fd/03/../../maps");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.guest_opens().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_rejects_writable_virtual_proc_self_maps() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0x20000);
    let guest_memory_reader = c_string_reader(0x9000, b"/proc/self/maps");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EACCES_FOR_OPEN_TEST)
        })
    );
    assert!(state.guest_opens().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_append_writes_at_guest_file_end() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let path = b"guest.txt\0".to_vec();
    let payload = b"append:new\n".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_RDWR_FOR_TEST | RISCV_LINUX_O_APPEND_FOR_TEST,
                    0,
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
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_LSEEK, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WRITE, [3, 0xa000, 11, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 11 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_LSEEK, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_READ, [3, 0xb000, 32, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 16 })
    );
    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0xb000, b"seed\nappend:new\n".to_vec())]
    );
    assert_eq!(state.guest_path_stat(b"guest.txt").unwrap().size(), 16);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_creat_applies_umask_to_regular_file_mode() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(0x9000, b"masked.txt");
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
                RISCV_LINUX_UMASK_FOR_OPEN_TEST,
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY_FOR_TEST | RISCV_LINUX_O_CREAT_FOR_TEST,
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
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_NEWFSTATAT,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0xa000, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_FSTAT, [3, 0xa100, 0, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_ACCESS,
                [0x9000, RISCV_LINUX_X_OK_FOR_OPEN_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EACCES_FOR_OPEN_TEST)
        })
    );

    let writes = writes.lock().unwrap();
    let path_stat = collect_guest_writes(&writes_in_range(&writes, 0xa000, 128), 0xa000, 128);
    let fd_stat = collect_guest_writes(&writes_in_range(&writes, 0xa100, 128), 0xa100, 128);
    assert_eq!(read_le_u32(&path_stat, 16), 0o100640);
    assert_eq!(read_le_u32(&fd_stat, 16), 0o100640);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_open_accepts_newlib_create_truncate_flags() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"created.txt\0".to_vec();
    let payload = b"alpha:17\n".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= 0x9000 {
            return path
                .get((address - 0x9000) as usize)
                .copied()
                .map(|byte| vec![byte]);
        }
        if address >= 0xa000 {
            let start = usize::try_from(address - 0xa000).ok()?;
            let end = start.checked_add(bytes)?;
            return payload.get(start..end).map(Vec::from);
        }
        None
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
                [
                    0x9000,
                    RISCV_LINUX_O_RDWR_FOR_TEST
                        | RISCV_NEWLIB_O_CREAT_FOR_TEST
                        | RISCV_NEWLIB_O_TRUNC_FOR_TEST,
                    0o666,
                    0,
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
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WRITE, [3, 0xa000, 9, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_LSEEK, [3, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_READ, [3, 0xb000, 16, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );

    assert_eq!(
        &*writes.lock().unwrap(),
        &[(0xb000, b"alpha:17\n".to_vec())]
    );
    assert_eq!(state.guest_path_stat(b"created.txt").unwrap().size(), 9);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_open_accepts_newlib_directory_flag() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let guest_memory_reader = c_string_reader(0x9000, b".");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPEN,
                [
                    0x9000,
                    RISCV_NEWLIB_O_DIRECTORY_FOR_TEST | RISCV_NEWLIB_O_CLOEXEC_FOR_TEST,
                    0,
                    0,
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

    let fd = GuestFd::new(3).unwrap();
    assert!(state.guest_fds().entry(fd).is_some());
    assert!(state.guest_fds().close_on_exec(fd).unwrap());
    assert_eq!(
        state.guest_fds().status_flags(fd).unwrap(),
        GuestFileStatusFlags::new((RISCV_LINUX_O_RDONLY | RISCV_LINUX_O_DIRECTORY_FOR_TEST) as u32)
    );
    assert_eq!(state.guest_opens().len(), 1);
    let open = &state.guest_opens()[0];
    assert_eq!(open.fd(), fd);
    assert_eq!(
        open.flags(),
        RISCV_LINUX_O_DIRECTORY_FOR_TEST | RISCV_LINUX_O_CLOEXEC_FOR_TEST
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_open_accepts_newlib_noctty_nofollow_regular_file_flags() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPEN,
                [
                    0x9000,
                    RISCV_NEWLIB_O_NOCTTY_FOR_TEST
                        | RISCV_NEWLIB_O_NOFOLLOW_FOR_TEST
                        | RISCV_NEWLIB_O_CLOEXEC_FOR_TEST,
                    0,
                    0,
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

    let fd = GuestFd::new(3).unwrap();
    assert!(state.guest_fds().entry(fd).is_some());
    assert!(state.guest_fds().close_on_exec(fd).unwrap());
    assert_eq!(
        state.guest_fds().status_flags(fd).unwrap(),
        GuestFileStatusFlags::new(RISCV_LINUX_O_RDONLY as u32)
    );
    assert_eq!(state.guest_opens().len(), 1);
    let open = &state.guest_opens()[0];
    assert_eq!(open.fd(), fd);
    assert_eq!(
        open.flags(),
        RISCV_LINUX_O_NOCTTY_FOR_TEST
            | RISCV_LINUX_O_NOFOLLOW_FOR_TEST
            | RISCV_LINUX_O_CLOEXEC_FOR_TEST
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_open_accepts_newlib_sync_regular_file_flag() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPEN,
                [0x9000, RISCV_NEWLIB_O_SYNC_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );

    let fd = GuestFd::new(3).unwrap();
    let expected_status = RISCV_LINUX_O_RDONLY | RISCV_LINUX_O_DSYNC_FOR_TEST;
    assert_eq!(
        state.guest_fds().status_flags(fd).unwrap(),
        GuestFileStatusFlags::new(expected_status as u32)
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: expected_status
        })
    );
    assert_eq!(state.guest_opens().len(), 1);
    let open = &state.guest_opens()[0];
    assert_eq!(open.fd(), fd);
    assert_eq!(open.flags(), RISCV_LINUX_O_DSYNC_FOR_TEST);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_normalizes_raw_sync_bit_to_public_sync_flag() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let guest_memory_reader = c_string_reader(0x9000, b"guest.txt");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_SYNC_INTERNAL_FOR_TEST,
                    0,
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

    let fd = GuestFd::new(3).unwrap();
    assert_eq!(
        state.guest_fds().status_flags(fd).unwrap(),
        GuestFileStatusFlags::new(RISCV_LINUX_O_SYNC_FOR_TEST as u32)
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_SYNC_FOR_TEST
        })
    );
    assert_eq!(state.guest_opens().len(), 1);
    assert_eq!(state.guest_opens()[0].flags(), RISCV_LINUX_O_SYNC_FOR_TEST);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_open_rejects_newlib_nofollow_registered_symlink() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let guest_memory_reader = c_string_reader(0x9000, b"/proc/self/exe");

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPEN,
                [0x9000, RISCV_NEWLIB_O_NOFOLLOW_FOR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ELOOP_FOR_OPEN_TEST)
        })
    );
    assert!(state.guest_opens().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_open_enforces_newlib_exclusive_create() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"created.txt", b"seed\n");
    let guest_memory_reader = c_string_reader_entries(&[(0x9000, b"created.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPEN,
                [
                    0x9000,
                    RISCV_LINUX_O_RDWR_FOR_TEST
                        | RISCV_NEWLIB_O_CREAT_FOR_TEST
                        | RISCV_NEWLIB_O_EXCL_FOR_TEST,
                    0o666,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST_FOR_OPEN_TEST)
        })
    );

    assert_eq!(state.guest_path_stat(b"created.txt").unwrap().size(), 5);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_legacy_open_rejects_unimplemented_newlib_direct_flag() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"seed\n");
    let guest_memory_reader = c_string_reader_entries(&[(0x9000, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPEN,
                [0x9000, RISCV_NEWLIB_O_DIRECT_FOR_TEST, 0o666, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL_FOR_OPEN_TEST)
        })
    );

    assert!(state.guest_opens().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_openat_creat_mode_survives_link_and_rename() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader_entries(&[
        (0x9000, b"created.txt"),
        (0x9010, b"alias.txt"),
        (0x9020, b"renamed.txt"),
    ]);
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
                RISCV_LINUX_UMASK_FOR_OPEN_TEST,
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
                RISCV_LINUX_OPENAT,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_WRONLY_FOR_TEST | RISCV_LINUX_O_CREAT_FOR_TEST,
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
                RISCV_LINUX_LINK_FOR_OPEN_TEST,
                [0x9000, 0x9010, 0, 0, 0, 0],
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
                RISCV_LINUX_RENAMEAT2_FOR_OPEN_TEST,
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
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_NEWFSTATAT,
                [RISCV_LINUX_AT_FDCWD, 0x9010, 0xa000, 0, 0, 0],
            ),
            &mut state,
            10,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_NEWFSTATAT,
                [RISCV_LINUX_AT_FDCWD, 0x9020, 0xa100, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let alias_stat = collect_guest_writes(&writes_in_range(&writes, 0xa000, 128), 0xa000, 128);
    let renamed_stat = collect_guest_writes(&writes_in_range(&writes, 0xa100, 128), 0xa100, 128);
    assert_eq!(read_le_u64(&alias_stat, 8), read_le_u64(&renamed_stat, 8));
    assert_eq!(read_le_u32(&alias_stat, 16), 0o100640);
    assert_eq!(read_le_u32(&renamed_stat, 16), 0o100640);
    assert_eq!(read_le_u32(&alias_stat, 20), 2);
    assert_eq!(read_le_u32(&renamed_stat, 20), 2);
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

fn c_string_reader_entries(entries: &[(u64, &'static [u8])]) -> RiscvGuestMemoryReader {
    let entries = entries
        .iter()
        .map(|(base, bytes)| (*base, [*bytes, b"\0"].concat()))
        .collect::<Vec<_>>();
    RiscvGuestMemoryReader::new(move |address, count| {
        if count != 1 {
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

fn writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<(u64, Vec<u8>)> {
    let end = base + len as u64;
    writes
        .iter()
        .filter(|(address, _bytes)| (base..end).contains(address))
        .cloned()
        .collect()
}
