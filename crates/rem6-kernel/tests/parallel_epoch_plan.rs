use rem6_kernel::{
    ParallelBatchUtilizationRatio, ParallelEpochPlannedWorkerRecord, PartitionId,
    PartitionedScheduler, ReadyPartition,
};

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

#[test]
fn scheduler_parallel_plan_exposes_planned_batch_occupancy_ticks() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 6, 2).unwrap();
    let cpu0 = PartitionId::new(0);
    let cpu1 = PartitionId::new(1);
    let cpu2 = PartitionId::new(2);

    scheduler.schedule_parallel_at(cpu0, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(cpu1, 2, |_| {}).unwrap();
    scheduler.schedule_parallel_at(cpu2, 5, |_| {}).unwrap();

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let batches = plan.parallel_batches();

    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0].start_tick(), 0);
    assert_eq!(batches[0].duration_ticks(), 6);
    assert_eq!(batches[0].worker_ticks(), 12);
    assert_eq!(batches[0].worker_capacity_ticks(2), 12);
    assert_eq!(batches[0].idle_worker_ticks(2), 0);
    assert_eq!(batches[1].start_tick(), 5);
    assert_eq!(batches[1].duration_ticks(), 1);
    assert_eq!(batches[1].worker_ticks(), 1);
    assert_eq!(batches[1].worker_capacity_ticks(2), 2);
    assert_eq!(batches[1].idle_worker_ticks(2), 1);
    assert_eq!(
        batches[1].utilization_ratio(2).unwrap(),
        ParallelBatchUtilizationRatio::new(1, 2).unwrap(),
    );
    assert_eq!(
        plan.parallel_batch_worker_count_tick_summaries(),
        vec![(1, 1), (2, 6)],
    );
    assert_eq!(plan.parallel_batch_ticks_for_worker_count(1), 1);
    assert_eq!(plan.parallel_batch_ticks_for_worker_count(2), 6);
    assert_eq!(plan.parallel_batch_ticks_at_or_above(2), 6);
    assert_eq!(plan.parallel_batch_worker_ticks(), 13);
    assert_eq!(plan.parallel_batch_worker_ticks_at_or_above(1), 13);
    assert_eq!(plan.parallel_batch_worker_ticks_at_or_above(2), 12);
    assert_eq!(plan.parallel_batch_worker_capacity_ticks(), 14);
    assert_eq!(plan.parallel_batch_idle_worker_ticks(), 1);
    assert_eq!(
        plan.parallel_batch_worker_slot_tick_summaries(),
        vec![(0, 7, 0), (1, 6, 1)],
    );
    assert_eq!(
        plan.parallel_batch_utilization_ratio().unwrap(),
        ParallelBatchUtilizationRatio::new(13, 14).unwrap(),
    );
}

#[test]
fn scheduler_parallel_plan_exposes_stable_host_worker_lane_assignments() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(5, 6, 2).unwrap();
    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);

    scheduler.schedule_parallel_at(core0, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core1, 2, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core2, 5, |_| {}).unwrap();

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let batches = plan.parallel_batches();

    assert_eq!(
        batches[0].planned_workers(),
        &[
            ParallelEpochPlannedWorkerRecord::new(0, core0, 0, 6),
            ParallelEpochPlannedWorkerRecord::new(1, core1, 2, 6),
        ],
    );
    assert_eq!(
        batches[1].planned_workers(),
        &[ParallelEpochPlannedWorkerRecord::new(0, core2, 5, 6)],
    );
    assert_eq!(
        batches[0]
            .planned_worker_for_partition(core1)
            .unwrap()
            .lane(),
        1
    );
    assert_eq!(
        batches[1].planned_worker_for_lane(0).unwrap().partition(),
        core2
    );
    assert_eq!(batches[1].planned_worker_for_lane(1), None);
    assert_eq!(
        plan.parallel_batch_planned_workers(),
        vec![
            ParallelEpochPlannedWorkerRecord::new(0, core0, 0, 6),
            ParallelEpochPlannedWorkerRecord::new(1, core1, 2, 6),
            ParallelEpochPlannedWorkerRecord::new(0, core2, 5, 6),
        ],
    );
    assert_eq!(
        plan.parallel_batch_lane_tick_summaries(),
        vec![(0, 7), (1, 4)],
    );
    assert_eq!(plan.parallel_batch_ticks_for_lane(0), 7);
    assert_eq!(plan.parallel_batch_ticks_for_lane(1), 4);

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    assert_eq!(
        run.planned_batch_planned_workers(),
        plan.parallel_batch_planned_workers(),
    );
    assert_eq!(
        run.planned_batch_lane_tick_summaries(),
        vec![(0, 7), (1, 4)]
    );
}

#[test]
fn recorded_parallel_run_preserves_planned_batch_shape_before_remote_wakeups() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(5, 5, 2).unwrap();

    scheduler
        .schedule_parallel_at(PartitionId::new(0), 0, |context| {
            context
                .schedule_remote_after(PartitionId::new(4), 5, |_| {})
                .unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(PartitionId::new(1), 1, |_| {})
        .unwrap();
    scheduler
        .schedule_parallel_at(PartitionId::new(2), 3, |_| {})
        .unwrap();

    let recorded = scheduler.run_until_idle_parallel_recorded().unwrap();
    let epoch = &recorded.epochs()[0];

    assert_eq!(recorded.epoch_count(), 1);
    assert_eq!(epoch.planned_parallel_worker_limit(), 2);
    assert_eq!(epoch.planned_batch_count(), 2);
    assert_eq!(
        epoch.planned_batch_worker_count_summaries(),
        vec![(1, 1), (2, 1)]
    );
    assert_eq!(epoch.planned_batch_count_for_worker_count(2), 1);
    assert_eq!(epoch.planned_batch_count_at_or_above(2), 1);
    assert_eq!(epoch.planned_batch_worker_count_total(), 3);
    assert_eq!(epoch.planned_batch_max_workers(), 2);
    assert_eq!(
        epoch.planned_batch_worker_count_tick_summaries(),
        vec![(1, 2), (2, 5)],
    );
    assert_eq!(epoch.planned_batch_ticks_for_worker_count(1), 2);
    assert_eq!(epoch.planned_batch_ticks_for_worker_count(2), 5);
    assert_eq!(epoch.planned_batch_ticks_at_or_above(1), 7);
    assert_eq!(epoch.planned_batch_ticks_at_or_above(2), 5);
    assert_eq!(epoch.planned_batch_worker_ticks(), 12);
    assert_eq!(epoch.planned_batch_worker_ticks_at_or_above(1), 12);
    assert_eq!(epoch.planned_batch_worker_ticks_at_or_above(2), 10);
    assert_eq!(epoch.planned_batch_worker_capacity_ticks(), 14);
    assert_eq!(epoch.planned_batch_idle_worker_ticks(), 2);
    assert_eq!(
        epoch.planned_batch_worker_slot_tick_summaries(),
        vec![(0, 7, 0), (1, 5, 2)],
    );
    assert_eq!(
        epoch.planned_batch_utilization_ratio().unwrap(),
        ParallelBatchUtilizationRatio::new(12, 14).unwrap(),
    );
    assert_eq!(
        epoch.planned_batch_partition_set_summaries(),
        vec![
            (vec![PartitionId::new(0), PartitionId::new(1)], 1),
            (vec![PartitionId::new(2)], 1),
        ],
    );
    assert_eq!(
        epoch.planned_batch_count_for_partition_set([PartitionId::new(2)]),
        1,
    );
    assert_eq!(
        epoch.planned_batches()[0].ready_partitions(),
        &[
            ReadyPartition {
                partition: PartitionId::new(0),
                next_tick: 0,
            },
            ReadyPartition {
                partition: PartitionId::new(1),
                next_tick: 1,
            },
        ],
    );

    assert_eq!(epoch.batch_worker_count_summaries(), vec![(2, 2)]);
    assert_eq!(
        epoch.batch_partition_set_summaries(),
        vec![
            (vec![PartitionId::new(0), PartitionId::new(1)], 1),
            (vec![PartitionId::new(2), PartitionId::new(4)], 1),
        ],
    );

    assert_eq!(recorded.planned_batch_count(), 2);
    assert_eq!(
        recorded.planned_batch_partition_set_summaries(),
        epoch.planned_batch_partition_set_summaries(),
    );
    assert_eq!(
        recorded.planned_batch_worker_count_summaries(),
        vec![(1, 1), (2, 1)]
    );
    assert_eq!(recorded.planned_batch_count_for_worker_count(2), 1);
    assert_eq!(recorded.planned_batch_count_at_or_above(2), 1);
    assert_eq!(recorded.planned_batch_worker_count_total(), 3);
    assert_eq!(recorded.planned_batch_max_workers(), 2);
    assert_eq!(
        recorded.planned_batch_worker_count_tick_summaries(),
        vec![(1, 2), (2, 5)],
    );
    assert_eq!(recorded.planned_batch_ticks_for_worker_count(1), 2);
    assert_eq!(recorded.planned_batch_ticks_for_worker_count(2), 5);
    assert_eq!(recorded.planned_batch_ticks_at_or_above(1), 7);
    assert_eq!(recorded.planned_batch_ticks_at_or_above(2), 5);
    assert_eq!(recorded.planned_batch_worker_ticks(), 12);
    assert_eq!(recorded.planned_batch_worker_ticks_at_or_above(1), 12);
    assert_eq!(recorded.planned_batch_worker_ticks_at_or_above(2), 10);
    assert_eq!(recorded.planned_batch_worker_capacity_ticks(), 14);
    assert_eq!(recorded.planned_batch_idle_worker_ticks(), 2);
    assert_eq!(
        recorded.planned_batch_worker_slot_tick_summaries(),
        vec![(0, 7, 0), (1, 5, 2)],
    );
    assert_eq!(
        recorded.planned_batch_utilization_ratio().unwrap(),
        ParallelBatchUtilizationRatio::new(12, 14).unwrap(),
    );
    assert_eq!(
        recorded.planned_batch_count_for_partition_set([PartitionId::new(2)]),
        1,
    );
    assert_eq!(recorded.batch_worker_count_summaries(), vec![(2, 2)]);
}
