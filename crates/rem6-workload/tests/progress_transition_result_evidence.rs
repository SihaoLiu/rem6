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
    let gpu_dma_scheduler = subject("gpu-dma-scheduler");
    let accelerator_dma_scheduler = subject("accelerator-dma-scheduler");
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
        ])
        .with_gpu_dma_scheduler_progress_transitions([transition(
            9,
            gpu_dma_scheduler.clone(),
            LivelockTransitionKind::MessageRetry,
            19,
            0,
        )])
        .with_accelerator_dma_scheduler_progress_transitions([transition(
            11,
            accelerator_dma_scheduler.clone(),
            LivelockTransitionKind::QueueRotation,
            23,
            0,
        )]);

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
            LivelockTransitionKind::MessageRetry,
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
            PartitionId::new(7),
            PartitionId::new(9),
            PartitionId::new(11)
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
        vec![
            accelerator_dma_scheduler,
            cpu_scheduler,
            data_cache_scheduler,
            gpu_dma_scheduler,
            shared_retry
        ],
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

#[test]
fn workload_result_returns_parallel_progress_transition_records_by_dimension() {
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
        summary.parallel_scheduler_progress_transition_records_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        vec![
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                9,
                1,
            ),
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                11,
                2,
            ),
        ],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_records_by_partition(
            PartitionId::new(7),
        ),
        vec![
            transition(
                7,
                data_cache_scheduler.clone(),
                LivelockTransitionKind::QueueRotation,
                13,
                0,
            ),
            transition(
                7,
                data_cache_scheduler.clone(),
                LivelockTransitionKind::QueueRotation,
                18,
                1,
            ),
        ],
    );
    assert_eq!(
        summary.full_system_progress_transition_records_by_kind(
            LivelockTransitionKind::ProtocolRetry,
        ),
        vec![
            transition(
                1,
                shared_retry.clone(),
                LivelockTransitionKind::ProtocolRetry,
                14,
                0,
            ),
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                9,
                1,
            ),
            transition(
                3,
                cpu_scheduler.clone(),
                LivelockTransitionKind::ProtocolRetry,
                11,
                2,
            ),
        ],
    );
    assert_eq!(
        summary.full_system_progress_transition_records_by_subject(&shared_retry),
        vec![
            transition(
                1,
                shared_retry.clone(),
                LivelockTransitionKind::SchedulerEpoch,
                4,
                0,
            ),
            transition(
                1,
                shared_retry,
                LivelockTransitionKind::ProtocolRetry,
                14,
                0,
            ),
        ],
    );
    assert!(summary
        .parallel_scheduler_progress_transition_records_by_subject(&data_cache_scheduler)
        .is_empty());
}

#[test]
fn workload_result_summarizes_parallel_progress_transition_dimensions() {
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
        summary.parallel_scheduler_progress_transition_kind_summaries(),
        vec![
            (LivelockTransitionKind::SchedulerEpoch, 1, 4, 4),
            (LivelockTransitionKind::ProtocolRetry, 2, 9, 11),
        ],
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_progress_transition_partition_summaries(),
        vec![
            (PartitionId::new(1), 1, 14, 14),
            (PartitionId::new(7), 2, 13, 18),
        ],
    );
    assert_eq!(
        summary.full_system_progress_transition_kind_summaries(),
        vec![
            (LivelockTransitionKind::SchedulerEpoch, 1, 4, 4),
            (LivelockTransitionKind::ProtocolRetry, 3, 9, 14),
            (LivelockTransitionKind::QueueRotation, 2, 13, 18),
        ],
    );
    assert_eq!(
        summary.full_system_progress_transition_subject_summaries(),
        vec![
            (cpu_scheduler, 2, 9, 11),
            (data_cache_scheduler, 2, 13, 18),
            (shared_retry, 2, 4, 14),
        ],
    );
}
