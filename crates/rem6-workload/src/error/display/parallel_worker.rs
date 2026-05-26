use std::fmt;

use super::super::WorkloadError;

pub(super) fn format_parallel_worker_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::ZeroExpectedParallelWorkerCount { scope } => write!(
            formatter,
            "expected {} worker use must require a positive maximum worker count",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelWorkerCount {
            scope,
            minimum_max_workers,
        } => write!(
            formatter,
            "expected {} worker use must require at least 2 workers, got {minimum_max_workers}",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelWorkerUse { scope } => write!(
            formatter,
            "expected {} worker use is already declared",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelWorkerActivity { scope } => write!(
            formatter,
            "expected {} worker activity must require a positive total worker count",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelWorkerActivity { scope } => write!(
            formatter,
            "expected {} worker activity is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelWorkerSummary {
            scope,
            minimum_max_workers,
        } => write!(
            formatter,
            "missing parallel summary for expected {} worker use with at least {minimum_max_workers} workers",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelWorkerCountBelowMinimum {
            scope,
            minimum_max_workers,
            actual_max_workers,
        } => write!(
            formatter,
            "expected {} worker use to reach at least {minimum_max_workers} workers, got {actual_max_workers}",
            scope.as_str()
        ),
        WorkloadError::MissingParallelWorkerActivitySummary {
            scope,
            minimum_total_workers,
        } => write!(
            formatter,
            "missing parallel summary for expected {} worker activity with at least {minimum_total_workers} total workers",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelWorkerActivityBelowMinimum {
            scope,
            minimum_total_workers,
            actual_total_workers,
        } => write!(
            formatter,
            "expected {} worker activity to reach at least {minimum_total_workers} total workers, got {actual_total_workers}",
            scope.as_str()
        ),
        _ => unreachable!("parallel worker display called for non-worker error"),
    }
}
