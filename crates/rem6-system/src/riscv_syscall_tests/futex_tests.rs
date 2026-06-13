use super::*;

#[test]
fn linux_table_wakes_guest_futex_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x180);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(7),
            PartitionId::new(1),
            20,
            3,
            3,
        ))
        .unwrap();
    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(8),
            PartitionId::new(2),
            21,
            3,
            3,
        ))
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 1, 1, 0, 0, 0]),
            &mut state,
            40,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(8)]
    );
}

#[test]
fn linux_table_wakes_guest_futex_waiters_by_bitset() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x184);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    state
        .guest_futexes_mut()
        .wait(
            GuestFutexWaitRequest::new(key, GuestThreadId::new(9), PartitionId::new(1), 22, 4, 4)
                .with_bitset(0b01),
        )
        .unwrap();
    state
        .guest_futexes_mut()
        .wait(
            GuestFutexWaitRequest::new(key, GuestThreadId::new(10), PartitionId::new(2), 23, 4, 4)
                .with_bitset(0b10),
        )
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 1, 0, 0, 0b01]),
            &mut state,
            41,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(10)]
    );
}

#[test]
fn linux_table_futex_wake_bitset_honors_wake_count() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x186);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(address, thread_group);

    for (thread, bitset) in [(11, 0b01), (12, 0b01), (13, 0b10)] {
        state
            .guest_futexes_mut()
            .wait(
                GuestFutexWaitRequest::new(
                    key,
                    GuestThreadId::new(thread),
                    PartitionId::new(1),
                    24,
                    5,
                    5,
                )
                .with_bitset(bitset),
            )
            .unwrap();
    }

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 1, 0, 0, 0b01]),
            &mut state,
            44,
        ),
        Some(RiscvSyscallOutcome::Return { value: 1 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(address, thread_group),
        vec![GuestThreadId::new(12), GuestThreadId::new(13)]
    );
}

#[test]
fn linux_table_futex_wait_mismatch_returns_eagain_without_queueing() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x188);
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, 0, 0, 0]),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(11)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
    assert!(!state.guest_futexes().is_waiting(GuestThreadId::new(100)));
}

#[test]
fn linux_table_futex_wait_bitset_zero_returns_einval() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18c);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| panic!("zero bitset must fail first"));

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 9, 2, 0, 0, 0]),
            &mut state,
            43,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}

#[test]
fn linux_table_futex_wake_bitset_zero_returns_einval() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x190);

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 10, 1, 0, 0, 0]),
            &mut state,
            43,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
}
