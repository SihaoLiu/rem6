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
