use rem6_cpu::{
    RiscvClusterRun, RiscvClusterSchedulerEpoch, RiscvClusterStopReason, RiscvClusterTurn,
};
use rem6_kernel::{
    LivelockTransitionKind, PartitionId, PartitionedScheduler, ProgressMonitorError, WaitForNode,
};

#[test]
fn cluster_scheduler_epoch_reports_progress_transitions() {
    let subject = component("cpu-scheduler");
    let mut scheduler = scheduler_with_progress_transitions(subject.clone());

    let epoch = run_next_cluster_scheduler_epoch(&mut scheduler);

    assert_eq!(epoch.progress_transition_count(), 3);
    assert_eq!(
        epoch.progress_transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        2,
    );
    assert_eq!(
        epoch.progress_transition_count_by_kind(LivelockTransitionKind::ResourceArbitration),
        1,
    );
    assert_eq!(
        epoch.progress_transition_count_by_kind(LivelockTransitionKind::MessageRetry),
        0,
    );

    let transitions = epoch.progress_transitions();
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[0].partition(), PartitionId::new(0));
    assert_eq!(transitions[0].subject(), &subject);
    assert_eq!(transitions[0].kind(), LivelockTransitionKind::ProtocolRetry,);
    assert_eq!(transitions[0].tick(), 0);
    assert_eq!(transitions[0].order(), 0);
    assert_eq!(transitions[1].partition(), PartitionId::new(0));
    assert_eq!(
        transitions[1].kind(),
        LivelockTransitionKind::ResourceArbitration,
    );
    assert_eq!(transitions[1].order(), 1);
    assert_eq!(transitions[2].partition(), PartitionId::new(1));

    let snapshot = epoch.progress_monitor_snapshot(3).unwrap();
    assert!(snapshot.has_livelock());
    assert_eq!(snapshot.transition_count(&subject), Some(3));
    assert_eq!(
        epoch.progress_monitor_snapshot(0),
        Err(ProgressMonitorError::ZeroTransitionThreshold),
    );
}

#[test]
fn cluster_run_reports_parallel_scheduler_progress_transitions() {
    let subject = component("cpu-scheduler");
    let mut scheduler = scheduler_with_progress_transitions(subject.clone());

    let run = run_cluster_scheduler_until_idle(&mut scheduler);

    assert_eq!(run.parallel_scheduler_progress_transition_count(), 3);
    assert_eq!(
        run.parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        2,
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::ResourceArbitration,
        ),
        1,
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::MessageRetry
        ),
        0,
    );

    let transitions = run.parallel_scheduler_progress_transitions();
    assert_eq!(transitions.len(), 3);
    assert_eq!(transitions[0].partition(), PartitionId::new(0));
    assert_eq!(transitions[0].subject(), &subject);
    assert_eq!(transitions[0].order(), 0);
    assert_eq!(transitions[1].order(), 1);
    assert_eq!(transitions[2].partition(), PartitionId::new(1));

    let snapshot = run.parallel_scheduler_progress_monitor_snapshot(3).unwrap();
    assert!(snapshot.has_livelock());
    assert_eq!(snapshot.transition_count(&subject), Some(3));
    assert_eq!(
        run.parallel_scheduler_progress_monitor_snapshot(0),
        Err(ProgressMonitorError::ZeroTransitionThreshold),
    );
}

fn scheduler_with_progress_transitions(subject: WaitForNode) -> PartitionedScheduler {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 3).unwrap();
    let core0 = PartitionId::new(0);
    let core1 = PartitionId::new(1);
    let core2 = PartitionId::new(2);

    let core0_subject = subject.clone();
    scheduler
        .schedule_parallel_at(core0, 0, move |context| {
            context.record_progress_transition(
                core0_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
            );
            context.record_progress_transition(
                core0_subject,
                LivelockTransitionKind::ResourceArbitration,
            );
        })
        .unwrap();
    scheduler
        .schedule_parallel_at(core1, 0, move |context| {
            context.record_progress_transition(subject, LivelockTransitionKind::ProtocolRetry);
        })
        .unwrap();
    scheduler.schedule_parallel_at(core2, 0, |_| {}).unwrap();

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

fn component(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}
