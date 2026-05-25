use crate::{WorkloadError, WorkloadReplayPlan, WorkloadResult};

pub(crate) fn verify_expected_clean_parallel_diagnostics(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_diagnostics = plan.expected_clean_parallel_diagnostics();
    if expected_diagnostics.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_diagnostics[0];
        return Err(WorkloadError::MissingParallelDiagnosticSummary {
            scope: expected.scope(),
        });
    };

    for expected in expected_diagnostics {
        let (wait_for_edge_count, deadlock_diagnostic_count) = expected.actual_counts(summary);
        if wait_for_edge_count != 0 || deadlock_diagnostic_count != 0 {
            return Err(WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
                scope: expected.scope(),
                wait_for_edge_count,
                deadlock_diagnostic_count,
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_data_cache_protocol_run_counts(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_counts = plan.expected_data_cache_protocol_run_counts();
    if expected_counts.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_counts[0];
        return Err(WorkloadError::MissingDataCacheProtocolSummary {
            protocol: expected.protocol(),
            minimum_run_count: expected.minimum_run_count(),
        });
    };

    for expected in expected_counts {
        let actual_run_count = expected.actual_run_count(summary);
        if actual_run_count < expected.minimum_run_count() {
            return Err(
                WorkloadError::ExpectedDataCacheProtocolRunCountBelowMinimum {
                    protocol: expected.protocol(),
                    minimum_run_count: expected.minimum_run_count(),
                    actual_run_count,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_scheduler_progress(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_progress = plan.expected_parallel_scheduler_progress();
    if expected_progress.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_progress[0];
        return Err(WorkloadError::MissingParallelSchedulerProgressSummary {
            scope: expected.scope(),
            minimum_epoch_count: expected.minimum_epoch_count(),
            minimum_dispatch_count: expected.minimum_dispatch_count(),
        });
    };

    for expected in expected_progress {
        let (actual_epoch_count, actual_dispatch_count) = expected.actual_counts(summary);
        if actual_epoch_count < expected.minimum_epoch_count()
            || actual_dispatch_count < expected.minimum_dispatch_count()
        {
            return Err(
                WorkloadError::ExpectedParallelSchedulerProgressBelowMinimum {
                    scope: expected.scope(),
                    minimum_epoch_count: expected.minimum_epoch_count(),
                    actual_epoch_count,
                    minimum_dispatch_count: expected.minimum_dispatch_count(),
                    actual_dispatch_count,
                },
            );
        }
    }
    Ok(())
}

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
