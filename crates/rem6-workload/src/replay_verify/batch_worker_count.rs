use crate::parallel_batch::{
    parallel_batch_count_at_or_above, parallel_batch_count_for_worker_count,
};
use rem6_kernel::Tick;

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

pub(crate) fn validate_worker_scope_batch_worker_tick_bucket_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
    worker_count: usize,
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchWorkerScope::FullSystem {
        return Ok(());
    }
    validate_full_system_batch_worker_tick_bucket_merge_summary(summary, worker_count)
}

pub(crate) fn validate_worker_scope_batch_worker_tick_activity_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchWorkerScope::FullSystem {
        return Ok(());
    }
    validate_full_system_batch_worker_tick_activity_merge_summary(summary, minimum_worker_count)
}

pub(crate) fn validate_worker_scope_batch_worker_tick_streak_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchWorkerScope::FullSystem {
        return Ok(());
    }
    validate_full_system_batch_worker_tick_streak_merge_summary(summary, minimum_worker_count)
}

pub(crate) fn validate_worker_scope_batch_worker_ticks_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchWorkerScope,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelBatchWorkerScope::FullSystem {
        return Ok(());
    }
    validate_full_system_batch_worker_ticks_merge_summary(summary, minimum_worker_count)
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
    validate_raw_full_system_batch_worker_counts(summary)?;
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

fn validate_raw_full_system_batch_worker_counts(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    let mut seen_worker_counts = Vec::new();
    for count in summary.raw_full_system_parallel_scheduler_batch_worker_counts() {
        if count.is_empty() || seen_worker_counts.contains(&count.worker_count()) {
            return Err(WorkloadError::UnexpectedParallelBatchWorkerCount {
                scope: WorkloadParallelBatchWorkerScope::FullSystem,
                worker_count: count.worker_count(),
                batch_count: count.batch_count(),
            });
        }
        seen_worker_counts.push(count.worker_count());
    }
    Ok(())
}

fn validate_full_system_batch_worker_tick_bucket_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
    worker_count: usize,
) -> Result<(), WorkloadError> {
    let merged_summaries =
        summary.explicit_full_system_parallel_scheduler_batch_worker_count_tick_summaries();
    if merged_summaries.is_empty() {
        return Ok(());
    }
    let lower_bound_ticks = batch_ticks_for_worker_count(
        &summary.scoped_full_system_parallel_scheduler_batch_worker_count_tick_summaries(),
        worker_count,
    );
    let actual_ticks = batch_ticks_for_worker_count(&merged_summaries, worker_count);
    if actual_ticks < lower_bound_ticks {
        return Err(
            WorkloadError::ExpectedParallelBatchWorkerTickBucketBelowMinimum {
                scope: WorkloadParallelBatchWorkerScope::FullSystem,
                worker_count,
                minimum_ticks: lower_bound_ticks,
                actual_ticks,
            },
        );
    }
    Ok(())
}

fn validate_full_system_batch_worker_tick_activity_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    let merged_summaries =
        summary.explicit_full_system_parallel_scheduler_batch_worker_count_tick_summaries();
    if merged_summaries.is_empty() {
        return Ok(());
    }
    let lower_bound_ticks = batch_ticks_at_or_above(
        &summary.scoped_full_system_parallel_scheduler_batch_worker_count_tick_summaries(),
        minimum_worker_count,
    );
    let actual_ticks = batch_ticks_at_or_above(&merged_summaries, minimum_worker_count);
    if actual_ticks < lower_bound_ticks {
        return Err(
            WorkloadError::ExpectedParallelBatchWorkerTickActivityBelowMinimum {
                scope: WorkloadParallelBatchWorkerScope::FullSystem,
                minimum_worker_count,
                minimum_ticks: lower_bound_ticks,
                actual_ticks,
            },
        );
    }
    Ok(())
}

fn validate_full_system_batch_worker_tick_streak_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    let merged_summaries =
        summary.explicit_full_system_parallel_scheduler_batch_worker_tick_streak_summaries();
    if merged_summaries.is_empty() {
        return Ok(());
    }
    let lower_bound_ticks = batch_tick_streak_at_or_above(
        &summary.scoped_full_system_parallel_scheduler_batch_worker_tick_streak_summaries(),
        minimum_worker_count,
    );
    let actual_ticks = batch_tick_streak_at_or_above(&merged_summaries, minimum_worker_count);
    if actual_ticks < lower_bound_ticks {
        return Err(
            WorkloadError::ExpectedParallelBatchWorkerTickStreakBelowMinimum {
                scope: WorkloadParallelBatchWorkerScope::FullSystem,
                minimum_worker_count,
                minimum_consecutive_ticks: lower_bound_ticks,
                actual_consecutive_ticks: actual_ticks,
            },
        );
    }
    Ok(())
}

fn validate_full_system_batch_worker_ticks_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
    minimum_worker_count: usize,
) -> Result<(), WorkloadError> {
    let merged_summaries =
        summary.explicit_full_system_parallel_scheduler_batch_worker_count_tick_summaries();
    if merged_summaries.is_empty() {
        return Ok(());
    }
    let lower_bound_ticks = batch_worker_ticks_at_or_above(
        &summary.scoped_full_system_parallel_scheduler_batch_worker_count_tick_summaries(),
        minimum_worker_count,
    );
    let actual_ticks = batch_worker_ticks_at_or_above(&merged_summaries, minimum_worker_count);
    if actual_ticks < lower_bound_ticks {
        return Err(
            WorkloadError::ExpectedParallelBatchWorkerTicksBelowMinimum {
                scope: WorkloadParallelBatchWorkerScope::FullSystem,
                minimum_worker_count,
                minimum_worker_ticks: lower_bound_ticks,
                actual_worker_ticks: actual_ticks,
            },
        );
    }
    Ok(())
}

fn batch_ticks_for_worker_count(summaries: &[(usize, Tick)], worker_count: usize) -> Tick {
    summaries
        .iter()
        .filter(|(count, _)| *count == worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

fn batch_ticks_at_or_above(summaries: &[(usize, Tick)], minimum_worker_count: usize) -> Tick {
    summaries
        .iter()
        .filter(|(count, _)| *count >= minimum_worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

fn batch_worker_ticks_at_or_above(
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    summaries
        .iter()
        .filter(|(count, _)| *count >= minimum_worker_count)
        .map(|(count, ticks)| ticks.saturating_mul(*count as Tick))
        .fold(0, Tick::saturating_add)
}

fn batch_tick_streak_at_or_above(summaries: &[(usize, Tick)], minimum_worker_count: usize) -> Tick {
    summaries
        .iter()
        .filter(|(worker_count, _)| *worker_count >= minimum_worker_count)
        .map(|(_, ticks)| *ticks)
        .max()
        .unwrap_or(0)
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
