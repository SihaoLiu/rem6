use std::fmt;

use super::super::WorkloadError;

pub(super) fn format_parallel_frontier_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::ZeroExpectedParallelFrontier {
            scope,
            stage,
            partition,
        } => write!(
            formatter,
            "expected {} {} frontier for partition {partition} must require positive time",
            scope.as_str(),
            stage.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelFrontier {
            scope,
            stage,
            partition,
        } => write!(
            formatter,
            "expected {} {} frontier for partition {partition} is already declared",
            scope.as_str(),
            stage.as_str()
        ),
        WorkloadError::MissingParallelFrontierSummary {
            scope,
            stage,
            partition,
            minimum_now,
            minimum_safe_until,
        } => write!(
            formatter,
            "missing parallel summary for expected {} {} frontier on partition {partition} with now at least {minimum_now} and safe-until at least {minimum_safe_until}",
            scope.as_str(),
            stage.as_str()
        ),
        WorkloadError::ExpectedParallelFrontierBelowMinimum {
            scope,
            stage,
            partition,
            minimum_now,
            actual_now,
            minimum_safe_until,
            actual_safe_until,
        } => {
            let actual_now = actual_now
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "none".to_string());
            let actual_safe_until = actual_safe_until
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "none".to_string());
            write!(
                formatter,
                "expected {} {} frontier on partition {partition} to reach now {minimum_now} and safe-until {minimum_safe_until}, got now {actual_now} and safe-until {actual_safe_until}",
                scope.as_str(),
                stage.as_str()
            )
        }
        WorkloadError::InvalidParallelFrontierSummary {
            scope,
            stage,
            partition,
            now,
            safe_until,
            next_tick,
            pending_events,
        } => {
            let next_tick = next_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "none".to_string());
            write!(
                formatter,
                "invalid {} {} frontier on partition {partition}: now {now}, safe-until {safe_until}, next tick {next_tick}, pending events {pending_events}",
                scope.as_str(),
                stage.as_str()
            )
        }
        _ => unreachable!("parallel frontier display called for unrelated workload error"),
    }
}
