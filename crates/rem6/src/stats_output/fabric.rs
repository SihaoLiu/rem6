use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::{Rem6CliError, Rem6RunFabricSummary};

use super::increment_stat;

pub(super) fn emit_run_fabric_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6RunFabricSummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.active_lanes"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.active_lanes(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.active_virtual_networks"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.active_virtual_networks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.transfers"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.transfers(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.bytes"),
        "Byte",
        StatResetPolicy::Monotonic,
        summary.bytes(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.occupied_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.occupied_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.queue_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.queue_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.max_queue_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.max_queue_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.contended_lanes"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.contended_lanes(),
    )
}
