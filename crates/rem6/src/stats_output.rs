use rem6_stats::{StatResetPolicy, StatSnapshot, StatsRegistry};

mod dram;
mod text;
mod trace_replay;

use super::formatting::json_escape;
use crate::gpu_cli::Rem6GpuRunExecutionSummary;

use super::{
    parallel_stats, stats_error, CliDataCacheSummary, Rem6CliError, Rem6DramSummary,
    Rem6ExecutionStop, Rem6ExecutionSummary, Rem6GpuRunConfig, Rem6GupsConfig,
    Rem6GupsExecutionSummary, Rem6LoadBlobSummary, Rem6MemoryDump, Rem6MemoryTransportCounters,
    Rem6MemoryTransportSummary, Rem6ReadfileSummary, Rem6ResourceAcquireArtifact, Rem6RunConfig,
    Rem6TraceReplayConfig, Rem6TraceReplayExecutionSummary, RequestedIsa,
};
use dram::emit_dram_stats;
use text::stats_snapshot_text;
use trace_replay::{
    emit_trace_replay_data_cache_stats, emit_trace_replay_fabric_stats,
    emit_trace_replay_resource_stats, emit_trace_replay_summary_stats,
};

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
    pub(super) readfiles: &'a [Rem6ReadfileSummary],
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

pub(super) struct Rem6GpuRunStatsInputs<'a> {
    pub(super) config: &'a Rem6GpuRunConfig,
    pub(super) execution: &'a Rem6GpuRunExecutionSummary,
    pub(super) data_cache: &'a CliDataCacheSummary,
    pub(super) dram: &'a Rem6DramSummary,
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
        "sim.readfiles",
        "Count",
        StatResetPolicy::Constant,
        inputs.readfiles.len() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.readfile_bytes",
        "Byte",
        StatResetPolicy::Constant,
        inputs
            .readfiles
            .iter()
            .map(Rem6ReadfileSummary::bytes)
            .sum(),
    )?;
    for (index, readfile) in inputs.readfiles.iter().enumerate() {
        increment_stat(
            &mut stats,
            &format!("sim.readfile{index}.base"),
            "Address",
            StatResetPolicy::Constant,
            readfile.base(),
        )?;
        increment_stat(
            &mut stats,
            &format!("sim.readfile{index}.size"),
            "Byte",
            StatResetPolicy::Constant,
            readfile.size(),
        )?;
        increment_stat(
            &mut stats,
            &format!("sim.readfile{index}.bytes"),
            "Byte",
            StatResetPolicy::Constant,
            readfile.bytes(),
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
            "sim.instructions.probes.events",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_probes.event_count,
        )?;
        increment_stat(
            &mut stats,
            "sim.instructions.probes.retired_events",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_probes.retired_instruction_events,
        )?;
        increment_stat(
            &mut stats,
            "sim.instructions.probes.tracked_insts",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_probes.tracked_instructions,
        )?;
        increment_stat(
            &mut stats,
            "sim.instructions.probes.pc_sample_events",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_probes.pc_sample_events,
        )?;
        increment_stat(
            &mut stats,
            "sim.instructions.probes.pc_target_counters",
            "Count",
            StatResetPolicy::Constant,
            execution.instruction_probes.pc_target_counters,
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
        emit_data_cache_summary_stats(
            &mut stats,
            "sim.instruction_cache",
            &execution.instruction_cache,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.identified",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.prefetch_identified,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.issued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.prefetch_issued,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.queue.enqueued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.prefetch_queue_enqueued,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.queue.issued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.prefetch_queue_issued,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.queue.dropped",
            "Count",
            StatResetPolicy::Monotonic,
            execution.instruction_cache.prefetch_queue_dropped,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.translation_queue.enqueued",
            "Count",
            StatResetPolicy::Monotonic,
            execution
                .instruction_cache
                .prefetch_translation_queue_enqueued,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.translation_queue.issued",
            "Count",
            StatResetPolicy::Monotonic,
            execution
                .instruction_cache
                .prefetch_translation_queue_issued,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.translation_queue.translated",
            "Count",
            StatResetPolicy::Monotonic,
            execution
                .instruction_cache
                .prefetch_translation_queue_translated,
        )?;
        increment_stat(
            &mut stats,
            "sim.instruction_cache.prefetch.translation_queue.dropped",
            "Count",
            StatResetPolicy::Monotonic,
            execution
                .instruction_cache
                .prefetch_translation_queue_dropped,
        )?;
        emit_data_cache_summary_stats(&mut stats, "sim.data_cache", &execution.data_cache)?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.identified",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_identified,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.issued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_issued,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.queue.enqueued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_queue_enqueued,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.queue.issued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_queue_issued,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.queue.dropped",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_queue_dropped,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.translation_queue.enqueued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_translation_queue_enqueued,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.translation_queue.issued",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_translation_queue_issued,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.translation_queue.translated",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_translation_queue_translated,
        )?;
        increment_stat(
            &mut stats,
            "sim.data_cache.prefetch.translation_queue.dropped",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_cache.prefetch_translation_queue_dropped,
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
                &format!("sim.cpu{}.pipeline.in_order.advanced", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_advanced,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.flushed", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_flushed,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.resource_blocked", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_resource_blocked,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.ordering_blocked", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_ordering_blocked,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.in_flight", core.cpu),
                "Count",
                StatResetPolicy::Constant,
                core.in_order_pipeline_in_flight,
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
                &format!("sim.cpu{}.pipeline.in_order.branch_predictions", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_branch_predictions,
            )?;
            increment_stat(
                &mut stats,
                &format!(
                    "sim.cpu{}.pipeline.in_order.branch_mispredictions",
                    core.cpu
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_branch_mispredictions,
            )?;
            increment_stat(
                &mut stats,
                &format!(
                    "sim.cpu{}.pipeline.in_order.branch_prediction_flushes",
                    core.cpu
                ),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_branch_prediction_flushes,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.pipeline.in_order.redirects", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.in_order_pipeline_redirects,
            )?;
            if let Some(checker) = &core.checker {
                increment_stat(
                    &mut stats,
                    &format!("sim.cpu{}.checker.checked_instructions", core.cpu),
                    "Count",
                    StatResetPolicy::Monotonic,
                    checker.checked_instructions,
                )?;
                increment_stat(
                    &mut stats,
                    &format!("sim.cpu{}.checker.mismatches", core.cpu),
                    "Count",
                    StatResetPolicy::Monotonic,
                    checker.mismatches,
                )?;
            }
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

pub(super) fn gpu_run_stats_output(
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
    emit_dram_stats(&mut stats, "sim.gpu_run.memory.dram", inputs.dram)?;
    emit_transport_stats(&mut stats, "sim.gpu_run.transport", inputs.transport)?;

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
    emit_trace_replay_data_cache_stats(&mut stats, inputs.execution.parallel_summary())?;
    emit_trace_replay_fabric_stats(&mut stats, inputs.execution.parallel_summary())?;
    emit_trace_replay_resource_stats(&mut stats, inputs.execution.parallel_summary())?;

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

fn emit_data_cache_summary_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &CliDataCacheSummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.runs"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.runs,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.msi.runs"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.msi_runs,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.mesi.runs"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.mesi_runs,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.moesi.runs"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.moesi_runs,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.chi.runs"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.chi_runs,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.cpu_responses"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.cpu_responses,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.directory_decisions"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.directory_decisions,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.dram_accesses"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.dram_accesses,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.bank.accepted"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.bank_accepted,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.bank.immediate_hits"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.bank_immediate_hits,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.bank.scheduled_misses"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.bank_scheduled_misses,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.bank.coalesced_misses"),
        "Count",
        StatResetPolicy::Monotonic,
        summary.bank_coalesced_misses,
    )?;
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

    use super::{stats_snapshot_json, stats_snapshot_text};

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
}
