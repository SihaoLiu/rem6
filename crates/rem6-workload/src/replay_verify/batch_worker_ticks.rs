use rem6_kernel::ParallelBatchUtilizationRatio;

use crate::{
    WorkloadError, WorkloadPlannedParallelBatchIdleExpectationError,
    WorkloadPlannedParallelBatchUtilizationExpectationError,
    WorkloadPlannedParallelBatchWorkerSlotExpectationError, WorkloadReplayPlan, WorkloadResult,
};

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

pub(crate) fn verify_expected_planned_parallel_batch_utilization(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_utilization = plan.expected_planned_parallel_batch_utilization();
    if expected_utilization.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_utilization[0];
        return Err(WorkloadError::PlannedParallelBatchUtilizationExpectation(
            WorkloadPlannedParallelBatchUtilizationExpectationError::MissingSummary {
                scope: expected.scope(),
                minimum_numerator: expected.minimum_numerator(),
                minimum_denominator: expected.minimum_denominator(),
            },
        ));
    };

    for expected in expected_utilization {
        let minimum = ParallelBatchUtilizationRatio::new(
            expected.minimum_numerator(),
            expected.minimum_denominator(),
        )
        .expect("planned utilization expectation rejects zero denominators");
        let Some(actual) = expected.actual_utilization(summary) else {
            return Err(WorkloadError::PlannedParallelBatchUtilizationExpectation(
                WorkloadPlannedParallelBatchUtilizationExpectationError::MissingSummary {
                    scope: expected.scope(),
                    minimum_numerator: expected.minimum_numerator(),
                    minimum_denominator: expected.minimum_denominator(),
                },
            ));
        };
        if !actual.meets_or_exceeds(minimum) {
            return Err(WorkloadError::PlannedParallelBatchUtilizationExpectation(
                WorkloadPlannedParallelBatchUtilizationExpectationError::BelowMinimum {
                    scope: expected.scope(),
                    minimum_numerator: expected.minimum_numerator(),
                    minimum_denominator: expected.minimum_denominator(),
                    actual_numerator: actual.numerator(),
                    actual_denominator: actual.denominator(),
                },
            ));
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_planned_parallel_batch_idle_worker_ticks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_idle = plan.expected_planned_parallel_batch_idle_worker_ticks();
    if expected_idle.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_idle[0];
        return Err(WorkloadError::PlannedParallelBatchIdleExpectation(
            WorkloadPlannedParallelBatchIdleExpectationError::MissingSummary {
                scope: expected.scope(),
                maximum_idle_worker_ticks: expected.maximum_idle_worker_ticks(),
            },
        ));
    };

    for expected in expected_idle {
        let Some(actual_idle_worker_ticks) = expected.actual_idle_worker_ticks(summary) else {
            return Err(WorkloadError::PlannedParallelBatchIdleExpectation(
                WorkloadPlannedParallelBatchIdleExpectationError::MissingSummary {
                    scope: expected.scope(),
                    maximum_idle_worker_ticks: expected.maximum_idle_worker_ticks(),
                },
            ));
        };
        if actual_idle_worker_ticks > expected.maximum_idle_worker_ticks() {
            return Err(WorkloadError::PlannedParallelBatchIdleExpectation(
                WorkloadPlannedParallelBatchIdleExpectationError::AboveMaximum {
                    scope: expected.scope(),
                    maximum_idle_worker_ticks: expected.maximum_idle_worker_ticks(),
                    actual_idle_worker_ticks,
                },
            ));
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_planned_parallel_batch_worker_slot_ticks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_slots = plan.expected_planned_parallel_batch_worker_slot_ticks();
    if expected_slots.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_slots[0];
        return Err(WorkloadError::PlannedParallelBatchWorkerSlotExpectation(
            WorkloadPlannedParallelBatchWorkerSlotExpectationError::MissingSummary {
                scope: expected.scope(),
                worker_slot: expected.worker_slot(),
                minimum_active_ticks: expected.minimum_active_ticks(),
                maximum_idle_ticks: expected.maximum_idle_ticks(),
            },
        ));
    };

    for expected in expected_slots {
        let Some((actual_active_ticks, actual_idle_ticks)) = expected.actual_slot_ticks(summary)
        else {
            return Err(WorkloadError::PlannedParallelBatchWorkerSlotExpectation(
                WorkloadPlannedParallelBatchWorkerSlotExpectationError::MissingSummary {
                    scope: expected.scope(),
                    worker_slot: expected.worker_slot(),
                    minimum_active_ticks: expected.minimum_active_ticks(),
                    maximum_idle_ticks: expected.maximum_idle_ticks(),
                },
            ));
        };
        if actual_active_ticks < expected.minimum_active_ticks() {
            return Err(WorkloadError::PlannedParallelBatchWorkerSlotExpectation(
                WorkloadPlannedParallelBatchWorkerSlotExpectationError::BelowMinimumActive {
                    scope: expected.scope(),
                    worker_slot: expected.worker_slot(),
                    minimum_active_ticks: expected.minimum_active_ticks(),
                    actual_active_ticks,
                },
            ));
        }
        if actual_idle_ticks > expected.maximum_idle_ticks() {
            return Err(WorkloadError::PlannedParallelBatchWorkerSlotExpectation(
                WorkloadPlannedParallelBatchWorkerSlotExpectationError::AboveMaximumIdle {
                    scope: expected.scope(),
                    worker_slot: expected.worker_slot(),
                    maximum_idle_ticks: expected.maximum_idle_ticks(),
                    actual_idle_ticks,
                },
            ));
        }
    }
    Ok(())
}
