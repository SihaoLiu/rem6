use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rem6_power::{
    ExternalPowerAnalysisKind, PowerAnalysisExport, PowerAnalysisRecord, PowerEstimate,
    PowerResidency, PowerStateKind,
};
use rem6_workload::WorkloadParallelExecutionSummary;

use crate::data_cache_runtime::CliDataCacheSummary;
use crate::gpu_cli::{Rem6GpuComputeUnitActivity, Rem6GpuRunExecutionSummary};
use crate::{
    PowerAnalysisFormat, Rem6CacheResourceSummary, Rem6CliError, Rem6CoreSummary, Rem6DramSummary,
    Rem6FabricResourceSummary, Rem6MemoryResourceSummary, Rem6TraceReplayExecutionSummary,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6PowerAnalysisArtifact {
    format: PowerAnalysisFormat,
    output: PathBuf,
    contents: String,
}

impl Rem6PowerAnalysisArtifact {
    pub(crate) fn output(&self) -> &Path {
        &self.output
    }

    pub(crate) fn contents(&self) -> &str {
        &self.contents
    }

    pub(crate) fn to_json(&self) -> String {
        format!(
            "{{\"format\":\"{}\",\"artifact\":\"{}\"}}",
            self.format.as_str(),
            crate::formatting::json_escape(&self.output.display().to_string()),
        )
    }
}

pub(crate) fn run_power_analysis_artifact(
    format: PowerAnalysisFormat,
    output: PathBuf,
    execution: &crate::Rem6ExecutionSummary,
) -> Result<Rem6PowerAnalysisArtifact, Rem6CliError> {
    build_power_analysis_artifact(
        format,
        output,
        execution.final_tick,
        run_power_analysis_records(execution),
    )
}

pub(crate) fn gpu_run_power_analysis_artifact(
    format: PowerAnalysisFormat,
    output: PathBuf,
    execution: &Rem6GpuRunExecutionSummary,
    data_cache: &CliDataCacheSummary,
    dram: &Rem6DramSummary,
) -> Result<Rem6PowerAnalysisArtifact, Rem6CliError> {
    build_power_analysis_artifact(
        format,
        output,
        execution.final_tick(),
        records_for_gpu_run(execution, data_cache, dram),
    )
}

pub(crate) fn trace_replay_power_analysis_artifact(
    format: PowerAnalysisFormat,
    output: PathBuf,
    execution: &Rem6TraceReplayExecutionSummary,
) -> Result<Rem6PowerAnalysisArtifact, Rem6CliError> {
    build_power_analysis_artifact(
        format,
        output,
        execution.final_tick(),
        records_for_trace_replay(execution),
    )
}

fn build_power_analysis_artifact(
    format: PowerAnalysisFormat,
    output: PathBuf,
    tick: u64,
    records: Vec<PowerAnalysisRecord>,
) -> Result<Rem6PowerAnalysisArtifact, Rem6CliError> {
    let export = PowerAnalysisExport::new(power_analysis_kind(format), tick, records)
        .map_err(power_error)?;
    let contents = match format {
        PowerAnalysisFormat::McpatXml => export.to_mcpat_compatible_xml(),
        PowerAnalysisFormat::DsentCsv => export.to_dsent_compatible_csv(),
    }
    .map_err(power_error)?;
    Ok(Rem6PowerAnalysisArtifact {
        format,
        output,
        contents,
    })
}

fn power_analysis_kind(format: PowerAnalysisFormat) -> ExternalPowerAnalysisKind {
    match format {
        PowerAnalysisFormat::McpatXml => ExternalPowerAnalysisKind::McPat,
        PowerAnalysisFormat::DsentCsv => ExternalPowerAnalysisKind::Dsent,
    }
}

pub(crate) fn run_power_analysis_records(
    execution: &crate::Rem6ExecutionSummary,
) -> Vec<PowerAnalysisRecord> {
    run_power_analysis_records_from_parts(
        execution.final_tick,
        &execution.cores,
        &execution.instruction_cache,
        &execution.data_cache,
        &execution.memory_resources,
        &execution.dram,
    )
}

pub(crate) fn run_power_analysis_records_from_parts(
    final_tick: u64,
    cores: &[Rem6CoreSummary],
    instruction_cache: &CliDataCacheSummary,
    data_cache: &CliDataCacheSummary,
    memory_resources: &Rem6MemoryResourceSummary,
    dram: &Rem6DramSummary,
) -> Vec<PowerAnalysisRecord> {
    let mut records = cores
        .iter()
        .map(|core| cpu_power_record(core, final_tick))
        .collect::<Vec<_>>();
    if let Some(record) = cpu_instruction_cache_power_record(instruction_cache, final_tick) {
        records.push(record);
    }
    if let Some(record) = cpu_data_cache_power_record(data_cache, final_tick) {
        records.push(record);
    }
    if let Some(record) =
        shared_cache_power_record("memory.cache.l2", &memory_resources.cache_l2, final_tick)
    {
        records.push(record);
    }
    if let Some(record) =
        shared_cache_power_record("memory.cache.l3", &memory_resources.cache_l3, final_tick)
    {
        records.push(record);
    }
    if let Some(record) = fabric_power_record(&memory_resources.fabric, final_tick) {
        records.push(record);
    }
    if let Some(record) = memory_transport_power_record(memory_resources, final_tick) {
        records.push(record);
    }
    if let Some(record) = dram_power_record(dram, final_tick) {
        records.push(record);
    }
    records.sort_by(|left, right| left.target().cmp(right.target()));
    records
}

fn records_for_gpu_run(
    execution: &Rem6GpuRunExecutionSummary,
    data_cache: &CliDataCacheSummary,
    dram: &Rem6DramSummary,
) -> Vec<PowerAnalysisRecord> {
    let mut records = execution
        .compute_unit_activity()
        .iter()
        .filter(|activity| gpu_compute_unit_is_active(activity))
        .map(gpu_compute_unit_power_record)
        .collect::<Vec<_>>();
    if let Some(record) = gpu_data_cache_power_record(data_cache, execution.final_tick()) {
        records.push(record);
    }
    if let Some(record) = dram_power_record(dram, execution.final_tick()) {
        records.push(record);
    }
    records
}

fn records_for_trace_replay(
    execution: &Rem6TraceReplayExecutionSummary,
) -> Vec<PowerAnalysisRecord> {
    let mut records = Vec::new();
    if let Some(record) = cache_power_record(
        "trace_replay.data_cache",
        execution.data_cache(),
        execution.final_tick(),
        39.0,
        0.012,
    ) {
        records.push(record);
    }
    if let Some(record) =
        trace_replay_fabric_power_record(execution.parallel_summary(), execution.final_tick())
    {
        records.push(record);
    }
    if let Some(record) = dram_power_record(
        &trace_replay_dram_summary(execution.data_cache_dram_summary()),
        execution.final_tick(),
    ) {
        records.push(record);
    }
    records
}

fn cpu_power_record(core: &Rem6CoreSummary, final_tick: u64) -> PowerAnalysisRecord {
    let residency_ticks = core
        .in_order_pipeline_cycles
        .max(core.committed_instructions)
        .max(final_tick)
        .max(1);
    let data_ops = core
        .data_loads
        .saturating_add(core.data_stores)
        .saturating_add(core.data_atomics);
    let data_bytes = core
        .data_load_bytes
        .saturating_add(core.data_store_bytes)
        .saturating_add(core.data_atomic_bytes);
    let dynamic_watts = watts_from_activity(
        core.committed_instructions,
        data_ops,
        data_bytes,
        0.000_010,
        0.000_020,
        0.000_001,
    );
    PowerAnalysisRecord::new(
        format!("cpu{}.core", core.cpu),
        PowerStateKind::On,
        PowerResidency::new(vec![(PowerStateKind::On, residency_ticks)]),
        40.0 + dynamic_watts.min(10.0),
        PowerEstimate::new(dynamic_watts, 0.025),
    )
    .expect("run CPU power records use non-empty names, valid residency, and finite watts")
}

fn gpu_compute_unit_is_active(activity: &Rem6GpuComputeUnitActivity) -> bool {
    activity.workgroup_completions() > 0
        || activity.busy_cycles() > 0
        || activity.coalesced_memory_accesses() > 0
}

fn gpu_compute_unit_power_record(activity: &Rem6GpuComputeUnitActivity) -> PowerAnalysisRecord {
    let active_window = match (activity.first_started_at(), activity.last_completed_at()) {
        (Some(start), Some(end)) if end > start => end - start,
        _ => 0,
    };
    let residency_ticks = activity
        .busy_cycles()
        .max(active_window)
        .max(activity.workgroup_completions())
        .max(activity.coalesced_memory_accesses())
        .max(1);
    let memory_ops = activity
        .global_memory_reads()
        .saturating_add(activity.global_memory_writes());
    let dynamic_watts = watts_from_activity(
        activity.workgroup_completions(),
        activity
            .coalesced_memory_accesses()
            .saturating_add(memory_ops),
        memory_ops.saturating_mul(64),
        0.000_020,
        0.000_015,
        0.000_000_5,
    );
    PowerAnalysisRecord::new(
        format!("gpu.compute_unit{}", activity.compute_unit()),
        PowerStateKind::On,
        PowerResidency::new(vec![(PowerStateKind::On, residency_ticks)]),
        42.0 + dynamic_watts.min(12.0),
        PowerEstimate::new(dynamic_watts, 0.020),
    )
    .expect("GPU compute-unit power records use non-empty names, valid residency, and finite watts")
}

fn gpu_data_cache_power_record(
    data_cache: &CliDataCacheSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    cache_power_record("gpu.data_cache", data_cache, final_tick, 39.0, 0.012)
}

fn cpu_instruction_cache_power_record(
    cache: &CliDataCacheSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    cache_power_record("cpu.instruction_cache", cache, final_tick, 39.0, 0.010)
}

fn cpu_data_cache_power_record(
    cache: &CliDataCacheSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    cache_power_record("cpu.data_cache", cache, final_tick, 39.0, 0.012)
}

fn cache_power_record(
    component: &str,
    cache: &CliDataCacheSummary,
    final_tick: u64,
    base_temperature_celsius: f64,
    static_watts: f64,
) -> Option<PowerAnalysisRecord> {
    if !cache_power_summary_is_active(cache) {
        return None;
    }
    let operations = cache
        .directory_decisions
        .saturating_add(cache.bank_accepted)
        .saturating_add(cache.bank_scheduled_misses)
        .saturating_add(cache.bank_coalesced_misses)
        .saturating_add(cache.prefetch_issued);
    let dynamic_watts = watts_from_activity(
        cache.runs,
        operations,
        cache.dram_accesses.saturating_mul(64),
        0.000_006,
        0.000_004,
        0.000_000_5,
    );
    Some(
        PowerAnalysisRecord::new(
            component,
            PowerStateKind::On,
            PowerResidency::new(vec![(
                PowerStateKind::On,
                final_tick.max(cache.runs).max(1),
            )]),
            base_temperature_celsius + dynamic_watts.min(6.0),
            PowerEstimate::new(dynamic_watts, static_watts),
        )
        .expect("cache power records use non-empty names, valid residency, and finite watts"),
    )
}

fn cache_power_summary_is_active(cache: &CliDataCacheSummary) -> bool {
    cache.runs != 0
        || cache.directory_decisions != 0
        || cache.dram_accesses != 0
        || cache.bank_accepted != 0
        || cache.prefetch_issued != 0
}

fn shared_cache_power_record(
    component: &str,
    cache: &Rem6CacheResourceSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    if !cache_resource_summary_is_active(cache) {
        return None;
    }
    let operations = cache
        .cpu_responses
        .saturating_add(cache.directory_decisions)
        .saturating_add(cache.bank_accepted)
        .saturating_add(cache.bank_scheduled_misses)
        .saturating_add(cache.bank_coalesced_misses)
        .saturating_add(cache.prefetch_issued);
    let dynamic_watts = watts_from_activity(
        cache.activity,
        operations,
        cache.dram_accesses.saturating_mul(64),
        0.000_005,
        0.000_003,
        0.000_000_5,
    );
    Some(
        PowerAnalysisRecord::new(
            component,
            PowerStateKind::On,
            PowerResidency::new(vec![(
                PowerStateKind::On,
                final_tick.max(cache.activity).max(1),
            )]),
            38.5 + dynamic_watts.min(5.0),
            PowerEstimate::new(dynamic_watts, 0.016),
        )
        .expect("shared-cache power records use valid residency and finite watts"),
    )
}

fn cache_resource_summary_is_active(cache: &Rem6CacheResourceSummary) -> bool {
    cache.activity != 0
        || cache.cpu_responses != 0
        || cache.directory_decisions != 0
        || cache.dram_accesses != 0
        || cache.bank_accepted != 0
        || cache.prefetch_issued != 0
}

fn fabric_power_record(
    fabric: &Rem6FabricResourceSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    if !fabric_resource_summary_is_active(fabric) {
        return None;
    }
    let operations = fabric
        .active
        .saturating_add(fabric.active_virtual_networks)
        .saturating_add(fabric.active_links)
        .saturating_add(fabric.active_hops)
        .saturating_add(fabric.flits)
        .saturating_add(fabric.contended_lanes);
    let dynamic_watts = watts_from_activity(
        fabric.activity,
        operations,
        fabric.bytes,
        0.000_004,
        0.000_006,
        0.000_000_25,
    );
    Some(
        PowerAnalysisRecord::new(
            "memory.fabric",
            PowerStateKind::On,
            PowerResidency::new(vec![(
                PowerStateKind::On,
                final_tick
                    .max(fabric.occupied_ticks)
                    .max(fabric.queue_delay_ticks)
                    .max(fabric.credit_delay_ticks)
                    .max(fabric.activity)
                    .max(1),
            )]),
            37.5 + dynamic_watts.min(4.0),
            PowerEstimate::new(dynamic_watts, 0.008),
        )
        .expect("fabric power records use valid residency and finite watts"),
    )
}

fn trace_replay_fabric_power_record(
    summary: &WorkloadParallelExecutionSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    if !summary.has_fabric_activity() {
        return None;
    }
    let transfers = summary.fabric_transfer_count() as u64;
    let operations = (summary.active_fabric_lane_count() as u64)
        .saturating_add(summary.active_fabric_virtual_network_count() as u64)
        .saturating_add(summary.active_fabric_link_count() as u64)
        .saturating_add(trace_replay_active_fabric_hop_count(summary))
        .saturating_add(summary.fabric_flit_count())
        .saturating_add(summary.contended_fabric_lane_count() as u64);
    let dynamic_watts = watts_from_activity(
        transfers,
        operations,
        summary.fabric_byte_count(),
        0.000_004,
        0.000_006,
        0.000_000_25,
    );
    Some(
        PowerAnalysisRecord::new(
            "trace_replay.fabric",
            PowerStateKind::On,
            PowerResidency::new(vec![(
                PowerStateKind::On,
                final_tick
                    .max(summary.fabric_occupied_ticks())
                    .max(summary.fabric_queue_delay_ticks())
                    .max(summary.fabric_credit_delay_ticks())
                    .max(transfers)
                    .max(1),
            )]),
            37.5 + dynamic_watts.min(4.0),
            PowerEstimate::new(dynamic_watts, 0.008),
        )
        .expect("trace replay fabric power records use valid residency and finite watts"),
    )
}

fn trace_replay_active_fabric_hop_count(summary: &WorkloadParallelExecutionSummary) -> u64 {
    summary
        .fabric_hop_activities()
        .iter()
        .map(|activity| {
            (
                activity.link().clone(),
                activity.virtual_network(),
                activity.hop_index(),
            )
        })
        .collect::<BTreeSet<_>>()
        .len() as u64
}

fn fabric_resource_summary_is_active(fabric: &Rem6FabricResourceSummary) -> bool {
    fabric.activity != 0
        || fabric.active != 0
        || fabric.active_virtual_networks != 0
        || fabric.active_links != 0
        || fabric.active_hops != 0
        || fabric.bytes != 0
        || fabric.flits != 0
        || fabric.occupied_ticks != 0
        || fabric.queue_delay_ticks != 0
        || fabric.credit_delay_ticks != 0
}

fn memory_transport_power_record(
    resources: &Rem6MemoryResourceSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    let transport = &resources.transport;
    if transport.activity == 0 && transport.active == 0 {
        return None;
    }
    let dynamic_watts = watts_from_activity(
        transport.activity,
        transport.active,
        transport.activity.saturating_mul(64),
        0.000_003,
        0.000_500,
        0.000_000_25,
    );
    Some(
        PowerAnalysisRecord::new(
            "memory.transport",
            PowerStateKind::On,
            PowerResidency::new(vec![(
                PowerStateKind::On,
                final_tick.max(transport.activity).max(1),
            )]),
            37.0 + dynamic_watts.min(4.0),
            PowerEstimate::new(dynamic_watts, 0.006),
        )
        .expect("memory transport power records use valid residency and finite watts"),
    )
}

fn dram_power_record(dram: &Rem6DramSummary, final_tick: u64) -> Option<PowerAnalysisRecord> {
    if dram.accesses == 0
        && dram.profiled_targets == 0
        && dram.active_banks == 0
        && dram.refreshes == 0
    {
        return None;
    }
    let residency_ticks = final_tick.max(dram.refresh_ticks).max(dram.accesses).max(1);
    let dynamic_watts = watts_from_activity(
        dram.accesses,
        dram.commands.saturating_add(dram.refreshes),
        dram.reads.saturating_add(dram.writes).saturating_mul(64),
        0.000_004,
        0.000_003,
        0.000_000_5,
    );
    let static_watts = 0.010 + (dram.active_banks.max(1) as f64 * 0.000_500);
    Some(
        PowerAnalysisRecord::new(
            "memory.dram",
            PowerStateKind::On,
            PowerResidency::new(vec![(PowerStateKind::On, residency_ticks)]),
            38.0 + dynamic_watts.min(8.0),
            PowerEstimate::new(dynamic_watts, static_watts),
        )
        .expect("run DRAM power records use valid residency and finite watts"),
    )
}

fn trace_replay_dram_summary(
    summary: &rem6_workload::WorkloadParallelExecutionSummary,
) -> Rem6DramSummary {
    Rem6DramSummary {
        active_targets: summary.active_dram_target_count() as u64,
        active_ports: summary.active_dram_port_count() as u64,
        active_banks: summary.active_dram_bank_count() as u64,
        accesses: summary.dram_access_count() as u64,
        reads: summary.dram_read_count() as u64,
        writes: summary.dram_write_count() as u64,
        row_hits: summary.dram_row_hit_count() as u64,
        row_misses: summary.dram_row_miss_count() as u64,
        commands: summary.dram_command_count() as u64,
        turnarounds: summary.dram_turnaround_count() as u64,
        total_ready_latency_ticks: summary.dram_total_ready_latency_cycles(),
        max_ready_latency_ticks: summary.dram_max_ready_latency_cycles(),
        profiled_targets: summary.active_dram_target_count() as u64,
        ..Rem6DramSummary::default()
    }
}

fn watts_from_activity(
    events: u64,
    operations: u64,
    bytes: u64,
    event_scale: f64,
    operation_scale: f64,
    byte_scale: f64,
) -> f64 {
    0.001
        + (events as f64 * event_scale)
        + (operations as f64 * operation_scale)
        + (bytes as f64 * byte_scale)
}

fn power_error(error: rem6_power::PowerError) -> Rem6CliError {
    Rem6CliError::PowerAnalysis {
        error: error.to_string(),
    }
}
