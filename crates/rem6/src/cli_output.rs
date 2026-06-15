use std::path::Path;

use crate::config::StatsFormat;
use crate::formatting::json_escape;
use crate::Rem6CliError;

pub(crate) struct ExtraCliArtifact<'a> {
    pub(crate) name: &'static str,
    pub(crate) path: &'a Path,
    pub(crate) contents: &'a str,
}

pub(crate) fn emit_cli_output(
    output: String,
    stats_json: &str,
    stats_text: &str,
    output_path: Option<&Path>,
    stats_output_path: Option<&Path>,
    stats_format: StatsFormat,
    extra_artifact: Option<ExtraCliArtifact<'_>>,
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
    if let Some(artifact) = &extra_artifact {
        std::fs::write(artifact.path, artifact.contents).map_err(|error| {
            Rem6CliError::WriteOutput {
                path: artifact.path.to_path_buf(),
                error: error.to_string(),
            }
        })?;
    }
    if let Some(path) = output_path {
        std::fs::write(path, output).map_err(|error| Rem6CliError::WriteOutput {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;
        return Ok(output_envelope_json(
            path,
            stats_output_path,
            stats_format,
            extra_artifact.as_ref(),
        ));
    }
    Ok(output)
}

fn output_envelope_json(
    artifact: &Path,
    stats_artifact: Option<&Path>,
    format: StatsFormat,
    extra_artifact: Option<&ExtraCliArtifact<'_>>,
) -> String {
    let mut fields = vec![
        "\"schema\":\"rem6.cli.output.v1\"".to_string(),
        format!("\"format\":\"{}\"", format.as_str()),
        format!(
            "\"artifact\":\"{}\"",
            json_escape(&artifact.display().to_string())
        ),
    ];
    if let Some(stats_artifact) = stats_artifact {
        fields.push(format!(
            "\"stats_artifact\":\"{}\"",
            json_escape(&stats_artifact.display().to_string())
        ));
    }
    if let Some(extra_artifact) = extra_artifact {
        fields.push(format!(
            "\"{}\":\"{}\"",
            extra_artifact.name,
            json_escape(&extra_artifact.path.display().to_string())
        ));
    }
    format!("{{{}}}\n", fields.join(","))
}
