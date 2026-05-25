use rem6_kernel::{
    LivelockTransitionKind, ParallelProgressTransitionRecord, PartitionId, WaitForNode,
};
use rem6_workload::WorkloadParallelExecutionSummary;

fn subject(name: &str) -> WaitForNode {
    WaitForNode::component(name).unwrap()
}

fn transition(
    partition: u32,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: u64,
    order: u64,
) -> ParallelProgressTransitionRecord {
    ParallelProgressTransitionRecord::new(PartitionId::new(partition), subject, kind, tick, order)
}

#[test]
fn workload_result_lists_parallel_progress_transition_dimensions() {
    let cpu_scheduler = subject("cpu-scheduler");
    let shared_retry = subject("shared-retry-loop");
    let data_cache_scheduler = subject("data-cache-scheduler");
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                9,
                1,
            ),
            transition(
                1,
                shared_retry.clone(),
                LivelockTransitionKind::SchedulerEpoch,
                4,
                0,
            ),
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                11,
                2,
            ),
        ])
        .with_data_cache_parallel_scheduler_progress_transitions([
            transition(
                7,
                data_cache_scheduler.clone(),
                LivelockTransitionKind::QueueRotation,
                13,
                0,
            ),
            transition(
                1,
                shared_retry.clone(),
                LivelockTransitionKind::ProtocolRetry,
                14,
                1,
            ),
        ]);

    assert_eq!(
        summary.parallel_scheduler_progress_transition_kinds(),
        vec![
            LivelockTransitionKind::SchedulerEpoch,
            LivelockTransitionKind::ProtocolRetry,
        ],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_kinds(),
        vec![
            LivelockTransitionKind::ProtocolRetry,
            LivelockTransitionKind::QueueRotation,
        ],
    );
    assert_eq!(
        summary.full_system_progress_transition_kinds(),
        vec![
            LivelockTransitionKind::SchedulerEpoch,
            LivelockTransitionKind::ProtocolRetry,
            LivelockTransitionKind::QueueRotation,
        ],
    );

    assert_eq!(
        summary.parallel_scheduler_progress_transition_partitions(),
        vec![PartitionId::new(1), PartitionId::new(3)],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_partitions(),
        vec![PartitionId::new(1), PartitionId::new(7)],
    );
    assert_eq!(
        summary.full_system_progress_transition_partitions(),
        vec![
            PartitionId::new(1),
            PartitionId::new(3),
            PartitionId::new(7)
        ],
    );

    assert_eq!(
        summary.parallel_scheduler_progress_transition_subjects(),
        vec![cpu_scheduler.clone(), shared_retry.clone()],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_subjects(),
        vec![data_cache_scheduler.clone(), shared_retry.clone()],
    );
    assert_eq!(
        summary.full_system_progress_transition_subjects(),
        vec![cpu_scheduler, data_cache_scheduler, shared_retry],
    );
}

#[test]
fn workload_result_reports_parallel_progress_transition_tick_windows() {
    let cpu_scheduler = subject("cpu-scheduler");
    let shared_retry = subject("shared-retry-loop");
    let data_cache_scheduler = subject("data-cache-scheduler");
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_progress_transitions([
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                11,
                2,
            ),
            transition(
                1,
                shared_retry.clone(),
                LivelockTransitionKind::SchedulerEpoch,
                4,
                0,
            ),
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                9,
                1,
            ),
        ])
        .with_data_cache_parallel_scheduler_progress_transitions([
            transition(
                7,
                data_cache_scheduler.clone(),
                LivelockTransitionKind::QueueRotation,
                18,
                1,
            ),
            transition(
                1,
                shared_retry.clone(),
                LivelockTransitionKind::ProtocolRetry,
                14,
                0,
            ),
            transition(
                7,
                data_cache_scheduler.clone(),
                LivelockTransitionKind::QueueRotation,
                13,
                0,
            ),
        ]);

    assert_eq!(
        summary.parallel_scheduler_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        Some((9, 11)),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::QueueRotation,
        ),
        Some((13, 18)),
    );
    assert_eq!(
        summary.full_system_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        Some((9, 14)),
    );
    assert_eq!(
        summary.full_system_progress_transition_tick_window_by_kind(
            LivelockTransitionKind::MessageRetry,
        ),
        None,
    );

    assert_eq!(
        summary
            .parallel_scheduler_progress_transition_tick_window_by_partition(PartitionId::new(3),),
        Some((9, 11)),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_tick_window_by_partition(
            PartitionId::new(7),
        ),
        Some((13, 18)),
    );
    assert_eq!(
        summary.full_system_progress_transition_tick_window_by_partition(PartitionId::new(1)),
        Some((4, 14)),
    );
    assert_eq!(
        summary.full_system_progress_transition_tick_window_by_partition(PartitionId::new(42)),
        None,
    );

    assert_eq!(
        summary.parallel_scheduler_progress_transition_tick_window_by_subject(&cpu_scheduler),
        Some((9, 11)),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_tick_window_by_subject(
            &data_cache_scheduler,
        ),
        Some((13, 18)),
    );
    assert_eq!(
        summary.full_system_progress_transition_tick_window_by_subject(&shared_retry),
        Some((4, 14)),
    );
    assert_eq!(
        summary.full_system_progress_transition_tick_window_by_subject(&subject("missing-subject")),
        None,
    );
}
