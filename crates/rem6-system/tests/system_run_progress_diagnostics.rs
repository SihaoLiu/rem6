use rem6_coherence::{ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs};
use rem6_cpu::RiscvClusterTurn;
use rem6_kernel::{
    LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId, PartitionedScheduler,
    WaitForGraph, WaitForNode,
};
use rem6_system::{RiscvSystemRun, RiscvSystemRunStopReason};

fn component(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

fn empty_wait_for_graphs() -> ParallelCoherenceWaitForGraphs {
    ParallelCoherenceWaitForGraphs::new(WaitForGraph::new(), WaitForGraph::new())
}

fn transition(
    partition: PartitionId,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
) -> ParallelProgressTransitionRecord {
    ParallelProgressTransitionRecord::new(partition, subject, kind, tick, order)
}

fn cpu_scheduler_turn(
    partition: PartitionId,
    subject: WaitForNode,
    transition_count: usize,
) -> RiscvClusterTurn {
    cpu_scheduler_turn_at(partition, 0, subject, transition_count)
}

fn cpu_scheduler_turn_at(
    partition: PartitionId,
    tick: u64,
    subject: WaitForNode,
    transition_count: usize,
) -> RiscvClusterTurn {
    cpu_scheduler_turn_at_with_kind(
        partition,
        tick,
        subject,
        LivelockTransitionKind::ProtocolRetry,
        transition_count,
    )
}

fn cpu_scheduler_turn_at_with_kind(
    partition: PartitionId,
    tick: u64,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    transition_count: usize,
) -> RiscvClusterTurn {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 1).unwrap();
    scheduler
        .schedule_parallel_at(partition, tick, move |context| {
            for _ in 0..transition_count {
                context.record_progress_transition(subject.clone(), kind);
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
    data_cache_run_at(partition, 0, subject, transition_count)
}

fn data_cache_run_at(
    partition: PartitionId,
    tick: u64,
    subject: WaitForNode,
    transition_count: usize,
) -> ParallelCoherenceRunSummary {
    data_cache_run_at_with_kind(
        partition,
        tick,
        subject,
        LivelockTransitionKind::MessageRetry,
        transition_count,
    )
}

fn data_cache_run_at_with_kind(
    partition: PartitionId,
    tick: u64,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    transition_count: usize,
) -> ParallelCoherenceRunSummary {
    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(3, 4, 1).unwrap();
    scheduler
        .schedule_parallel_at(partition, tick, move |context| {
            for _ in 0..transition_count {
                context.record_progress_transition(subject.clone(), kind);
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
fn system_run_returns_livelock_diagnostic_records_by_scope() {
    let cpu = PartitionId::new(0);
    let cache = PartitionId::new(1);
    let shared_subject = component("shared-progress-loop");
    let run = RiscvSystemRun::new(
        vec![cpu_scheduler_turn_at(cpu, 0, shared_subject.clone(), 1)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![data_cache_run_at(cache, 3, shared_subject.clone(), 1)]);

    let scheduler_diagnostics = run.parallel_scheduler_livelock_diagnostics(1).unwrap();
    assert_eq!(scheduler_diagnostics.len(), 1);
    assert_eq!(scheduler_diagnostics[0].subject(), &shared_subject);
    assert_eq!(scheduler_diagnostics[0].threshold(), 1);
    assert_eq!(scheduler_diagnostics[0].transition_count(), 1);
    assert_eq!(
        scheduler_diagnostics[0].transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        1,
    );
    assert_eq!(scheduler_diagnostics[0].first_transition_tick(), 0);
    assert_eq!(scheduler_diagnostics[0].last_transition_tick(), 0);

    let data_cache_diagnostics = run
        .data_cache_parallel_scheduler_livelock_diagnostics(1)
        .unwrap();
    assert_eq!(data_cache_diagnostics.len(), 1);
    assert_eq!(data_cache_diagnostics[0].subject(), &shared_subject);
    assert_eq!(data_cache_diagnostics[0].threshold(), 1);
    assert_eq!(data_cache_diagnostics[0].transition_count(), 1);
    assert_eq!(
        data_cache_diagnostics[0].transition_count_by_kind(LivelockTransitionKind::MessageRetry),
        1,
    );
    assert_eq!(data_cache_diagnostics[0].first_transition_tick(), 3);
    assert_eq!(data_cache_diagnostics[0].last_transition_tick(), 3);

    assert!(run
        .parallel_scheduler_livelock_diagnostics(2)
        .unwrap()
        .is_empty());
    assert!(run
        .data_cache_parallel_scheduler_livelock_diagnostics(2)
        .unwrap()
        .is_empty());

    let full_system_diagnostics = run.full_system_livelock_diagnostics(2).unwrap();
    assert_eq!(full_system_diagnostics.len(), 1);
    assert_eq!(full_system_diagnostics[0].subject(), &shared_subject);
    assert_eq!(full_system_diagnostics[0].threshold(), 2);
    assert_eq!(full_system_diagnostics[0].transition_count(), 2);
    assert_eq!(
        full_system_diagnostics[0].transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        1,
    );
    assert_eq!(
        full_system_diagnostics[0].transition_count_by_kind(LivelockTransitionKind::MessageRetry),
        1,
    );
    assert_eq!(full_system_diagnostics[0].first_transition_tick(), 0);
    assert_eq!(full_system_diagnostics[0].last_transition_tick(), 3);
    assert_eq!(run.full_system_livelock_diagnostic_count(2).unwrap(), 1);
}

#[test]
fn system_run_summarizes_livelock_diagnostic_kind_windows() {
    let cpu = PartitionId::new(0);
    let cache = PartitionId::new(1);
    let shared_subject = component("shared-progress-loop");
    let cache_subject = component("cache-progress-loop");
    let run = RiscvSystemRun::new(
        vec![
            cpu_scheduler_turn_at_with_kind(
                cpu,
                0,
                shared_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                2,
            ),
            cpu_scheduler_turn_at_with_kind(
                cpu,
                0,
                shared_subject.clone(),
                LivelockTransitionKind::QueueRotation,
                1,
            ),
        ],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 24 },
    )
    .with_data_cache_runs(vec![
        data_cache_run_at_with_kind(
            cache,
            3,
            cache_subject.clone(),
            LivelockTransitionKind::MessageRetry,
            2,
        ),
        data_cache_run_at_with_kind(
            cache,
            8,
            cache_subject,
            LivelockTransitionKind::ProtocolRetry,
            1,
        ),
    ]);

    assert_eq!(
        run.parallel_scheduler_livelock_diagnostic_transition_kind_window_summaries(1)
            .unwrap(),
        vec![
            (LivelockTransitionKind::ProtocolRetry, 1, 2, 0, 0),
            (LivelockTransitionKind::QueueRotation, 1, 1, 0, 0),
        ],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_livelock_diagnostic_transition_kind_window_summaries(1)
            .unwrap(),
        vec![
            (LivelockTransitionKind::ProtocolRetry, 1, 1, 8, 8),
            (LivelockTransitionKind::MessageRetry, 1, 2, 3, 3),
        ],
    );
    assert_eq!(
        run.full_system_livelock_diagnostic_transition_kind_window_summaries(1)
            .unwrap(),
        vec![
            (LivelockTransitionKind::ProtocolRetry, 2, 3, 0, 8),
            (LivelockTransitionKind::QueueRotation, 1, 1, 0, 0),
            (LivelockTransitionKind::MessageRetry, 1, 2, 3, 3),
        ],
    );
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
fn system_run_reports_progress_transition_counts_and_windows_by_dimension() {
    let cpu = PartitionId::new(0);
    let cache = PartitionId::new(1);
    let cpu_subject = component("cpu-scheduler");
    let cache_subject = component("data-cache-scheduler");
    let missing_subject = component("missing-scheduler");
    let run = RiscvSystemRun::new(
        vec![cpu_scheduler_turn_at(cpu, 0, cpu_subject.clone(), 2)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![
        data_cache_run_at(cache, 3, cache_subject.clone(), 2),
        data_cache_run_at(cache, 6, cache_subject.clone(), 1),
    ]);

    assert_eq!(
        run.parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        2,
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        Some((0, 0)),
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_count_by_partition(cpu),
        2,
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_tick_window_by_partition(cpu),
        Some((0, 0)),
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_count_by_subject(&cpu_subject),
        2,
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_tick_window_by_subject(&cpu_subject),
        Some((0, 0)),
    );

    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_count_by_kind(
            LivelockTransitionKind::MessageRetry,
        ),
        3,
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::MessageRetry,
        ),
        Some((3, 6)),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_count_by_partition(cache),
        3,
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_tick_window_by_partition(cache),
        Some((3, 6)),
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_count_by_subject(&cache_subject),
        3,
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_tick_window_by_subject(
            &cache_subject
        ),
        Some((3, 6)),
    );

    assert_eq!(
        run.full_system_progress_transition_count_by_kind(LivelockTransitionKind::ProtocolRetry),
        2,
    );
    assert_eq!(
        run.full_system_progress_transition_count_by_partition(cache),
        3,
    );
    assert_eq!(
        run.full_system_progress_transition_count_by_subject(&cache_subject),
        3,
    );
    assert_eq!(
        run.full_system_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::ProtocolRetry
        ),
        Some((0, 0)),
    );
    assert_eq!(
        run.full_system_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::MessageRetry
        ),
        Some((3, 6)),
    );
    assert_eq!(
        run.full_system_progress_transition_tick_window_by_partition(cache),
        Some((3, 6)),
    );
    assert_eq!(
        run.full_system_progress_transition_tick_window_by_subject(&cache_subject),
        Some((3, 6)),
    );
    assert_eq!(
        run.full_system_progress_transition_count_by_subject(&missing_subject),
        0,
    );
    assert_eq!(
        run.full_system_progress_transition_tick_window_by_subject(&missing_subject),
        None,
    );
}

#[test]
fn system_run_lists_and_filters_progress_transition_records_by_dimension() {
    let cpu = PartitionId::new(0);
    let cache_a = PartitionId::new(1);
    let cache_b = PartitionId::new(2);
    let cpu_subject = component("cpu-scheduler");
    let cache_a_subject = component("cache-scheduler-a");
    let cache_b_subject = component("cache-scheduler-b");
    let missing_subject = component("missing-scheduler");
    let run = RiscvSystemRun::new(
        vec![cpu_scheduler_turn_at(cpu, 0, cpu_subject.clone(), 2)],
        Vec::new(),
        RiscvSystemRunStopReason::Idle { tick: 8 },
    )
    .with_data_cache_runs(vec![
        data_cache_run_at(cache_a, 3, cache_a_subject.clone(), 2),
        data_cache_run_at(cache_b, 6, cache_b_subject.clone(), 1),
    ]);

    assert_eq!(
        run.parallel_scheduler_progress_transition_kinds(),
        vec![LivelockTransitionKind::ProtocolRetry],
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_partitions(),
        vec![cpu],
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_subjects(),
        vec![cpu_subject.clone()],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_kinds(),
        vec![LivelockTransitionKind::MessageRetry],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_partitions(),
        vec![cache_a, cache_b],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_subjects(),
        vec![cache_a_subject.clone(), cache_b_subject.clone()],
    );
    assert_eq!(
        run.full_system_progress_transition_kinds(),
        vec![
            LivelockTransitionKind::ProtocolRetry,
            LivelockTransitionKind::MessageRetry,
        ],
    );
    assert_eq!(
        run.full_system_progress_transition_partitions(),
        vec![cpu, cache_a, cache_b],
    );
    assert_eq!(
        run.full_system_progress_transition_subjects(),
        vec![
            cache_a_subject.clone(),
            cache_b_subject.clone(),
            cpu_subject.clone(),
        ],
    );

    assert_eq!(
        run.parallel_scheduler_progress_transition_records_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        vec![
            transition(
                cpu,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                0,
                0,
            ),
            transition(
                cpu,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                0,
                1,
            ),
        ],
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_records_by_partition(cpu),
        vec![
            transition(
                cpu,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                0,
                0,
            ),
            transition(
                cpu,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                0,
                1,
            ),
        ],
    );
    assert_eq!(
        run.parallel_scheduler_progress_transition_records_by_subject(&cpu_subject),
        vec![
            transition(
                cpu,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                0,
                0,
            ),
            transition(
                cpu,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                0,
                1,
            ),
        ],
    );

    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_records_by_kind(
            LivelockTransitionKind::MessageRetry,
        ),
        vec![
            transition(
                cache_a,
                cache_a_subject.clone(),
                LivelockTransitionKind::MessageRetry,
                3,
                0,
            ),
            transition(
                cache_a,
                cache_a_subject.clone(),
                LivelockTransitionKind::MessageRetry,
                3,
                1,
            ),
            transition(
                cache_b,
                cache_b_subject.clone(),
                LivelockTransitionKind::MessageRetry,
                6,
                0,
            ),
        ],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_records_by_partition(cache_b),
        vec![transition(
            cache_b,
            cache_b_subject.clone(),
            LivelockTransitionKind::MessageRetry,
            6,
            0,
        )],
    );
    assert_eq!(
        run.data_cache_parallel_scheduler_progress_transition_records_by_subject(&cache_a_subject),
        vec![
            transition(
                cache_a,
                cache_a_subject.clone(),
                LivelockTransitionKind::MessageRetry,
                3,
                0,
            ),
            transition(
                cache_a,
                cache_a_subject.clone(),
                LivelockTransitionKind::MessageRetry,
                3,
                1,
            ),
        ],
    );

    assert_eq!(
        run.full_system_progress_transition_records_by_kind(LivelockTransitionKind::ProtocolRetry),
        vec![
            transition(
                cpu,
                cpu_subject.clone(),
                LivelockTransitionKind::ProtocolRetry,
                0,
                0,
            ),
            transition(
                cpu,
                cpu_subject,
                LivelockTransitionKind::ProtocolRetry,
                0,
                1,
            ),
        ],
    );
    assert_eq!(
        run.full_system_progress_transition_records_by_partition(cache_a),
        vec![
            transition(
                cache_a,
                cache_a_subject.clone(),
                LivelockTransitionKind::MessageRetry,
                3,
                0,
            ),
            transition(
                cache_a,
                cache_a_subject,
                LivelockTransitionKind::MessageRetry,
                3,
                1,
            ),
        ],
    );
    assert!(run
        .full_system_progress_transition_records_by_subject(&missing_subject)
        .is_empty());
    assert_eq!(
        run.full_system_progress_transition_records_by_subject(&cache_b_subject),
        vec![transition(
            cache_b,
            cache_b_subject,
            LivelockTransitionKind::MessageRetry,
            6,
            0,
        )],
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
    assert_eq!(
        run.full_system_livelock_diagnostics(0),
        Err(rem6_kernel::ProgressMonitorError::ZeroTransitionThreshold),
    );
}
