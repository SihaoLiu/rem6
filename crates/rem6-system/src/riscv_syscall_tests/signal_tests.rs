use super::*;

const SIGKILL_MASK: u64 = 1 << (9 - 1);
const SIGSTOP_MASK: u64 = 1 << (19 - 1);

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

fn written_signal_mask_at(writes: &[(u64, Vec<u8>)], address: u64) -> [u8; 8] {
    let (_, bytes) = writes
        .iter()
        .find(|(write_address, _)| *write_address == address)
        .expect("signal mask write must exist");
    bytes.as_slice().try_into().unwrap()
}
