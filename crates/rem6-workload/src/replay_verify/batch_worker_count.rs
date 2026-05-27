use crate::parallel_batch::{
    parallel_batch_count_at_or_above, parallel_batch_count_for_worker_count,
};
use crate::{
    WorkloadError, WorkloadParallelBatchWorkerScope, WorkloadParallelExecutionSummary,
    WorkloadParallelRemoteFlowScope,
};

pub(crate) fn validate_worker_scope_batch_activity_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchWorkerScope::FullSystem {
        return Ok(());
    }
    validate_full_system_batch_activity_merge_summary(summary, minimum_worker_count)
}

pub(crate) fn validate_worker_scope_batch_worker_count_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
    worker_count: usize,
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchWorkerScope::FullSystem {
        return Ok(());
    }
    validate_full_system_batch_worker_count_merge_summary(summary, worker_count)
}

fn validate_full_system_batch_worker_count_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
    worker_count: usize,
) -> Result<(), WorkloadError> {
    let merged_counts = summary.explicit_full_system_parallel_scheduler_batch_worker_counts();
    if merged_counts.is_empty() {
        return Ok(());
    }
    let lower_bound_batch_count = parallel_batch_count_for_worker_count(
        &summary.scoped_full_system_parallel_scheduler_batch_worker_counts(),
        worker_count,
    );
    let actual_batch_count = parallel_batch_count_for_worker_count(&merged_counts, worker_count);
    if actual_batch_count < lower_bound_batch_count {
        return Err(WorkloadError::ParallelBatchWorkerCountBelowMinimum {
            scope: WorkloadParallelRemoteFlowScope::FullSystem,
            worker_count,
            minimum_batch_count: lower_bound_batch_count,
            actual_batch_count,
        });
    }
    Ok(())
}

fn validate_full_system_batch_activity_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    let merged_counts = summary.explicit_full_system_parallel_scheduler_batch_worker_counts();
    if merged_counts.is_empty() {
        return Ok(());
    }
    let lower_bound_batch_count = parallel_batch_count_at_or_above(
        &summary.scoped_full_system_parallel_scheduler_batch_worker_counts(),
        minimum_worker_count,
    );
    let actual_batch_count = parallel_batch_count_at_or_above(&merged_counts, minimum_worker_count);
    if actual_batch_count < lower_bound_batch_count {
        return Err(WorkloadError::ExpectedParallelBatchActivityBelowMinimum {
            scope: WorkloadParallelBatchWorkerScope::FullSystem,
            minimum_worker_count,
            minimum_batch_count: lower_bound_batch_count,
            actual_batch_count,
        });
    }
    Ok(())
}
