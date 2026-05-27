use rem6_kernel::{ParallelPartitionActivity, PartitionId};

use crate::{
    parallel_batch::{
        collect_parallel_batch_partition_sets_from_timeline,
        collect_parallel_batch_partition_streaks_from_timeline,
        parallel_batch_active_partition_count, parallel_batch_partition_activity_for_partition,
        parallel_batch_streak_activity_for_partition,
    },
    result_partition_activity::merge_parallel_partition_activity_evidence_options,
    WorkloadParallelBatchPartitionScope, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerScope, WorkloadParallelExecutionSummary,
};

pub(super) fn planned_batch_max_workers(
    scope: WorkloadParallelBatchWorkerScope,
    summary: &WorkloadParallelExecutionSummary,
) -> usize {
    planned_batch_worker_timeline(scope, summary)
        .into_iter()
        .filter(WorkloadParallelBatchTimelineRecord::is_parallel_evidence)
        .map(|record| record.worker_count())
        .max()
        .unwrap_or(0)
}

pub(super) fn planned_batch_total_workers(
    scope: WorkloadParallelBatchWorkerScope,
    summary: &WorkloadParallelExecutionSummary,
) -> usize {
    planned_batch_worker_timeline(scope, summary)
        .into_iter()
        .filter(WorkloadParallelBatchTimelineRecord::is_parallel_evidence)
        .map(|record| record.worker_count())
        .sum()
}

pub(super) fn planned_batch_count_at_or_above(
    scope: WorkloadParallelBatchWorkerScope,
    summary: &WorkloadParallelExecutionSummary,
    minimum_worker_count: usize,
) -> usize {
    planned_batch_worker_timeline(scope, summary)
        .into_iter()
        .filter(WorkloadParallelBatchTimelineRecord::is_parallel_evidence)
        .filter(|record| record.worker_count() >= minimum_worker_count)
        .count()
}

pub(super) fn planned_partition_activity(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
    partition: PartitionId,
) -> Option<ParallelPartitionActivity> {
    let sets = planned_partition_sets(scope, summary);
    let streaks = planned_partition_streaks(scope, summary);
    merge_parallel_partition_activity_evidence_options(
        parallel_batch_partition_activity_for_partition(&sets, partition),
        parallel_batch_streak_activity_for_partition(&streaks, partition),
    )
}

pub(super) fn planned_active_partition_count(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
) -> usize {
    let sets = planned_partition_sets(scope, summary);
    let streaks = planned_partition_streaks(scope, summary);
    parallel_batch_active_partition_count(&sets, &streaks)
}

fn planned_batch_worker_timeline(
    scope: WorkloadParallelBatchWorkerScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    match scope {
        WorkloadParallelBatchWorkerScope::PlannedScheduler => {
            summary.parallel_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler => summary
            .data_cache_parallel_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler => {
            summary.gpu_dma_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler => summary
            .accelerator_dma_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchWorkerScope::PlannedDmaScheduler => {
            summary.dma_scheduler_planned_batch_timeline()
        }
        WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
            summary.full_system_parallel_scheduler_planned_batch_timeline()
        }
        _ => Vec::new(),
    }
}

fn planned_partition_sets(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<WorkloadParallelBatchPartitionSet> {
    let timeline = planned_partition_batch_timeline(scope, summary);
    collect_parallel_batch_partition_sets_from_timeline(&timeline)
}

fn planned_partition_streaks(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<WorkloadParallelBatchPartitionStreak> {
    let timeline = planned_partition_batch_timeline(scope, summary);
    collect_parallel_batch_partition_streaks_from_timeline(&timeline)
}

fn planned_partition_batch_timeline(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    match scope {
        WorkloadParallelBatchPartitionScope::PlannedScheduler => {
            summary.parallel_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchPartitionScope::PlannedDataCacheScheduler => summary
            .data_cache_parallel_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchPartitionScope::PlannedGpuDmaScheduler => {
            summary.gpu_dma_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchPartitionScope::PlannedAcceleratorDmaScheduler => summary
            .accelerator_dma_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchPartitionScope::PlannedDmaScheduler => {
            summary.dma_scheduler_planned_batch_timeline()
        }
        WorkloadParallelBatchPartitionScope::PlannedFullSystem => {
            summary.full_system_parallel_scheduler_planned_batch_timeline()
        }
        _ => Vec::new(),
    }
}
