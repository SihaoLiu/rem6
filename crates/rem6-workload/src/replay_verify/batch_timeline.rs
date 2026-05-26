use crate::{
    parallel_batch_timeline_expectation::actual_parallel_batch_timeline_records, WorkloadError,
    WorkloadParallelBatchPartitionScope, WorkloadParallelBatchTimelineScope,
    WorkloadParallelBatchWorkerScope, WorkloadParallelExecutionSummary,
    WorkloadParallelSchedulerScope,
};

pub(crate) fn validate_scheduler_scope_batch_timeline_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
) -> Result<(), WorkloadError> {
    validate_batch_timeline_records_for_scope(
        summary,
        batch_timeline_scope_for_scheduler_scope(scope),
    )
}

pub(crate) fn validate_worker_scope_batch_timeline_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
) -> Result<(), WorkloadError> {
    validate_batch_timeline_records_for_scope(summary, batch_timeline_scope_for_worker_scope(scope))
}

pub(crate) fn validate_partition_scope_batch_timeline_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchPartitionScope,
) -> Result<(), WorkloadError> {
    validate_batch_timeline_records_for_scope(
        summary,
        batch_timeline_scope_for_partition_scope(scope),
    )
}

fn validate_batch_timeline_records_for_scope(
    summary: &WorkloadParallelExecutionSummary,
    timeline_scope: WorkloadParallelBatchTimelineScope,
) -> Result<(), WorkloadError> {
    for record in actual_parallel_batch_timeline_records(timeline_scope, summary) {
        if record.is_empty() {
            return Err(WorkloadError::UnexpectedParallelBatchTimelineRecord {
                scope: timeline_scope,
                batch_scope: record.scope(),
                start_tick: record.start_tick(),
                horizon: record.horizon(),
                partitions: record
                    .partitions()
                    .iter()
                    .map(|partition| partition.index())
                    .collect(),
                worker_count: record.worker_count(),
            });
        }
    }
    Ok(())
}

fn batch_timeline_scope_for_worker_scope(
    scope: WorkloadParallelBatchWorkerScope,
) -> WorkloadParallelBatchTimelineScope {
    match scope {
        WorkloadParallelBatchWorkerScope::Scheduler => {
            WorkloadParallelBatchTimelineScope::Scheduler
        }
        WorkloadParallelBatchWorkerScope::DataCacheScheduler => {
            WorkloadParallelBatchTimelineScope::DataCacheScheduler
        }
        WorkloadParallelBatchWorkerScope::GpuDmaScheduler => {
            WorkloadParallelBatchTimelineScope::GpuDmaScheduler
        }
        WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => {
            WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler
        }
        WorkloadParallelBatchWorkerScope::FullSystem => {
            WorkloadParallelBatchTimelineScope::FullSystem
        }
    }
}

fn batch_timeline_scope_for_partition_scope(
    scope: WorkloadParallelBatchPartitionScope,
) -> WorkloadParallelBatchTimelineScope {
    match scope {
        WorkloadParallelBatchPartitionScope::Scheduler => {
            WorkloadParallelBatchTimelineScope::Scheduler
        }
        WorkloadParallelBatchPartitionScope::DataCacheScheduler => {
            WorkloadParallelBatchTimelineScope::DataCacheScheduler
        }
        WorkloadParallelBatchPartitionScope::GpuDmaScheduler => {
            WorkloadParallelBatchTimelineScope::GpuDmaScheduler
        }
        WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler => {
            WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler
        }
        WorkloadParallelBatchPartitionScope::FullSystem => {
            WorkloadParallelBatchTimelineScope::FullSystem
        }
    }
}

fn batch_timeline_scope_for_scheduler_scope(
    scope: WorkloadParallelSchedulerScope,
) -> WorkloadParallelBatchTimelineScope {
    match scope {
        WorkloadParallelSchedulerScope::Scheduler => WorkloadParallelBatchTimelineScope::Scheduler,
        WorkloadParallelSchedulerScope::DataCacheScheduler => {
            WorkloadParallelBatchTimelineScope::DataCacheScheduler
        }
        WorkloadParallelSchedulerScope::GpuDmaScheduler => {
            WorkloadParallelBatchTimelineScope::GpuDmaScheduler
        }
        WorkloadParallelSchedulerScope::AcceleratorDmaScheduler => {
            WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler
        }
        WorkloadParallelSchedulerScope::FullSystem => {
            WorkloadParallelBatchTimelineScope::FullSystem
        }
    }
}
