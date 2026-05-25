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

pub(crate) fn verify_expected_data_cache_run_attribution(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let Some(expected) = plan.expected_data_cache_run_attribution() else {
        return Ok(());
    };
    let Some(summary) = result.parallel_execution_summary() else {
        return Err(WorkloadError::MissingDataCacheRunAttributionSummary {
            minimum_attributed_run_count: expected.minimum_attributed_run_count(),
            maximum_unattributed_run_count: expected.maximum_unattributed_run_count(),
        });
    };

    let (actual_attributed_run_count, actual_unattributed_run_count) =
        expected.actual_counts(summary);
    if actual_attributed_run_count < expected.minimum_attributed_run_count() {
        return Err(WorkloadError::ExpectedDataCacheRunAttributionBelowMinimum {
            minimum_attributed_run_count: expected.minimum_attributed_run_count(),
            actual_attributed_run_count,
        });
    }
    if actual_unattributed_run_count > expected.maximum_unattributed_run_count() {
        return Err(WorkloadError::ExpectedDataCacheRunAttributionAboveMaximum {
            maximum_unattributed_run_count: expected.maximum_unattributed_run_count(),
            actual_unattributed_run_count,
        });
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

pub(crate) fn verify_expected_parallel_batch_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_parallel_batch_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_activity[0];
        return Err(WorkloadError::MissingParallelBatchActivitySummary {
            scope: expected.scope(),
            minimum_worker_count: expected.minimum_worker_count(),
            minimum_batch_count: expected.minimum_batch_count(),
        });
    };

    for expected in expected_activity {
        let actual_batch_count = expected.actual_batch_count(summary);
        if actual_batch_count < expected.minimum_batch_count() {
            return Err(WorkloadError::ExpectedParallelBatchActivityBelowMinimum {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
                minimum_batch_count: expected.minimum_batch_count(),
                actual_batch_count,
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_partition_sets(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_sets = plan.expected_parallel_batch_partition_sets();
    if expected_sets.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_sets[0];
        return Err(WorkloadError::MissingParallelBatchPartitionSetSummary {
            scope: expected.scope(),
            partitions: expected.partition_indexes(),
            minimum_batch_count: expected.minimum_batch_count(),
        });
    };

    for expected in expected_sets {
        let actual_batch_count = expected.actual_batch_count(summary);
        if actual_batch_count < expected.minimum_batch_count() {
            return Err(
                WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
                    scope: expected.scope(),
                    partitions: expected.partition_indexes(),
                    minimum_batch_count: expected.minimum_batch_count(),
                    actual_batch_count,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_partition_streaks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_streaks = plan.expected_parallel_batch_partition_streaks();
    if expected_streaks.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_streaks[0];
        return Err(WorkloadError::MissingParallelBatchPartitionStreakSummary {
            scope: expected.scope(),
            partitions: expected.partition_indexes(),
            minimum_consecutive_batch_count: expected.minimum_consecutive_batch_count(),
        });
    };

    for expected in expected_streaks {
        let actual_consecutive_batch_count = expected.actual_consecutive_batch_count(summary);
        if actual_consecutive_batch_count < expected.minimum_consecutive_batch_count() {
            return Err(
                WorkloadError::ExpectedParallelBatchPartitionStreakBelowMinimum {
                    scope: expected.scope(),
                    partitions: expected.partition_indexes(),
                    minimum_consecutive_batch_count: expected.minimum_consecutive_batch_count(),
                    actual_consecutive_batch_count,
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
