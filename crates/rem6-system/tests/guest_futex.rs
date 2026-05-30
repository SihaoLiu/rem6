use rem6_kernel::PartitionId;
use rem6_system::{
    GuestFutexAddress, GuestFutexError, GuestFutexKey, GuestFutexTable, GuestFutexWaitOutcome,
    GuestFutexWaitRequest, GuestThreadGroupId, GuestThreadId,
};

#[test]
fn guest_futex_wait_records_all_multicore_barrier_waiters_until_wake() {
    let mut table = GuestFutexTable::new();
    let address = GuestFutexAddress::new(0x181c08);
    let thread_group = GuestThreadGroupId::new(42);
    let key = GuestFutexKey::new(address, thread_group);

    for core in 0..8 {
        let thread = GuestThreadId::new(core);
        let outcome = table
            .wait(GuestFutexWaitRequest::new(
                key,
                thread,
                PartitionId::new(core as u32),
                100 + core,
                0x189,
                0x189,
            ))
            .unwrap();

        assert_eq!(
            outcome,
            GuestFutexWaitOutcome::Queued {
                thread,
                waiter_count: core as usize + 1,
            }
        );
    }

    assert_eq!(table.total_waiter_count(), 8);
    assert_eq!(table.waiter_count(address, thread_group), 8);
    assert!(table.is_waiting(GuestThreadId::new(0)));

    let wake = table.wake(address, thread_group, usize::MAX, 200).unwrap();

    assert_eq!(wake.woken_count(), 8);
    assert_eq!(
        wake.woken_threads(),
        vec![
            GuestThreadId::new(0),
            GuestThreadId::new(1),
            GuestThreadId::new(2),
            GuestThreadId::new(3),
            GuestThreadId::new(4),
            GuestThreadId::new(5),
            GuestThreadId::new(6),
            GuestThreadId::new(7),
        ]
    );
    assert_eq!(wake.records()[0].waiter().partition(), PartitionId::new(0));
    assert_eq!(wake.records()[0].waiter().enqueued_tick(), 100);
    assert_eq!(wake.records()[0].wake_tick(), 200);
    assert!(table.is_empty());
}

#[test]
fn guest_futex_wait_would_block_and_rejects_duplicate_or_zero_bitset_without_mutation() {
    let mut table = GuestFutexTable::new();
    let address = GuestFutexAddress::new(0x2000);
    let thread_group = GuestThreadGroupId::new(7);
    let key = GuestFutexKey::new(address, thread_group);
    let thread = GuestThreadId::new(3);

    assert_eq!(
        table
            .wait(GuestFutexWaitRequest::new(
                key,
                thread,
                PartitionId::new(3),
                10,
                0,
                1,
            ))
            .unwrap(),
        GuestFutexWaitOutcome::WouldBlock {
            expected: 0,
            observed: 1,
        }
    );
    assert!(table.is_empty());

    table
        .wait(
            GuestFutexWaitRequest::new(key, thread, PartitionId::new(3), 11, 5, 5)
                .with_bitset(0xffff_ffff),
        )
        .unwrap();

    assert_eq!(
        table
            .wait(
                GuestFutexWaitRequest::new(
                    GuestFutexKey::new(GuestFutexAddress::new(0x3000), thread_group),
                    thread,
                    PartitionId::new(3),
                    12,
                    9,
                    9,
                )
                .with_bitset(0xffff_ffff),
            )
            .unwrap_err(),
        GuestFutexError::DuplicateWaiter { thread }
    );
    assert_eq!(
        table
            .wait(
                GuestFutexWaitRequest::new(
                    key,
                    GuestThreadId::new(4),
                    PartitionId::new(4),
                    13,
                    5,
                    5,
                )
                .with_bitset(0),
            )
            .unwrap_err(),
        GuestFutexError::ZeroBitset {
            thread: GuestThreadId::new(4),
        }
    );
    assert_eq!(table.total_waiter_count(), 1);
}

#[test]
fn guest_futex_requeue_preserves_fifo_order_and_waiting_index() {
    let mut table = GuestFutexTable::new();
    let first_address = GuestFutexAddress::new(0x4000);
    let second_address = GuestFutexAddress::new(0x5000);
    let thread_group = GuestThreadGroupId::new(9);
    let first_key = GuestFutexKey::new(first_address, thread_group);

    for raw_thread in 1..=4 {
        table
            .wait(GuestFutexWaitRequest::new(
                first_key,
                GuestThreadId::new(raw_thread),
                PartitionId::new(raw_thread as u32),
                20 + raw_thread,
                17,
                17,
            ))
            .unwrap();
    }

    let requeue = table
        .requeue(first_address, second_address, thread_group, 1, 2, 40)
        .unwrap();

    assert_eq!(requeue.woken_threads(), vec![GuestThreadId::new(1)]);
    assert_eq!(
        requeue.requeued_threads(),
        vec![GuestThreadId::new(2), GuestThreadId::new(3)]
    );
    assert!(!table.is_waiting(GuestThreadId::new(1)));
    assert!(table.is_waiting(GuestThreadId::new(2)));
    assert!(table.is_waiting(GuestThreadId::new(3)));
    assert!(table.is_waiting(GuestThreadId::new(4)));
    assert_eq!(
        table.waiter_threads(first_address, thread_group),
        vec![GuestThreadId::new(4)]
    );
    assert_eq!(
        table.waiter_threads(second_address, thread_group),
        vec![GuestThreadId::new(2), GuestThreadId::new(3)]
    );

    let wake = table.wake(second_address, thread_group, 10, 44).unwrap();

    assert_eq!(
        wake.woken_threads(),
        vec![GuestThreadId::new(2), GuestThreadId::new(3)]
    );
    assert_eq!(table.waiter_count(first_address, thread_group), 1);
    assert!(!table.is_waiting(GuestThreadId::new(2)));
    assert!(!table.is_waiting(GuestThreadId::new(3)));
}
