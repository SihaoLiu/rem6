use std::path::{Path, PathBuf};

use rem6_power::{
    ExternalPowerAnalysisKind, PowerAnalysisExport, PowerAnalysisRecord, PowerEstimate,
    PowerResidency, PowerStateKind,
};

use crate::data_cache_runtime::CliDataCacheSummary;
use crate::gpu_cli::{Rem6GpuComputeUnitActivity, Rem6GpuRunExecutionSummary};
use crate::{
    PowerAnalysisFormat, Rem6CliError, Rem6CoreSummary, Rem6DramSummary, Rem6MemoryResourceSummary,
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

fn memory_transport_power_record(
    resources: &Rem6MemoryResourceSummary,
    final_tick: u64,
) -> Option<PowerAnalysisRecord> {
    if resources.transport_activity == 0 && resources.active_transports == 0 {
        return None;
    }
    let dynamic_watts = watts_from_activity(
        resources.transport_activity,
        resources.active_transports,
        resources.transport_activity.saturating_mul(64),
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
                final_tick.max(resources.transport_activity).max(1),
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
