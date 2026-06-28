use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::text::stats_snapshot_text;
use super::{
    increment_stat, stats_snapshot_json, Rem6AcceleratorRunStatsInputs, Rem6CliError,
    Rem6StatsOutput,
};

pub(crate) fn accelerator_run_stats_output(
    inputs: Rem6AcceleratorRunStatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.accelerator_run.engine",
        "Value",
        StatResetPolicy::Constant,
        u64::from(inputs.config.engine()),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.lanes",
        "Count",
        StatResetPolicy::Constant,
        u64::from(inputs.config.lanes()),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.command_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.command_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.commands",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.command_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.commands.gpu_kernel",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.gpu_kernel_command_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.commands.npu_inference",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.npu_inference_command_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.completions",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.completion_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.completions.gpu_kernel",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.gpu_kernel_completion_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.completions.npu_inference",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.npu_inference_completion_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.trace_events",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.trace_event_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.execution.final_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.scheduler.epochs",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.scheduler_epoch_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.scheduler.dispatches",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.scheduler_dispatch_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.accelerator_run.scheduler.active_partitions",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.scheduler_active_partition_count(),
    )?;

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}
