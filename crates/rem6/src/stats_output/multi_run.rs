use rem6_stats::{StatResetPolicy, StatsRegistry};

use crate::Rem6CliError;

use super::{
    increment_stat, stats_snapshot_json, text::stats_snapshot_text, Rem6MultiRunStatsInputs,
    Rem6StatsOutput,
};

pub(crate) fn multi_run_stats_output(
    inputs: Rem6MultiRunStatsInputs,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.multi_run.runs",
        "Count",
        StatResetPolicy::Constant,
        inputs.runs,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.succeeded",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.succeeded,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.failed",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.failed,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.total_final_tick,
    )?;
    increment_stat(
        &mut stats,
        "sim.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.total_final_tick,
    )?;
    increment_stat(
        &mut stats,
        "simTicks",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.total_final_tick,
    )?;
    increment_stat(
        &mut stats,
        "finalTick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.total_final_tick,
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
        inputs.total_committed_instructions,
    )?;
    increment_stat(
        &mut stats,
        "simInsts",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_committed_instructions,
    )?;
    increment_stat(
        &mut stats,
        "simOps",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_committed_instructions,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.scheduled_requests",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_scheduled_requests,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.accelerator.commands",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_accelerator_commands,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.accelerator.completions",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_accelerator_completions,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoints",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoints,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoint_restores",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoint_restores,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoint_components",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoint_component_count,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoint_chunks",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoint_chunk_count,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoint_payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoint_payload_bytes,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoint_restored_components",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoint_restored_component_count,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoint_restored_chunks",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoint_restored_chunk_count,
    )?;
    increment_stat(
        &mut stats,
        "sim.multi_run.checkpoint_restored_payload_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        inputs.total_checkpoint_restored_payload_bytes,
    )?;

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}
