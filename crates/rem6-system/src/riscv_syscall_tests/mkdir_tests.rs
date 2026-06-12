use super::*;

const RISCV_LINUX_MKDIRAT_FOR_TEST: u64 = 34;
const RISCV_LINUX_ACCESS_FOR_MKDIR_TEST: u64 = 1033;
const RISCV_LINUX_CHDIR_FOR_TEST: u64 = 49;
const RISCV_LINUX_OPENAT_FOR_MKDIR_TEST: u64 = 56;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;
const RISCV_LINUX_W_OK_FOR_MKDIR_TEST: u64 = 2;
const RISCV_LINUX_EEXIST_FOR_TEST: u64 = 17;

#[test]
fn linux_table_mkdirat_creates_empty_guest_directory_for_chdir_and_open() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[(0x9000, b"made")]);

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
                RISCV_LINUX_OPENAT_FOR_MKDIR_TEST,
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
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_CHDIR_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.current_directory(), b"/made");
}

#[test]
fn linux_table_mkdirat_uses_directory_fd_for_relative_child() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"parent"),
        (0x9010, b"child"),
        (0x9020, b"parent/child"),
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
                RISCV_LINUX_OPENAT_FOR_MKDIR_TEST,
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
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [3, 0x9010, 0o700, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.guest_path_stat(b"parent/child").is_some());
}

#[test]
fn linux_table_mkdirat_mode_reaches_stat_and_access() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = c_string_reader(&[(0x9000, b"private")]);
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
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o700, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
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
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_ACCESS_FOR_MKDIR_TEST,
                [0x9000, RISCV_LINUX_W_OK_FOR_MKDIR_TEST, 0, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let stat = collect_guest_writes(&writes.lock().unwrap(), 0xa000, 128);
    assert_eq!(read_le_u32(&stat, 16), 0o040700);
}

#[test]
fn linux_table_mkdirat_rejects_existing_guest_nodes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"made"), (0x9010, b"guest.txt")]);

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
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0o755, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST_FOR_TEST)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_MKDIRAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9010, 0o755, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST_FOR_TEST)
        })
    );
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
