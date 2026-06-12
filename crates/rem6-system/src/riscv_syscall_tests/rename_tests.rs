use super::*;

const RISCV_LINUX_LINK_FOR_RENAME_TEST: u64 = 1025;
const RISCV_LINUX_RENAMEAT2_FOR_TEST: u64 = 276;

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
