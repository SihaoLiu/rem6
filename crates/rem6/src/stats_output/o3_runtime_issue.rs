use rem6_cpu::{O3LiveIssueTelemetry, O3RuntimeStats};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::Rem6CliError;

pub(super) fn emit_o3_runtime_issue_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    o3: O3RuntimeStats,
    queue: O3LiveIssueTelemetry,
) -> Result<(), Rem6CliError> {
    for (name, unit, value) in [
        ("issue_cycles", "Cycle", o3.issue_cycles()),
        ("issued_rows", "Count", o3.issued_rows()),
        (
            "resource_blocked_row_cycles",
            "Cycle",
            o3.resource_blocked_row_cycles(),
        ),
        (
            "dependency_blocked_row_cycles",
            "Cycle",
            o3.dependency_blocked_row_cycles(),
        ),
        ("max_rows_per_cycle", "Count", o3.max_rows_per_cycle()),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for (name, value) in [
        ("enqueued_rows", queue.enqueued_rows()),
        ("service_turns", queue.service_turns()),
        ("wake_requests", queue.wake_requests()),
        ("current_occupancy", queue.current_occupancy()),
        ("peak_occupancy", queue.peak_occupancy()),
        (
            "issued_by_class.scalar_integer",
            queue.scalar_integer_issued_rows(),
        ),
        (
            "issued_by_class.integer_mul_div",
            queue.integer_mul_div_issued_rows(),
        ),
        ("issued_by_class.memory_agu", queue.memory_agu_issued_rows()),
        ("issued_by_class.control", queue.control_issued_rows()),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.issue_queue.{name}"),
            "Count",
            StatResetPolicy::Resettable,
            value,
        )?;
    }
    Ok(())
}
