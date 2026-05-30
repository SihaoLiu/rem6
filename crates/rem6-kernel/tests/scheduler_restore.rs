use rem6_kernel::{
    PartitionId, PartitionSnapshot, PartitionedScheduler, SchedulerError, SchedulerSnapshot,
};

#[test]
fn scheduler_quiescent_restore_rejects_partition_id_mismatch() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let snapshot = SchedulerSnapshot::with_parallel_worker_limit(
        8,
        5,
        1,
        vec![
            PartitionSnapshot::quiescent(memory, 8, 0, 0),
            PartitionSnapshot::quiescent(core, 8, 0, 0),
        ],
    );
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap();

    assert_eq!(
        scheduler.restore_quiescent(&snapshot).unwrap_err(),
        SchedulerError::SnapshotPartitionIdMismatch {
            expected_partition: core,
            snapshot_partition: memory,
        }
    );
}

#[test]
fn scheduler_quiescent_restore_rejects_global_tick_before_partition_clock_without_mutation() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let snapshot = SchedulerSnapshot::with_parallel_worker_limit(
        4,
        5,
        1,
        vec![
            PartitionSnapshot::quiescent(core, 4, 0, 0),
            PartitionSnapshot::quiescent(memory, 8, 0, 0),
        ],
    );
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap();
    scheduler.schedule_parallel_at(core, 10, |_| {}).unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    let before = scheduler.snapshot();

    assert_eq!(
        scheduler.restore_quiescent(&snapshot).unwrap_err(),
        SchedulerError::SnapshotGlobalTickBeforePartitionClock {
            snapshot_now: 4,
            partition: memory,
            partition_now: 8,
        }
    );
    assert_eq!(scheduler.snapshot(), before);
}

#[test]
fn scheduler_quiescent_restore_rejects_partition_clock_before_global_tick_without_mutation() {
    let core = PartitionId::new(0);
    let memory = PartitionId::new(1);
    let snapshot = SchedulerSnapshot::with_parallel_worker_limit(
        12,
        5,
        1,
        vec![
            PartitionSnapshot::quiescent(core, 12, 0, 0),
            PartitionSnapshot::quiescent(memory, 8, 0, 0),
        ],
    );
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(2, 5, 1).unwrap();
    scheduler.schedule_parallel_at(core, 10, |_| {}).unwrap();
    scheduler.run_until_idle_parallel().unwrap();
    let before = scheduler.snapshot();

    assert_eq!(
        scheduler.restore_quiescent(&snapshot).unwrap_err(),
        SchedulerError::SnapshotPartitionClockBeforeGlobalTick {
            snapshot_now: 12,
            partition: memory,
            partition_now: 8,
        }
    );
    assert_eq!(scheduler.snapshot(), before);
}
