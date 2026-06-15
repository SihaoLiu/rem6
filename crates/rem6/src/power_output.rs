use std::path::{Path, PathBuf};

use rem6_power::{
    ExternalPowerAnalysisKind, PowerAnalysisExport, PowerAnalysisRecord, PowerEstimate,
    PowerResidency, PowerStateKind,
};

use crate::{PowerAnalysisFormat, Rem6CliError, Rem6CoreSummary, Rem6DramSummary};

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
    let kind = match format {
        PowerAnalysisFormat::McpatXml => ExternalPowerAnalysisKind::McPat,
        PowerAnalysisFormat::DsentCsv => ExternalPowerAnalysisKind::Dsent,
    };
    let export = PowerAnalysisExport::new(kind, execution.final_tick, records_for_run(execution))
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

fn records_for_run(execution: &crate::Rem6ExecutionSummary) -> Vec<PowerAnalysisRecord> {
    let mut records = execution
        .cores
        .iter()
        .map(|core| cpu_power_record(core, execution.final_tick))
        .collect::<Vec<_>>();
    if let Some(record) = dram_power_record(&execution.dram, execution.final_tick) {
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
