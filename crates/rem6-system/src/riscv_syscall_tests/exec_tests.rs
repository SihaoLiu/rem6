use super::super::*;

const RISCV_LINUX_EXECVE_FOR_TEST: u64 = 221;
const RISCV_LINUX_EXECVEAT_FOR_TEST: u64 = 281;
const RISCV_LINUX_AT_FDCWD_FOR_TEST: u64 = (-100_i64) as u64;

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
fn linux_table_execveat_missing_registered_path_returns_enoent_without_unknown_record() {
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
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_EXECVEAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0],
            ),
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
fn linux_table_execveat_path_fault_returns_efault_without_unknown_record() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let guest_memory_reader = RiscvGuestMemoryReader::new(|_address, _bytes| None);

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_EXECVEAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0],
            ),
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
fn linux_table_execveat_existing_path_remains_unsupported_and_recorded() {
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
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_EXECVEAT_FOR_TEST,
                [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0],
            ),
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
            RISCV_LINUX_EXECVEAT_FOR_TEST,
            [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0, 0],
            5
        )]
    );
}

#[test]
fn linux_table_execveat_nonzero_flags_remains_unsupported_and_recorded() {
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
    let args = [RISCV_LINUX_AT_FDCWD_FOR_TEST, 0x9000, 0, 0, 0x1000, 0];

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXECVEAT_FOR_TEST, args),
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
            RISCV_LINUX_EXECVEAT_FOR_TEST,
            args,
            5
        )]
    );
}

#[test]
fn linux_table_execveat_fd_relative_path_remains_unsupported_and_recorded() {
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
    let args = [3, 0x9000, 0, 0, 0, 0];

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXECVEAT_FOR_TEST, args),
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
            RISCV_LINUX_EXECVEAT_FOR_TEST,
            args,
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
