use rem6_stats::{StatResetPolicy, StatSnapshot, StatsRegistry};

use super::formatting::json_escape;
use super::{
    parallel_stats, stats_error, Rem6CliError, Rem6DramSummary, Rem6ExecutionStop,
    Rem6ExecutionSummary, Rem6GupsConfig, Rem6GupsExecutionSummary, Rem6LoadBlobSummary,
    Rem6MemoryDump, Rem6MemoryTransportCounters, Rem6MemoryTransportSummary, Rem6RunConfig,
    Rem6TraceReplayConfig, Rem6TraceReplayExecutionSummary, RequestedIsa,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6StatsOutput {
    pub(super) json: String,
    pub(super) text: String,
}

pub(super) struct Rem6StatsInputs<'a> {
    pub(super) binary_bytes: u64,
    pub(super) load_segments: u64,
    pub(super) load_blobs: &'a [Rem6LoadBlobSummary],
    pub(super) start_address: u64,
    pub(super) config: &'a Rem6RunConfig,
    pub(super) execution: Option<&'a Rem6ExecutionSummary>,
}

pub(super) struct Rem6GupsStatsInputs<'a> {
    pub(super) config: &'a Rem6GupsConfig,
    pub(super) execution: &'a Rem6GupsExecutionSummary,
    pub(super) transport: &'a Rem6MemoryTransportSummary,
    pub(super) memory_dumps: &'a [Rem6MemoryDump],
}

pub(super) struct Rem6TraceReplayStatsInputs<'a> {
    pub(super) config: &'a Rem6TraceReplayConfig,
    pub(super) execution: &'a Rem6TraceReplayExecutionSummary,
}

pub(super) fn run_stats_output(
    inputs: Rem6StatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.binary.bytes",
        "Byte",
        StatResetPolicy::Constant,
        inputs.binary_bytes,
    )?;
    increment_stat(
        &mut stats,
        "sim.elf.load_segments",
        "Count",
        StatResetPolicy::Constant,
        inputs.load_segments,
    )?;
    increment_stat(
        &mut stats,
        "sim.load_blobs",
        "Count",
        StatResetPolicy::Constant,
        inputs.load_blobs.len() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.load_blob_bytes",
        "Byte",
        StatResetPolicy::Constant,
        inputs
            .load_blobs
            .iter()
            .map(Rem6LoadBlobSummary::bytes)
            .sum(),
    )?;
    for (index, blob) in inputs.load_blobs.iter().enumerate() {
        increment_stat(
            &mut stats,
            &format!("sim.load_blob{index}.address"),
            "Address",
            StatResetPolicy::Constant,
            blob.address(),
        )?;
        increment_stat(
            &mut stats,
            &format!("sim.load_blob{index}.bytes"),
            "Byte",
            StatResetPolicy::Constant,
            blob.bytes(),
        )?;
    }
    increment_stat(
        &mut stats,
        "sim.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.max_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.start_address",
        "Address",
        StatResetPolicy::Constant,
        inputs.start_address,
    )?;
    if inputs.config.isa() == RequestedIsa::Riscv {
        increment_stat(
            &mut stats,
            "sim.riscv.boot.a0",
            "Value",
            StatResetPolicy::Constant,
            inputs.config.riscv_boot_a0(),
        )?;
        increment_stat(
            &mut stats,
            "sim.riscv.boot.a1",
            "Value",
            StatResetPolicy::Constant,
            inputs.config.riscv_boot_a1(),
        )?;
    }
    increment_stat(
        &mut stats,
        "sim.parallel.scheduler.min_remote_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.min_remote_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.memory.route_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.memory_route_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.host.event_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.host_event_delay(),
    )?;
    if let Some(max_instructions) = inputs.config.max_instructions() {
        increment_stat(
            &mut stats,
            "sim.instructions.limit",
            "Count",
            StatResetPolicy::Constant,
            max_instructions,
        )?;
    }
    increment_stat(
        &mut stats,
        "sim.cores",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.cores() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.parallel.scheduler.worker_limit",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.parallel_workers() as u64,
    )?;

    if let Some(execution) = inputs.execution {
        increment_stat(
            &mut stats,
            "sim.instructions.committed",
            "Count",
            StatResetPolicy::Monotonic,
            execution.committed_instructions,
        )?;
        increment_stat(
            &mut stats,
            "sim.final_tick",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.final_tick,
        )?;
        match execution.stop {
            Rem6ExecutionStop::HostTrap { stop_code, .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.host_trap",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
                increment_stat(
                    &mut stats,
                    "sim.stop_code",
                    "Count",
                    StatResetPolicy::Constant,
                    stop_code as u64,
                )?;
            }
            Rem6ExecutionStop::TickLimit { .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.tick_limit",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
            }
            Rem6ExecutionStop::InstructionLimit { .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.instruction_limit",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
            }
        }
        increment_stat(
            &mut stats,
            "sim.memory.dumps",
            "Count",
            StatResetPolicy::Constant,
            execution.memory_dumps.len() as u64,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.loads",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_loads,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.stores",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_stores,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.atomics",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_atomics,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.load_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_load_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.store_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_store_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.atomic_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_atomic_bytes,
        )?;
        parallel_stats::emit_scheduler_stats(&mut stats, execution)?;
        emit_transport_stats(&mut stats, "sim.memory.fetch", &execution.fetch_transport)?;
        emit_transport_stats(&mut stats, "sim.memory.data", &execution.data_transport)?;
        emit_dram_stats(&mut stats, "sim.memory.dram", execution.dram)?;
        for core in &execution.cores {
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.instructions.committed", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.committed_instructions,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.loads", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_loads,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.stores", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_stores,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.atomics", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_atomics,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.load_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_load_bytes,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.store_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_store_bytes,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.atomic_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_atomic_bytes,
            )?;
        }
    }

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

pub(super) fn gups_stats_output(
    inputs: Rem6GupsStatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.gups.memory_start",
        "Address",
        StatResetPolicy::Constant,
        inputs.config.memory_start(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.memory_size",
        "Byte",
        StatResetPolicy::Constant,
        inputs.config.memory_size(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.updates",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.updates(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.max_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.rng_state",
        "Value",
        StatResetPolicy::Constant,
        inputs.config.rng_state(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.scheduler.min_remote_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.min_remote_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.memory.route_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.memory_route_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.memory.dumps",
        "Count",
        StatResetPolicy::Constant,
        inputs.memory_dumps.len() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.execution.final_tick,
    )?;
    increment_stat(
        &mut stats,
        "sim.gups.scheduled_requests",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.execution.scheduled_requests,
    )?;
    emit_gups_response_stats(&mut stats, inputs.execution)?;
    emit_transport_stats(&mut stats, "sim.gups.transport", inputs.transport)?;

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

pub(super) fn trace_replay_stats_output(
    inputs: Rem6TraceReplayStatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.trace_replay.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.max_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.memory_start",
        "Address",
        StatResetPolicy::Constant,
        inputs.config.memory_start(),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.memory_size",
        "Byte",
        StatResetPolicy::Constant,
        inputs.config.memory_size(),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.tick_frequency",
        "Hz",
        StatResetPolicy::Constant,
        inputs.config.tick_frequency(),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.line_bytes",
        "Byte",
        StatResetPolicy::Constant,
        inputs.config.line_bytes(),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.agent",
        "Value",
        StatResetPolicy::Constant,
        u64::from(inputs.config.agent()),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.control_partition",
        "Value",
        StatResetPolicy::Constant,
        u64::from(inputs.config.control_partition()),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.scheduler.min_remote_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.min_remote_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.memory.route_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.memory_route_delay(),
    )?;
    increment_stat(
        &mut stats,
        "sim.trace_replay.final_tick",
        "Tick",
        StatResetPolicy::Monotonic,
        inputs.execution.final_tick(),
    )?;
    emit_trace_replay_summary_stats(&mut stats, inputs.execution.summary())?;

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

fn emit_trace_replay_summary_stats(
    stats: &mut StatsRegistry,
    summary: &rem6_workload::WorkloadTrafficTraceReplaySummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        "sim.trace_replay.scheduled",
        "Count",
        StatResetPolicy::Monotonic,
        summary.scheduled_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.responses.delivered",
        "Count",
        StatResetPolicy::Monotonic,
        summary.response_delivery_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.responses.completed",
        "Count",
        StatResetPolicy::Monotonic,
        summary.trace_completed_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.responses.retry",
        "Count",
        StatResetPolicy::Monotonic,
        summary.trace_retry_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.responses.store_conditional_failed",
        "Count",
        StatResetPolicy::Monotonic,
        summary.trace_store_conditional_failed_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.responses.reads",
        "Count",
        StatResetPolicy::Monotonic,
        summary.trace_read_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.responses.writes",
        "Count",
        StatResetPolicy::Monotonic,
        summary.trace_write_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.response_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.trace_response_data_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.response_fill_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.trace_response_fill_data_byte_count(),
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.memory_failures",
        "Count",
        StatResetPolicy::Monotonic,
        summary.memory_failure_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.memory_failures.read",
        "Count",
        StatResetPolicy::Monotonic,
        summary.memory_failure_read_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.memory_failures.write",
        "Count",
        StatResetPolicy::Monotonic,
        summary.memory_failure_write_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.memory_failures.functional_read",
        "Count",
        StatResetPolicy::Monotonic,
        summary.memory_failure_functional_read_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.memory_failures.functional_write",
        "Count",
        StatResetPolicy::Monotonic,
        summary.memory_failure_functional_write_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.control_acks",
        "Count",
        StatResetPolicy::Monotonic,
        summary.control_ack_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.control_acks.sync",
        "Count",
        StatResetPolicy::Monotonic,
        summary.sync_control_ack_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.control_failures",
        "Count",
        StatResetPolicy::Monotonic,
        summary.control_failure_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.control_failures.sync",
        "Count",
        StatResetPolicy::Monotonic,
        summary.sync_control_failure_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.sideband_events",
        "Count",
        StatResetPolicy::Monotonic,
        summary.sideband_event_count() as u64,
    )
}

fn emit_gups_response_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6GupsExecutionSummary,
) -> Result<(), Rem6CliError> {
    let response_stats = &execution.response_stats;
    increment_stat(
        stats,
        "sim.gups.responses",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.completed",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.completed_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.retry",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.retry_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.store_conditional_failed",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.store_conditional_failed_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.reads",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.read_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.responses.writes",
        "Count",
        StatResetPolicy::Monotonic,
        response_stats.write_response_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.gups.response_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        response_stats.response_data_byte_count(),
    )
}

fn emit_dram_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: Rem6DramSummary,
) -> Result<(), Rem6CliError> {
    emit_dram_counter(
        stats,
        prefix,
        "active_targets",
        "Count",
        summary.active_targets,
    )?;
    emit_dram_counter(stats, prefix, "active_ports", "Count", summary.active_ports)?;
    emit_dram_counter(stats, prefix, "active_banks", "Count", summary.active_banks)?;
    emit_dram_counter(stats, prefix, "accesses", "Count", summary.accesses)?;
    emit_dram_counter(stats, prefix, "reads", "Count", summary.reads)?;
    emit_dram_counter(stats, prefix, "writes", "Count", summary.writes)?;
    emit_dram_counter(stats, prefix, "row_hits", "Count", summary.row_hits)?;
    emit_dram_counter(stats, prefix, "row_misses", "Count", summary.row_misses)?;
    emit_dram_counter(stats, prefix, "commands", "Count", summary.commands)?;
    emit_dram_counter(stats, prefix, "turnarounds", "Count", summary.turnarounds)?;
    emit_dram_counter(
        stats,
        prefix,
        "total_ready_latency_ticks",
        "Tick",
        summary.total_ready_latency_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "max_ready_latency_ticks",
        "Tick",
        summary.max_ready_latency_ticks,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.profiled_targets",
        "Count",
        summary.profiled_targets,
    )?;
    for technology in ["ddr", "hbm", "lpddr", "nvm"] {
        emit_dram_constant(
            stats,
            prefix,
            &format!("profile.technology.{technology}"),
            "Count",
            u64::from(summary.profile_technology == Some(technology)),
        )?;
    }
    emit_dram_constant(
        stats,
        prefix,
        "profile.parallel_ports",
        "Count",
        summary.profile_parallel_ports,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.topology_units",
        "Count",
        summary.profile_topology_units,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.scheduler_banks",
        "Count",
        summary.profile_scheduler_banks,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.topology_banks",
        "Count",
        summary.profile_topology_banks,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.scheduler_bank_groups",
        "Count",
        summary.profile_scheduler_bank_groups,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.bank_count",
        "Count",
        summary.profile_geometry_bank_count,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.row_size",
        "Byte",
        summary.profile_geometry_row_size,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.line_size",
        "Byte",
        summary.profile_geometry_line_size,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.lines_per_row",
        "Count",
        summary.profile_geometry_lines_per_row,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.geometry.bank_group_count",
        "Count",
        summary.profile_geometry_bank_group_count,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.activate_latency",
        "Tick",
        summary.profile_timing_activate_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.read_latency",
        "Tick",
        summary.profile_timing_read_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.write_latency",
        "Tick",
        summary.profile_timing_write_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.precharge_latency",
        "Tick",
        summary.profile_timing_precharge_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.bus_turnaround",
        "Tick",
        summary.profile_timing_bus_turnaround,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.burst_spacing",
        "Tick",
        summary.profile_timing_burst_spacing,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.same_bank_group_burst_spacing",
        "Tick",
        summary.profile_timing_same_bank_group_burst_spacing,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.command_window.window_cycles",
        "Tick",
        summary.profile_timing_command_window_cycles,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.timing.command_window.max_commands",
        "Count",
        summary.profile_timing_command_window_max_commands,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.precharge_powerdown_entry_delay",
        "Tick",
        summary.profile_low_power_precharge_powerdown_entry_delay,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.self_refresh_entry_delay",
        "Tick",
        summary.profile_low_power_self_refresh_entry_delay,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.exit_latency",
        "Tick",
        summary.profile_low_power_exit_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.low_power_timing.self_refresh_exit_latency",
        "Tick",
        summary.profile_low_power_self_refresh_exit_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.read_media_latency",
        "Tick",
        summary.profile_nvm_media_read_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.write_media_latency",
        "Tick",
        summary.profile_nvm_media_write_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.send_latency",
        "Tick",
        summary.profile_nvm_media_send_latency,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.max_pending_reads",
        "Count",
        summary.profile_nvm_media_max_pending_reads,
    )?;
    emit_dram_constant(
        stats,
        prefix,
        "profile.nvm_media.max_pending_writes",
        "Count",
        summary.profile_nvm_media_max_pending_writes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.persistent_writes",
        "Count",
        summary.nvm_persistent_writes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.persistent_write_bytes",
        "Byte",
        summary.nvm_persistent_write_bytes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.max_pending_reads",
        "Count",
        summary.nvm_max_pending_reads,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "nvm.max_pending_persistent_writes",
        "Count",
        summary.nvm_max_pending_persistent_writes,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.active_powerdown.entries",
        "Count",
        summary.low_power_active_powerdown_entries,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.active_powerdown.ticks",
        "Tick",
        summary.low_power_active_powerdown_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.precharge_powerdown.entries",
        "Count",
        summary.low_power_precharge_powerdown_entries,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.precharge_powerdown.ticks",
        "Tick",
        summary.low_power_precharge_powerdown_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.self_refresh.entries",
        "Count",
        summary.low_power_self_refresh_entries,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.self_refresh.ticks",
        "Tick",
        summary.low_power_self_refresh_ticks,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.exits",
        "Count",
        summary.low_power_exits,
    )?;
    emit_dram_counter(
        stats,
        prefix,
        "low_power.exit_latency_ticks",
        "Tick",
        summary.low_power_exit_latency_ticks,
    )
}

fn emit_dram_counter(
    stats: &mut StatsRegistry,
    prefix: &str,
    name: &str,
    unit: &str,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.{name}"),
        unit,
        StatResetPolicy::Monotonic,
        value,
    )
}

fn emit_dram_constant(
    stats: &mut StatsRegistry,
    prefix: &str,
    name: &str,
    unit: &str,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.{name}"),
        unit,
        StatResetPolicy::Constant,
        value,
    )
}

fn stats_snapshot_json(snapshot: &StatSnapshot) -> String {
    let samples = snapshot
        .samples()
        .iter()
        .map(|sample| {
            let scope = sample
                .scope()
                .iter()
                .map(|segment| format!("\"{}\"", json_escape(segment)))
                .collect::<Vec<_>>()
                .join(",");
            let description = sample
                .description()
                .map(|description| format!("\"{}\"", json_escape(description)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"id\":{},\"path\":\"{}\",\"scope\":[{}],\"name\":\"{}\",\"unit\":\"{}\",\"value\":{},\"reset_policy\":\"{}\",\"description\":{}}}",
                sample.id().get(),
                json_escape(sample.path()),
                scope,
                json_escape(sample.name()),
                json_escape(sample.unit()),
                sample.value(),
                sample.reset_policy(),
                description
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{samples}]")
}

fn stats_snapshot_text(snapshot: &StatSnapshot) -> String {
    let mut output = "\n---------- Begin Simulation Statistics ----------\n".to_string();
    for sample in snapshot.samples() {
        output.push_str(&format!(
            "{:<64} {:>20} # unit={} reset_policy={}\n",
            sample.path(),
            sample.value(),
            sample.unit(),
            sample.reset_policy()
        ));
    }
    output.push_str("\n---------- End Simulation Statistics   ----------\n");
    output
}

pub(super) fn increment_stat(
    stats: &mut StatsRegistry,
    path: &str,
    unit: &str,
    reset_policy: StatResetPolicy,
    value: u64,
) -> Result<(), Rem6CliError> {
    let stat = stats
        .register_counter_with_reset_policy(path, unit, reset_policy)
        .map_err(stats_error)?;
    stats.increment(stat, value).map_err(stats_error)
}

fn emit_transport_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6MemoryTransportSummary,
) -> Result<(), Rem6CliError> {
    emit_transport_counters(stats, prefix, &summary.counters)?;
    for route in &summary.routes {
        let route_prefix = format!(
            "{prefix}.route{}.source.{}",
            route.route.get(),
            endpoint_stat_path(&route.source)
        );
        emit_transport_counters(stats, &route_prefix, &route.counters)?;
    }
    Ok(())
}

fn emit_transport_counters(
    stats: &mut StatsRegistry,
    prefix: &str,
    counters: &Rem6MemoryTransportCounters,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.requests"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.requests,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.request_arrivals"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.request_arrivals,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.responses"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.responses,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.response_arrivals"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.response_arrivals,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.round_trip_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        counters.round_trip_ticks,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.max_round_trip_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        counters.max_round_trip_ticks,
    )
}

fn endpoint_stat_path(endpoint: &str) -> String {
    endpoint
        .split('.')
        .map(stat_path_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn stat_path_segment(segment: &str) -> String {
    let mut output = String::new();
    for (index, character) in segment.chars().enumerate() {
        if index == 0 {
            if character.is_ascii_alphabetic() || character == '_' {
                output.push(character);
            } else {
                output.push('_');
                if character.is_ascii_alphanumeric() {
                    output.push(character);
                }
            }
        } else if character.is_ascii_alphanumeric() || character == '_' {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "_".to_string()
    } else {
        output
    }
}
