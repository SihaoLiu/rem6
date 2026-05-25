use crate::{WorkloadError, WorkloadReplayPlan, WorkloadResult};

pub(crate) fn verify_expected_parallel_scheduler_idle_bounds(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_bounds = plan.expected_parallel_scheduler_idle_bounds();
    if expected_bounds.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_bounds[0];
        return Err(WorkloadError::MissingParallelSchedulerIdleSummary {
            scope: expected.scope(),
            maximum_empty_epoch_count: expected.maximum_empty_epoch_count(),
        });
    };

    for expected in expected_bounds {
        let actual_empty_epoch_count = expected.actual_empty_epoch_count(summary);
        if actual_empty_epoch_count > expected.maximum_empty_epoch_count() {
            return Err(WorkloadError::ExpectedParallelSchedulerIdleAboveMaximum {
                scope: expected.scope(),
                maximum_empty_epoch_count: expected.maximum_empty_epoch_count(),
                actual_empty_epoch_count,
            });
        }
    }
    Ok(())
}
