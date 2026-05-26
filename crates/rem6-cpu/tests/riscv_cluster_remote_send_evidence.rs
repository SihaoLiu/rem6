use rem6_cpu::{
    RiscvClusterRun, RiscvClusterSchedulerEpoch, RiscvClusterStopReason, RiscvClusterTurn,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};

#[test]
fn cluster_scheduler_epoch_reports_remote_send_counts_and_endpoints() {
    let (mut scheduler, core0, core1, memory, io) = scheduler_with_remote_sends();

    let epoch = run_next_cluster_scheduler_epoch(&mut scheduler);

    assert_eq!(epoch.total_remote_send_count(), 4);
    assert_eq!(epoch.remote_send_count(core0, memory), 2);
    assert_eq!(epoch.remote_send_count(core0, io), 1);
    assert_eq!(epoch.remote_send_count(core1, memory), 1);
    assert_eq!(epoch.remote_send_count(memory, core0), 0);
    assert_eq!(epoch.remote_source_partitions(), vec![core0, core1]);
    assert_eq!(epoch.remote_target_partitions(), vec![memory, io]);

    let sends = epoch.remote_sends();
    assert_eq!(sends.len(), 4);
    assert_eq!(sends[0].source(), core0);
    assert_eq!(sends[0].target(), memory);
    assert_eq!(sends[0].source_tick(), 0);
    assert_eq!(sends[0].delivery_tick(), 4);
    assert_eq!(sends[0].delay(), 4);
    assert_eq!(sends[0].order(), 0);
    assert_eq!(sends[1].source(), core0);
    assert_eq!(sends[1].target(), memory);
    assert_eq!(sends[1].order(), 1);
    assert_eq!(sends[2].source(), core0);
    assert_eq!(sends[2].target(), io);
    assert_eq!(sends[3].source(), core1);
    assert_eq!(sends[3].target(), memory);
}

#[test]
fn cluster_run_reports_parallel_scheduler_remote_send_evidence() {
    let (mut scheduler, core0, core1, memory, io) = scheduler_with_remote_sends();

    let run = run_cluster_scheduler_until_idle(&mut scheduler);

    assert_eq!(run.parallel_scheduler_total_remote_send_count(), 4);
    assert_eq!(run.parallel_scheduler_remote_send_count(core0, memory), 2);
    assert_eq!(run.parallel_scheduler_remote_send_count(core0, io), 1);
    assert_eq!(run.parallel_scheduler_remote_send_count(core1, memory), 1);
    assert_eq!(run.parallel_scheduler_remote_send_count(memory, core0), 0);
    assert_eq!(
        run.parallel_scheduler_remote_source_partitions(),
        vec![core0, core1],
    );
    assert_eq!(
        run.parallel_scheduler_remote_target_partitions(),
        vec![memory, io],
    );

    let sends = run.parallel_scheduler_remote_sends();
    assert_eq!(sends.len(), 4);
    assert_eq!(sends[0].source(), core0);
    assert_eq!(sends[0].target(), memory);
    assert_eq!(sends[0].order(), 0);
    assert_eq!(sends[1].source(), core0);
    assert_eq!(sends[1].target(), memory);
    assert_eq!(sends[1].order(), 1);
    assert_eq!(sends[2].source(), core0);
    assert_eq!(sends[2].target(), io);
    assert_eq!(sends[3].source(), core1);
    assert_eq!(sends[3].target(), memory);
}

fn scheduler_with_remote_sends() -> (
    PartitionedScheduler,
    PartitionId,
    PartitionId,
    PartitionId,
    PartitionId,
) {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(4, 4, 2).unwrap();

    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let memory = PartitionId::new(2);
    let io = PartitionId::new(3);

    scheduler
        .schedule_parallel_at(core0, 0, move |context| {
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
            context.schedule_remote_after(io, 8, |_| {}).unwrap();
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(core1, 0, move |context| {
            context.schedule_remote_after(memory, 4, |_| {}).unwrap();
        })
        .unwrap();

    (scheduler, core0, core1, memory, io)
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
