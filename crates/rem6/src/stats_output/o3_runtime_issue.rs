use rem6_cpu::O3RuntimeStats;
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::Rem6CliError;

pub(super) fn emit_o3_runtime_issue_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    o3: O3RuntimeStats,
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
    Ok(())
}
