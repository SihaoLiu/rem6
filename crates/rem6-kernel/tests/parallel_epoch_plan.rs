use rem6_kernel::{PartitionId, PartitionedScheduler, ReadyPartition};

#[test]
fn scheduler_parallel_plan_exposes_worker_limited_batch_shape_without_dispatching() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(5, 8, 2).unwrap();
    let partitions = [
        PartitionId::new(0),
        PartitionId::new(1),
        PartitionId::new(2),
        PartitionId::new(3),
        PartitionId::new(4),
    ];

    for (index, partition) in partitions.iter().copied().enumerate() {
        scheduler
            .schedule_parallel_at(partition, 2 + index as u64, |_| {})
            .unwrap();
    }

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();

    assert_eq!(plan.horizon(), 8);
    assert_eq!(plan.ready_partition_count(), 5);
    assert_eq!(plan.parallel_worker_limit(), 2);
    assert_eq!(plan.parallel_batch_count(), 3);
    assert_eq!(
        plan.parallel_batch_worker_count_summaries(),
        vec![(1, 1), (2, 2)]
    );
    assert_eq!(plan.parallel_batch_count_for_worker_count(2), 2);
    assert_eq!(plan.parallel_batch_count_at_or_above(2), 2);
    assert_eq!(plan.parallel_batch_worker_count_total(), 5);
    assert_eq!(plan.parallel_batch_max_workers(), 2);
    assert_eq!(
        plan.parallel_batch_partition_set_summaries(),
        vec![
            (vec![PartitionId::new(0), PartitionId::new(1)], 1),
            (vec![PartitionId::new(2), PartitionId::new(3)], 1),
            (vec![PartitionId::new(4)], 1),
        ],
    );
    assert_eq!(
        plan.parallel_batch_count_for_partition_set([PartitionId::new(0), PartitionId::new(1)]),
        1,
    );
    assert_eq!(plan.parallel_batches()[0].worker_count(), 2);
    assert_eq!(
        plan.parallel_batches()[0].worker_partitions(),
        vec![PartitionId::new(0), PartitionId::new(1)],
    );
    assert_eq!(plan.parallel_batches()[2].worker_count(), 1);
    assert_eq!(
        plan.parallel_batches()[2].ready_partitions(),
        &[ReadyPartition {
            partition: PartitionId::new(4),
            next_tick: 6,
        }],
    );

    assert_eq!(scheduler.now(), 0);
    for (index, partition) in partitions.iter().copied().enumerate() {
        assert_eq!(
            scheduler.next_pending_tick(partition).unwrap(),
            Some(2 + index as u64),
        );
    }
}
