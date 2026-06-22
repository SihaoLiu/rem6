use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::Rem6CliError;

use super::{increment_stat, stats_snapshot_json, text::stats_snapshot_text, Rem6StatsOutput};

pub(crate) fn multi_run_stats_output(
    runs: u64,
    succeeded: u64,
    failed: u64,
    total_final_tick: u64,
    total_committed_instructions: u64,
    total_scheduled_requests: u64,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.multi_run.runs",
        "Count",
        StatResetPolicy::Constant,
        runs,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.succeeded",
        "Count",
        StatResetPolicy::Monotonic,
        succeeded,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.failed",
        "Count",
        StatResetPolicy::Monotonic,
        failed,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        total_final_tick,
    )?;
    increment_stat(
        &mut stats,
        "sim.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        total_final_tick,
    )?;
    increment_stat(
        &mut stats,
        "simTicks",
        "Tick",
        StatResetPolicy::Monotonic,
        total_final_tick,
    )?;
    increment_stat(
        &mut stats,
        "finalTick",
        "Tick",
        StatResetPolicy::Monotonic,
        total_final_tick,
    )?;
    increment_stat(
        &mut stats,
        "simFreq",
        "Hz",
        StatResetPolicy::Constant,
        super::GEM5_COMPAT_SIM_FREQ_HZ,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.instructions.committed",
        "Count",
        StatResetPolicy::Monotonic,
        total_committed_instructions,
    )?;
    increment_stat(
        &mut stats,
        "simInsts",
        "Count",
        StatResetPolicy::Monotonic,
        total_committed_instructions,
    )?;
    increment_stat(
        &mut stats,
        "simOps",
        "Count",
        StatResetPolicy::Monotonic,
        total_committed_instructions,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.scheduled_requests",
        "Count",
        StatResetPolicy::Monotonic,
        total_scheduled_requests,
    )?;

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}
