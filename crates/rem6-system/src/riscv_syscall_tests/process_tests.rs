use super::super::*;
use super::{collect_guest_writes, read_le_u32};
use crate::{GuestChildStatus, GuestProcessGroupId, GuestProcessId, GuestWaitStatus};

const RISCV_LINUX_GETRESUID_FOR_TEST: u64 = 148;
const RISCV_LINUX_SETRESUID_FOR_TEST: u64 = 147;
const RISCV_LINUX_SETUID_FOR_TEST: u64 = 146;
const RISCV_LINUX_SETGID_FOR_TEST: u64 = 144;
const RISCV_LINUX_SETREGID_FOR_TEST: u64 = 143;
const RISCV_LINUX_SETREUID_FOR_TEST: u64 = 145;
const RISCV_LINUX_SETRESGID_FOR_TEST: u64 = 149;
const RISCV_LINUX_GETRESGID_FOR_TEST: u64 = 150;
const RISCV_LINUX_GETGROUPS_FOR_TEST: u64 = 158;
const RISCV_LINUX_SETGROUPS_FOR_TEST: u64 = 159;
const RISCV_LINUX_PRCTL_FOR_TEST: u64 = 167;
const RISCV_LINUX_PR_SET_NO_NEW_PRIVS_FOR_TEST: u64 = 38;
const RISCV_LINUX_PR_GET_NO_NEW_PRIVS_FOR_TEST: u64 = 39;
const RISCV_LINUX_EXECVE_FOR_TEST: u64 = 221;

fn child(pid: u32, process_group: u32, status: GuestWaitStatus) -> GuestChildStatus {
    GuestChildStatus::new(
        GuestProcessId::new(pid).unwrap(),
        GuestProcessGroupId::new(process_group).unwrap(),
        status,
    )
}

#[test]
fn linux_table_execve_missing_registered_path_returns_enoent_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let path = b"/missing\0".to_vec();
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXECVE_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            5,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOENT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_execve_path_fault_returns_efault_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXECVE_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            5,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_execve_existing_path_remains_unsupported_and_recorded() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/bin/app", b"elf");
    let path = b"/bin/app\0".to_vec();
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXECVE_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            5,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8000,
            RISCV_LINUX_EXECVE_FOR_TEST,
            [0x9000, 0, 0, 0, 0, 0],
            5
        )]
    );
}

#[test]
fn linux_table_execve_relative_existing_path_remains_unsupported_and_recorded() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    state.register_guest_file(b"/bin/app", b"elf");
    let path = b"bin/app\0".to_vec();
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXECVE_FOR_TEST, [0x9000, 0, 0, 0, 0, 0],),
            &mut state,
            5,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8000,
            RISCV_LINUX_EXECVE_FOR_TEST,
            [0x9000, 0, 0, 0, 0, 0],
            5
        )]
    );
}

#[test]
fn linux_table_returns_process_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (number, value) in [
        (RISCV_LINUX_GETPID, 41),
        (RISCV_LINUX_GETTID, 42),
        (RISCV_LINUX_GETPPID, 43),
        (RISCV_LINUX_GETUID, 7),
        (RISCV_LINUX_GETEUID, 8),
        (RISCV_LINUX_GETGID, 9),
        (RISCV_LINUX_GETEGID, 10),
    ] {
        assert_eq!(
            table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
            Some(RiscvSyscallOutcome::Return { value })
        );
    }
}

#[test]
fn linux_table_writes_resuid_and_resgid_identity_triples() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            5,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 7);
    assert_eq!(read_le_u32(&bytes, 4), 8);
    assert_eq!(read_le_u32(&bytes, 8), 8);
    assert_eq!(read_le_u32(&bytes, 16), 9);
    assert_eq!(read_le_u32(&bytes, 20), 10);
    assert_eq!(read_le_u32(&bytes, 24), 10);
}

#[test]
fn linux_table_resuid_returns_efault_when_guest_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|address, _bytes| address != 0x9004);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            5,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setresuid_and_setresgid_update_identity_triples() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
                RISCV_LINUX_SETRESUID_FOR_TEST,
                [8, 7, u64::MAX, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETRESGID_FOR_TEST,
                [10, 9, u64::MAX, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 8);
    assert_eq!(read_le_u32(&bytes, 4), 7);
    assert_eq!(read_le_u32(&bytes, 8), 8);
    assert_eq!(read_le_u32(&bytes, 16), 10);
    assert_eq!(read_le_u32(&bytes, 20), 9);
    assert_eq!(read_le_u32(&bytes, 24), 10);
}

#[test]
fn linux_table_effective_root_setresuid_and_setresgid_update_identity_triples() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 0, 9, 10));
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
                RISCV_LINUX_SETRESGID_FOR_TEST,
                [14, 15, 16, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETRESUID_FOR_TEST,
                [11, 12, 13, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 11);
    assert_eq!(read_le_u32(&bytes, 4), 12);
    assert_eq!(read_le_u32(&bytes, 8), 13);
    assert_eq!(read_le_u32(&bytes, 16), 14);
    assert_eq!(read_le_u32(&bytes, 20), 15);
    assert_eq!(read_le_u32(&bytes, 24), 16);
}

#[test]
fn linux_table_setresuid_rejects_unprivileged_identity_change() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETRESUID_FOR_TEST,
                [11, u64::MAX, u64::MAX, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETUID, [0; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 7 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setreuid_and_setregid_update_effective_identity_when_allowed() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
                RISCV_LINUX_SETREUID_FOR_TEST,
                [u64::MAX, 7, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETREGID_FOR_TEST,
                [u64::MAX, 9, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 7);
    assert_eq!(read_le_u32(&bytes, 4), 7);
    assert_eq!(read_le_u32(&bytes, 8), 8);
    assert_eq!(read_le_u32(&bytes, 16), 9);
    assert_eq!(read_le_u32(&bytes, 20), 9);
    assert_eq!(read_le_u32(&bytes, 24), 10);
}

#[test]
fn linux_table_setreuid_and_setregid_reject_unprivileged_new_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
                RISCV_LINUX_SETREUID_FOR_TEST,
                [11, u64::MAX, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETREGID_FOR_TEST,
                [12, u64::MAX, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 7);
    assert_eq!(read_le_u32(&bytes, 4), 8);
    assert_eq!(read_le_u32(&bytes, 8), 8);
    assert_eq!(read_le_u32(&bytes, 16), 9);
    assert_eq!(read_le_u32(&bytes, 20), 10);
    assert_eq!(read_le_u32(&bytes, 24), 10);
}

#[test]
fn linux_table_setreuid_and_setregid_reject_saved_id_as_real_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETRESUID_FOR_TEST, [7, 7, 8, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETRESGID_FOR_TEST, [9, 9, 10, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SETREUID_FOR_TEST,
                [8, u64::MAX, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_SETREGID_FOR_TEST,
                [10, u64::MAX, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8014,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 7);
    assert_eq!(read_le_u32(&bytes, 4), 7);
    assert_eq!(read_le_u32(&bytes, 8), 8);
    assert_eq!(read_le_u32(&bytes, 16), 9);
    assert_eq!(read_le_u32(&bytes, 20), 9);
    assert_eq!(read_le_u32(&bytes, 24), 10);
}

#[test]
fn linux_table_effective_root_setreuid_and_setregid_update_identity_triples() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 0, 9, 10));
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETREGID_FOR_TEST, [14, 15, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETREUID_FOR_TEST, [11, 12, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 11);
    assert_eq!(read_le_u32(&bytes, 4), 12);
    assert_eq!(read_le_u32(&bytes, 8), 12);
    assert_eq!(read_le_u32(&bytes, 16), 14);
    assert_eq!(read_le_u32(&bytes, 20), 15);
    assert_eq!(read_le_u32(&bytes, 24), 15);
}

#[test]
fn linux_table_setuid_and_setgid_update_effective_identity_when_allowed() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETUID_FOR_TEST, [7, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETGID_FOR_TEST, [9, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 7);
    assert_eq!(read_le_u32(&bytes, 4), 7);
    assert_eq!(read_le_u32(&bytes, 8), 8);
    assert_eq!(read_le_u32(&bytes, 16), 9);
    assert_eq!(read_le_u32(&bytes, 20), 9);
    assert_eq!(read_le_u32(&bytes, 24), 10);
}

#[test]
fn linux_table_setuid_and_setgid_can_switch_to_saved_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SETRESUID_FOR_TEST,
                [u64::MAX, 7, u64::MAX, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SETRESGID_FOR_TEST,
                [u64::MAX, 9, u64::MAX, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_SETUID_FOR_TEST, [8, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_SETGID_FOR_TEST, [10, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8010, RISCV_LINUX_GETEUID, [0, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 8 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8014, RISCV_LINUX_GETEGID, [0, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 10 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_effective_root_setuid_and_setgid_update_identity_triples() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 0, 9, 10));
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETGID_FOR_TEST, [12, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETUID_FOR_TEST, [11, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 11);
    assert_eq!(read_le_u32(&bytes, 4), 11);
    assert_eq!(read_le_u32(&bytes, 8), 11);
    assert_eq!(read_le_u32(&bytes, 16), 12);
    assert_eq!(read_le_u32(&bytes, 20), 12);
    assert_eq!(read_le_u32(&bytes, 24), 12);
}

#[test]
fn linux_table_setuid_and_setgid_reject_unprivileged_new_identity() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETUID_FOR_TEST, [11, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_SETGID_FOR_TEST, [12, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_GETRESUID_FOR_TEST,
                [0x9000, 0x9004, 0x9008, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_GETRESGID_FOR_TEST,
                [0x9010, 0x9014, 0x9018, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());

    let bytes = collect_guest_writes(&writes.lock().unwrap(), 0x9000, 28);
    assert_eq!(read_le_u32(&bytes, 0), 7);
    assert_eq!(read_le_u32(&bytes, 4), 8);
    assert_eq!(read_le_u32(&bytes, 8), 8);
    assert_eq!(read_le_u32(&bytes, 16), 9);
    assert_eq!(read_le_u32(&bytes, 20), 10);
    assert_eq!(read_le_u32(&bytes, 24), 10);
}

#[test]
fn linux_table_getgroups_reports_empty_supplementary_group_list() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
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
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETGROUPS_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
            5,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETGROUPS_FOR_TEST,
                [2, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            6,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(writes.lock().unwrap().is_empty());
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getgroups_rejects_negative_group_count() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let guest_memory_writer = RiscvGuestMemoryWriter::new(|_address, _bytes| true);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_GETGROUPS_FOR_TEST,
                [u64::MAX, 0, 0, 0, 0, 0],
            ),
            &mut state,
            5,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setgroups_rejects_unprivileged_group_list_changes() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETGROUPS_FOR_TEST, [0, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_uses_gem5_default_process_identity() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for number in [
        RISCV_LINUX_GETPID,
        RISCV_LINUX_GETTID,
        RISCV_LINUX_GETUID,
        RISCV_LINUX_GETEUID,
        RISCV_LINUX_GETGID,
        RISCV_LINUX_GETEGID,
    ] {
        assert_eq!(
            table.handle(RiscvSyscallRequest::new(0x8000, number, [0; 6]), &mut state,),
            Some(RiscvSyscallOutcome::Return { value: 100 })
        );
    }
}

#[test]
fn linux_table_personality_queries_and_sets_process_persona() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PERSONALITY,
                [0xffff_ffff, 0, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PERSONALITY,
                [0x0004_0000, 0, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PERSONALITY,
                [0xffff_ffff, 0, 0, 0, 0, 0]
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0x0004_0000 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_getpgid_and_getsid_report_current_process_scope() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETPGID, [41, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_GETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x800c, RISCV_LINUX_GETSID, [41, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_process_group_queries_validate_pid_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_GETPGID, [99, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_GETSID,
                [0x0000_0000_ffff_ffff, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setpgid_rejects_current_session_leader_and_preserves_wait_group() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let original_group_child = child(51, 41, GuestWaitStatus::exited(1));
    let new_group_child = child(52, 77, GuestWaitStatus::exited(2));
    state.push_wait_child(original_group_child);
    state.push_wait_child(new_group_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETPGID, [0, 77, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 51 })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[new_group_child]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setpgid_can_create_group_for_current_nonleader() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::with_identity_process_group_and_session(
        0,
        RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10),
        GuestProcessGroupId::new(77).unwrap(),
        77,
    );
    let old_group_child = child(51, 77, GuestWaitStatus::exited(1));
    let new_group_child = child(52, 41, GuestWaitStatus::exited(2));
    state.push_wait_child(old_group_child);
    state.push_wait_child(new_group_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 52 })
    );
    assert_eq!(
        state.guest_wait_queue().snapshot().pending(),
        &[old_group_child]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setpgid_validates_target_and_group_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    for (pc, arguments, error) in [
        (0x8000, [99, 99, 0, 0, 0, 0], RISCV_LINUX_ESRCH),
        (
            0x8004,
            [0x0000_0000_ffff_ffff, 0, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x8008,
            [0, 0x0000_0000_ffff_ffff, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
        (
            0x800c,
            [99, 0x0000_0000_ffff_ffff, 0, 0, 0, 0],
            RISCV_LINUX_EINVAL,
        ),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_SETPGID, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(error)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setsid_rejects_current_process_group_leader() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_setsid_creates_session_for_current_nonleader() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::with_identity_process_group_and_session(
        0,
        RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10),
        GuestProcessGroupId::new(77).unwrap(),
        77,
    );

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_GETSID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_GETPGID, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 41 })
    );
    assert_eq!(
        state.guest_wait_queue().current_process_group(),
        GuestProcessGroupId::new(41).unwrap()
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_set_and_get_name_roundtrips_process_name() {
    const RISCV_LINUX_PRCTL: u64 = 167;
    const RISCV_LINUX_PR_SET_NAME: u64 = 15;
    const RISCV_LINUX_PR_GET_NAME: u64 = 16;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let name = b"rem6-worker-thread-name-is-truncated\0".to_vec();
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        let offset = usize::try_from(address.checked_sub(0x9000)?).ok()?;
        let end = offset.checked_add(bytes)?;
        (end <= name.len()).then(|| name[offset..end].to_vec())
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
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PRCTL,
                [RISCV_LINUX_PR_SET_NAME, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PRCTL,
                [RISCV_LINUX_PR_GET_NAME, 0xa000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let writes = writes.lock().unwrap();
    assert_eq!(
        writes.as_slice(),
        &[(0xa000, b"rem6-worker-thr\0".to_vec())]
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_set_name_clears_bytes_after_first_nul() {
    const RISCV_LINUX_PRCTL: u64 = 167;
    const RISCV_LINUX_PR_SET_NAME: u64 = 15;
    const RISCV_LINUX_PR_GET_NAME: u64 = 16;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        let source = b"ab\0junk-data-here";
        let offset = usize::try_from(address.checked_sub(0x9000)?).ok()?;
        let end = offset.checked_add(bytes)?;
        (end <= source.len()).then(|| source[offset..end].to_vec())
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
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PRCTL,
                [RISCV_LINUX_PR_SET_NAME, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PRCTL,
                [RISCV_LINUX_PR_GET_NAME, 0xa000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let mut expected = [0; 16];
    expected[0] = b'a';
    expected[1] = b'b';
    let writes = writes.lock().unwrap();
    assert_eq!(writes.as_slice(), &[(0xa000, expected.to_vec())]);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_set_name_reads_only_through_first_nul() {
    const RISCV_LINUX_PRCTL: u64 = 167;
    const RISCV_LINUX_PR_SET_NAME: u64 = 15;
    const RISCV_LINUX_PR_GET_NAME: u64 = 16;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        let offset = address.checked_sub(0x9000)? as usize;
        let source = b"xy\0";
        if bytes == 1 && offset < source.len() {
            Some(vec![source[offset]])
        } else {
            None
        }
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
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PRCTL,
                [RISCV_LINUX_PR_SET_NAME, 0x9000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PRCTL,
                [RISCV_LINUX_PR_GET_NAME, 0xa000, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let mut expected = [0; 16];
    expected[0] = b'x';
    expected[1] = b'y';
    let writes = writes.lock().unwrap();
    assert_eq!(writes.as_slice(), &[(0xa000, expected.to_vec())]);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_set_name_reports_efault_on_guest_address_overflow() {
    const RISCV_LINUX_PRCTL: u64 = 167;
    const RISCV_LINUX_PR_SET_NAME: u64 = 15;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if bytes == 1 && address >= u64::MAX - 1 {
            Some(vec![b'z'])
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PRCTL,
                [RISCV_LINUX_PR_SET_NAME, u64::MAX - 1, 0, 0, 0, 0],
            ),
            &mut state,
            0,
            Some(&guest_memory_reader),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_no_new_privs_queries_and_sets_state() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_NO_NEW_PRIVS_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_SET_NO_NEW_PRIVS_FOR_TEST, 1, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_NO_NEW_PRIVS_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_no_new_privs_rejects_invalid_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, arguments) in [
        (
            0x8000,
            [RISCV_LINUX_PR_SET_NO_NEW_PRIVS_FOR_TEST, 0, 0, 0, 0, 0],
        ),
        (
            0x8004,
            [RISCV_LINUX_PR_SET_NO_NEW_PRIVS_FOR_TEST, 1, 1, 0, 0, 0],
        ),
        (
            0x8008,
            [RISCV_LINUX_PR_GET_NO_NEW_PRIVS_FOR_TEST, 1, 0, 0, 0, 0],
        ),
    ] {
        assert_eq!(
            table.handle(
                RiscvSyscallRequest::new(pc, RISCV_LINUX_PRCTL_FOR_TEST, arguments),
                &mut state,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_PRCTL_FOR_TEST,
                [RISCV_LINUX_PR_GET_NO_NEW_PRIVS_FOR_TEST, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_prctl_rejects_unknown_option_without_unknown_syscall_record() {
    const RISCV_LINUX_PRCTL: u64 = 167;

    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_PRCTL, [999, 0, 0, 0, 0, 0],),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_wait4_after_rejected_setpgid_still_uses_current_process_group() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));
    let original_group_child = child(51, 41, GuestWaitStatus::exited(1));
    state.push_wait_child(original_group_child);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_SETPGID, [0, 77, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_WAIT4, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 51 })
    );
    assert!(state.guest_wait_queue().is_empty());
}
