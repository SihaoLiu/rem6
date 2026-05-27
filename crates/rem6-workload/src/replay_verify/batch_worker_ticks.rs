use crate::{WorkloadError, WorkloadReplayPlan, WorkloadResult};

use super::{
    validate_worker_scope_batch_timeline_evidence,
    validate_worker_scope_batch_worker_tick_activity_evidence,
    validate_worker_scope_batch_worker_tick_bucket_evidence,
    validate_worker_scope_batch_worker_tick_streak_evidence,
    validate_worker_scope_batch_worker_ticks_evidence,
};

pub(crate) fn verify_expected_parallel_batch_worker_tick_buckets(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_buckets = plan.expected_parallel_batch_worker_tick_buckets();
    if expected_buckets.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_buckets[0];
        return Err(WorkloadError::MissingParallelBatchWorkerTickBucketSummary {
            scope: expected.scope(),
            worker_count: expected.worker_count(),
            minimum_ticks: expected.minimum_ticks(),
        });
    };

    for expected in expected_buckets {
        validate_worker_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_worker_scope_batch_worker_tick_bucket_evidence(
            summary,
            expected.scope(),
            expected.worker_count(),
        )?;
        let actual_ticks = expected.actual_ticks(summary);
        if actual_ticks < expected.minimum_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTickBucketBelowMinimum {
                    scope: expected.scope(),
                    worker_count: expected.worker_count(),
                    minimum_ticks: expected.minimum_ticks(),
                    actual_ticks,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_worker_tick_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_parallel_batch_worker_tick_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_activity[0];
        return Err(
            WorkloadError::MissingParallelBatchWorkerTickActivitySummary {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
                minimum_ticks: expected.minimum_ticks(),
            },
        );
    };

    for expected in expected_activity {
        validate_worker_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_worker_scope_batch_worker_tick_activity_evidence(
            summary,
            expected.scope(),
            expected.minimum_worker_count(),
        )?;
        let actual_ticks = expected.actual_ticks(summary);
        if actual_ticks < expected.minimum_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTickActivityBelowMinimum {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                    minimum_ticks: expected.minimum_ticks(),
                    actual_ticks,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_worker_tick_streaks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_streaks = plan.expected_parallel_batch_worker_tick_streaks();
    if expected_streaks.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_streaks[0];
        return Err(WorkloadError::MissingParallelBatchWorkerTickStreakSummary {
            scope: expected.scope(),
            minimum_worker_count: expected.minimum_worker_count(),
            minimum_consecutive_ticks: expected.minimum_consecutive_ticks(),
        });
    };

    for expected in expected_streaks {
        validate_worker_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_worker_scope_batch_worker_tick_streak_evidence(
            summary,
            expected.scope(),
            expected.minimum_worker_count(),
        )?;
        let actual_consecutive_ticks = expected.actual_consecutive_ticks(summary);
        if actual_consecutive_ticks < expected.minimum_consecutive_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTickStreakBelowMinimum {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                    minimum_consecutive_ticks: expected.minimum_consecutive_ticks(),
                    actual_consecutive_ticks,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_worker_ticks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_worker_ticks = plan.expected_parallel_batch_worker_ticks();
    if expected_worker_ticks.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_worker_ticks[0];
        return Err(WorkloadError::MissingParallelBatchWorkerTicksSummary {
            scope: expected.scope(),
            minimum_worker_count: expected.minimum_worker_count(),
            minimum_worker_ticks: expected.minimum_worker_ticks(),
        });
    };

    for expected in expected_worker_ticks {
        validate_worker_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_worker_scope_batch_worker_ticks_evidence(
            summary,
            expected.scope(),
            expected.minimum_worker_count(),
        )?;
        let actual_worker_ticks = expected.actual_worker_ticks(summary);
        if actual_worker_ticks < expected.minimum_worker_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTicksBelowMinimum {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                    minimum_worker_ticks: expected.minimum_worker_ticks(),
                    actual_worker_ticks,
                },
            );
        }
    }
    Ok(())
}
