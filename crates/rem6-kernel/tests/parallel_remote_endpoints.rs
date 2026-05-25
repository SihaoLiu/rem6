use rem6_kernel::{PartitionId, PartitionedScheduler};

#[test]
fn recorded_parallel_summaries_report_remote_endpoint_partitions() {
    let source0 = PartitionId::new(0);
    let source1 = PartitionId::new(1);
    let target0 = PartitionId::new(2);
    let target1 = PartitionId::new(3);
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    scheduler
        .schedule_parallel_at(source0, 0, move |context| {
            context.schedule_remote_after(target0, 4, |_| {}).unwrap();
            context.schedule_remote_after(target1, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(source1, 0, move |context| {
            context.schedule_remote_after(target0, 4, |_| {}).unwrap();
        })
        .unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let first_epoch = &run.epochs()[0];
    let source_batch = &first_epoch.batches()[0];

    assert_eq!(
        source_batch.remote_source_partitions(),
        vec![source0, source1]
    );
    assert_eq!(source_batch.remote_source_partition_count(), 2);
    assert_eq!(
        source_batch.remote_target_partitions(),
        vec![target0, target1]
    );
    assert_eq!(source_batch.remote_target_partition_count(), 2);

    assert_eq!(
        first_epoch.remote_source_partitions(),
        vec![source0, source1]
    );
    assert_eq!(first_epoch.remote_source_partition_count(), 2);
    assert_eq!(
        first_epoch.remote_target_partitions(),
        vec![target0, target1]
    );
    assert_eq!(first_epoch.remote_target_partition_count(), 2);

    assert_eq!(run.remote_source_partitions(), vec![source0, source1]);
    assert_eq!(run.remote_source_partition_count(), 2);
    assert_eq!(run.remote_target_partitions(), vec![target0, target1]);
    assert_eq!(run.remote_target_partition_count(), 2);
}
