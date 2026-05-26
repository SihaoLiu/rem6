use rem6_cpu::{
    RiscvClusterRun, RiscvClusterSchedulerEpoch, RiscvClusterStopReason, RiscvClusterTurn,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};

#[test]
fn cluster_scheduler_epoch_reports_batch_partition_sets_and_streaks() {
    let (mut scheduler, core0, core1, memory0, memory1) = scheduler_with_batch_partition_sets();

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let epoch = RiscvClusterSchedulerEpoch::new(plan, recorded);

    let core_set = vec![core0, core1];
    let memory_set = vec![memory0, memory1];

    assert_eq!(epoch.batches()[0].partition_set(), core_set);
    assert_eq!(epoch.batches()[1].partition_set(), memory_set);
    assert_eq!(
        epoch.batch_partition_set_summaries(),
        vec![(core_set.clone(), 1), (memory_set.clone(), 1)]
    );
    assert_eq!(
        epoch.batch_count_for_partition_set([core1, core0, core0]),
        1
    );
    assert_eq!(
        epoch.batch_partition_streak_summaries(),
        vec![(core_set.clone(), 1), (memory_set.clone(), 1)]
    );
    assert_eq!(
        epoch.max_consecutive_batch_count_for_partition_set([memory1, memory0]),
        1
    );
}

#[test]
fn cluster_run_reports_parallel_scheduler_batch_partition_sets_and_streaks() {
    let (mut scheduler, core0, core1, memory0, memory1) = scheduler_with_batch_partition_sets();

    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    let run = RiscvClusterRun::new(
        vec![RiscvClusterTurn::parallel_scheduler(plan, recorded)],
        RiscvClusterStopReason::StopCondition,
    );

    let core_set = vec![core0, core1];
    let memory_set = vec![memory0, memory1];

    assert_eq!(
        run.parallel_scheduler_batch_partition_set_summaries(),
        vec![(core_set.clone(), 1), (memory_set.clone(), 1)]
    );
    assert_eq!(
        run.parallel_scheduler_batch_count_for_partition_set([core1, core0, core0]),
        1
    );
    assert_eq!(
        run.parallel_scheduler_batch_partition_streak_summaries(),
        vec![(core_set.clone(), 1), (memory_set.clone(), 1)]
    );
    assert_eq!(
        run.parallel_scheduler_max_consecutive_batch_count_for_partition_set([memory1, memory0]),
        1
    );
}

fn scheduler_with_batch_partition_sets() -> (
    PartitionedScheduler,
    PartitionId,
    PartitionId,
    PartitionId,
    PartitionId,
) {
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

    (scheduler, core0, core1, memory0, memory1)
}
