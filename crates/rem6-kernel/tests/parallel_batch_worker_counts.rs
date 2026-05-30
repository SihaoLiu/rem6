use rem6_kernel::{
    ParallelBatchUtilizationRatio, ParallelWorkerRecord, PartitionId, PartitionedScheduler,
};

#[test]
fn unbounded_worker_limit_reports_observed_batch_capacity() {
    let mut scheduler = PartitionedScheduler::new(3).unwrap();

    for partition in [
        PartitionId::new(0),
        PartitionId::new(1),
        PartitionId::new(2),
    ] {
        scheduler
            .schedule_parallel_at(partition, 0, |_| {})
            .unwrap();
    }

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();

    assert_eq!(plan.parallel_worker_limit(), usize::MAX);
    assert_eq!(plan.parallel_batch_worker_capacity_ticks(), 3);
    assert_eq!(plan.parallel_batch_idle_worker_ticks(), 0);
    assert_eq!(
        plan.parallel_batch_worker_slot_tick_summaries(),
        vec![(0, 1, 0), (1, 1, 0), (2, 1, 0)]
    );

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();

    assert_eq!(run.batch_worker_capacity_ticks(), 3);
    assert_eq!(run.batch_idle_worker_ticks(), 0);
    assert_eq!(
        run.batch_worker_slot_tick_summaries(),
        vec![(0, 1, 0), (1, 1, 0), (2, 1, 0)]
    );
    assert_eq!(
        run.batch_utilization_ratio().unwrap(),
        ParallelBatchUtilizationRatio::new(3, 3).unwrap()
    );
}

#[test]
fn recorded_parallel_batches_expose_actual_worker_lane_assignments() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 6, 2).unwrap();
    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);

    scheduler.schedule_parallel_at(core0, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core1, 2, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core2, 5, |_| {}).unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let epoch = &run.epochs()[0];
    let batches = epoch.batches();

    assert_eq!(
        batches[0].workers(),
        &[
            ParallelWorkerRecord::new(0, core0, 0, 6, Some(0), 1),
            ParallelWorkerRecord::new(1, core1, 0, 6, Some(2), 1),
        ],
    );
    assert_eq!(
        batches[1].workers(),
        &[ParallelWorkerRecord::new(0, core2, 0, 6, Some(5), 1)],
    );
    assert_eq!(batches[0].worker_for_lane(0).unwrap().partition(), core0);
    assert_eq!(batches[0].worker_for_lane(1).unwrap().partition(), core1);
    assert_eq!(batches[1].worker_for_lane(0).unwrap().partition(), core2);
    assert_eq!(batches[1].worker_for_lane(1), None);
    assert_eq!(batches[0].worker_for_partition(core1).unwrap().lane(), 1);
    assert_eq!(batches[0].worker_ticks_for_lane(0), 6);
    assert_eq!(batches[0].worker_ticks_for_lane(1), 6);
    assert_eq!(
        batches[0].worker_lane_tick_summaries(),
        vec![(0, 6), (1, 6)]
    );
    assert_eq!(batches[1].worker_ticks_for_lane(0), 6);
    assert_eq!(batches[1].worker_lane_tick_summaries(), vec![(0, 6)]);
    assert_eq!(
        epoch.batch_worker_lane_tick_summaries(),
        vec![(0, 12), (1, 6)],
    );
    assert_eq!(epoch.batch_worker_ticks_for_lane(0), 12);
    assert_eq!(epoch.batch_worker_ticks_for_lane(1), 6);
    assert_eq!(
        run.batch_worker_lane_tick_summaries(),
        vec![(0, 12), (1, 6)]
    );
    assert_eq!(run.batch_worker_ticks_for_lane(0), 12);
    assert_eq!(run.batch_worker_ticks_for_lane(1), 6);
}

#[test]
fn recorded_parallel_runs_report_exact_batch_worker_count_buckets() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);
    let memory = PartitionId::new(3);

    scheduler.schedule_parallel_at(core0, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core1, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core2, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(memory, 5, |_| {}).unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let first_epoch = &run.epochs()[0];
    let second_epoch = &run.epochs()[1];

    assert_eq!(
        first_epoch.batch_worker_count_summaries(),
        vec![(1, 1), (2, 1)]
    );
    assert_eq!(first_epoch.batch_count_for_worker_count(1), 1);
    assert_eq!(first_epoch.batch_count_for_worker_count(2), 1);
    assert_eq!(first_epoch.batch_count_at_or_above(1), 2);
    assert_eq!(first_epoch.batch_count_at_or_above(2), 1);
    assert_eq!(first_epoch.batch_count_at_or_above(3), 0);
    assert_eq!(first_epoch.batches()[0].start_tick(), 0);
    assert_eq!(first_epoch.batches()[0].duration_ticks(), 4);
    assert_eq!(first_epoch.batches()[0].worker_ticks(), 8);
    assert_eq!(first_epoch.batches()[1].start_tick(), 0);
    assert_eq!(first_epoch.batches()[1].duration_ticks(), 4);
    assert_eq!(first_epoch.batches()[1].worker_ticks(), 4);
    assert_eq!(
        first_epoch.batch_worker_count_tick_summaries(),
        vec![(1, 4), (2, 4)]
    );
    assert_eq!(first_epoch.batch_ticks_for_worker_count(1), 4);
    assert_eq!(first_epoch.batch_ticks_for_worker_count(2), 4);
    assert_eq!(first_epoch.batch_ticks_at_or_above(1), 8);
    assert_eq!(first_epoch.batch_ticks_at_or_above(2), 4);
    assert_eq!(first_epoch.batch_worker_ticks(), 12);
    assert_eq!(first_epoch.batch_worker_ticks_at_or_above(1), 12);
    assert_eq!(first_epoch.batch_worker_ticks_at_or_above(2), 8);
    assert_eq!(first_epoch.batch_worker_ticks_at_or_above(3), 0);
    assert_eq!(first_epoch.batches()[0].worker_capacity_ticks(2), 8);
    assert_eq!(first_epoch.batches()[0].idle_worker_ticks(2), 0);
    assert_eq!(
        first_epoch.batches()[0].utilization_ratio(2).unwrap(),
        ParallelBatchUtilizationRatio::new(8, 8).unwrap()
    );
    assert_eq!(first_epoch.batches()[1].worker_capacity_ticks(2), 8);
    assert_eq!(first_epoch.batches()[1].idle_worker_ticks(2), 4);
    assert_eq!(
        first_epoch.batches()[1].utilization_ratio(2).unwrap(),
        ParallelBatchUtilizationRatio::new(4, 8).unwrap()
    );
    assert_eq!(first_epoch.batch_worker_capacity_ticks(), 16);
    assert_eq!(first_epoch.batch_idle_worker_ticks(), 4);
    assert_eq!(
        first_epoch.batch_worker_slot_tick_summaries(),
        vec![(0, 8, 0), (1, 4, 4)]
    );
    assert_eq!(
        first_epoch.batch_utilization_ratio().unwrap(),
        ParallelBatchUtilizationRatio::new(12, 16).unwrap()
    );

    assert_eq!(second_epoch.batch_worker_count_summaries(), vec![(1, 1)]);
    assert_eq!(
        second_epoch.batch_worker_count_tick_summaries(),
        vec![(1, 4)]
    );
    assert_eq!(second_epoch.batch_worker_ticks(), 4);
    assert_eq!(second_epoch.batch_worker_ticks_at_or_above(2), 0);
    assert_eq!(run.batch_worker_count_summaries(), vec![(1, 2), (2, 1)]);
    assert_eq!(run.batch_count_for_worker_count(1), 2);
    assert_eq!(run.batch_count_for_worker_count(2), 1);
    assert_eq!(run.batch_count_at_or_above(1), 3);
    assert_eq!(run.batch_count_at_or_above(2), 1);
    assert_eq!(run.batch_count_at_or_above(3), 0);
    assert_eq!(
        run.batch_worker_count_tick_summaries(),
        vec![(1, 8), (2, 4)]
    );
    assert_eq!(run.batch_ticks_for_worker_count(1), 8);
    assert_eq!(run.batch_ticks_for_worker_count(2), 4);
    assert_eq!(run.batch_ticks_at_or_above(1), 12);
    assert_eq!(run.batch_ticks_at_or_above(2), 4);
    assert_eq!(run.batch_worker_ticks(), 16);
    assert_eq!(run.batch_worker_ticks_at_or_above(1), 16);
    assert_eq!(run.batch_worker_ticks_at_or_above(2), 8);
    assert_eq!(run.batch_worker_ticks_at_or_above(3), 0);
    assert_eq!(run.batch_worker_capacity_ticks(), 24);
    assert_eq!(run.batch_idle_worker_ticks(), 8);
    assert_eq!(
        run.batch_worker_slot_tick_summaries(),
        vec![(0, 12, 0), (1, 4, 8)]
    );
    assert_eq!(
        run.batch_utilization_ratio().unwrap(),
        ParallelBatchUtilizationRatio::new(16, 24).unwrap()
    );
}

#[test]
fn recorded_parallel_runs_report_exact_batch_partition_sets_and_streaks() {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let memory0 = PartitionId::new(2);
    let memory1 = PartitionId::new(3);

    scheduler
        .schedule_parallel_at(core0, 0, move |context| {
            context.schedule_remote_after(memory0, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(core1, 0, move |context| {
            context.schedule_remote_after(memory1, 4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(memory0, 4, move |context| {
            context.schedule_local_after(4, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(memory1, 4, move |context| {
            context.schedule_local_after(4, |_| {}).unwrap();
        })
        .unwrap();

    let run = scheduler.run_until_idle_parallel_recorded().unwrap();
    let first_epoch = &run.epochs()[0];
    let second_epoch = &run.epochs()[1];
    let core_set = vec![core0, core1];
    let memory_set = vec![memory0, memory1];

    assert_eq!(first_epoch.batches()[0].partition_set(), core_set);
    assert_eq!(first_epoch.batches()[1].partition_set(), memory_set);
    assert_eq!(
        first_epoch.batch_partition_set_summaries(),
        vec![(core_set.clone(), 1), (memory_set.clone(), 1)]
    );
    assert_eq!(
        first_epoch.batch_partition_streak_summaries(),
        vec![(core_set.clone(), 1), (memory_set.clone(), 1)]
    );
    assert_eq!(
        first_epoch.batch_count_for_partition_set([core1, core0, core0]),
        1
    );
    assert_eq!(
        first_epoch.max_consecutive_batch_count_for_partition_set([memory1, memory0]),
        1
    );

    assert_eq!(
        second_epoch.batch_partition_set_summaries(),
        vec![(memory_set.clone(), 1)]
    );
    assert_eq!(
        second_epoch.batch_partition_streak_summaries(),
        vec![(memory_set.clone(), 1)]
    );

    assert_eq!(
        run.batch_partition_set_summaries(),
        vec![(core_set.clone(), 1), (memory_set.clone(), 2)]
    );
    assert_eq!(run.batch_count_for_partition_set([memory1, memory0]), 2);
    assert_eq!(
        run.batch_partition_streak_summaries(),
        vec![(core_set, 1), (memory_set.clone(), 2)]
    );
    assert_eq!(
        run.max_consecutive_batch_count_for_partition_set([memory0, memory1]),
        2
    );
    assert_eq!(
        run.max_consecutive_batch_count_for_partition_set([core0, memory0]),
        0
    );
}
