use super::*;

const SIGKILL_MASK: u64 = 1 << (9 - 1);
const SIGSTOP_MASK: u64 = 1 << (19 - 1);
const SIGUSR1: u64 = 10;
const SIGUSR2: u64 = 12;
const SIGRTMAX: u64 = 64;
const EAGAIN: u64 = 11;
const RISCV_LINUX_SIGALTSTACK_FOR_TEST: u64 = 132;
const RISCV_LINUX_SIGINFO_T_BYTES: usize = 128;
const SS_DISABLE: u64 = 2;
const LINUX_STACK_T_BYTES: usize = 24;

#[test]
fn linux_table_kill_signal_zero_accepts_current_process() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_KILL, [100, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_kill_signal_zero_reports_missing_process() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_KILL, [101, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_kill_signal_zero_uses_linux_pid_t_arguments() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_KILL,
                [0x0000_0000_ffff_ffff, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_KILL, [0, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_KILL,
                [0x0000_0000_ffff_ff9c, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_kill_rejects_invalid_signal() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_KILL, [100, 65, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_kill_nonzero_signal_is_not_silently_delivered() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_KILL, [100, SIGUSR1, 0, 0, 0, 0]),
            &mut state,
            7,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8000,
            RISCV_LINUX_KILL,
            [100, SIGUSR1, 0, 0, 0, 0],
            7
        )]
    );
}

#[test]
fn linux_table_kill_uses_linux_int_signal_argument() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_KILL,
                [100, 0x0000_0001_0000_000a, 0, 0, 0, 0],
            ),
            &mut state,
            9,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_KILL,
                [100, 0x0000_0000_ffff_ffff, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_tkill_signal_zero_checks_current_thread() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_TKILL, [42, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_TKILL, [41, 0, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_TKILL, [42, 65, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_tkill_reports_missing_thread_before_invalid_signal() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_TKILL, [43, 65, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_tgkill_signal_zero_checks_current_thread_group() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_TGKILL, [41, 42, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_TGKILL, [40, 42, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8008, RISCV_LINUX_TGKILL, [41, 43, 0, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_tgkill_reports_missing_thread_before_invalid_signal() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_TGKILL, [40, 42, 65, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_TGKILL, [41, 43, 65, 0, 0, 0]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_thread_signal_nonzero_records_unimplemented_delivery() {
    let table = RiscvSyscallTable::new();
    let mut state =
        RiscvSyscallState::with_identity(0, RiscvSyscallIdentity::new(41, 42, 43, 7, 8, 9, 10));

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_TGKILL, [41, 42, SIGUSR1, 0, 0, 0],),
            &mut state,
            11,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8000,
            RISCV_LINUX_TGKILL,
            [41, 42, SIGUSR1, 0, 0, 0],
            11
        )]
    );
    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_TKILL, [42, SIGUSR2, 0, 0, 0, 0]),
            &mut state,
            12,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[
            RiscvUnknownSyscallRecord::new(
                0x8000,
                RISCV_LINUX_TGKILL,
                [41, 42, SIGUSR1, 0, 0, 0],
                11,
            ),
            RiscvUnknownSyscallRecord::new(
                0x8004,
                RISCV_LINUX_TKILL,
                [42, SIGUSR2, 0, 0, 0, 0],
                12,
            ),
        ]
    );
}

#[test]
fn linux_table_rt_sigprocmask_blocks_mask_and_writes_previous_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let set_address = 0x2000;
    let first_old_address = 0x3000;
    let second_old_address = 0x3010;
    let requested_mask = 0x12_u64;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == set_address && bytes == 8 {
            Some(requested_mask.to_le_bytes().to_vec())
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
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, set_address, first_old_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, 0, second_old_address, 8, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), first_old_address),
        0_u64.to_le_bytes()
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), second_old_address),
        requested_mask.to_le_bytes()
    );
}

#[test]
fn linux_table_rt_sigprocmask_read_fault_returns_efault_without_changing_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let set_address = 0x2000;
    let old_address = 0x3000;
    let initial_mask = 0x40_u64;
    let valid_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == set_address && bytes == 8 {
            Some(initial_mask.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGPROCMASK,
                [2, set_address, 0, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&valid_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    let faulting_reader = RiscvGuestMemoryReader::new(|_, _| None);
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_RT_SIGPROCMASK, [2, 0x2100, 0, 8, 0, 0],),
            &mut state,
            8,
            Some(&faulting_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

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
                0x8008,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, 0, old_address, 8, 0, 0],
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), old_address),
        initial_mask.to_le_bytes()
    );
}

#[test]
fn linux_table_rt_sigprocmask_write_fault_still_installs_new_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let set_address = 0x2200;
    let old_address = 0x3200;
    let query_address = 0x3210;
    let requested_mask = 0x80_u64;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == set_address && bytes == 8 {
            Some(requested_mask.to_le_bytes().to_vec())
        } else {
            None
        }
    });
    let faulting_writer = RiscvGuestMemoryWriter::new(|_, _| false);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGPROCMASK,
                [2, set_address, old_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

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
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, 0, query_address, 8, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), query_address),
        requested_mask.to_le_bytes()
    );
}

#[test]
fn linux_table_rt_sigprocmask_setmask_and_unblock_filter_unblockable_signals() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let first_set_address = 0x2300;
    let second_set_address = 0x2310;
    let first_query_address = 0x3300;
    let second_query_address = 0x3310;
    let blockable_mask = 0x12_u64;
    let requested_set_mask = blockable_mask | SIGKILL_MASK | SIGSTOP_MASK;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == first_set_address && bytes == 8 {
            Some(requested_set_mask.to_le_bytes().to_vec())
        } else if address == second_set_address && bytes == 8 {
            Some(0x10_u64.to_le_bytes().to_vec())
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
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGPROCMASK,
                [2, first_set_address, 0, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, 0, first_query_address, 8, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_RT_SIGPROCMASK,
                [1, second_set_address, 0, 8, 0, 0],
            ),
            &mut state,
            9,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, 0, second_query_address, 8, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), first_query_address),
        blockable_mask.to_le_bytes()
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), second_query_address),
        0x02_u64.to_le_bytes()
    );
}

#[test]
fn linux_table_rt_sigprocmask_rejects_bad_size_and_bad_how() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let set_address = 0x2400;
    let query_address = 0x3400;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == set_address && bytes == 8 {
            Some(0x22_u64.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGPROCMASK,
                [2, set_address, 0, 4, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPROCMASK,
                [99, set_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

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
                0x8008,
                RISCV_LINUX_RT_SIGPROCMASK,
                [0, 0, query_address, 8, 0, 0],
            ),
            &mut state,
            9,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), query_address),
        0_u64.to_le_bytes()
    );
}

#[test]
fn linux_table_rt_sigpending_writes_empty_guest_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let pending_address = 0x3f00;
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
                RISCV_LINUX_RT_SIGPENDING,
                [pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_mask_at(&writes.lock().unwrap(), pending_address),
        0_u64.to_le_bytes()
    );
}

#[test]
fn linux_table_rt_sigpending_allows_short_guest_sigset_size() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let pending_address = 0x3f40;
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
                RISCV_LINUX_RT_SIGPENDING,
                [pending_address, 4, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_signal_bytes_at(&writes.lock().unwrap(), pending_address),
        vec![0, 0, 0, 0]
    );
}

#[test]
fn linux_table_rt_sigpending_reports_oversized_sigset_and_write_fault_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let pending_address = 0x3f80;

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGPENDING,
                [pending_address, 9, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

    let faulting_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        assert_eq!(address, pending_address);
        assert_eq!(bytes, 0_u64.to_le_bytes());
        false
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGPENDING,
                [pending_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_rt_sigtimedwait_returns_eagain_without_pending_signal() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x3fc0;
    let timeout_address = 0x3fe0;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR1 - 1)).to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(0, 0))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(EAGAIN)
        })
    );
}

#[test]
fn linux_table_rt_sigtimedwait_without_timeout_wait_is_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4000;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR2 - 1)).to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, 0, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        None
    );
}

#[test]
fn linux_table_rt_sigtimedwait_with_positive_timeout_wait_is_unhandled() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4010;
    let timeout_address = 0x4018;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR2 - 1)).to_le_bytes().to_vec())
        } else if address == timeout_address && bytes == 16 {
            Some(timespec64_bytes(0, 1))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        None
    );
}

#[test]
fn linux_table_rt_sigtimedwait_reports_sigset_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4020;

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, 0, 4, 0, 0],
            ),
            &mut state,
            7,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

    let faulting_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        assert_eq!(address, signal_set_address);
        assert_eq!(bytes, 8);
        None
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&faulting_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_rt_sigtimedwait_reports_timeout_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4040;
    let timeout_address = 0x4060;
    let faulting_timeout_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some((1_u64 << (SIGUSR1 - 1)).to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGTIMEDWAIT,
                [signal_set_address, 0, timeout_address, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&faulting_timeout_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

    for (pc, seconds, nanoseconds) in [(0x8004, -1, 0), (0x8008, 0, -1), (0x800c, 0, 1_000_000_000)]
    {
        let invalid_timeout_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
            if address == signal_set_address && bytes == 8 {
                Some((1_u64 << (SIGUSR1 - 1)).to_le_bytes().to_vec())
            } else if address == timeout_address && bytes == 16 {
                Some(timespec64_bytes(seconds, nanoseconds))
            } else {
                None
            }
        });

        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_RT_SIGTIMEDWAIT,
                    [signal_set_address, 0, timeout_address, 8, 0, 0],
                ),
                &mut state,
                8,
                Some(&invalid_timeout_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
    }
}

#[test]
fn linux_table_rt_sigsuspend_reports_sigset_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4080;

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGSUSPEND,
                [signal_set_address, 4, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            None,
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );

    let faulting_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        assert_eq!(address, signal_set_address);
        assert_eq!(bytes, 8);
        None
    });
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGSUSPEND,
                [signal_set_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            8,
            Some(&faulting_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(state.signal_mask(), 0);
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigsuspend_blocks_with_blockable_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_set_address = 0x4090;
    let requested_mask = (1_u64 << (SIGUSR1 - 1)) | SIGKILL_MASK | SIGSTOP_MASK;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == signal_set_address && bytes == 8 {
            Some(requested_mask.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGSUSPEND,
                [signal_set_address, 8, 0, 0, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Blocked)
    );
    assert_eq!(state.signal_mask(), 1_u64 << (SIGUSR1 - 1));
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigqueueinfo_nonzero_signal_is_not_silently_delivered() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_info_address = 0x40a0;
    let guest_memory_reader = signal_info_reader(signal_info_address, -1);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGQUEUEINFO,
                [100, SIGUSR1, signal_info_address, 0, 0, 0],
            ),
            &mut state,
            13,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.unknown_syscalls(),
        &[RiscvUnknownSyscallRecord::new(
            0x8000,
            RISCV_LINUX_RT_SIGQUEUEINFO,
            [100, SIGUSR1, signal_info_address, 0, 0, 0],
            13
        )]
    );
}

#[test]
fn linux_table_rt_sigqueueinfo_reports_argument_errors() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_info_address = 0x40c0;
    let guest_memory_reader = signal_info_reader(signal_info_address, -1);
    let user_signal_info_address = 0x40d0;
    let user_guest_memory_reader = signal_info_reader(user_signal_info_address, 0);

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGQUEUEINFO,
                [0, 0, signal_info_address, 0, 0, 0],
            ),
            &mut state,
            14,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_RT_SIGQUEUEINFO,
                [100, SIGRTMAX + 1, signal_info_address, 0, 0, 0],
            ),
            &mut state,
            15,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_RT_SIGQUEUEINFO,
                [101, 0, signal_info_address, 0, 0, 0],
            ),
            &mut state,
            16,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ESRCH)
        })
    );
    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_RT_SIGQUEUEINFO,
                [101, 0, user_signal_info_address, 0, 0, 0],
            ),
            &mut state,
            17,
            Some(&user_guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EPERM)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigqueueinfo_rejects_kernel_siginfo_codes_for_current_process() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    for (pc, signal_info_address, si_code) in [(0x8000, 0x40f0, 0), (0x8004, 0x4170, -6)] {
        let guest_memory_reader = signal_info_reader(signal_info_address, si_code);
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    pc,
                    RISCV_LINUX_RT_SIGQUEUEINFO,
                    [100, 0, signal_info_address, 0, 0, 0],
                ),
                &mut state,
                18,
                Some(&guest_memory_reader),
                None,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EPERM)
            })
        );
    }
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_rt_sigqueueinfo_reports_guest_siginfo_faults() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let signal_info_address = 0x40e0;
    let faulting_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        assert_eq!(address, signal_info_address);
        assert_eq!(bytes, RISCV_LINUX_SIGINFO_T_BYTES);
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGQUEUEINFO,
                [100, 0, signal_info_address, 0, 0, 0],
            ),
            &mut state,
            17,
            Some(&faulting_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert!(state.unknown_syscalls().is_empty());
}

#[test]
fn linux_table_sigaltstack_queries_sets_and_disables_alt_stack() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let reader = stack_reader(vec![
        (0x9000, stack_t_bytes(0x9100, 0, 8192)),
        (0x9020, stack_t_bytes(0, SS_DISABLE, 0)),
    ]);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writer = recording_stack_writer(writes.clone());

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_SIGALTSTACK_FOR_TEST,
                [0, 0xa000, 0, 0, 0, 0],
            ),
            &mut state,
            11,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_stack_at(&writes.lock().unwrap(), 0xa000),
        (0, SS_DISABLE, 0)
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8004,
                RISCV_LINUX_SIGALTSTACK_FOR_TEST,
                [0x9000, 0xa020, 0, 0, 0, 0],
            ),
            &mut state,
            12,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_stack_at(&writes.lock().unwrap(), 0xa020),
        (0, SS_DISABLE, 0)
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_SIGALTSTACK_FOR_TEST,
                [0, 0xa040, 0, 0, 0, 0],
            ),
            &mut state,
            13,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_stack_at(&writes.lock().unwrap(), 0xa040),
        (0x9100, 0, 8192)
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x800c,
                RISCV_LINUX_SIGALTSTACK_FOR_TEST,
                [0x9020, 0xa060, 0, 0, 0, 0],
            ),
            &mut state,
            14,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_stack_at(&writes.lock().unwrap(), 0xa060),
        (0x9100, 0, 8192)
    );

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8010,
                RISCV_LINUX_SIGALTSTACK_FOR_TEST,
                [0, 0xa080, 0, 0, 0, 0],
            ),
            &mut state,
            15,
            Some(&reader),
            Some(&writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_stack_at(&writes.lock().unwrap(), 0xa080),
        (0, SS_DISABLE, 0)
    );
    assert!(state.unknown_syscalls().is_empty());
}

fn written_signal_mask_at(writes: &[(u64, Vec<u8>)], address: u64) -> [u8; 8] {
    let (_, bytes) = writes
        .iter()
        .find(|(write_address, _)| *write_address == address)
        .expect("signal mask write must exist");
    bytes.as_slice().try_into().unwrap()
}

fn written_signal_bytes_at(writes: &[(u64, Vec<u8>)], address: u64) -> Vec<u8> {
    let (_, bytes) = writes
        .iter()
        .find(|(write_address, _)| *write_address == address)
        .expect("signal mask write must exist");
    bytes.clone()
}

fn stack_t_bytes(sp: u64, flags: u64, size: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(LINUX_STACK_T_BYTES);
    bytes.extend_from_slice(&sp.to_le_bytes());
    bytes.extend_from_slice(&(flags as u32).to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&size.to_le_bytes());
    bytes
}

fn timespec64_bytes(seconds: i64, nanoseconds: i64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(16);
    bytes.extend_from_slice(&seconds.to_le_bytes());
    bytes.extend_from_slice(&nanoseconds.to_le_bytes());
    bytes
}

fn signal_info_reader(address: u64, si_code: i32) -> RiscvGuestMemoryReader {
    let mut bytes = vec![0; RISCV_LINUX_SIGINFO_T_BYTES];
    bytes[8..12].copy_from_slice(&si_code.to_le_bytes());
    RiscvGuestMemoryReader::new(move |read_address, read_bytes| {
        if read_address == address && read_bytes == RISCV_LINUX_SIGINFO_T_BYTES {
            Some(bytes.clone())
        } else {
            None
        }
    })
}

fn stack_reader(regions: Vec<(u64, Vec<u8>)>) -> RiscvGuestMemoryReader {
    RiscvGuestMemoryReader::new(move |address, bytes| {
        regions
            .iter()
            .find(|(base, region)| *base == address && region.len() == bytes)
            .map(|(_, region)| region.clone())
    })
}

type StackWriteLog = std::sync::Arc<std::sync::Mutex<Vec<(u64, Vec<u8>)>>>;

fn recording_stack_writer(writes: StackWriteLog) -> RiscvGuestMemoryWriter {
    RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes.lock().unwrap().push((address, bytes.to_vec()));
        true
    })
}

fn written_stack_at(writes: &[(u64, Vec<u8>)], address: u64) -> (u64, u64, u64) {
    let (_, bytes) = writes
        .iter()
        .find(|(write_address, _)| *write_address == address)
        .expect("signal stack write must exist");
    assert_eq!(bytes.len(), LINUX_STACK_T_BYTES);
    (
        read_le_u64(bytes, 0),
        u64::from(read_le_u32(bytes, 8)),
        read_le_u64(bytes, 16),
    )
}
