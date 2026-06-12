use super::*;

const RISCV_LINUX_LINK_FOR_TEST: u64 = 1025;
const RISCV_LINUX_EEXIST_FOR_TEST: u64 = 17;

#[test]
fn linux_table_link_adds_registered_guest_file_alias() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"alias.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LINK_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let stat = state.guest_path_stat(b"alias.txt").unwrap();
    assert_eq!(stat.size(), 18);
    assert_eq!(
        state.guest_file_contents(b"alias.txt"),
        Some(&b"file-backed input\n"[..])
    );
}

#[test]
fn linux_table_link_preserves_shared_inode_and_link_count_in_stat() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"alias.txt")]);
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
                RISCV_LINUX_LINK_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_STAT, [0x9000, 0x9200, 0, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_STAT, [0x9100, 0x9300, 0, 0, 0, 0]),
            &mut state,
            9,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let original_writes = writes_in_range(&writes, 0x9200, 128);
    let alias_writes = writes_in_range(&writes, 0x9300, 128);
    let original_stat = collect_guest_writes(&original_writes, 0x9200, 128);
    let alias_stat = collect_guest_writes(&alias_writes, 0x9300, 128);
    assert_eq!(read_le_u64(&original_stat, 8), read_le_u64(&alias_stat, 8));
    assert_eq!(read_le_u32(&original_stat, 20), 2);
    assert_eq!(read_le_u32(&alias_stat, 20), 2);
}

#[test]
fn linux_table_link_and_unlink_update_open_fd_link_count() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"alias.txt")]);
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
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_LINK_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_FSTAT, [3, 0x9200, 0, 0, 0, 0]),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_UNLINK, [0x9000, 0, 0, 0, 0, 0]),
            &mut state,
            10,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_FSTAT, [3, 0x9300, 0, 0, 0, 0]),
            &mut state,
            11,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_UNLINK, [0x9100, 0, 0, 0, 0, 0]),
            &mut state,
            12,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8018, RISCV_LINUX_FSTAT, [3, 0x9400, 0, 0, 0, 0]),
            &mut state,
            13,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    let linked_stat = collect_guest_writes(&writes_in_range(&writes, 0x9200, 128), 0x9200, 128);
    let unlinked_source_stat =
        collect_guest_writes(&writes_in_range(&writes, 0x9300, 128), 0x9300, 128);
    let unlinked_all_stat =
        collect_guest_writes(&writes_in_range(&writes, 0x9400, 128), 0x9400, 128);
    assert_eq!(read_le_u32(&linked_stat, 20), 2);
    assert_eq!(read_le_u32(&unlinked_source_stat, 20), 1);
    assert_eq!(read_le_u32(&unlinked_all_stat, 20), 0);
}

#[test]
fn linux_table_link_adds_registered_symlink_alias() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_symlink(b"/proc/self/exe", b"/bin/rem6");
    let guest_memory_reader =
        c_string_reader(&[(0x9000, b"/proc/self/exe"), (0x9100, b"/tmp/exe-alias")]);
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
                RISCV_LINUX_LINK_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
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
                RISCV_LINUX_READLINKAT,
                [RISCV_LINUX_AT_FDCWD, 0x9100, 0x9200, 32, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 9 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(collect_guest_writes(&writes, 0x9200, 9), b"/bin/rem6");
}

#[test]
fn linux_table_link_rejects_missing_source_and_existing_destination() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    state.register_guest_file(b"alias.txt", b"existing\n");
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"missing.txt"),
        (0x9100, b"guest.txt"),
        (0x9200, b"alias.txt"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LINK_FOR_TEST,
                [0x9000, 0x9200, 0, 0, 0, 0],
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
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_LINK_FOR_TEST,
                [0x9100, 0x9200, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST_FOR_TEST)
        })
    );
}

#[test]
fn linux_table_link_rejects_existing_registered_symlink_destination() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    state.register_guest_symlink(b"alias.txt", b"guest.txt");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt"), (0x9100, b"alias.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_LINK_FOR_TEST,
                [0x9000, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EEXIST_FOR_TEST)
        })
    );
    assert_eq!(
        state.guest_link_target(b"alias.txt"),
        Some(&b"guest.txt"[..])
    );
}

fn c_string_reader(entries: &[(u64, &'static [u8])]) -> RiscvGuestMemoryReader {
    let entries = entries
        .iter()
        .map(|(base, bytes)| (*base, bytes.to_vec()))
        .collect::<Vec<_>>();
    RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes != 1 {
            return None;
        }
        entries.iter().find_map(|(base, data)| {
            if address < *base {
                return None;
            }
            let offset = usize::try_from(address - *base).ok()?;
            if offset == data.len() {
                return Some(vec![0]);
            }
            data.get(offset).copied().map(|byte| vec![byte])
        })
    })
}

fn writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: u64) -> Vec<(u64, Vec<u8>)> {
    writes
        .iter()
        .filter(|(address, _bytes)| *address >= base && *address < base + len)
        .cloned()
        .collect()
}
