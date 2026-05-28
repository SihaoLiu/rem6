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
fn workload_parallel_execution_summary_preserves_recorded_batch_capacity() {
    let cpu = partition(0);
    let cache = partition(1);
    let gpu = partition(2);
    let accelerator = partition(3);

    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [cpu],
            1,
        )])
        .with_parallel_scheduler_recorded_batch_worker_capacity_ticks(8)
        .with_parallel_scheduler_recorded_batch_worker_slot_tick_summaries([(0, 4, 0), (1, 0, 4)])
        .with_data_cache_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::DataCacheScheduler,
            4,
            8,
            [cpu, cache],
            2,
        )])
        .with_data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(8)
        .with_data_cache_parallel_scheduler_recorded_batch_worker_slot_tick_summaries([
            (0, 4, 0),
            (1, 4, 0),
        ])
        .with_gpu_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            8,
            10,
            [gpu, cache],
            2,
        )])
        .with_gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(6)
        .with_gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries([
            (0, 2, 0),
            (1, 2, 0),
            (2, 0, 2),
        ])
        .with_accelerator_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::AcceleratorDmaScheduler,
            10,
            13,
            [accelerator],
            1,
        )])
        .with_accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks(6)
        .with_accelerator_dma_scheduler_recorded_batch_worker_slot_tick_summaries([
            (0, 3, 0),
            (1, 0, 3),
        ]);

    assert_eq!(summary.parallel_scheduler_batch_worker_ticks(), 0);
    assert_eq!(summary.parallel_scheduler_recorded_batch_worker_ticks(), 4);
    assert_eq!(
        summary.parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        8,
    );
    assert_eq!(
        summary.parallel_scheduler_recorded_batch_idle_worker_ticks(),
        4,
    );
    assert_eq!(
        summary.parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
        vec![(0, 4, 0), (1, 0, 4)],
    );
    assert_eq!(
        summary
            .parallel_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(4, 8).unwrap(),
    );

    assert_eq!(
        summary.data_cache_parallel_scheduler_recorded_batch_worker_ticks(),
        8,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        8,
    );
    assert_eq!(
        summary.data_cache_parallel_scheduler_recorded_batch_idle_worker_ticks(),
        0,
    );
    assert_eq!(
        summary
            .data_cache_parallel_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(8, 8).unwrap(),
    );

    assert_eq!(summary.gpu_dma_scheduler_recorded_batch_worker_ticks(), 4);
    assert_eq!(
        summary.gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(),
        6,
    );
    assert_eq!(
        summary.gpu_dma_scheduler_recorded_batch_idle_worker_ticks(),
        2,
    );
    assert_eq!(
        summary
            .gpu_dma_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(4, 6).unwrap(),
    );

    assert_eq!(
        summary.accelerator_dma_scheduler_recorded_batch_worker_ticks(),
        3,
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks(),
        6,
    );
    assert_eq!(
        summary.accelerator_dma_scheduler_recorded_batch_idle_worker_ticks(),
        3,
    );
    assert_eq!(
        summary
            .accelerator_dma_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(3, 6).unwrap(),
    );

    assert_eq!(summary.dma_scheduler_recorded_batch_worker_ticks(), 7);
    assert_eq!(
        summary.dma_scheduler_recorded_batch_worker_capacity_ticks(),
        12,
    );
    assert_eq!(summary.dma_scheduler_recorded_batch_idle_worker_ticks(), 5);
    assert_eq!(
        summary.dma_scheduler_recorded_batch_worker_slot_tick_summaries(),
        vec![(0, 5, 0), (1, 2, 3), (2, 0, 2)],
    );
    assert_eq!(
        summary
            .dma_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(7, 12).unwrap(),
    );

    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_ticks(),
        19,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        28,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_idle_worker_ticks(),
        9,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
        vec![(0, 13, 0), (1, 6, 7), (2, 0, 2)],
    );
    assert_eq!(
        summary
            .full_system_parallel_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(19, 28).unwrap(),
    );
}

#[test]
fn workload_parallel_execution_summary_uses_scoped_recorded_capacity_when_explicit_full_system_is_weaker(
) {
    let cpu = partition(0);
    let cache = partition(1);
    let gpu = partition(2);
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [cpu],
            1,
        )])
        .with_parallel_scheduler_recorded_batch_worker_capacity_ticks(8)
        .with_parallel_scheduler_recorded_batch_worker_slot_tick_summaries([(0, 4, 0), (1, 0, 4)])
        .with_gpu_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            4,
            8,
            [gpu, cache],
            2,
        )])
        .with_gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(12)
        .with_gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries([
            (0, 4, 0),
            (1, 4, 0),
            (2, 0, 4),
        ])
        .with_full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(12);

    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_ticks(),
        12,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        20,
    );
    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_idle_worker_ticks(),
        8,
    );
    assert_eq!(
        summary
            .full_system_parallel_scheduler_recorded_batch_utilization_ratio()
            .unwrap(),
        ParallelBatchUtilizationRatio::new(12, 20).unwrap(),
    );
}

#[test]
fn workload_parallel_execution_summary_uses_scoped_recorded_slots_when_explicit_full_system_is_weaker(
) {
    let cpu = partition(0);
    let cache = partition(1);
    let gpu = partition(2);
    let summary = WorkloadParallelExecutionSummary::default()
        .with_parallel_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::Scheduler,
            0,
            4,
            [cpu],
            1,
        )])
        .with_parallel_scheduler_recorded_batch_worker_capacity_ticks(8)
        .with_parallel_scheduler_recorded_batch_worker_slot_tick_summaries([(0, 4, 0), (1, 0, 4)])
        .with_gpu_dma_scheduler_batch_timeline([timeline_record(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            4,
            8,
            [gpu, cache],
            2,
        )])
        .with_gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(12)
        .with_gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries([
            (0, 4, 0),
            (1, 4, 0),
            (2, 0, 4),
        ])
        .with_full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(20)
        .with_full_system_parallel_scheduler_recorded_batch_worker_slot_tick_summaries([(0, 4, 0)]);

    assert_eq!(
        summary.full_system_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
        vec![(0, 8, 0), (1, 4, 4), (2, 0, 4)],
    );
}
