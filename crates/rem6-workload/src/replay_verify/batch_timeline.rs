use crate::{
    parallel_batch_timeline_expectation::actual_parallel_batch_timeline_records, WorkloadError,
    WorkloadParallelBatchPartitionScope, WorkloadParallelBatchTimelineScope,
    WorkloadParallelBatchWorkerScope, WorkloadParallelExecutionSummary,
    WorkloadParallelSchedulerScope,
};

const SCOPED_PLANNED_BATCH_TIMELINE_SCOPES: [WorkloadParallelBatchTimelineScope; 4] = [
    WorkloadParallelBatchTimelineScope::PlannedScheduler,
    WorkloadParallelBatchTimelineScope::PlannedDataCacheScheduler,
    WorkloadParallelBatchTimelineScope::PlannedGpuDmaScheduler,
    WorkloadParallelBatchTimelineScope::PlannedAcceleratorDmaScheduler,
];

const SCOPED_BATCH_TIMELINE_SCOPES: [WorkloadParallelBatchTimelineScope; 4] = [
    WorkloadParallelBatchTimelineScope::Scheduler,
    WorkloadParallelBatchTimelineScope::DataCacheScheduler,
    WorkloadParallelBatchTimelineScope::GpuDmaScheduler,
    WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler,
];

pub(crate) fn validate_scheduler_scope_batch_timeline_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
) -> Result<(), WorkloadError> {
    validate_batch_timeline_scope_evidence(summary, batch_timeline_scope_for_scheduler_scope(scope))
}

pub(crate) fn validate_worker_scope_batch_timeline_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
) -> Result<(), WorkloadError> {
    validate_batch_timeline_scope_evidence(summary, batch_timeline_scope_for_worker_scope(scope))
}

pub(crate) fn validate_partition_scope_batch_timeline_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchPartitionScope,
) -> Result<(), WorkloadError> {
    validate_batch_timeline_scope_evidence(summary, batch_timeline_scope_for_partition_scope(scope))
}

pub(crate) fn validate_planned_full_system_batch_timeline_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    validate_full_system_batch_timeline_merge_summary_for_scopes(
        summary,
        WorkloadParallelBatchTimelineScope::PlannedFullSystem,
        SCOPED_PLANNED_BATCH_TIMELINE_SCOPES,
    )
}

pub(crate) fn validate_full_system_batch_timeline_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    validate_full_system_batch_timeline_merge_summary_for_scopes(
        summary,
        WorkloadParallelBatchTimelineScope::FullSystem,
        SCOPED_BATCH_TIMELINE_SCOPES,
    )
}

fn validate_full_system_batch_timeline_merge_summary_for_scopes(
    summary: &WorkloadParallelExecutionSummary,
    full_system_scope: WorkloadParallelBatchTimelineScope,
    scoped_timeline_scopes: [WorkloadParallelBatchTimelineScope; 4],
) -> Result<(), WorkloadError> {
    validate_batch_timeline_records_for_scope(summary, full_system_scope)?;
    let merged =
        full_system_batch_timeline_records_for_merge_validation(summary, full_system_scope);
    for scope in scoped_timeline_scopes {
        validate_batch_timeline_records_for_scope(summary, scope)?;
        for scoped in actual_parallel_batch_timeline_records(scope, summary) {
            if !merged.iter().any(|record| record == &scoped) {
                return Err(WorkloadError::InvalidParallelBatchTimelineMergeSummary {
                    scope: full_system_scope,
                    batch_scope: scoped.scope(),
                    start_tick: scoped.start_tick(),
                    horizon: scoped.horizon(),
                    partitions: scoped
                        .partitions()
                        .iter()
                        .map(|partition| partition.index())
                        .collect(),
                    worker_count: scoped.worker_count(),
                });
            }
        }
    }
    Ok(())
}

fn full_system_batch_timeline_records_for_merge_validation(
    summary: &WorkloadParallelExecutionSummary,
    full_system_scope: WorkloadParallelBatchTimelineScope,
) -> Vec<crate::WorkloadParallelBatchTimelineRecord> {
    match full_system_scope {
        WorkloadParallelBatchTimelineScope::FullSystem
            if !summary
                .explicit_full_system_parallel_scheduler_batch_timeline()
                .is_empty() =>
        {
            summary
                .explicit_full_system_parallel_scheduler_batch_timeline()
                .to_vec()
        }
        _ => actual_parallel_batch_timeline_records(full_system_scope, summary),
    }
}

fn validate_batch_timeline_scope_evidence(
    summary: &WorkloadParallelExecutionSummary,
    timeline_scope: WorkloadParallelBatchTimelineScope,
) -> Result<(), WorkloadError> {
    validate_batch_timeline_records_for_scope(summary, timeline_scope)?;
    if timeline_scope == WorkloadParallelBatchTimelineScope::FullSystem {
        validate_full_system_batch_timeline_merge_summary(summary)?;
    }
    if timeline_scope == WorkloadParallelBatchTimelineScope::PlannedFullSystem {
        validate_planned_full_system_batch_timeline_merge_summary(summary)?;
    }
    Ok(())
}

fn validate_batch_timeline_records_for_scope(
    summary: &WorkloadParallelExecutionSummary,
    timeline_scope: WorkloadParallelBatchTimelineScope,
) -> Result<(), WorkloadError> {
    let mut seen = Vec::new();
    for record in actual_parallel_batch_timeline_records(timeline_scope, summary) {
        if record.is_empty() || seen.iter().any(|seen_record| seen_record == &record) {
            return Err(unexpected_batch_timeline_record(timeline_scope, &record));
        }
        seen.push(record);
    }
    Ok(())
}

fn unexpected_batch_timeline_record(
    timeline_scope: WorkloadParallelBatchTimelineScope,
    record: &crate::WorkloadParallelBatchTimelineRecord,
) -> WorkloadError {
    WorkloadError::UnexpectedParallelBatchTimelineRecord {
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
    }
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
        WorkloadParallelBatchWorkerScope::DmaScheduler => {
            WorkloadParallelBatchTimelineScope::DmaScheduler
        }
        WorkloadParallelBatchWorkerScope::FullSystem => {
            WorkloadParallelBatchTimelineScope::FullSystem
        }
        WorkloadParallelBatchWorkerScope::PlannedScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedScheduler
        }
        WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedDataCacheScheduler
        }
        WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedGpuDmaScheduler
        }
        WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedAcceleratorDmaScheduler
        }
        WorkloadParallelBatchWorkerScope::PlannedDmaScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedDmaScheduler
        }
        WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
            WorkloadParallelBatchTimelineScope::PlannedFullSystem
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
        WorkloadParallelBatchPartitionScope::DmaScheduler => {
            WorkloadParallelBatchTimelineScope::DmaScheduler
        }
        WorkloadParallelBatchPartitionScope::FullSystem => {
            WorkloadParallelBatchTimelineScope::FullSystem
        }
        WorkloadParallelBatchPartitionScope::PlannedScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedScheduler
        }
        WorkloadParallelBatchPartitionScope::PlannedDataCacheScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedDataCacheScheduler
        }
        WorkloadParallelBatchPartitionScope::PlannedGpuDmaScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedGpuDmaScheduler
        }
        WorkloadParallelBatchPartitionScope::PlannedAcceleratorDmaScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedAcceleratorDmaScheduler
        }
        WorkloadParallelBatchPartitionScope::PlannedDmaScheduler => {
            WorkloadParallelBatchTimelineScope::PlannedDmaScheduler
        }
        WorkloadParallelBatchPartitionScope::PlannedFullSystem => {
            WorkloadParallelBatchTimelineScope::PlannedFullSystem
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
        WorkloadParallelSchedulerScope::DmaScheduler => {
            WorkloadParallelBatchTimelineScope::DmaScheduler
        }
        WorkloadParallelSchedulerScope::FullSystem => {
            WorkloadParallelBatchTimelineScope::FullSystem
        }
    }
}
