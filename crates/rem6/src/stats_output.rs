use rem6_stats::{StatResetPolicy, StatSnapshot, StatsRegistry};

mod dram;
mod text;

use super::formatting::json_escape;
use super::{
    parallel_stats, stats_error, Rem6CliError, Rem6ExecutionStop, Rem6ExecutionSummary,
    Rem6GupsConfig, Rem6GupsExecutionSummary, Rem6LoadBlobSummary, Rem6MemoryDump,
    Rem6MemoryTransportCounters, Rem6MemoryTransportSummary, Rem6ResourceAcquireArtifact,
    Rem6RunConfig, Rem6TraceReplayConfig, Rem6TraceReplayExecutionSummary, RequestedIsa,
};
use dram::emit_dram_stats;
use text::stats_snapshot_text;

const GEM5_COMPAT_SIM_FREQ_HZ: u64 = 1_000_000_000_000;

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

pub(super) struct Rem6ResourceAcquireStatsInputs<'a> {
    pub(super) artifact: &'a Rem6ResourceAcquireArtifact,
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
        increment_stat(
            &mut stats,
            "sim.riscv.se",
            "Count",
            StatResetPolicy::Constant,
            u64::from(inputs.config.riscv_se()),
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
            "simInsts",
            "Count",
            StatResetPolicy::Monotonic,
            execution.committed_instructions,
        )?;
        increment_stat(
            &mut stats,
            "simOps",
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
        increment_stat(
            &mut stats,
            "simTicks",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.final_tick,
        )?;
        increment_stat(
            &mut stats,
            "finalTick",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.final_tick,
        )?;
        increment_stat(
            &mut stats,
            "simFreq",
            "Hz",
            StatResetPolicy::Constant,
            GEM5_COMPAT_SIM_FREQ_HZ,
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
            Rem6ExecutionStop::HostStop { stop_code } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.host_stop",
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
        increment_stat(
            &mut stats,
            "sim.instruction_cache.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.msi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.msi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.mesi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.mesi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.moesi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.moesi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.chi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.chi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.cpu_responses",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.cpu_responses,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.directory_decisions",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.directory_decisions,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.dram_accesses",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.dram_accesses,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.msi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.msi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.mesi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.mesi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.moesi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.moesi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.chi.runs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.chi_runs,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.cpu_responses",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.cpu_responses,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.directory_decisions",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.directory_decisions,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.dram_accesses",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.dram_accesses,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.samples",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_access_probes.sample_count,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.stack_distance.infinite_samples",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_access_probes.stack_distance_infinite_samples,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.stack_distance.finite_samples",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_access_probes.stack_distance_finite_samples,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.stack_distance.stack_depth",
            "Count",
            StatResetPolicy::Constant,
            execution.data_access_probes.stack_distance_stack_depth,
        )?;
        emit_histogram_stat(
            &mut stats,
            "sim.data.probes.stack_distance.read_linear",
            "Count",
            StatResetPolicy::Monotonic,
            &execution.data_access_probes.stack_distance_read_linear,
        )?;
        emit_histogram_stat(
            &mut stats,
            "sim.data.probes.stack_distance.write_linear",
            "Count",
            StatResetPolicy::Monotonic,
            &execution.data_access_probes.stack_distance_write_linear,
        )?;
        emit_histogram_stat(
            &mut stats,
            "sim.data.probes.stack_distance.read_log",
            "Count",
            StatResetPolicy::Monotonic,
            &execution.data_access_probes.stack_distance_read_log,
        )?;
        emit_histogram_stat(
            &mut stats,
            "sim.data.probes.stack_distance.write_log",
            "Count",
            StatResetPolicy::Monotonic,
            &execution.data_access_probes.stack_distance_write_log,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.memory_footprint.cache_line_bytes",
            "Byte",
            StatResetPolicy::Constant,
            execution
                .data_access_probes
                .memory_footprint_cache_line_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.memory_footprint.cache_line_total_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution
                .data_access_probes
                .memory_footprint_cache_line_total_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.memory_footprint.page_bytes",
            "Byte",
            StatResetPolicy::Constant,
            execution.data_access_probes.memory_footprint_page_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.probes.memory_footprint.page_total_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution
                .data_access_probes
                .memory_footprint_page_total_bytes,
        )?;
        if inputs.config.isa() == RequestedIsa::Riscv {
            increment_stat(
                &mut stats,
                "sim.riscv.unknown_syscalls",
                "Count",
                StatResetPolicy::Monotonic,
                execution.riscv_unknown_syscalls.len() as u64,
            )?;
        }
        parallel_stats::emit_scheduler_stats(&mut stats, execution)?;
        emit_transport_stats(&mut stats, "sim.memory.fetch", &execution.fetch_transport)?;
        emit_transport_stats(&mut stats, "sim.memory.data", &execution.data_transport)?;
        emit_dram_stats(&mut stats, "sim.memory.dram", &execution.dram)?;
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
                &format!("sim.cpu{}.pipeline.in_order.cycles", core.cpu),
                "Cycle",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_cycles,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.retired", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_retired,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.fetch_wait_cycles", core.cpu),
                "Cycle",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_fetch_wait_cycles,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.data_wait_cycles", core.cpu),
                "Cycle",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_data_wait_cycles,
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
    emit_gups_profile_stats(&mut stats, inputs.execution)?;
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

pub(super) fn resource_acquire_stats_output(
    inputs: Rem6ResourceAcquireStatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.resource_acquire.boot_entry",
        "Address",
        StatResetPolicy::Constant,
        inputs.artifact.config.boot_entry(),
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.resources",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.config.resource_count() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.required_resources",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.required_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.acquired_resources",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.artifact.acquired_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.resolved_resources",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.artifact.resolved_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.acquired_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        inputs.artifact.acquired_bytes,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_manifests",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.suite_manifests,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_required_resources",
        "Count",
        StatResetPolicy::Constant,
        inputs.artifact.suite_required_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_acquired_resources",
        "Count",
        StatResetPolicy::Monotonic,
        inputs.artifact.suite_acquired_resources,
    )?;
    increment_stat(
        &mut stats,
        "sim.resource_acquire.suite_acquired_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        inputs.artifact.suite_acquired_bytes,
    )?;
    for (index, resource) in inputs.artifact.resources.iter().enumerate() {
        increment_stat(
            &mut stats,
            &format!("sim.resource_acquire.resource{index}.bytes"),
            "Byte",
            StatResetPolicy::Monotonic,
            resource.size_bytes,
        )?;
    }

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
    emit_trace_count(
        stats,
        "sim.trace_replay.scheduled",
        summary.scheduled_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.delivered",
        summary.response_delivery_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.completed",
        summary.trace_completed_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.retry",
        summary.trace_retry_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.store_conditional_failed",
        summary.trace_store_conditional_failed_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.reads",
        summary.trace_read_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.writes",
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
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures",
        summary.memory_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.read",
        summary.memory_failure_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.write",
        summary.memory_failure_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.functional_read",
        summary.memory_failure_functional_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.functional_write",
        summary.memory_failure_functional_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_acks",
        summary.control_ack_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_acks.sync",
        summary.sync_control_ack_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures",
        summary.control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.sync",
        summary.sync_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband_events",
        summary.sideband_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.prefetch",
        summary.trace_prefetch_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.invalidate",
        summary.trace_invalidate_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.clean",
        summary.trace_clean_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.upgrade",
        summary.trace_upgrade_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.llsc",
        summary.trace_llsc_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.locked_rmw",
        summary.trace_locked_rmw_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.writable_intent",
        summary.trace_writable_intent_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory.events",
        summary.memory_trace_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory.write_completions",
        summary.memory_write_completion_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache",
        summary.trace_data_cache_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache.maintenance",
        summary.trace_data_cache_maintenance_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache.clean_maintenance",
        summary.trace_data_cache_clean_maintenance_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.responses.cache.invalidate_maintenance",
        summary.trace_data_cache_invalidate_maintenance_response_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache",
        summary.trace_data_cache_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.invalid_destination",
        summary.trace_data_cache_invalid_destination_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.bad_address",
        summary.trace_data_cache_bad_address_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.read",
        summary.trace_data_cache_read_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.write",
        summary.trace_data_cache_write_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.functional_read",
        summary.trace_data_cache_functional_read_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.cache.functional_write",
        summary.trace_data_cache_functional_write_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.invalid_destination",
        summary.memory_failure_invalid_destination_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.memory_failures.bad_address",
        summary.memory_failure_bad_address_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors",
        summary.trace_error_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.invalid_destination",
        summary.trace_error_invalid_destination_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.bad_address",
        summary.trace_error_bad_address_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.read",
        summary.trace_error_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.write",
        summary.trace_error_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.functional_read",
        summary.trace_error_functional_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.trace_errors.functional_write",
        summary.trace_error_functional_write_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.memory.write_completion_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.memory_write_completion_byte_count(),
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.htm.access",
        summary.trace_htm_access_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.htm.begin",
        summary.trace_htm_begin_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_acks.htm",
        summary.htm_control_ack_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.invalid_destination",
        summary.control_failure_invalid_destination_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.bad_address",
        summary.control_failure_bad_address_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.read",
        summary.control_failure_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.write",
        summary.control_failure_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.functional_read",
        summary.control_failure_functional_read_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.functional_write",
        summary.control_failure_functional_write_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.tlb",
        summary.tlb_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.cache",
        summary.cache_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.htm",
        summary.htm_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.control_failures.diagnostic",
        summary.diagnostic_control_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.failures",
        summary.trace_sideband_failure_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.tlb_sync_events",
        summary.tlb_sync_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.tlb_sync",
        summary.trace_tlb_sync_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.tlb_sync_flushed_entries",
        summary.trace_tlb_sync_flushed_entry_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.cache_flush_events",
        summary.cache_flush_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.cache_flush",
        summary.trace_cache_flush_count() as u64,
    )?;
    increment_stat(
        stats,
        "sim.trace_replay.sideband.cache_flush_data_bytes",
        "Byte",
        StatResetPolicy::Monotonic,
        summary.trace_cache_flush_data_byte_count(),
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.l1_invalidation",
        summary.trace_l1_invalidation_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.diagnostic_print_events",
        summary.diagnostic_print_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.diagnostic",
        summary.trace_diagnostic_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.htm_abort_events",
        summary.htm_abort_event_count() as u64,
    )?;
    emit_trace_count(
        stats,
        "sim.trace_replay.sideband.htm_abort",
        summary.trace_htm_abort_count() as u64,
    )
}

fn emit_trace_count(stats: &mut StatsRegistry, path: &str, value: u64) -> Result<(), Rem6CliError> {
    increment_stat(stats, path, "Count", StatResetPolicy::Monotonic, value)
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

fn emit_gups_profile_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6GupsExecutionSummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        "sim.gups.traffic_profiles",
        "Count",
        StatResetPolicy::Constant,
        execution.profile_summaries.len() as u64,
    )?;

    for (index, summary) in execution.profile_summaries.iter().enumerate() {
        let profile = summary.profile();
        let generator_summary = profile.summary();
        let prefix = format!("sim.gups.traffic_profile{index}");
        increment_stat(
            stats,
            &format!("{prefix}.state"),
            "Value",
            StatResetPolicy::Constant,
            u64::from(summary.state().get()),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.generator_class"),
            "Value",
            StatResetPolicy::Constant,
            profile.generator_class().stat_code(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.memory_profile"),
            "Value",
            StatResetPolicy::Constant,
            profile.memory_profile().stat_code(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.packets"),
            "Count",
            StatResetPolicy::Monotonic,
            generator_summary.packet_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.reads"),
            "Count",
            StatResetPolicy::Monotonic,
            generator_summary.read_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.writes"),
            "Count",
            StatResetPolicy::Monotonic,
            generator_summary.write_count(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.bytes_read"),
            "Byte",
            StatResetPolicy::Monotonic,
            generator_summary.bytes_read(),
        )?;
        increment_stat(
            stats,
            &format!("{prefix}.bytes_written"),
            "Byte",
            StatResetPolicy::Monotonic,
            generator_summary.bytes_written(),
        )?;
        if let Some(first_tick) = generator_summary.first_tick() {
            increment_stat(
                stats,
                &format!("{prefix}.first_tick"),
                "Tick",
                StatResetPolicy::Monotonic,
                first_tick,
            )?;
        }
        if let Some(last_tick) = generator_summary.last_tick() {
            increment_stat(
                stats,
                &format!("{prefix}.last_tick"),
                "Tick",
                StatResetPolicy::Monotonic,
                last_tick,
            )?;
        }
    }

    Ok(())
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
            let buckets = sample
                .histogram_buckets()
                .iter()
                .map(|bucket| {
                    format!(
                        "{{\"bucket\":{},\"count\":{}}}",
                        bucket.bucket(),
                        bucket.count()
                    )
                })
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"id\":{},\"path\":\"{}\",\"scope\":[{}],\"name\":\"{}\",\"kind\":\"{}\",\"unit\":\"{}\",\"value\":{},\"reset_policy\":\"{}\",\"description\":{},\"buckets\":[{}]}}",
                sample.id().get(),
                json_escape(sample.path()),
                scope,
                json_escape(sample.name()),
                sample.kind(),
                json_escape(sample.unit()),
                sample.value(),
                sample.reset_policy(),
                description,
                buckets
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{samples}]")
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

fn emit_histogram_stat(
    stats: &mut StatsRegistry,
    path: &str,
    unit: &str,
    reset_policy: StatResetPolicy,
    buckets: &[(u64, u64)],
) -> Result<(), Rem6CliError> {
    let stat = stats
        .register_histogram_with_reset_policy(path, unit, reset_policy)
        .map_err(stats_error)?;
    for (bucket, count) in buckets {
        stats
            .observe_histogram_count(stat, *bucket, *count)
            .map_err(stats_error)?;
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use rem6_stats::StatsRegistry;
    use rem6_workload::{WorkloadRouteId, WorkloadTrafficTraceReplaySummary};

    use super::{emit_trace_replay_summary_stats, stats_snapshot_json, stats_snapshot_text};

    #[test]
    fn trace_replay_stats_emit_nonzero_cache_and_sideband_counters() {
        let summary = WorkloadTrafficTraceReplaySummary::new(route_id("cpu0.data"), 3)
            .with_trace_invalidate_response_count(1)
            .with_trace_clean_response_count(1)
            .with_trace_data_cache_response_count(3)
            .with_trace_data_cache_maintenance_response_count(2)
            .with_trace_data_cache_clean_maintenance_response_count(1)
            .with_trace_data_cache_invalidate_maintenance_response_count(1)
            .with_trace_error_count(2)
            .with_trace_error_write_count(1)
            .with_trace_error_functional_write_count(1)
            .with_trace_cache_flush_count(1)
            .with_trace_cache_flush_data_byte_count(64)
            .with_trace_l1_invalidation_count(1)
            .with_trace_diagnostic_count(1);
        let mut stats = StatsRegistry::new();

        emit_trace_replay_summary_stats(&mut stats, &summary).unwrap();
        let json = stats_snapshot_json(&stats.snapshot(0));

        assert_stat_value(&json, "sim.trace_replay.responses.cache", "Count", 3);
        assert_stat_value(&json, "sim.trace_replay.responses.invalidate", "Count", 1);
        assert_stat_value(&json, "sim.trace_replay.responses.clean", "Count", 1);
        assert_stat_value(
            &json,
            "sim.trace_replay.responses.cache.maintenance",
            "Count",
            2,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.responses.cache.clean_maintenance",
            "Count",
            1,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.responses.cache.invalidate_maintenance",
            "Count",
            1,
        );
        assert_stat_value(&json, "sim.trace_replay.trace_errors", "Count", 2);
        assert_stat_value(&json, "sim.trace_replay.trace_errors.write", "Count", 1);
        assert_stat_value(
            &json,
            "sim.trace_replay.trace_errors.functional_write",
            "Count",
            1,
        );
        assert_stat_value(&json, "sim.trace_replay.sideband.cache_flush", "Count", 1);
        assert_stat_value(
            &json,
            "sim.trace_replay.sideband.cache_flush_data_bytes",
            "Byte",
            64,
        );
        assert_stat_value(
            &json,
            "sim.trace_replay.sideband.l1_invalidation",
            "Count",
            1,
        );
        assert_stat_value(&json, "sim.trace_replay.sideband.diagnostic", "Count", 1);
    }

    #[test]
    fn stats_output_renders_histogram_samples_with_typed_buckets() {
        let mut stats = StatsRegistry::new();
        let latency = stats
            .register_histogram("system.l2.read_latency", "Cycle")
            .unwrap();
        stats.observe_histogram(latency, 4).unwrap();
        stats.observe_histogram(latency, 8).unwrap();
        stats.observe_histogram(latency, 8).unwrap();
        let snapshot = stats.snapshot(0);

        let json = stats_snapshot_json(&snapshot);
        assert!(json.contains("\"path\":\"system.l2.read_latency\""));
        assert!(json.contains("\"kind\":\"histogram\""));
        assert!(json.contains("\"value\":3"));
        assert!(
            json.contains("\"buckets\":[{\"bucket\":4,\"count\":1},{\"bucket\":8,\"count\":2}]")
        );

        let text = stats_snapshot_text(&snapshot);
        assert!(text.contains("system.l2.read_latency"));
        assert!(text.contains("kind=histogram"));
        assert!(text.contains("histogram_bucket=4"));
        assert!(text.contains("histogram_bucket=8"));
    }

    fn assert_stat_value(json: &str, path: &str, unit: &str, value: u64) {
        let path_field = format!("\"path\":\"{path}\"");
        let path_index = json
            .find(&path_field)
            .unwrap_or_else(|| panic!("missing stat path {path} in {json}"));
        let sample_start = json[..path_index]
            .rfind('{')
            .unwrap_or_else(|| panic!("missing stat object start for {path} in {json}"));
        let sample_end = json[path_index..]
            .find('}')
            .map(|offset| path_index + offset + 1)
            .unwrap_or_else(|| panic!("missing stat object end for {path} in {json}"));
        let sample = &json[sample_start..sample_end];
        assert!(sample.contains(&format!("\"unit\":\"{unit}\"")));
        assert!(sample.contains(&format!("\"value\":{value}")));
    }

    fn route_id(value: &str) -> WorkloadRouteId {
        WorkloadRouteId::new(value).unwrap()
    }
}
