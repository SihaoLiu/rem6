use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
use rem6_cpu::RiscvClusterTurn;
use rem6_kernel::{
    LivelockTransitionKind, PartitionId, PartitionedScheduler, WaitForGraph, WaitForNode,
};
use rem6_system::{RiscvSystemRun, RiscvSystemRunStopReason};

fn component(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

fn empty_wait_for_graphs() -> ParallelCoherenceWaitForGraphs {
    ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new())
}

fn cpu_scheduler_turn(
    partition: PartitionId,
    subject: WaitForNode,
    transition_count: usize,
) -> RiscvClusterTurn {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 1).unwrap();
    scheduler
        .schedule_parallel_at(partition, 0, move |context| {
            for _ in 0..transition_count {
                context.record_progress_transition(
                    subject.clone(),
                    LivelockTransitionKind::ProtocolRetry,
                );
            }
        })
        .unwrap();
    let plan = scheduler.plan_next_parallel_epoch().unwrap().unwrap();
    let recorded = scheduler.run_next_epoch_parallel_recorded().unwrap();
    RiscvClusterTurn::parallel_scheduler(plan, recorded)
}

fn data_cache_run(
    partition: PartitionId,
    subject: WaitForNode,
    transition_count: usize,
) -> ParallelCoherenceRunSummary {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 1).unwrap();
    scheduler
        .schedule_parallel_at(partition, 0, move |context| {
            for _ in 0..transition_count {
                context.record_progress_transition(
                    subject.clone(),
                    LivelockTransitionKind::MessageRetry,
                );
            }
        })
        .unwrap();

    ParallelCoherenceRunSummary::new(
        scheduler.run_until_idle_parallel_recorded().unwrap(),
        0,
        0,
        0,
        Vec::new(),
        Vec::new(),
        empty_wait_for_graphs(),
    )
}

#[test]
fn system_run_reports_scheduler_progress_and_livelock_diagnostics() {
    let cpu = PartitionId::new(0);
    let cache = PartitionId::new(1);
    let run = RiscvSystemRun::new(
        vec![cpu_scheduler_turn(cpu, component("cpu-scheduler"), 3)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run(
        cache,
        component("data-cache-scheduler"),
        2,
    )]);

    assert_eq!(run.parallel_scheduler_progress_transition_count(), 3);
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_count(),
        2
    );
    assert_eq!(run.full_system_progress_transition_count(), 5);

    assert_eq!(
        run.parallel_scheduler_livelock_diagnostic_count(2).unwrap(),
        1
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_livelock_diagnostic_count(2)
            .unwrap(),
        1,
    );
    assert_eq!(run.full_system_livelock_diagnostic_count(2).unwrap(), 2);
    assert!(run.has_full_system_livelock_diagnostics(2).unwrap());
    assert_eq!(run.full_system_livelock_diagnostic_count(4).unwrap(), 0);
    assert!(!run.has_full_system_livelock_diagnostics(4).unwrap());
}

#[test]
fn system_run_summarizes_progress_transition_dimensions() {
    let cpu = PartitionId::new(0);
    let cache = PartitionId::new(1);
    let cpu_subject = component("cpu-scheduler");
    let cache_subject = component("data-cache-scheduler");
    let run = RiscvSystemRun::new(
        vec![cpu_scheduler_turn(cpu, cpu_subject.clone(), 3)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run(cache, cache_subject.clone(), 2)]);

    assert_eq!(
        run.parallel_scheduler_progress_transition_kind_summaries(),
        vec![(LivelockTransitionKind::ProtocolRetry, 3, 0, 0)],
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_partition_summaries(),
        vec![(cpu, 3, 0, 0)],
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_subject_summaries(),
        vec![(cpu_subject.clone(), 3, 0, 0)],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_kind_summaries(),
        vec![(LivelockTransitionKind::MessageRetry, 2, 0, 0)],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_partition_summaries(),
        vec![(cache, 2, 0, 0)],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_subject_summaries(),
        vec![(cache_subject.clone(), 2, 0, 0)],
    );
    assert_eq!(
        run.full_system_progress_transition_kind_summaries(),
        vec![
            (LivelockTransitionKind::ProtocolRetry, 3, 0, 0),
            (LivelockTransitionKind::MessageRetry, 2, 0, 0),
        ],
    );
    assert_eq!(
        run.full_system_progress_transition_partition_summaries(),
        vec![(cpu, 3, 0, 0), (cache, 2, 0, 0)],
    );
    assert_eq!(
        run.full_system_progress_transition_subject_summaries(),
        vec![(cpu_subject, 3, 0, 0), (cache_subject, 2, 0, 0)],
    );
}

#[test]
fn system_run_rejects_zero_livelock_transition_threshold() {
    let run = RiscvSystemRun::new(
        Vec::new(),
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 0 },
    );

    assert_eq!(
        run.full_system_livelock_diagnostic_count(0),
        Err(rem6_kernel::ProgressMonitorError::ZeroTransitionThreshold),
    );
}
