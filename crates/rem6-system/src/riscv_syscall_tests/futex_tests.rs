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
fn linux_table_futex_requeue_wakes_and_moves_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x187);
    let target = GuestFutexAddress::new(0x288);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);

    for thread in 1..=4 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                30 + thread,
                7,
                7,
            ))
            .unwrap();
    }

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 3, 1, 2, target.get(), 0]),
            &mut state,
            45,
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(4)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        vec![GuestThreadId::new(2), GuestThreadId::new(3)]
    );
}

#[test]
fn linux_table_futex_cmp_requeue_mismatch_returns_eagain_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18d);
    let target = GuestFutexAddress::new(0x28d);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == source.get() && bytes == 4 {
            Some(4_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    for thread in 5..=6 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                50 + thread,
                9,
                9,
            ))
            .unwrap();
    }

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 4, 1, 1, target.get(), 9]),
            &mut state,
            46,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(11)
        })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(5), GuestThreadId::new(6)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        Vec::<GuestThreadId>::new()
    );
}

#[test]
fn linux_table_futex_cmp_requeue_match_wakes_and_moves_waiters() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18a);
    let target = GuestFutexAddress::new(0x28a);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == source.get() && bytes == 4 {
            Some(9_i32.to_le_bytes().to_vec())
        } else {
            None
        }
    });

    for thread in 10..=12 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                80 + thread,
                9,
                9,
            ))
            .unwrap();
    }

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 4, 1, 2, target.get(), 9]),
            &mut state,
            49,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return { value: 3 })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        Vec::<GuestThreadId>::new()
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        vec![GuestThreadId::new(11), GuestThreadId::new(12)]
    );
}

#[test]
fn linux_table_futex_cmp_requeue_bad_address_returns_efault_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18c);
    let target = GuestFutexAddress::new(0x28c);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);
    let guest_memory = RiscvGuestMemoryReader::new(|_, _| None);

    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(13),
            PartitionId::new(13),
            90,
            15,
            15,
        ))
        .unwrap();

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 4, 0, 1, target.get(), 15]),
            &mut state,
            50,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(13)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        Vec::<GuestThreadId>::new()
    );
}

#[test]
fn linux_table_futex_requeue_rejects_negative_counts_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18e);
    let target = GuestFutexAddress::new(0x28e);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);

    for thread in 7..=8 {
        state
            .guest_futexes_mut()
            .wait(GuestFutexWaitRequest::new(
                key,
                GuestThreadId::new(thread),
                PartitionId::new(thread as u32),
                60 + thread,
                11,
                11,
            ))
            .unwrap();
    }

    for arguments in [
        [source.get(), 3, (-1_i64) as u64, 1, target.get(), 0],
        [source.get(), 3, 1, (-1_i64) as u64, target.get(), 0],
    ] {
        assert_eq!(
            table.handle_at_tick(
                RiscvSyscallRequest::new(0x8000, 98, arguments),
                &mut state,
                47,
            ),
            Some(RiscvSyscallOutcome::Return {
                value: linux_error(RISCV_LINUX_EINVAL)
            })
        );
        assert_eq!(
            state.guest_futexes().waiter_threads(source, thread_group),
            vec![GuestThreadId::new(7), GuestThreadId::new(8)]
        );
        assert_eq!(
            state.guest_futexes().waiter_threads(target, thread_group),
            Vec::<GuestThreadId>::new()
        );
    }
}

#[test]
fn linux_table_futex_requeue_rejects_clock_realtime_without_mutation() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let source = GuestFutexAddress::new(0x18f);
    let target = GuestFutexAddress::new(0x28f);
    let thread_group = GuestThreadGroupId::new(100);
    let key = GuestFutexKey::new(source, thread_group);

    state
        .guest_futexes_mut()
        .wait(GuestFutexWaitRequest::new(
            key,
            GuestThreadId::new(9),
            PartitionId::new(9),
            70,
            13,
            13,
        ))
        .unwrap();

    assert_eq!(
        table.handle_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [source.get(), 3 | 256, 0, 1, target.get(), 0]),
            &mut state,
            48,
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_ENOSYS)
        })
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(source, thread_group),
        vec![GuestThreadId::new(9)]
    );
    assert_eq!(
        state.guest_futexes().waiter_threads(target, thread_group),
        Vec::<GuestThreadId>::new()
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
fn linux_table_futex_wait_mismatch_still_validates_bad_timeout_pointer() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x189);
    let timeout_address = 0x3000;
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
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, timeout_address, 0, 0],),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EFAULT)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
}

#[test]
fn linux_table_futex_wait_mismatch_still_validates_invalid_timeout() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18a);
    let timeout_address = 0x3010;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else if read_address == timeout_address && bytes == 16 {
            let mut timeout = Vec::new();
            timeout.extend_from_slice(&0_i64.to_le_bytes());
            timeout.extend_from_slice(&1_000_000_000_i64.to_le_bytes());
            Some(timeout)
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, timeout_address, 0, 0],),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(RISCV_LINUX_EINVAL)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
}

#[test]
fn linux_table_futex_wait_mismatch_with_zero_timeout_returns_eagain() {
    let table = RiscvSyscallTable::new();
    let mut state = RiscvSyscallState::new(0);
    let address = GuestFutexAddress::new(0x18b);
    let timeout_address = 0x3020;
    let thread_group = GuestThreadGroupId::new(100);
    let guest_memory = RiscvGuestMemoryReader::new(move |read_address, bytes| {
        if read_address == address.get() && bytes == 4 {
            Some(1_i32.to_le_bytes().to_vec())
        } else if read_address == timeout_address && bytes == 16 {
            Some(vec![0; 16])
        } else {
            None
        }
    });

    assert_eq!(
        table.handle_with_guest_memory_at_tick(
            RiscvSyscallRequest::new(0x8000, 98, [address.get(), 0, 2, timeout_address, 0, 0],),
            &mut state,
            42,
            Some(&guest_memory),
        ),
        Some(RiscvSyscallOutcome::Return {
            value: linux_error(11)
        })
    );
    assert_eq!(state.guest_futexes().waiter_count(address, thread_group), 0);
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
