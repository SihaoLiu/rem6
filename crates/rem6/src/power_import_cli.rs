use std::path::{Path, PathBuf};

use rem6_power::{PowerAnalysisExport, PowerAnalysisRecord, PowerStateKind};

use crate::cli_config::required_value;
use crate::formatting::json_escape;
use crate::{cli_output, Rem6CliError, StatsFormat};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PowerImportFormat {
    McpatXml,
    McpatReport,
    DsentCsv,
    DsentReport,
}

impl PowerImportFormat {
    fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "mcpat-xml" => Ok(Self::McpatXml),
            "mcpat-report" => Ok(Self::McpatReport),
            "dsent-csv" => Ok(Self::DsentCsv),
            "dsent-report" => Ok(Self::DsentReport),
            _ => Err(Rem6CliError::UnsupportedPowerAnalysisFormat {
                format: value.to_string(),
            }),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::McpatXml => "mcpat-xml",
            Self::McpatReport => "mcpat-report",
            Self::DsentCsv => "dsent-csv",
            Self::DsentReport => "dsent-report",
        }
    }

    const fn requires_tick(self) -> bool {
        matches!(self, Self::McpatReport | Self::DsentReport)
    }
}

pub(crate) fn run_power_import_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let mut format = None;
    let mut input = None;
    let mut output = None;
    let mut tick = None;
    let mut args = args.into_iter();
    let _command = args.next();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--format" => {
                format = Some(PowerImportFormat::parse(&required_value(
                    &flag,
                    args.next(),
                )?)?)
            }
            "--input" => input = Some(PathBuf::from(required_value(&flag, args.next())?)),
            "--output" => output = Some(PathBuf::from(required_value(&flag, args.next())?)),
            "--tick" => tick = Some(parse_tick(&flag, required_value(&flag, args.next())?)?),
            _ => return Err(Rem6CliError::UnknownFlag { flag }),
        }
    }
    let format = format.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--format" })?;
    let input = input.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--input" })?;
    let tick = match (format.requires_tick(), tick) {
        (true, Some(tick)) => Some(tick),
        (true, None) => {
            return Err(Rem6CliError::MissingRequiredFlag { flag: "--tick" });
        }
        (false, Some(_)) => {
            return Err(Rem6CliError::PowerAnalysis {
                error: "--tick only applies to report import formats".to_string(),
            });
        }
        (false, None) => None,
    };
    let contents =
        std::fs::read_to_string(&input).map_err(|error| Rem6CliError::PowerAnalysis {
            error: format!("failed to read {}: {error}", input.display()),
        })?;
    let export = match format {
        PowerImportFormat::McpatXml => PowerAnalysisExport::from_mcpat_compatible_xml(&contents),
        PowerImportFormat::McpatReport => {
            PowerAnalysisExport::from_mcpat_report_text(&contents, tick.expect("tick is required"))
        }
        PowerImportFormat::DsentCsv => PowerAnalysisExport::from_dsent_compatible_csv(&contents),
        PowerImportFormat::DsentReport => {
            PowerAnalysisExport::from_dsent_report_text(&contents, tick.expect("tick is required"))
        }
    }
    .map_err(power_error)?;
    let json = power_import_json(format, &input, &export);
    cli_output::emit_cli_output(
        json,
        "{}",
        "",
        output.as_deref(),
        None,
        StatsFormat::Json,
        &[],
    )
}

fn parse_tick(flag: &str, value: String) -> Result<u64, Rem6CliError> {
    value
        .parse::<u64>()
        .map_err(|_| Rem6CliError::PowerAnalysis {
            error: format!("{flag} must be a valid tick"),
        })
}

fn power_import_json(
    format: PowerImportFormat,
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
