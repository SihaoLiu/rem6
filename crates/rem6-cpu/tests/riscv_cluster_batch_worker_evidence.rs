use rem6_cpu::{
    RiscvClusterRun, RiscvClusterSchedulerEpoch, RiscvClusterStopReason, RiscvClusterTurn,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};

#[test]
fn cluster_scheduler_epoch_reports_batch_worker_count_and_tick_evidence() {
    let mut scheduler = scheduler_with_worker_count_batches();

    let epoch = run_next_cluster_scheduler_epoch(&mut scheduler);

    assert_eq!(epoch.batch_worker_count_summaries(), vec![(1, 1), (2, 1)]);
    assert_eq!(epoch.batch_count_for_worker_count(1), 1);
    assert_eq!(epoch.batch_count_for_worker_count(2), 1);
    assert_eq!(epoch.batch_count_for_worker_count(3), 0);
    assert_eq!(epoch.batch_count_at_or_above(1), 2);
    assert_eq!(epoch.batch_count_at_or_above(2), 1);
    assert_eq!(epoch.batch_count_at_or_above(3), 0);
    assert_eq!(
        epoch.batch_worker_count_tick_summaries(),
        vec![(1, 4), (2, 4)]
    );
    assert_eq!(epoch.batch_ticks_for_worker_count(1), 4);
    assert_eq!(epoch.batch_ticks_for_worker_count(2), 4);
    assert_eq!(epoch.batch_ticks_for_worker_count(3), 0);
    assert_eq!(epoch.batch_ticks_at_or_above(1), 8);
    assert_eq!(epoch.batch_ticks_at_or_above(2), 4);
    assert_eq!(epoch.batch_ticks_at_or_above(3), 0);
    assert_eq!(epoch.batch_worker_ticks(), 12);
    assert_eq!(epoch.batch_worker_ticks_at_or_above(1), 12);
    assert_eq!(epoch.batch_worker_ticks_at_or_above(2), 8);
    assert_eq!(epoch.batch_worker_ticks_at_or_above(3), 0);
}

#[test]
fn cluster_run_reports_parallel_scheduler_batch_worker_count_and_tick_evidence() {
    let mut scheduler = scheduler_with_worker_count_batches();

    let run = run_cluster_scheduler_until_idle(&mut scheduler);

    assert_eq!(
        run.parallel_scheduler_batch_worker_count_summaries(),
        vec![(1, 2), (2, 1)]
    );
    assert_eq!(run.parallel_scheduler_batch_count_for_worker_count(1), 2);
    assert_eq!(run.parallel_scheduler_batch_count_for_worker_count(2), 1);
    assert_eq!(run.parallel_scheduler_batch_count_for_worker_count(3), 0);
    assert_eq!(run.parallel_scheduler_batch_count_at_or_above(1), 3);
    assert_eq!(run.parallel_scheduler_batch_count_at_or_above(2), 1);
    assert_eq!(run.parallel_scheduler_batch_count_at_or_above(3), 0);
    assert_eq!(
        run.parallel_scheduler_batch_worker_count_tick_summaries(),
        vec![(1, 8), (2, 4)]
    );
    assert_eq!(run.parallel_scheduler_batch_ticks_for_worker_count(1), 8);
    assert_eq!(run.parallel_scheduler_batch_ticks_for_worker_count(2), 4);
    assert_eq!(run.parallel_scheduler_batch_ticks_for_worker_count(3), 0);
    assert_eq!(run.parallel_scheduler_batch_ticks_at_or_above(1), 12);
    assert_eq!(run.parallel_scheduler_batch_ticks_at_or_above(2), 4);
    assert_eq!(run.parallel_scheduler_batch_ticks_at_or_above(3), 0);
    assert_eq!(run.parallel_scheduler_batch_worker_ticks(), 16);
    assert_eq!(run.parallel_scheduler_batch_worker_ticks_at_or_above(1), 16);
    assert_eq!(run.parallel_scheduler_batch_worker_ticks_at_or_above(2), 8);
    assert_eq!(run.parallel_scheduler_batch_worker_ticks_at_or_above(3), 0);
}

fn scheduler_with_worker_count_batches() -> PartitionedScheduler {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);
    let memory = PartitionId::new(3);

    scheduler.schedule_parallel_at(core0, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core1, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(core2, 0, |_| {}).unwrap();
    scheduler.schedule_parallel_at(memory, 5, |_| {}).unwrap();

    scheduler
}

fn run_cluster_scheduler_until_idle(scheduler: &mut PartitionedScheduler) -> RiscvClusterRun {
    let mut turns = Vec::new();
    while let Some(plan) = scheduler.plan_next_parallel_epoch().unwrap() {
        let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
        turns.push(RiscvClusterTurn::parallel_scheduler(plan, recorded));
    }
    RiscvClusterRun::new(turns, RiscvClusterStopReason::StopCondition)
}

fn run_next_cluster_scheduler_epoch(
    scheduler: &mut PartitionedScheduler,
) -> RiscvClusterSchedulerEpoch {
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    RiscvClusterSchedulerEpoch::new(plan, recorded)
}
