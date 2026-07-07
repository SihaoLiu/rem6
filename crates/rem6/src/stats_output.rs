use rem6_boot::{BootElfInterpreter, BootElfMetadata};
use rem6_stats::{StatResetPolicy, StatSample, StatSnapshot, StatsRegistry};

mod accelerator_run;
mod cpu;
mod data_cache;
mod debug;
mod dram;
mod elf;
mod fabric;
mod gpu_run;
mod gups;
mod host_actions;
mod json_aliases;
mod memory_resources;
mod multi_run;
mod o3_runtime;
mod pipeline;
mod resource_acquire;
mod riscv;
mod text;
mod trace_replay;
mod wait_for;

use super::formatting::json_escape;
use crate::gpu_cli::{Rem6GpuFabricSummary, Rem6GpuRunExecutionSummary};

use super::{
    parallel_stats, stats_error, CliDataCacheSummary, Rem6AcceleratorRunConfig,
    Rem6AcceleratorRunExecutionSummary, Rem6CliError, Rem6DramSummary, Rem6ExecutionStop,
    Rem6ExecutionSummary, Rem6GpuRunConfig, Rem6GupsConfig, Rem6GupsExecutionSummary,
    Rem6LoadBlobSummary, Rem6MemoryDump, Rem6MemoryTransportCounters, Rem6MemoryTransportSummary,
    Rem6ReadfileSummary, Rem6ResourceAcquireArtifact, Rem6RunConfig, Rem6TraceReplayConfig,
    Rem6TraceReplayExecutionSummary, Rem6TraceReplayExternalAdapterSummary, RequestedIsa,
};
pub(super) use accelerator_run::accelerator_run_stats_output;
use cpu::emit_cpu_run_stats;
use data_cache::{
    emit_data_cache_prefetch_summary_stats, emit_data_cache_summary_stats,
    emit_gem5_cache_prefetcher_alias_stats,
};
use debug::emit_debug_stats;
use dram::{emit_dram_stats, emit_gem5_mem_ctrl_dram_alias_stats};
use elf::emit_elf_run_stats;
use fabric::emit_run_fabric_stats;
pub(super) use gpu_run::gpu_run_stats_output;
pub(super) use gups::gups_stats_output;
use host_actions::emit_run_host_action_stats;
use json_aliases::append_gem5_json_alias_stats;
use memory_resources::emit_memory_resource_stats;
pub(crate) use multi_run::multi_run_stats_output;
pub(super) use resource_acquire::resource_acquire_stats_output;
use riscv::emit_riscv_run_stats;
use text::stats_snapshot_text;
use trace_replay::{
    emit_trace_replay_data_cache_stats, emit_trace_replay_dram_stats,
    emit_trace_replay_external_adapter_stats, emit_trace_replay_fabric_stats,
    emit_trace_replay_host_action_stats, emit_trace_replay_resource_stats,
    emit_trace_replay_summary_stats,
};

const GEM5_COMPAT_SIM_FREQ_HZ: u64 = 1_000_000_000_000;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct Rem6StatsOutput {
    pub(super) json: String,
    pub(super) text: String,
}

pub(super) struct Rem6StatsInputs<'a> {
    pub(super) binary_bytes: u64,
    pub(super) metadata: BootElfMetadata,
    pub(super) interpreter: Option<&'a BootElfInterpreter>,
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
    pub(super) fabric: &'a Rem6GpuFabricSummary,
    pub(super) memory_dumps: &'a [Rem6MemoryDump],
}

pub(super) struct Rem6TraceReplayStatsInputs<'a> {
    pub(super) config: &'a Rem6TraceReplayConfig,
    pub(super) execution: &'a Rem6TraceReplayExecutionSummary,
    pub(super) external_adapter: Option<&'a Rem6TraceReplayExternalAdapterSummary>,
}

pub(super) struct Rem6ResourceAcquireStatsInputs<'a> {
    pub(super) artifact: &'a Rem6ResourceAcquireArtifact,
}

pub(super) struct Rem6AcceleratorRunStatsInputs<'a> {
    pub(super) config: &'a Rem6AcceleratorRunConfig,
    pub(super) execution: &'a Rem6AcceleratorRunExecutionSummary,
}

pub(super) struct Rem6MultiRunStatsInputs {
    pub(super) runs: u64,
    pub(super) succeeded: u64,
    pub(super) failed: u64,
    pub(super) total_final_tick: u64,
    pub(super) total_committed_instructions: u64,
    pub(super) total_scheduled_requests: u64,
    pub(super) total_accelerator_commands: u64,
    pub(super) total_accelerator_completions: u64,
    pub(super) total_checkpoints: u64,
    pub(super) total_checkpoint_restores: u64,
    pub(super) total_checkpoint_component_count: u64,
    pub(super) total_checkpoint_chunk_count: u64,
    pub(super) total_checkpoint_payload_bytes: u64,
    pub(super) total_checkpoint_restored_component_count: u64,
    pub(super) total_checkpoint_restored_chunk_count: u64,
    pub(super) total_checkpoint_restored_payload_bytes: u64,
}

pub(super) fn run_stats_output(
    inputs: Rem6StatsInputs<'_>,
) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    emit_elf_run_stats(
        &mut stats,
        inputs.binary_bytes,
        &inputs.metadata,
        inputs.interpreter,
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
        emit_riscv_run_stats(&mut stats, inputs.config, inputs.execution)?;
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
            Rem6ExecutionStop::Idle => {
                increment_stat(
                    &mut stats,
                    "sim.stop.idle",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
            }
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
        emit_data_cache_summary_stats(
            &mut stats,
            "sim.instruction_cache.l2",
            &execution.instruction_cache_l2,
        )?;
        emit_data_cache_summary_stats(
            &mut stats,
            "sim.instruction_cache.l3",
            &execution.instruction_cache_l3,
        )?;
        emit_data_cache_prefetch_summary_stats(
            &mut stats,
            "sim.instruction_cache",
            &execution.instruction_cache,
        )?;
        if execution.cores.len() == 1 && inputs.config.instruction_cache_prefetcher().is_some() {
            emit_gem5_cache_prefetcher_alias_stats(
                &mut stats,
                "system.cpu.icache",
                &execution.instruction_cache,
            )?;
        }
        emit_data_cache_summary_stats(&mut stats, "sim.data_cache", &execution.data_cache)?;
        emit_data_cache_prefetch_summary_stats(
            &mut stats,
            "sim.data_cache",
            &execution.data_cache,
        )?;
        if execution.cores.len() == 1 && inputs.config.data_cache_prefetcher().is_some() {
            emit_gem5_cache_prefetcher_alias_stats(
                &mut stats,
                "system.cpu.dcache",
                &execution.data_cache,
            )?;
        }
        emit_data_cache_summary_stats(&mut stats, "sim.data_cache.l2", &execution.data_cache_l2)?;
        emit_data_cache_summary_stats(&mut stats, "sim.data_cache.l3", &execution.data_cache_l3)?;
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
        emit_run_fabric_stats(&mut stats, "sim.memory.fabric", &execution.fabric)?;
        emit_dram_stats(&mut stats, "sim.memory.dram", &execution.dram)?;
        emit_gem5_mem_ctrl_dram_alias_stats(&mut stats, &execution.dram)?;
        emit_memory_resource_stats(&mut stats, execution)?;
        emit_cpu_run_stats(&mut stats, &execution.cores)?;
        emit_debug_stats(&mut stats, execution)?;
        emit_run_host_action_stats(&mut stats, &execution.host_actions)?;
    }

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
    emit_trace_replay_data_cache_stats(
        &mut stats,
        inputs.execution.parallel_summary(),
        inputs.execution.data_cache(),
        inputs.execution.data_cache_dram_accesses(),
    )?;
    emit_trace_replay_fabric_stats(&mut stats, inputs.execution.parallel_summary())?;
    emit_trace_replay_dram_stats(&mut stats, inputs.execution.data_cache_dram_summary())?;
    emit_trace_replay_resource_stats(&mut stats, inputs.execution.parallel_summary())?;
    emit_trace_replay_host_action_stats(&mut stats, inputs.execution.host_actions())?;
    if let Some(external_adapter) = inputs.external_adapter {
        emit_trace_replay_external_adapter_stats(&mut stats, external_adapter)?;
    }

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

fn stats_snapshot_json(snapshot: &StatSnapshot) -> String {
    let mut records = snapshot
        .samples()
        .iter()
        .map(json_record_for_sample)
        .collect::<Vec<_>>();
    append_gem5_json_alias_stats(snapshot, &mut records);
    let samples = records.join(",");
    format!("[{samples}]")
}

fn json_record_for_sample(sample: &StatSample) -> String {
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
}

fn json_record_for_derived_counter(
    id: u64,
    path: &str,
    unit: &str,
    value: u64,
    reset_policy: StatResetPolicy,
) -> String {
    let (scope, name) = json_scope_and_name(path);
    format!(
        "{{\"id\":{},\"path\":\"{}\",\"scope\":[{}],\"name\":\"{}\",\"kind\":\"counter\",\"unit\":\"{}\",\"value\":{},\"reset_policy\":\"{}\",\"description\":null,\"buckets\":[]}}",
        id,
        json_escape(path),
        scope,
        json_escape(name),
        json_escape(unit),
        value,
        reset_policy
    )
}

fn json_scope_and_name(path: &str) -> (String, &str) {
    let Some((scope, name)) = path.rsplit_once('.') else {
        return (String::new(), path);
    };
    let scope = scope
        .split('.')
        .map(|segment| format!("\"{}\"", json_escape(segment)))
        .collect::<Vec<_>>()
        .join(",");
    (scope, name)
}

fn snapshot_sample<'a>(snapshot: &'a StatSnapshot, path: &str) -> Option<&'a StatSample> {
    snapshot
        .samples()
        .iter()
        .find(|sample| sample.path() == path)
}

fn snapshot_sample_value(snapshot: &StatSnapshot, path: &str) -> Option<u64> {
    snapshot_sample(snapshot, path).map(StatSample::value)
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

pub(super) fn stat_path_segment(segment: &str) -> String {
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
