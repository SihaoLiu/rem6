use std::sync::{Arc, Mutex};

use super::*;

const RISCV_LINUX_CHDIR_FOR_TEST: u64 = 49;
const RISCV_LINUX_FCHDIR_FOR_TEST: u64 = 50;
const RISCV_LINUX_OPENAT_FOR_CWD_TEST: u64 = 56;
const RISCV_LINUX_READ_FOR_CWD_TEST: u64 = 63;
const RISCV_LINUX_O_DIRECTORY_FOR_TEST: u64 = 0o200000;

#[test]
fn linux_table_chdir_updates_cwd_and_resolves_relative_guest_paths() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"sub/guest.txt", b"nested input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"sub"), (0x9010, b"guest.txt")]);
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CHDIR_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.current_directory(), b"/sub");
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETCWD, [0xa000, 16, 0, 0, 0, 0]),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_OPENAT_FOR_CWD_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_READ_FOR_CWD_TEST,
                [3, 0xa100, 32, 0, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 13 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa000, 5), 0xa000, 5),
        b"/sub\0"
    );
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa100, 13), 0xa100, 13),
        b"nested input\n"
    );
}

#[test]
fn linux_table_fchdir_uses_open_guest_directory_fd_as_cwd() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"sub/guest.txt", b"nested input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"sub"), (0x9010, b"guest.txt")]);
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
                RISCV_LINUX_OPENAT_FOR_CWD_TEST,
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
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_FCHDIR_FOR_TEST, [3, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(state.current_directory(), b"/sub");

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_OPENAT_FOR_CWD_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9010, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 4 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_READ_FOR_CWD_TEST,
                [4, 0xa000, 32, 0, 0, 0],
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 13 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        collect_guest_writes(&writes_in_range(&writes, 0xa000, 13), 0xa000, 13),
        b"nested input\n"
    );
}

#[test]
fn linux_table_chdir_rejects_missing_directory_without_cwd_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"sub/guest.txt", b"nested input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"missing")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CHDIR_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert_eq!(state.current_directory(), b"/");
}

#[test]
fn linux_table_chdir_rejects_registered_file_as_directory() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[(0x9000, b"guest.txt")]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_CHDIR_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            7,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTDIR)
        })
    );
    assert_eq!(state.current_directory(), b"/");
}

#[test]
fn linux_table_openat_rejects_dotdot_through_invalid_directory_components() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"guest.txt", b"file-backed input\n");
    let guest_memory_reader = c_string_reader(&[
        (0x9000, b"missing/../guest.txt"),
        (0x9100, b"guest.txt/../guest.txt"),
    ]);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_OPENAT_FOR_CWD_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9000, 0, 0, 0, 0],
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
                RISCV_LINUX_OPENAT_FOR_CWD_TEST,
                [RISCV_LINUX_AT_FDCWD, 0x9100, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOTDIR)
        })
    );
    assert!(state.guest_opens().is_empty());
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

fn writes_in_range(writes: &[(u64, Vec<u8>)], base: u64, len: u64) -> Vec<(u64, Vec<u8>)> {
    let end = base + len;
    writes
        .iter()
        .filter(|(address, bytes)| {
            let write_end = *address + bytes.len() as u64;
            *address >= base && write_end <= end
        })
        .cloned()
        .collect()
}
