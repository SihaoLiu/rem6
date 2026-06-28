use std::path::{Path, PathBuf};

use rem6_power::{PowerAnalysisExport, PowerAnalysisRecord, PowerStateKind};

use crate::formatting::json_escape;
use crate::{PowerAnalysisFormat, Rem6CliError};

pub(crate) fn run_power_import_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let mut format = None;
    let mut input = None;
    let mut args = args.into_iter();
    let _command = args.next();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--format" => {
                format = Some(PowerAnalysisFormat::parse(&required_value(
                    &flag,
                    args.next(),
                )?)?)
            }
            "--input" => input = Some(PathBuf::from(required_value(&flag, args.next())?)),
            _ => return Err(Rem6CliError::UnknownFlag { flag }),
        }
    }
    let format = format.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--format" })?;
    let input = input.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--input" })?;
    let contents =
        std::fs::read_to_string(&input).map_err(|error| Rem6CliError::PowerAnalysis {
            error: format!("failed to read {}: {error}", input.display()),
        })?;
    let export = match format {
        PowerAnalysisFormat::McpatXml => PowerAnalysisExport::from_mcpat_compatible_xml(&contents),
        PowerAnalysisFormat::DsentCsv => PowerAnalysisExport::from_dsent_compatible_csv(&contents),
    }
    .map_err(power_error)?;
    Ok(power_import_json(format, &input, &export))
}

fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
}

fn power_import_json(
    format: PowerAnalysisFormat,
    input: &Path,
    export: &PowerAnalysisExport,
) -> String {
    let records = export
        .records()
        .iter()
        .map(power_import_record_json)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"schema\":\"rem6.power-import.v1\",\"format\":\"{}\",\"input\":\"{}\",\"tick\":{},\"record_count\":{},\"totals\":{{\"dynamic_watts\":{:.6},\"static_watts\":{:.6},\"total_watts\":{:.6}}},\"records\":[{}]}}\n",
        format.as_str(),
        json_escape(&input.display().to_string()),
        export.tick(),
        export.records().len(),
        export.total_dynamic_watts(),
        export.total_static_watts(),
        export.total_watts(),
        records,
    )
}

fn power_import_record_json(record: &PowerAnalysisRecord) -> String {
    format!(
        "{{\"target\":\"{}\",\"state\":\"{}\",\"temperature_c\":{:.6},\"dynamic_watts\":{:.6},\"static_watts\":{:.6},\"total_watts\":{:.6},\"residency\":{{\"on_ticks\":{},\"clock_gated_ticks\":{},\"sram_retention_ticks\":{},\"off_ticks\":{}}}}}",
        json_escape(record.target()),
        power_state_name(record.current_state()),
        record.temperature_c(),
        record.dynamic_watts(),
        record.static_watts(),
        record.total_watts(),
        record.residency_ticks(PowerStateKind::On),
        record.residency_ticks(PowerStateKind::ClockGated),
        record.residency_ticks(PowerStateKind::SramRetention),
        record.residency_ticks(PowerStateKind::Off),
    )
}

fn power_state_name(state: PowerStateKind) -> &'static str {
    match state {
        PowerStateKind::Undefined => "Undefined",
        PowerStateKind::On => "On",
        PowerStateKind::ClockGated => "ClockGated",
        PowerStateKind::SramRetention => "SramRetention",
        PowerStateKind::Off => "Off",
    }
}

fn power_error(error: rem6_power::PowerError) -> Rem6CliError {
    Rem6CliError::PowerAnalysis {
        error: error.to_string(),
    }
}
