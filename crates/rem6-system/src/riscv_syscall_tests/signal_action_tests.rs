use super::*;

const SIGKILL_MASK: u64 = 1 << (9 - 1);
const SIGSTOP_MASK: u64 = 1 << (19 - 1);
const SIGUSR1: u64 = 10;
const SIGUSR2: u64 = 12;
const SIGRTMAX: u64 = 64;
const SA_SIGINFO: u64 = 0x0000_0004;
const SA_UNSUPPORTED: u64 = 0x0000_0400;
const SA_RESTART: u64 = 0x1000_0000;

#[test]
fn linux_table_rt_sigaction_installs_and_queries_guest_action() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let action_address = 0x4000;
    let old_action_address = 0x4100;
    let query_address = 0x4200;
    let handler = 0x1234_5678_9abc_def0_u64;
    let flags = SA_SIGINFO | SA_RESTART;
    let mask = 0x12_u64;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == action_address && bytes == 24 {
            Some(sigaction_bytes(handler, flags, mask))
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
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR1, action_address, old_action_address, 8, 0, 0],
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
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR1, 0, query_address, 8, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    assert_eq!(
        written_sigaction_at(&writes.lock().unwrap(), old_action_address),
        (0, 0, 0)
    );
    assert_eq!(
        written_sigaction_at(&writes.lock().unwrap(), query_address),
        (handler, flags, mask)
    );
}

#[test]
fn linux_table_rt_sigaction_filters_guest_flags_and_unblockable_mask() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let action_address = 0x4300;
    let query_address = 0x4400;
    let handler = 0x2000_u64;
    let requested_flags = SA_SIGINFO | SA_RESTART | SA_UNSUPPORTED | (1_u64 << 48);
    let expected_flags = SA_SIGINFO | SA_RESTART;
    let requested_mask = 0x62_u64 | SIGKILL_MASK | SIGSTOP_MASK;
    let expected_mask = 0x62_u64;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == action_address && bytes == 24 {
            Some(sigaction_bytes(handler, requested_flags, requested_mask))
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
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR2, action_address, 0, 8, 0, 0],
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
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR2, 0, query_address, 8, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    assert_eq!(
        written_sigaction_at(&writes.lock().unwrap(), query_address),
        (handler, expected_flags, expected_mask)
    );
}

#[test]
fn linux_table_rt_sigaction_allows_kernel_only_signal_queries() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        true
    });

    for (signal, query_address) in [(9, 0x4900), (19, 0x4920)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_RT_SIGACTION,
                    [signal, 0, query_address, 8, 0, 0],
                ),
                &mut state,
                7,
                None,
                Some(&guest_memory_writer),
            ),
            Some(RiscvSyscallOutcome::Return { value: 0 })
        );
        assert_eq!(
            written_sigaction_at(&writes.lock().unwrap(), query_address),
            (0, 0, 0)
        );
    }
}

#[test]
fn linux_table_rt_sigaction_supports_highest_guest_signal_mask_bit() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let action_address = 0x4a00;
    let query_address = 0x4a20;
    let handler = 0x6000_u64;
    let mask = 1_u64 << 63;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == action_address && bytes == 24 {
            Some(sigaction_bytes(handler, SA_RESTART, mask))
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
                RISCV_LINUX_RT_SIGACTION,
                [SIGRTMAX, action_address, 0, 8, 0, 0],
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
                RISCV_LINUX_RT_SIGACTION,
                [SIGRTMAX, 0, query_address, 8, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );

    assert_eq!(
        written_sigaction_at(&writes.lock().unwrap(), query_address),
        (handler, SA_RESTART, mask)
    );
}

#[test]
fn linux_table_rt_sigaction_faults_match_linux_state_ordering() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let valid_action_address = 0x4500;
    let faulting_action_address = 0x4518;
    let old_action_address = 0x4600;
    let query_address = 0x4618;
    let first_handler = 0x3000_u64;
    let second_handler = 0x4000_u64;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == valid_action_address && bytes == 24 {
            Some(sigaction_bytes(first_handler, SA_RESTART, 0x20))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR1, valid_action_address, 0, 8, 0, 0],
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
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR1, faulting_action_address, 0, 8, 0, 0],
            ),
            &mut state,
            8,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );

    let write_attempts = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let write_attempts_for_writer = std::sync::Arc::clone(&write_attempts);
    let faulting_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        write_attempts_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        false
    });
    let second_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == valid_action_address && bytes == 24 {
            Some(sigaction_bytes(second_handler, SA_SIGINFO, 0x40))
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8008,
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR1, valid_action_address, old_action_address, 8, 0, 0],
            ),
            &mut state,
            9,
            Some(&second_reader),
            Some(&faulting_writer),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        written_sigaction_at(&write_attempts.lock().unwrap(), old_action_address),
        (first_handler, SA_RESTART, 0x20)
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
                0x800c,
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR1, 0, query_address, 8, 0, 0],
            ),
            &mut state,
            10,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_sigaction_at(&writes.lock().unwrap(), query_address),
        (second_handler, SA_SIGINFO, 0x40)
    );
}

#[test]
fn linux_table_rt_sigaction_reads_action_before_signal_validation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let faulting_action_address = 0x4b00;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        assert_eq!(address, faulting_action_address);
        assert_eq!(bytes, 24);
        None
    });

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(
                0x8000,
                RISCV_LINUX_RT_SIGACTION,
                [0, faulting_action_address, 0, 8, 0, 0],
            ),
            &mut state,
            7,
            Some(&guest_memory_reader),
            None,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
}

#[test]
fn linux_table_rt_sigaction_rejects_bad_size_signal_and_kernel_only_install() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let action_address = 0x4700;
    let query_address = 0x4800;
    let guest_memory_reader = RiscvGuestMemoryReader::new(move |address, bytes| {
        if address == action_address && bytes == 24 {
            Some(sigaction_bytes(0x5000, SA_RESTART, 0x11))
        } else {
            None
        }
    });

    for (signal, size) in [(SIGUSR1, 4), (0, 8), (65, 8), (9, 8), (19, 8)] {
        assert_eq!(
            table.handle_with_guest_memory_io_at_tick(
                RiscvSyscallRequest::new(
                    0x8000,
                    RISCV_LINUX_RT_SIGACTION,
                    [signal, action_address, 0, size, 0, 0],
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
    }

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
                RISCV_LINUX_RT_SIGACTION,
                [SIGUSR1, 0, query_address, 8, 0, 0],
            ),
            &mut state,
            8,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Return { value: 0 })
    );
    assert_eq!(
        written_sigaction_at(&writes.lock().unwrap(), query_address),
        (0, 0, 0)
    );
}

fn sigaction_bytes(handler: u64, flags: u64, mask: u64) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(24);
    bytes.extend_from_slice(&handler.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&mask.to_le_bytes());
    bytes
}

fn written_sigaction_at(writes: &[(u64, Vec<u8>)], address: u64) -> (u64, u64, u64) {
    let (_, bytes) = writes
        .iter()
        .find(|(write_address, _)| *write_address == address)
        .expect("signal action write must exist");
    assert_eq!(bytes.len(), 24);
    (
        read_le_u64(bytes, 0),
        read_le_u64(bytes, 8),
        read_le_u64(bytes, 16),
    )
}
