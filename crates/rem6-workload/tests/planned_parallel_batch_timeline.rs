use rem6_kernel::{ParallelBatchUtilizationRatio, PartitionId};
use rem6_workload::{
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelExecutionSummary,
};

fn partition(index: u32) -> PartitionId {
    PartitionId::new(index)
}

fn timeline_record(
    scope: WorkloadParallelBatchScope,
    start_tick: u64,
    horizon: u64,
    partitions: impl IntoIterator<Item = PartitionId>,
    worker_count: usize,
) -> WorkloadParallelBatchTimelineRecord {
    WorkloadParallelBatchTimelineRecord::new(scope, start_tick, horizon, partitions, worker_count)
}

#[test]
fn workload_parallel_execution_summary_preserves_planned_parallel_batch_timelines() {
    let cpu = partition(0);
    let cache = partition(1);
    let actual_scheduler = vec![timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [cpu],
        1,
    )];
    let planned_scheduler = vec![timeline_record(
        WorkloadParallelBatchScope::Scheduler,
        0,
        4,
        [cpu, cache],
        2,
    )];
    let planned_data_cache = vec![timeline_record(
        WorkloadParallelBatchScope::DataCacheScheduler,
        4,
        8,
        [cache],
        1,
    )];
    let planned_full_system = vec![
        timeline_record(WorkloadParallelBatchScope::Scheduler, 0, 4, [cpu, cache], 2),
        timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [cache],
            1,
        ),
    ];

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline(actual_scheduler.clone())
        .with_parallel_scheduler_planned_batch_timeline(planned_scheduler.clone())
        .with_parallel_scheduler_planned_batch_worker_capacity_ticks(8)
        .with_data_cache_parallel_scheduler_planned_batch_timeline(planned_data_cache.clone())
        .with_data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(8)
        .with_full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(16)
        .with_full_system_parallel_scheduler_planned_batch_timeline(planned_full_system.clone());

    assert_eq!(
        summary.parallel_scheduler_batch_timeline(),
        actual_scheduler.as_slice(),
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_timeline(),
        planned_scheduler.as_slice(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_timeline(),
        planned_data_cache.as_slice(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_timeline(),
        planned_full_system,
    );
    assert_eq!(summary.parallel_scheduler_planned_batch_worker_ticks(), 8);
    assert_eq!(
        summary.parallel_scheduler_planned_batch_worker_capacity_ticks(),
        8,
    );
    assert_eq!(
        summary.parallel_scheduler_planned_batch_idle_worker_ticks(),
        0
    );
    assert_eq!(
        summary
            .parallel_scheduler_planned_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(8, 8).unwrap(),
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_worker_ticks(),
        4,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        8,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_planned_batch_idle_worker_ticks(),
        4,
    );
    assert_eq!(
        summary
            .data_cache_parallel_scheduler_planned_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(4, 8).unwrap(),
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_worker_ticks(),
        12,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        16,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_planned_batch_idle_worker_ticks(),
        4,
    );
    assert_eq!(
        summary
            .full_system_parallel_scheduler_planned_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(12, 16).unwrap(),
    );
    assert_ne!(
        summary.parallel_scheduler_batch_timeline(),
        summary.parallel_scheduler_planned_batch_timeline(),
    );
}
