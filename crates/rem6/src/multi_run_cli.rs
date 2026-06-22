use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::cli_output;
use crate::config::StatsFormat;
use crate::formatting::json_escape;
use crate::stats_output::{multi_run_stats_output, Rem6MultiRunStatsInputs};
use crate::{
    run_config, run_gpu_run_config, run_gups_config, run_trace_replay_config, Rem6CliError,
    Rem6ExecutionStop, Rem6ExecutionSummary, Rem6GpuRunConfig, Rem6GupsConfig, Rem6RunConfig,
    Rem6TraceReplayConfig,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6MultiRunConfig {
    suite_id: String,
    runs: Vec<Rem6MultiRunEntry>,
    continue_on_failure: bool,
    stats_format: StatsFormat,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6MultiRunEntry {
    id: String,
    command: Rem6MultiRunCommand,
    config: PathBuf,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum Rem6MultiRunCommand {
    #[default]
    Run,
    Gups,
    GpuRun,
    TraceReplay,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6MultiRunArtifact {
    schema: &'static str,
    config: Rem6MultiRunConfig,
    runs: Vec<Rem6MultiRunSummary>,
    stats_json: String,
    stats_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6MultiRunSummary {
    id: String,
    command: Rem6MultiRunCommand,
    config: PathBuf,
    child_schema: &'static str,
    status: &'static str,
    executed: bool,
    final_tick: u64,
    committed_instructions: u64,
    scheduled_requests: u64,
    checkpoint_count: u64,
    checkpoint_restored_count: u64,
    artifact: Option<PathBuf>,
    stats_artifact: Option<PathBuf>,
    extra_artifacts: Vec<Rem6MultiRunExtraArtifact>,
    error: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6MultiRunExtraArtifact {
    name: &'static str,
    path: PathBuf,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6MultiRunFileRoot {
    multi_run: Option<Rem6MultiRunFileConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6MultiRunFileConfig {
    suite_id: Option<String>,
    stats_format: Option<String>,
    continue_on_failure: Option<bool>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    runs: Option<Vec<Rem6MultiRunFileEntry>>,
    #[serde(skip)]
    config_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6MultiRunFileEntry {
    id: Option<String>,
    command: Option<String>,
    config: Option<PathBuf>,
}

impl Rem6MultiRunConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "multi-run" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }
        let remaining_args = args.collect::<Vec<_>>();
        let file_config = multi_run_file_config_from_args(&remaining_args)?
            .map(|path| load_multi_run_file_config(&path))
            .transpose()?
            .unwrap_or_default();

        let mut suite_id = file_config.suite_id.clone();
        let mut runs = file_config.runs()?;
        let mut continue_on_failure = file_config.continue_on_failure.unwrap_or(false);
        let mut stats_format = file_config
            .stats_format
            .as_deref()
            .map(StatsFormat::parse)
            .transpose()?
            .unwrap_or(StatsFormat::Json);
        let mut output = file_config
            .output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut stats_output = file_config
            .stats_output
            .as_deref()
            .map(|path| file_config.resolve_path(path));

        let mut args = remaining_args.into_iter();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--config" => {
                    let _ = required_value(&flag, args.next())?;
                }
                "--suite-id" => {
                    suite_id = Some(required_value(&flag, args.next())?);
                }
                "--run" => {
                    runs.push(parse_multi_run_entry(&required_value(&flag, args.next())?)?);
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--continue-on-failure" => {
                    continue_on_failure = true;
                }
                "--output" => {
                    output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--stats-output" => {
                    stats_output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                _ => return Err(Rem6CliError::UnknownFlag { flag }),
            }
        }

        if let (Some(output), Some(stats_output)) = (&output, &stats_output) {
            if output == stats_output {
                return Err(Rem6CliError::ConflictingOutputPaths {
                    path: output.to_path_buf(),
                });
            }
        }
        require_unique_run_ids(&runs)?;

        Ok(Self {
            suite_id: suite_id.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "multi_run.suite_id",
            })?,
            continue_on_failure,
            runs: (!runs.is_empty())
                .then_some(runs)
                .ok_or(Rem6CliError::MissingRequiredFlag {
                    flag: "multi_run.runs",
                })?,
            stats_format,
            output,
            stats_output,
        })
    }

    const fn stats_format(&self) -> StatsFormat {
        self.stats_format
    }

    fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
    }

    const fn continue_on_failure(&self) -> bool {
        self.continue_on_failure
    }
}

impl Rem6MultiRunFileConfig {
    fn resolve_path(&self, path: &Path) -> PathBuf {
        if path.is_relative() {
            self.config_dir
                .as_deref()
                .map(|dir| dir.join(path))
                .unwrap_or_else(|| path.to_path_buf())
        } else {
            path.to_path_buf()
        }
    }

    fn runs(&self) -> Result<Vec<Rem6MultiRunEntry>, Rem6CliError> {
        self.runs
            .as_deref()
            .unwrap_or_default()
            .iter()
            .map(|entry| {
                Ok(Rem6MultiRunEntry {
                    id: entry.id.clone().ok_or(Rem6CliError::MissingRequiredFlag {
                        flag: "multi_run.runs.id",
                    })?,
                    command: entry
                        .command
                        .as_deref()
                        .map(Rem6MultiRunCommand::parse)
                        .transpose()?
                        .unwrap_or_default(),
                    config: self.resolve_path(entry.config.as_deref().ok_or(
                        Rem6CliError::MissingRequiredFlag {
                            flag: "multi_run.runs.config",
                        },
                    )?),
                })
            })
            .collect()
    }
}

pub fn run_multi_run_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let config = Rem6MultiRunConfig::parse_args(args)?;
    let artifact = run_multi_run_config(config)?;
    let output = match artifact.config.stats_format() {
        StatsFormat::Json => artifact.to_json(),
        StatsFormat::Text => artifact.stats_text.clone(),
    };
    cli_output::emit_cli_output(
        output,
        &artifact.stats_json,
        &artifact.stats_text,
        artifact.config.output(),
        artifact.config.stats_output(),
        artifact.config.stats_format(),
        &[],
    )
}

pub fn run_multi_run_config(
    config: Rem6MultiRunConfig,
) -> Result<Rem6MultiRunArtifact, Rem6CliError> {
    let mut run_summaries = Vec::with_capacity(config.runs.len());
    for run in &config.runs {
        match run_child(run) {
            Ok(summary) => run_summaries.push(summary),
            Err(error) if config.continue_on_failure() => {
                run_summaries.push(Rem6MultiRunSummary::from_error(run, error));
            }
            Err(error) => return Err(error),
        }
    }

    let succeeded = run_summaries
        .iter()
        .filter(|summary| summary.succeeded())
        .count() as u64;
    let failed = run_summaries.len() as u64 - succeeded;
    let total_final_tick = run_summaries
        .iter()
        .map(|summary| summary.final_tick)
        .sum::<u64>();
    let total_committed_instructions = run_summaries
        .iter()
        .map(|summary| summary.committed_instructions)
        .sum::<u64>();
    let total_scheduled_requests = run_summaries
        .iter()
        .map(|summary| summary.scheduled_requests)
        .sum::<u64>();
    let total_checkpoints = run_summaries
        .iter()
        .map(|summary| summary.checkpoint_count)
        .sum::<u64>();
    let total_checkpoint_restores = run_summaries
        .iter()
        .map(|summary| summary.checkpoint_restored_count)
        .sum::<u64>();
    let stats = multi_run_stats_output(Rem6MultiRunStatsInputs {
        runs: config.runs.len() as u64,
        succeeded,
        failed,
        total_final_tick,
        total_committed_instructions,
        total_scheduled_requests,
        total_checkpoints,
        total_checkpoint_restores,
    })?;

    Ok(Rem6MultiRunArtifact {
        schema: "rem6.cli.multi-run.v1",
        config,
        runs: run_summaries,
        stats_json: stats.json,
        stats_text: stats.text,
    })
}

fn run_child(run: &Rem6MultiRunEntry) -> Result<Rem6MultiRunSummary, Rem6CliError> {
    match run.command {
        Rem6MultiRunCommand::Run => {
            let child_config = Rem6RunConfig::parse_args([
                "run".to_string(),
                "--config".to_string(),
                path_arg(&run.config),
            ])?;
            let artifact = run_config(child_config)?;
            artifact.emit_configured_output()?;
            Ok(Rem6MultiRunSummary::from_run_artifact(run, &artifact))
        }
        Rem6MultiRunCommand::Gups => {
            let child_config = Rem6GupsConfig::parse_args([
                "gups".to_string(),
                "--config".to_string(),
                path_arg(&run.config),
            ])?;
            let artifact = run_gups_config(child_config)?;
            artifact.emit_configured_output()?;
            Ok(Rem6MultiRunSummary::from_gups_artifact(run, &artifact))
        }
        Rem6MultiRunCommand::GpuRun => {
            let child_config = Rem6GpuRunConfig::parse_args([
                "gpu-run".to_string(),
                "--config".to_string(),
                path_arg(&run.config),
            ])?;
            let artifact = run_gpu_run_config(child_config)?;
            artifact.emit_configured_output()?;
            Ok(Rem6MultiRunSummary::from_gpu_run_artifact(run, &artifact))
        }
        Rem6MultiRunCommand::TraceReplay => {
            let child_config = Rem6TraceReplayConfig::parse_args([
                "trace-replay".to_string(),
                "--config".to_string(),
                path_arg(&run.config),
            ])?;
            let artifact = run_trace_replay_config(child_config)?;
            artifact.emit_configured_output()?;
            Ok(Rem6MultiRunSummary::from_trace_replay_artifact(
                run, &artifact,
            ))
        }
    }
}

impl Rem6MultiRunCommand {
    fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "run" => Ok(Self::Run),
            "gups" => Ok(Self::Gups),
            "gpu-run" => Ok(Self::GpuRun),
            "trace-replay" => Ok(Self::TraceReplay),
            _ => Err(Rem6CliError::UnsupportedCommand {
                command: value.to_string(),
            }),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Run => "run",
            Self::Gups => "gups",
            Self::GpuRun => "gpu-run",
            Self::TraceReplay => "trace-replay",
        }
    }
}

impl Rem6MultiRunSummary {
    fn from_run_artifact(run: &Rem6MultiRunEntry, artifact: &crate::Rem6RunArtifact) -> Self {
        let (
            status,
            final_tick,
            committed_instructions,
            checkpoint_count,
            checkpoint_restored_count,
        ) = artifact
            .execution
            .as_ref()
            .map_or(("loaded", 0, 0, 0, 0), |execution| {
                (
                    execution_status(execution),
                    execution.final_tick,
                    execution.committed_instructions,
                    execution.host_actions.checkpoints.len() as u64,
                    execution.host_actions.checkpoint_restored_count,
                )
            });
        Self {
            id: run.id.clone(),
            command: run.command,
            config: run.config.clone(),
            child_schema: artifact.schema,
            status,
            executed: artifact.execution.is_some(),
            final_tick,
            committed_instructions,
            scheduled_requests: 0,
            checkpoint_count,
            checkpoint_restored_count,
            artifact: artifact.config.output().map(Path::to_path_buf),
            stats_artifact: artifact.config.stats_output().map(Path::to_path_buf),
            extra_artifacts: artifact
                .power_analysis
                .as_ref()
                .map(|artifact| Rem6MultiRunExtraArtifact::new("power_artifact", artifact.output()))
                .into_iter()
                .collect(),
            error: None,
        }
    }

    fn from_gups_artifact(run: &Rem6MultiRunEntry, artifact: &crate::Rem6GupsArtifact) -> Self {
        Self {
            id: run.id.clone(),
            command: run.command,
            config: run.config.clone(),
            child_schema: artifact.schema,
            status: "completed",
            executed: true,
            final_tick: artifact.execution.final_tick,
            committed_instructions: 0,
            scheduled_requests: artifact.execution.scheduled_requests,
            checkpoint_count: 0,
            checkpoint_restored_count: 0,
            artifact: artifact.config.output().map(Path::to_path_buf),
            stats_artifact: artifact.config.stats_output().map(Path::to_path_buf),
            extra_artifacts: Vec::new(),
            error: None,
        }
    }

    fn from_gpu_run_artifact(
        run: &Rem6MultiRunEntry,
        artifact: &crate::Rem6GpuRunArtifact,
    ) -> Self {
        let execution = artifact.execution();
        Self {
            id: run.id.clone(),
            command: run.command,
            config: run.config.clone(),
            child_schema: artifact.schema(),
            status: "completed",
            executed: true,
            final_tick: execution.final_tick(),
            committed_instructions: 0,
            scheduled_requests: execution.global_memory_requests(),
            checkpoint_count: 0,
            checkpoint_restored_count: 0,
            artifact: artifact.configured_output().map(Path::to_path_buf),
            stats_artifact: artifact.configured_stats_output().map(Path::to_path_buf),
            extra_artifacts: artifact
                .configured_extra_artifacts()
                .into_iter()
                .map(|(name, path)| Rem6MultiRunExtraArtifact::new(name, path))
                .collect(),
            error: None,
        }
    }

    fn from_trace_replay_artifact(
        run: &Rem6MultiRunEntry,
        artifact: &crate::Rem6TraceReplayArtifact,
    ) -> Self {
        Self {
            id: run.id.clone(),
            command: run.command,
            config: run.config.clone(),
            child_schema: artifact.schema,
            status: "completed",
            executed: true,
            final_tick: artifact.execution.final_tick,
            committed_instructions: 0,
            scheduled_requests: artifact.execution.summary.scheduled_count() as u64,
            checkpoint_count: 0,
            checkpoint_restored_count: 0,
            artifact: artifact.config.output().map(Path::to_path_buf),
            stats_artifact: artifact.config.stats_output().map(Path::to_path_buf),
            extra_artifacts: artifact
                .power_analysis
                .as_ref()
                .map(|artifact| Rem6MultiRunExtraArtifact::new("power_artifact", artifact.output()))
                .into_iter()
                .collect(),
            error: None,
        }
    }

    fn from_error(run: &Rem6MultiRunEntry, error: Rem6CliError) -> Self {
        Self {
            id: run.id.clone(),
            command: run.command,
            config: run.config.clone(),
            child_schema: "rem6.cli.error.v1",
            status: "failed",
            executed: false,
            final_tick: 0,
            committed_instructions: 0,
            scheduled_requests: 0,
            checkpoint_count: 0,
            checkpoint_restored_count: 0,
            artifact: None,
            stats_artifact: None,
            extra_artifacts: Vec::new(),
            error: Some(error.to_string()),
        }
    }

    fn succeeded(&self) -> bool {
        self.error.is_none()
    }

    fn to_json(&self) -> String {
        let artifact = optional_path_json(self.artifact.as_deref());
        let stats_artifact = optional_path_json(self.stats_artifact.as_deref());
        let extra_artifacts = self
            .extra_artifacts
            .iter()
            .map(Rem6MultiRunExtraArtifact::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let error = self
            .error
            .as_ref()
            .map(|error| format!("\"{}\"", json_escape(error)))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"id\":\"{}\",\"command\":\"{}\",\"config\":\"{}\",\"child_schema\":\"{}\",\"run_schema\":\"{}\",\"status\":\"{}\",\"executed\":{},\"final_tick\":{},\"committed_instructions\":{},\"scheduled_requests\":{},\"checkpoint_count\":{},\"checkpoint_restored_count\":{},\"artifact\":{},\"stats_artifact\":{},\"extra_artifacts\":[{}],\"error\":{}}}",
            json_escape(&self.id),
            self.command.as_str(),
            json_escape(&self.config.display().to_string()),
            self.child_schema,
            self.child_schema,
            self.status,
            self.executed,
            self.final_tick,
            self.committed_instructions,
            self.scheduled_requests,
            self.checkpoint_count,
            self.checkpoint_restored_count,
            artifact,
            stats_artifact,
            extra_artifacts,
            error,
        )
    }
}

impl Rem6MultiRunExtraArtifact {
    fn new(name: &'static str, path: &Path) -> Self {
        Self {
            name,
            path: path.to_path_buf(),
        }
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"name\":\"{}\",\"artifact\":\"{}\"}}",
            self.name,
            json_escape(&self.path.display().to_string())
        )
    }
}

fn optional_path_json(path: Option<&Path>) -> String {
    path.map(|path| format!("\"{}\"", json_escape(&path.display().to_string())))
        .unwrap_or_else(|| "null".to_string())
}

impl Rem6MultiRunArtifact {
    pub fn to_json(&self) -> String {
        let runs = self
            .runs
            .iter()
            .map(Rem6MultiRunSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let total_final_tick = self
            .runs
            .iter()
            .map(|summary| summary.final_tick)
            .sum::<u64>();
        let total_committed_instructions = self
            .runs
            .iter()
            .map(|summary| summary.committed_instructions)
            .sum::<u64>();
        let total_scheduled_requests = self
            .runs
            .iter()
            .map(|summary| summary.scheduled_requests)
            .sum::<u64>();
        let total_checkpoints = self
            .runs
            .iter()
            .map(|summary| summary.checkpoint_count)
            .sum::<u64>();
        let total_checkpoint_restores = self
            .runs
            .iter()
            .map(|summary| summary.checkpoint_restored_count)
            .sum::<u64>();
        let succeeded = self
            .runs
            .iter()
            .filter(|summary| summary.succeeded())
            .count();
        let failed = self.runs.len() - succeeded;
        format!(
            "{{\"schema\":\"{}\",\"suite_id\":\"{}\",\"runs\":{},\"succeeded\":{},\"failed\":{},\"total_final_tick\":{},\"total_committed_instructions\":{},\"total_scheduled_requests\":{},\"total_checkpoints\":{},\"total_checkpoint_restores\":{},\"run_summaries\":[{}],\"stats\":{}}}\n",
            self.schema,
            json_escape(&self.config.suite_id),
            self.runs.len(),
            succeeded,
            failed,
            total_final_tick,
            total_committed_instructions,
            total_scheduled_requests,
            total_checkpoints,
            total_checkpoint_restores,
            runs,
            self.stats_json,
        )
    }
}

fn execution_status(execution: &Rem6ExecutionSummary) -> &'static str {
    match execution.stop {
        Rem6ExecutionStop::Idle => "idle",
        Rem6ExecutionStop::HostTrap { .. } => "executed_until_trap",
        Rem6ExecutionStop::HostStop { .. } => "host_stop",
        Rem6ExecutionStop::TickLimit { .. } => "stopped_at_tick_limit",
        Rem6ExecutionStop::InstructionLimit { .. } => "stopped_at_instruction_limit",
    }
}

fn multi_run_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    let mut path = None;
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--config" => {
                path = Some(PathBuf::from(args.get(index + 1).cloned().ok_or_else(
                    || Rem6CliError::MissingFlagValue { flag: flag.clone() },
                )?));
                index += 2;
            }
            "--suite-id" | "--run" | "--stats-format" | "--output" | "--stats-output" => {
                index += 2;
            }
            "--continue-on-failure" => {
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(path)
}

fn load_multi_run_file_config(path: &Path) -> Result<Rem6MultiRunFileConfig, Rem6CliError> {
    let text = std::fs::read_to_string(path).map_err(|error| Rem6CliError::ReadConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })?;
    let mut config = toml::from_str::<Rem6MultiRunFileRoot>(&text)
        .map_err(|error| Rem6CliError::ParseConfig {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?
        .multi_run
        .unwrap_or_default();
    config.config_dir = path.parent().map(Path::to_path_buf);
    Ok(config)
}

fn parse_multi_run_entry(value: &str) -> Result<Rem6MultiRunEntry, Rem6CliError> {
    let Some((id, entry)) = value.split_once(':') else {
        return Err(Rem6CliError::MissingRequiredFlag {
            flag: "--run <id>:<config>",
        });
    };
    let (command, config) = entry
        .split_once(':')
        .and_then(|(candidate, config)| {
            Rem6MultiRunCommand::parse(candidate)
                .ok()
                .map(|command| (command, config))
        })
        .unwrap_or((Rem6MultiRunCommand::Run, entry));
    Ok(Rem6MultiRunEntry {
        id: id.to_string(),
        command,
        config: PathBuf::from(config),
    })
}

fn require_unique_run_ids(runs: &[Rem6MultiRunEntry]) -> Result<(), Rem6CliError> {
    let mut ids = HashSet::new();
    for run in runs {
        if !ids.insert(run.id.as_str()) {
            return Err(Rem6CliError::DuplicateMultiRunId { id: run.id.clone() });
        }
    }
    Ok(())
}

fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
}

fn path_arg(path: &Path) -> String {
    path.display().to_string()
}
