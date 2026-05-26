use rem6_cpu::{
    RiscvClusterRun, RiscvClusterSchedulerEpoch, RiscvClusterStopReason, RiscvClusterTurn,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};

#[test]
fn cluster_scheduler_epoch_reports_batch_timeline_evidence() {
    let mut scheduler = scheduler_with_batch_timeline();
    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);

    let epoch = run_next_cluster_scheduler_epoch(&mut scheduler);

    let timeline = epoch.batch_timeline();
    assert_eq!(timeline.len(), 2);
    assert_eq!(timeline[0].start_tick(), 0);
    assert_eq!(timeline[0].horizon(), 4);
    assert_eq!(timeline[0].duration_ticks(), 4);
    assert_eq!(timeline[0].worker_count(), 2);
    assert_eq!(timeline[0].partitions(), &[core0, core1]);
    assert_eq!(timeline[1].start_tick(), 0);
    assert_eq!(timeline[1].horizon(), 4);
    assert_eq!(timeline[1].duration_ticks(), 4);
    assert_eq!(timeline[1].worker_count(), 1);
    assert_eq!(timeline[1].partitions(), &[core2]);
    assert_eq!(epoch.longest_batch_tick_streak_at_or_above(1), 4);
    assert_eq!(epoch.longest_batch_tick_streak_at_or_above(2), 4);
    assert_eq!(epoch.longest_batch_tick_streak_at_or_above(3), 0);
}

#[test]
fn cluster_run_reports_parallel_scheduler_batch_timeline_evidence() {
    let mut scheduler = scheduler_with_batch_timeline();
    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);
    let memory = PartitionId::new(3);

    let run = run_cluster_scheduler_until_idle(&mut scheduler);

    let timeline = run.parallel_scheduler_batch_timeline();
    assert_eq!(timeline.len(), 3);
    assert_eq!(timeline[0].start_tick(), 0);
    assert_eq!(timeline[0].horizon(), 4);
    assert_eq!(timeline[0].duration_ticks(), 4);
    assert_eq!(timeline[0].worker_count(), 2);
    assert_eq!(timeline[0].partitions(), &[core0, core1]);
    assert_eq!(timeline[1].start_tick(), 0);
    assert_eq!(timeline[1].horizon(), 4);
    assert_eq!(timeline[1].duration_ticks(), 4);
    assert_eq!(timeline[1].worker_count(), 1);
    assert_eq!(timeline[1].partitions(), &[core2]);
    assert_eq!(timeline[2].start_tick(), 4);
    assert_eq!(timeline[2].horizon(), 8);
    assert_eq!(timeline[2].duration_ticks(), 4);
    assert_eq!(timeline[2].worker_count(), 1);
    assert_eq!(timeline[2].partitions(), &[memory]);
    assert_eq!(
        run.parallel_scheduler_longest_batch_tick_streak_at_or_above(1),
        8,
    );
    assert_eq!(
        run.parallel_scheduler_longest_batch_tick_streak_at_or_above(2),
        4,
    );
    assert_eq!(
        run.parallel_scheduler_longest_batch_tick_streak_at_or_above(3),
        0,
    );
}

fn scheduler_with_batch_timeline() -> PartitionedScheduler {
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
