use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::dram::emit_dram_stats;
use super::text::stats_snapshot_text;
use super::{
    emit_data_cache_prefetch_summary_stats, emit_data_cache_summary_stats, emit_transport_stats,
    increment_stat, stats_snapshot_json, Rem6CliError, Rem6GpuRunStatsInputs, Rem6StatsOutput,
};

pub(crate) fn gpu_run_stats_output(
    inputs: Rem6GpuRunStatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.gpu_run.workgroups",
        "Count",
        StatResetPolicy::Constant,
        u64::from(inputs.config.workgroups()),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.compute_units",
        "Count",
        StatResetPolicy::Constant,
        u64::from(inputs.config.compute_units()),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory_start",
        "Address",
        StatResetPolicy::Constant,
        inputs.config.memory_start(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory_size",
        "Byte",
        StatResetPolicy::Constant,
        inputs.config.memory_size(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.max_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.scheduler.min_remote_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.min_remote_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory.route_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.memory_route_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory.dumps",
        "Count",
        StatResetPolicy::Constant,
        inputs.memory_dumps.len() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.execution.final_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.workgroup_completions",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.workgroup_completions(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.workgroup_queue_wait_count",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.workgroup_queue_wait_count(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.workgroup_queue_wait_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.execution.workgroup_queue_wait_ticks(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.max_workgroup_queue_wait_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.execution.max_workgroup_queue_wait_ticks(),
    )?;
    for activity in inputs.execution.compute_unit_activity() {
        let prefix = format!("sim.gpu_run.compute_unit.cu{}", activity.compute_unit());
        increment_stat(
            &mut stats,
            &format!("{prefix}.workgroup_completions"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.workgroup_completions(),
        )?;
        increment_stat(
            &mut stats,
            &format!("{prefix}.workgroup_queue_wait_count"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.workgroup_queue_wait_count(),
        )?;
        increment_stat(
            &mut stats,
            &format!("{prefix}.workgroup_queue_wait_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.workgroup_queue_wait_ticks(),
        )?;
        increment_stat(
            &mut stats,
            &format!("{prefix}.max_workgroup_queue_wait_ticks"),
            "Tick",
            StatResetPolicy::Monotonic,
            activity.max_workgroup_queue_wait_ticks(),
        )?;
        increment_stat(
            &mut stats,
            &format!("{prefix}.busy_cycles"),
            "Cycle",
            StatResetPolicy::Monotonic,
            activity.busy_cycles(),
        )?;
        increment_stat(
            &mut stats,
            &format!("{prefix}.coalesced_memory_accesses"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.coalesced_memory_accesses(),
        )?;
        increment_stat(
            &mut stats,
            &format!("{prefix}.global_memory_reads"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.global_memory_reads(),
        )?;
        increment_stat(
            &mut stats,
            &format!("{prefix}.global_memory_writes"),
            "Count",
            StatResetPolicy::Monotonic,
            activity.global_memory_writes(),
        )?;
        if let Some(first_started_at) = activity.first_started_at() {
            increment_stat(
                &mut stats,
                &format!("{prefix}.first_started_at"),
                "Tick",
                StatResetPolicy::Monotonic,
                first_started_at,
            )?;
        }
        if let Some(last_completed_at) = activity.last_completed_at() {
            increment_stat(
                &mut stats,
                &format!("{prefix}.last_completed_at"),
                "Tick",
                StatResetPolicy::Monotonic,
                last_completed_at,
            )?;
        }
    }
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory_accesses",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.memory_accesses(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.coalesced_memory_accesses",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.coalesced_memory_accesses(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.global_memory_requests",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.global_memory_requests(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory_responses",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.memory_responses(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.scheduler.epochs",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.scheduler_epochs(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.scheduler.dispatches",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.scheduler_dispatches(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory_scheduler.epochs",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.memory_scheduler_epochs(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gpu_run.memory_scheduler.dispatches",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.memory_scheduler_dispatches(),
    )?;
    emit_data_cache_summary_stats(&mut stats, "sim.gpu_run.data_cache", inputs.data_cache)?;
    emit_data_cache_prefetch_summary_stats(
        &mut stats,
        "sim.gpu_run.data_cache",
        inputs.data_cache,
    )?;
    emit_dram_stats(&mut stats, "sim.gpu_run.memory.dram", inputs.dram)?;
    emit_transport_stats(&mut stats, "sim.gpu_run.transport", inputs.transport)?;
    emit_gpu_fabric_stats(&mut stats, "sim.gpu_run.fabric", inputs.fabric)?;

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

fn emit_gpu_fabric_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &crate::gpu_cli::Rem6GpuFabricSummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.active_lanes"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.active_lane_count() as u64,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.active_virtual_networks"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.active_virtual_network_count() as u64,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.transfers"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.transfer_count() as u64,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.bytes"),
        "Byte",
        StatResetPolicy::Monotonic,
        summary.byte_count(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.flits"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.flit_count(),
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
        &format!("{prefix}.credit_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.credit_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.max_credit_delay_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        summary.max_credit_delay_ticks(),
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.contended_lanes"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.contended_lane_count() as u64,
    )
}
