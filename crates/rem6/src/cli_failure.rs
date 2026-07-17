use std::error::Error;
use std::fmt;

use crate::{
    accelerator_cli, cli_output, gpu_cli, gups_cli, multi_run_cli, power_import_cli,
    resource_acquire_cli, run_config_with_capture_policy, trace_replay_cli, Rem6CliError,
    Rem6RunConfig, StatsFormat,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum DiagnosticCapturePolicy {
    Disabled,
    Enabled,
}

impl DiagnosticCapturePolicy {
    pub(crate) fn capture<T>(self, capture: impl FnOnce() -> Option<T>) -> Option<T> {
        match self {
            Self::Disabled => None,
            Self::Enabled => capture(),
        }
    }
}

pub(crate) fn run_cli_with_capture_policy<I, S>(
    args: I,
    diagnostic_capture: DiagnosticCapturePolicy,
) -> Result<String, Rem6CliFailure>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let Some(command) = args.first() else {
        return Err(Rem6CliError::MissingCommand.into());
    };
    match command.as_str() {
        "run" => run_run_cli_with_capture_policy(args, diagnostic_capture),
        "multi-run" => multi_run_cli::run_multi_run_cli(args).map_err(Into::into),
        "accelerator-run" => accelerator_cli::run_accelerator_run_cli(args).map_err(Into::into),
        "gpu-run" => gpu_cli::run_gpu_run_cli(args).map_err(Into::into),
        "gups" => gups_cli::run_gups_cli(args).map_err(Into::into),
        "power-import" => power_import_cli::run_power_import_cli(args).map_err(Into::into),
        "trace-replay" => trace_replay_cli::run_trace_replay_cli(args).map_err(Into::into),
        "resource-acquire" => {
            resource_acquire_cli::run_resource_acquire_cli(args).map_err(Into::into)
        }
        _ => Err(Rem6CliError::UnsupportedCommand {
            command: command.clone(),
        }
        .into()),
    }
}

fn run_run_cli_with_capture_policy(
    args: Vec<String>,
    diagnostic_capture: DiagnosticCapturePolicy,
) -> Result<String, Rem6CliFailure> {
    let config = Rem6RunConfig::parse_args(args)?;
    let artifact = run_config_with_capture_policy(config, diagnostic_capture)?;
    let stats_format = artifact.config.stats_format();
    let output = match stats_format {
        StatsFormat::Json => artifact.to_json(),
        StatsFormat::Text => artifact.stats_text.clone(),
    };
    let extra_artifacts = artifact
        .power_analysis
        .as_ref()
        .map(|artifact| {
            vec![cli_output::ExtraCliArtifact {
                name: "power_artifact",
                path: artifact.output(),
                contents: artifact.contents(),
            }]
        })
        .unwrap_or_default();
    Ok(cli_output::emit_cli_output(
        output,
        &artifact.stats_json,
        &artifact.stats_text,
        artifact.config.output(),
        artifact.config.stats_output(),
        stats_format,
        &extra_artifacts,
    )?)
}

#[doc(hidden)]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6CliFailure {
    error: Rem6CliError,
    diagnostic_json: Option<String>,
}

impl Rem6CliFailure {
    pub(crate) fn with_diagnostic(error: Rem6CliError, diagnostic_json: String) -> Self {
        Self {
            error,
            diagnostic_json: Some(diagnostic_json),
        }
    }

    pub fn diagnostic_json(&self) -> Option<&str> {
        self.diagnostic_json.as_deref()
    }

    pub fn into_error(self) -> Rem6CliError {
        self.error
    }
}

impl From<Rem6CliError> for Rem6CliFailure {
    fn from(error: Rem6CliError) -> Self {
        Self {
            error,
            diagnostic_json: None,
        }
    }
}

impl fmt::Display for Rem6CliFailure {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(formatter)
    }
}

impl Error for Rem6CliFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.error)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    #[test]
    fn diagnostic_capture_policy_evaluates_only_when_enabled() {
        let captures = Cell::new(0);
        let disabled = DiagnosticCapturePolicy::Disabled.capture(|| {
            captures.set(captures.get() + 1);
            Some("disabled")
        });
        assert_eq!(disabled, None);
        assert_eq!(captures.get(), 0);

        let enabled = DiagnosticCapturePolicy::Enabled.capture(|| {
            captures.set(captures.get() + 1);
            Some("enabled")
        });
        assert_eq!(enabled, Some("enabled"));
        assert_eq!(captures.get(), 1);
    }
}
