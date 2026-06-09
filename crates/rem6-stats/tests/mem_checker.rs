use rem6_stats::{
    MemChecker, MemCheckerByteSnapshot, MemCheckerReadFailure, MemCheckerReadResult,
    MemCheckerSnapshot, MemCheckerTransaction, MemCheckerWriteClusterSnapshot, StatsError,
};

#[test]
fn mem_checker_accepts_observed_values_and_reports_impossible_reads() {
    let mut checker = MemChecker::new();

    let write = checker.start_write(10, 0x1000, &[0xaa, 0xbb]).unwrap();
    assert_eq!(write.serial(), 1);
    checker
        .complete_write(write.serial(), 20, 0x1000, 2)
        .unwrap();

    let read = checker.start_read(30, 0x1000, 2).unwrap();
    assert_eq!(
        checker
            .complete_read(read.serial(), 40, 0x1000, &[0xaa, 0xbb])
            .unwrap(),
        MemCheckerReadResult::valid(read.serial(), 2, 0)
    );

    let failed = checker.start_read(50, 0x1000, 2).unwrap();
    assert_eq!(
        checker
            .complete_read(failed.serial(), 60, 0x1000, &[0xaa, 0xcc])
            .unwrap(),
        MemCheckerReadResult::invalid(
            failed.serial(),
            2,
            0,
            vec![MemCheckerReadFailure::new(0x1001, 0xcc, vec![0xbb])]
        )
    );
}

#[test]
fn mem_checker_accepts_values_from_overlapping_write_clusters() {
    let mut checker = MemChecker::new();

    let first = checker.start_write(10, 0x2000, &[0x11]).unwrap();
    let second = checker.start_write(12, 0x2000, &[0x22]).unwrap();
    let read = checker.start_read(13, 0x2000, 1).unwrap();

    assert_eq!(
        checker
            .complete_read(read.serial(), 18, 0x2000, &[0x22])
            .unwrap(),
        MemCheckerReadResult::valid(read.serial(), 1, 0)
    );
    checker
        .complete_write(first.serial(), 20, 0x2000, 1)
        .unwrap();
    checker
        .complete_write(second.serial(), 22, 0x2000, 1)
        .unwrap();

    let later = checker.start_read(30, 0x2000, 1).unwrap();
    assert_eq!(
        checker
            .complete_read(later.serial(), 40, 0x2000, &[0x33])
            .unwrap(),
        MemCheckerReadResult::invalid(
            later.serial(),
            1,
            0,
            vec![MemCheckerReadFailure::new(0x2000, 0x33, vec![0x22, 0x11])]
        )
    );
}

#[test]
fn mem_checker_reset_range_ignores_late_completions_without_reusing_serials() {
    let mut checker = MemChecker::new();

    let write = checker.start_write(10, 0x3000, &[0x44, 0x55]).unwrap();
    let read = checker.start_read(11, 0x3000, 2).unwrap();
    checker.reset_range(0x3000, 2).unwrap();

    assert_eq!(
        checker
            .complete_read(read.serial(), 12, 0x3000, &[0x99, 0xaa])
            .unwrap(),
        MemCheckerReadResult::valid(read.serial(), 0, 2)
    );
    assert_eq!(
        checker
            .complete_write(write.serial(), 13, 0x3000, 2)
            .unwrap()
            .ignored_bytes(),
        2
    );

    let later = checker.start_read(20, 0x3000, 1).unwrap();
    assert_eq!(later.serial(), 3);
    assert_eq!(
        checker
            .complete_read(later.serial(), 21, 0x3000, &[0x77])
            .unwrap(),
        MemCheckerReadResult::valid(later.serial(), 1, 0)
    );
}

#[test]
fn mem_checker_abort_of_only_write_reuses_pristine_cluster() {
    let mut checker = MemChecker::new();

    let aborted = checker.start_write(10, 0x3800, &[0x12]).unwrap();
    assert_eq!(
        checker.abort_write(aborted.serial(), 0x3800, 1).unwrap(),
        rem6_stats::MemCheckerWriteResult::new(aborted.serial(), 1, 0)
    );

    let earlier = checker.start_write(5, 0x3800, &[0x34]).unwrap();
    checker
        .complete_write(earlier.serial(), 6, 0x3800, 1)
        .unwrap();
    let read = checker.start_read(7, 0x3800, 1).unwrap();
    assert_eq!(
        checker
            .complete_read(read.serial(), 8, 0x3800, &[0x34])
            .unwrap(),
        MemCheckerReadResult::valid(read.serial(), 1, 0)
    );

    let removed = checker.start_write(20, 0x3801, &[0x56]).unwrap();
    checker.abort_write(removed.serial(), 0x3801, 1).unwrap();
    assert_eq!(
        MemChecker::from_snapshot(&checker.snapshot())
            .unwrap()
            .snapshot(),
        checker.snapshot()
    );
}

#[test]
fn mem_checker_rejects_ambiguous_runtime_inputs() {
    let mut checker = MemChecker::new();

    assert_eq!(
        checker.start_read(10, 0x4000, 0),
        Err(StatsError::InvalidMemCheckerAccessSize { size: 0 })
    );
    assert_eq!(
        checker.start_write(10, 0x4000, &[]),
        Err(StatsError::InvalidMemCheckerAccessSize { size: 0 })
    );
    assert_eq!(
        checker.start_read(10, u64::MAX, 2),
        Err(StatsError::MemCheckerAddressRangeOverflow {
            address: u64::MAX,
            size: 2,
        })
    );

    let read = checker.start_read(10, 0x4000, 1).unwrap();
    assert_eq!(
        checker.complete_read(read.serial(), 9, 0x4000, &[0x00]),
        Err(StatsError::MemCheckerTransactionTimeWentBack {
            serial: read.serial(),
            start_tick: 10,
            complete_tick: 9,
        })
    );

    let write = checker.start_write(20, 0x4001, &[0x66]).unwrap();
    assert_eq!(
        checker.complete_write(write.serial(), 19, 0x4001, 1),
        Err(StatsError::MemCheckerTransactionTimeWentBack {
            serial: write.serial(),
            start_tick: 20,
            complete_tick: 19,
        })
    );

    let first = checker.start_write(30, 0x4100, &[0x10]).unwrap();
    let second = checker.start_write(31, 0x4100, &[0x20]).unwrap();
    checker
        .complete_write(first.serial(), 40, 0x4100, 1)
        .unwrap();
    let before_duplicate_complete = checker.snapshot();
    assert_eq!(
        checker.complete_write(first.serial(), 41, 0x4100, 1),
        Err(StatsError::MemCheckerWriteAlreadyCompleted {
            serial: first.serial()
        })
    );
    assert_eq!(checker.snapshot(), before_duplicate_complete);
    checker
        .complete_write(second.serial(), 42, 0x4100, 1)
        .unwrap();

    let partial = checker.start_write(50, 0x4200, &[0x44, 0x55]).unwrap();
    checker
        .complete_write(partial.serial(), 60, 0x4201, 1)
        .unwrap();
    let before_partial_duplicate = checker.snapshot();
    assert_eq!(
        checker.complete_write(partial.serial(), 61, 0x4200, 2),
        Err(StatsError::MemCheckerWriteAlreadyCompleted {
            serial: partial.serial()
        })
    );
    assert_eq!(checker.snapshot(), before_partial_duplicate);

    let completed_then_aborted = checker.start_write(70, 0x4300, &[0x88]).unwrap();
    checker
        .complete_write(completed_then_aborted.serial(), 80, 0x4300, 1)
        .unwrap();
    let before_abort_completed = checker.snapshot();
    assert_eq!(
        checker.abort_write(completed_then_aborted.serial(), 0x4300, 1),
        Err(StatsError::MemCheckerWriteAlreadyCompleted {
            serial: completed_then_aborted.serial()
        })
    );
    assert_eq!(checker.snapshot(), before_abort_completed);
}

#[test]
fn mem_checker_failed_multi_byte_write_start_is_transactional() {
    let mut checker = MemChecker::new();

    let first_byte = checker.start_write(10, 0x4800, &[0xaa]).unwrap();
    let second_byte = checker.start_write(20, 0x4801, &[0xbb]).unwrap();
    let before_failed_write = checker.snapshot();

    assert_eq!(
        checker.start_write(5, 0x4800, &[0xcc, 0xdd]),
        Err(StatsError::MemCheckerTransactionTimeWentBack {
            serial: 3,
            start_tick: 10,
            complete_tick: 5,
        })
    );
    assert_eq!(checker.snapshot(), before_failed_write);

    checker
        .complete_write(first_byte.serial(), 30, 0x4800, 1)
        .unwrap();
    checker
        .complete_write(second_byte.serial(), 31, 0x4801, 1)
        .unwrap();
}

#[test]
fn mem_checker_snapshot_restore_preserves_state_and_rejects_ambiguous_state() {
    let mut checker = MemChecker::new();
    let write = checker.start_write(10, 0x5000, &[0x88]).unwrap();
    let read = checker.start_read(12, 0x5000, 1).unwrap();

    let snapshot = checker.snapshot();
    let mut restored = MemChecker::from_snapshot(&snapshot).unwrap();
    assert_eq!(restored.snapshot(), snapshot);
    assert_eq!(
        restored
            .complete_read(read.serial(), 14, 0x5000, &[0x88])
            .unwrap(),
        MemCheckerReadResult::valid(read.serial(), 1, 0)
    );
    assert_eq!(
        restored
            .complete_write(write.serial(), 16, 0x5000, 1)
            .unwrap()
            .completed_bytes(),
        1
    );

    let duplicate_address = MemCheckerSnapshot::new(
        3,
        vec![
            MemCheckerByteSnapshot::initial(0x6000),
            MemCheckerByteSnapshot::initial(0x6000),
        ],
    );
    assert_eq!(
        MemChecker::from_snapshot(&duplicate_address),
        Err(StatsError::DuplicateMemCheckerSnapshotAddress { address: 0x6000 })
    );

    let duplicate_serial = MemCheckerSnapshot::new(
        3,
        vec![MemCheckerByteSnapshot::new(
            0x6000,
            vec![
                MemCheckerTransaction::read(1, 10),
                MemCheckerTransaction::read(1, 12),
            ],
            vec![MemCheckerTransaction::observed_read(0, 0, 0, 0)],
            Vec::new(),
        )],
    );
    assert_eq!(
        MemChecker::from_snapshot(&duplicate_serial),
        Err(StatsError::DuplicateMemCheckerSnapshotSerial { serial: 1 })
    );

    let duplicate_observation_serial = MemCheckerSnapshot::new(
        4,
        vec![MemCheckerByteSnapshot::new(
            0x6000,
            Vec::new(),
            vec![
                MemCheckerTransaction::observed_read(0, 0, 0, 0),
                MemCheckerTransaction::observed_read(2, 10, 11, 0xaa),
                MemCheckerTransaction::observed_read(2, 12, 13, 0xbb),
            ],
            Vec::new(),
        )],
    );
    assert_eq!(
        MemChecker::from_snapshot(&duplicate_observation_serial),
        Err(StatsError::DuplicateMemCheckerSnapshotSerial { serial: 2 })
    );

    let reused_cursor = MemCheckerSnapshot::new(
        2,
        vec![MemCheckerByteSnapshot::new(
            0x6000,
            Vec::new(),
            vec![
                MemCheckerTransaction::observed_read(0, 0, 0, 0),
                MemCheckerTransaction::observed_read(2, 10, 11, 0xaa),
            ],
            Vec::new(),
        )],
    );
    assert_eq!(
        MemChecker::from_snapshot(&reused_cursor),
        Err(StatsError::MemCheckerSnapshotSerialCursorBehind {
            next_serial: 2,
            highest_serial: 2,
        })
    );

    let bad_cluster = MemCheckerSnapshot::new(
        3,
        vec![MemCheckerByteSnapshot::new(
            0x6000,
            Vec::new(),
            vec![MemCheckerTransaction::observed_read(0, 0, 0, 0)],
            vec![MemCheckerWriteClusterSnapshot::new(
                10,
                20,
                19,
                1,
                vec![MemCheckerTransaction::write(2, 10, 20, 0xaa)],
            )],
        )],
    );
    assert_eq!(
        MemChecker::from_snapshot(&bad_cluster),
        Err(StatsError::MemCheckerSnapshotClusterIncompleteMismatch {
            expected: 0,
            observed: 1,
        })
    );

    let complete_tick_future_for_complete_cluster = MemCheckerSnapshot::new(
        3,
        vec![MemCheckerByteSnapshot::new(
            0x6000,
            Vec::new(),
            vec![MemCheckerTransaction::observed_read(0, 0, 0, 0)],
            vec![MemCheckerWriteClusterSnapshot::new(
                10,
                u64::MAX,
                20,
                0,
                vec![MemCheckerTransaction::write(2, 10, 20, 0xaa)],
            )],
        )],
    );
    assert_eq!(
        MemChecker::from_snapshot(&complete_tick_future_for_complete_cluster),
        Err(StatsError::MemCheckerSnapshotClusterCompletionMismatch {
            expected: 20,
            observed: u64::MAX,
        })
    );

    let complete_tick_closed_for_incomplete_cluster = MemCheckerSnapshot::new(
        3,
        vec![MemCheckerByteSnapshot::new(
            0x6000,
            Vec::new(),
            vec![MemCheckerTransaction::observed_read(0, 0, 0, 0)],
            vec![MemCheckerWriteClusterSnapshot::new(
                10,
                20,
                0,
                1,
                vec![MemCheckerTransaction::write(2, 10, u64::MAX, 0xaa)],
            )],
        )],
    );
    assert_eq!(
        MemChecker::from_snapshot(&complete_tick_closed_for_incomplete_cluster),
        Err(StatsError::MemCheckerSnapshotClusterCompletionMismatch {
            expected: u64::MAX,
            observed: 20,
        })
    );

    let bad_incomplete_complete_max = MemCheckerSnapshot::new(
        4,
        vec![MemCheckerByteSnapshot::new(
            0x6000,
            Vec::new(),
            vec![MemCheckerTransaction::observed_read(0, 0, 0, 0)],
            vec![MemCheckerWriteClusterSnapshot::new(
                10,
                u64::MAX,
                99,
                1,
                vec![
                    MemCheckerTransaction::write(2, 10, 20, 0xaa),
                    MemCheckerTransaction::write(3, 11, u64::MAX, 0xbb),
                ],
            )],
        )],
    );
    assert_eq!(
        MemChecker::from_snapshot(&bad_incomplete_complete_max),
        Err(StatsError::MemCheckerSnapshotClusterCompletionMismatch {
            expected: 20,
            observed: 99,
        })
    );
}
