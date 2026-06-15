use super::*;

#[test]
fn linux_table_maps_exit_numbers_to_stop_codes() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT, [17; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Exit { code: 17 })
    );
    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(0x8000, RISCV_LINUX_EXIT_GROUP, [19; 6]),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Exit { code: 19 })
    );
}

#[test]
fn linux_table_exit_clears_child_tid_and_wakes_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let clear_tid = 0x188;
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(GuestFutexAddress::new(clear_tid), thread_group);
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
                thread::RISCV_LINUX_SET_TID_ADDRESS,
                [clear_tid, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 100 })
    );
    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(7),
            PartitionId::new(1),
            20,
            0,
            0,
        ))
        .unwrap();
    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(8),
            PartitionId::new(2),
            21,
            0,
            0,
        ))
        .unwrap();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_EXIT, [17, 0, 0, 0, 0, 0]),
            &mut state,
            21,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Exit { code: 17 })
    );
    assert_eq!(state.child_clear_tid(), None);
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(clear_tid, 0_i32.to_le_bytes().to_vec())]
    );
    assert_eq!(
        state
            .guest_futexes()
            .waiter_threads(GuestFutexAddress::new(clear_tid), thread_group),
        vec![GuestThreadId::new(8)]
    );
}

#[test]
fn linux_table_exit_preserves_child_tid_and_waiters_when_clear_write_fails() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let clear_tid = 0x18c;
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(GuestFutexAddress::new(clear_tid), thread_group);
    let writes = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let writes_for_writer = std::sync::Arc::clone(&writes);
    let guest_memory_writer = RiscvGuestMemoryWriter::new(move |address, bytes| {
        writes_for_writer
            .lock()
            .unwrap()
            .push((address, bytes.to_vec()));
        false
    });

    assert_eq!(
        table.handle(
            RiscvSyscallRequest::new(
                0x8000,
                thread::RISCV_LINUX_SET_TID_ADDRESS,
                [clear_tid, 0, 0, 0, 0, 0],
            ),
            &mut state,
        ),
        Some(RiscvSyscallOutcome::Return { value: 100 })
    );
    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(8),
            PartitionId::new(2),
            30,
            0,
            0,
        ))
        .unwrap();

    assert_eq!(
        table.handle_with_guest_memory_io_at_tick(
            RiscvSyscallRequest::new(0x8004, RISCV_LINUX_EXIT_GROUP, [19, 0, 0, 0, 0, 0]),
            &mut state,
            31,
            None,
            Some(&guest_memory_writer),
        ),
        Some(RiscvSyscallOutcome::Exit { code: 19 })
    );
    assert_eq!(
        writes.lock().unwrap().as_slice(),
        &[(clear_tid, 0_i32.to_le_bytes().to_vec())]
    );
    assert_eq!(state.child_clear_tid(), Some(clear_tid));
    assert_eq!(
        state
            .guest_futexes()
            .waiter_threads(GuestFutexAddress::new(clear_tid), thread_group),
        vec![GuestThreadId::new(8)]
    );
}
