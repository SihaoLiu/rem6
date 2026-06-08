use std::path::Path;

use crate::config::StatsFormat;
use crate::formatting::json_escape;
use crate::Rem6CliError;

pub(crate) fn emit_cli_output(
    output: String,
    stats_json: &str,
    stats_text: &str,
    output_path: Option<&Path>,
    stats_output_path: Option<&Path>,
    stats_format: StatsFormat,
) -> Result<String, Rem6CliError> {
    if let Some(path) = stats_output_path {
        let stats_output = match stats_format {
            StatsFormat::Json => format!("{stats_json}\n"),
            StatsFormat::Text => stats_text.to_string(),
        };
        std::fs::write(path, stats_output).map_err(|error| Rem6CliError::WriteOutput {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;
    }
    if let Some(path) = output_path {
        std::fs::write(path, output).map_err(|error| Rem6CliError::WriteOutput {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;
        return Ok(output_envelope_json(path, stats_output_path, stats_format));
    }
    Ok(output)
}

fn output_envelope_json(
    artifact: &Path,
    stats_artifact: Option<&Path>,
    format: StatsFormat,
) -> String {
    let artifact = json_escape(&artifact.display().to_string());
    match stats_artifact {
        Some(stats_artifact) => format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"{}\",\"artifact\":\"{}\",\"stats_artifact\":\"{}\"}}\n",
            format.as_str(),
            artifact,
            json_escape(&stats_artifact.display().to_string())
        ),
        None => format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"{}\",\"artifact\":\"{}\"}}\n",
            format.as_str(),
            artifact
        ),
    }
}
