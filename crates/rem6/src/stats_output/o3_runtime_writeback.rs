use rem6_cpu::O3RuntimeStats;
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::Rem6CliError;

pub(super) fn emit_o3_runtime_writeback_port_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    o3: O3RuntimeStats,
) -> Result<(), Rem6CliError> {
    for (name, unit, value) in [
        ("cycles", "Cycle", o3.writeback_port_cycles()),
        ("admitted_rows", "Count", o3.writeback_port_admitted_rows()),
        ("deferred_rows", "Count", o3.writeback_port_deferred_rows()),
        (
            "deferred_row_cycles",
            "Cycle",
            o3.writeback_port_deferred_row_cycles(),
        ),
        (
            "max_ready_rows_per_cycle",
            "Count",
            o3.writeback_port_max_ready_rows_per_cycle(),
        ),
        (
            "max_deferred_rows",
            "Count",
            o3.writeback_port_max_deferred_rows(),
        ),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.writeback_port.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}
