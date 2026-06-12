use std::sync::{Arc, Mutex};

use super::*;

const RISCV_LINUX_GETDENTS64_FOR_TEST: u64 = 61;
const RISCV_LINUX_OPENAT_FOR_DIRENT_TEST: u64 = 56;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;
const RISCV_LINUX_SEEK_SET_FOR_TEST: u64 = 0;

#[test]
fn linux_table_getdents64_lists_registered_guest_file_directory_entries() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b".")]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writes_for_writer = Arc::clone(&writes);
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
                RISCV_LINUX_OPENAT_FOR_DIRENT_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_TEST | RISCV_LINUX_O_CLOEXEC,
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
    let first = table.handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8004,
            RISCV_LINUX_GETDENTS64_FOR_TEST,
            [3, 0xa000, 256, 0, 0, 0],
        ),
        &mut state,
        8,
        None,
        Some(&guest_memory_writer),
    );
    let Some(RiscvSyscallOutcome::Return { value: first_bytes }) = first else {
        panic!("expected getdents64 return");
    };
    assert!(first_bytes > 0);
    let writes = writes.lock().unwrap();
    let dirent_bytes = collect_writes_in_range(&writes, 0xa000, first_bytes as usize);
    let entries = linux_dirent64_names(&dirent_bytes);
    assert_eq!(entries, vec![".", "..", "guest.txt"]);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETDENTS64_FOR_TEST,
                [3, 0xa100, 256, 0, 0, 0],
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
}

#[test]
fn linux_table_getdents64_uses_seekable_directory_cookies() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b".")]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writes_for_writer = Arc::clone(&writes);
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
                RISCV_LINUX_OPENAT_FOR_DIRENT_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_TEST | RISCV_LINUX_O_CLOEXEC,
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
    let Some(RiscvSyscallOutcome::Return { value: first_bytes }) = table
        .handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETDENTS64_FOR_TEST,
                [3, 0xa000, 24, 0, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        )
    else {
        panic!("expected getdents64 return");
    };
    let writes_snapshot = writes.lock().unwrap().clone();
    let first_entries = linux_dirent64_entries(&collect_writes_in_range(
        &writes_snapshot,
        0xa000,
        first_bytes as usize,
    ));
    assert_eq!(
        first_entries,
        vec![LinuxDirent64Entry {
            name: ".".to_string(),
            next_offset: 24,
        }]
    );

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_LSEEK,
                [3, 0, RISCV_LINUX_SEEK_SET_FOR_TEST, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    let Some(RiscvSyscallOutcome::Return {
        value: rewound_bytes,
    }) = table.handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x800c,
            RISCV_LINUX_GETDENTS64_FOR_TEST,
            [3, 0xa100, 256, 0, 0, 0],
        ),
        &mut state,
        9,
        None,
        Some(&guest_memory_writer),
    )
    else {
        panic!("expected getdents64 return after rewind");
    };
    let writes_snapshot = writes.lock().unwrap().clone();
    assert_eq!(
        linux_dirent64_names(&collect_writes_in_range(
            &writes_snapshot,
            0xa100,
            rewound_bytes as usize
        )),
        vec![".", "..", "guest.txt"]
    );

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_LSEEK,
                [
                    3,
                    first_entries[0].next_offset,
                    RISCV_LINUX_SEEK_SET_FOR_TEST,
                    0,
                    0,
                    0,
                ],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: first_entries[0].next_offset
        })
    );
    let Some(RiscvSyscallOutcome::Return { value: tail_bytes }) = table
        .handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_GETDENTS64_FOR_TEST,
                [3, 0xa200, 256, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        )
    else {
        panic!("expected getdents64 return after cookie seek");
    };
    let writes_snapshot = writes.lock().unwrap().clone();
    assert_eq!(
        linux_dirent64_names(&collect_writes_in_range(
            &writes_snapshot,
            0xa200,
            tail_bytes as usize
        )),
        vec!["..", "guest.txt"]
    );
}

#[test]
fn linux_table_directory_enumeration_treats_root_and_dot_as_same_root_view() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"relative file\n");
    state.register_guest_file(b"/abs.txt", b"absolute file\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"/"), (0x9010, b".")]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writes_for_writer = Arc::clone(&writes);
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
                RISCV_LINUX_OPENAT_FOR_DIRENT_TEST,
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
                RISCV_LINUX_OPENAT_FOR_DIRENT_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9010,
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
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );

    let root_bytes = getdents64_bytes(&table, &mut state, &guest_memory_writer, 3, 0xa000);
    let dot_bytes = getdents64_bytes(&table, &mut state, &guest_memory_writer, 4, 0xa100);
    let writes_snapshot = writes.lock().unwrap().clone();
    let root_names = linux_dirent64_names(&collect_writes_in_range(
        &writes_snapshot,
        0xa000,
        root_bytes as usize,
    ));
    let dot_names = linux_dirent64_names(&collect_writes_in_range(
        &writes_snapshot,
        0xa100,
        dot_bytes as usize,
    ));

    assert_eq!(root_names, vec![".", "..", "abs.txt", "guest.txt"]);
    assert_eq!(dot_names, root_names);
}

#[test]
fn linux_table_stats_and_accesses_registered_guest_directories() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"sub/guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"sub")]);
    let writes = Arc::new(Mutex::new(Vec::new()));
    let writes_for_writer = Arc::clone(&writes);
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
    let writes_snapshot = writes.lock().unwrap().clone();
    let stat = collect_guest_writes(&writes_snapshot, 0xa000, 128);
    assert_eq!(read_le_u32(&stat, 16), 0o040555);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_ACCESS, [0x9000, 5, 0, 0, 0, 0]),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_ACCESS, [0x9000, 2, 0, 0, 0, 0]),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(13)
        })
    );
}

#[test]
fn linux_table_fcntl_preserves_directory_open_flags_when_setting_status_flags() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b".")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_DIRENT_TEST,
                [
                    RISCV_LINUX_AT_FDCWD,
                    0x9000,
                    RISCV_LINUX_O_DIRECTORY_FOR_TEST | RISCV_LINUX_O_NONBLOCK,
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
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_DIRECTORY_FOR_TEST | RISCV_LINUX_O_NONBLOCK
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_SETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_FCNTL,
                [3, RISCV_LINUX_F_GETFL, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: RISCV_LINUX_O_DIRECTORY_FOR_TEST
        })
    );
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct LinuxDirent64Entry {
    name: String,
    next_offset: u64,
}

fn getdents64_bytes(
    table: &RiscvSyscallTable,
    state: &mut RiscvSyscallState,
    guest_memory_writer: &RiscvGuestMemoryWriter,
    fd: u64,
    address: u64,
) -> u64 {
    let Some(RiscvSyscallOutcome::Return { value }) = table.handle_with_guest_memory_io_at_tick(
        RiscvSyscallRequest::new(
            0x8100 + address,
            RISCV_LINUX_GETDENTS64_FOR_TEST,
            [fd, address, 256, 0, 0, 0],
        ),
        state,
        12,
        None,
        Some(guest_memory_writer),
    ) else {
        panic!("expected getdents64 return");
    };
    value
}

fn linux_dirent64_names(bytes: &[u8]) -> Vec<String> {
    linux_dirent64_entries(bytes)
        .into_iter()
        .map(|entry| entry.name)
        .collect()
}

fn collect_writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: usize) -> Vec<u8> {
    let mut bytes = vec![0; len];
    let end = base.checked_add(len as u64).unwrap();
    for (address, chunk) in writes {
        let chunk_end = address.checked_add(chunk.len() as u64).unwrap();
        if *address < base || chunk_end > end {
            continue;
        }
        let offset = usize::try_from(address - base).unwrap();
        bytes[offset..offset + chunk.len()].copy_from_slice(chunk);
    }
    bytes
}

fn linux_dirent64_entries(bytes: &[u8]) -> Vec<LinuxDirent64Entry> {
    let mut names = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let record_len = read_le_u16(bytes, offset + 16) as usize;
        assert!(record_len >= 24);
        assert!(offset + record_len <= bytes.len());
        let name_start = offset + 19;
        let name_end = bytes[name_start..offset + record_len]
            .iter()
            .position(|byte| *byte == 0)
            .map(|position| name_start + position)
            .unwrap();
        names.push(LinuxDirent64Entry {
            name: String::from_utf8(bytes[name_start..name_end].to_vec()).unwrap(),
            next_offset: read_le_u64(bytes, offset + 8),
        });
        offset += record_len;
    }
    names
}

fn read_le_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes(bytes[offset..offset + 2].try_into().unwrap())
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
