use std::path::{Path, PathBuf};

use rem6_accelerator::{
    AcceleratorCommand, AcceleratorCommandId, AcceleratorCommandKind, AcceleratorEngine,
    AcceleratorEngineConfig, AcceleratorEngineId,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use serde::Deserialize;

use crate::cli_config::{
    accelerator_run_file_config_from_args, read_toml_config, required_value, resolve_config_path,
};
use crate::cli_output;
use crate::config::StatsFormat;
use crate::stats_output::{accelerator_run_stats_output, Rem6AcceleratorRunStatsInputs};
use crate::{execute_error, Rem6CliError};

const ACCELERATOR_RUN_HOST_PARTITION: PartitionId = PartitionId::new(0);
const ACCELERATOR_RUN_ACCELERATOR_PARTITION: PartitionId = PartitionId::new(1);
const ACCELERATOR_RUN_PARTITIONS: u32 = 2;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6AcceleratorRunConfig {
    engine: u32,
    lanes: u32,
    command_delay: u64,
    commands: Vec<Rem6AcceleratorRunCommand>,
    stats_format: StatsFormat,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6AcceleratorRunArtifact {
    schema: &'static str,
    config: Rem6AcceleratorRunConfig,
    execution: Rem6AcceleratorRunExecutionSummary,
    stats_json: String,
    stats_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6AcceleratorRunExecutionSummary {
    final_tick: u64,
    command_count: u64,
    gpu_kernel_command_count: u64,
    npu_inference_command_count: u64,
    completion_count: u64,
    gpu_kernel_completion_count: u64,
    npu_inference_completion_count: u64,
    trace_event_count: u64,
    scheduler_epoch_count: u64,
    scheduler_dispatch_count: u64,
    scheduler_active_partition_count: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6AcceleratorRunCommand {
    id: u64,
    kind: Rem6AcceleratorRunCommandKind,
    latency: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum Rem6AcceleratorRunCommandKind {
    GpuKernel { workgroups: u32 },
    NpuInference { tiles: u32 },
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6AcceleratorRunFileRoot {
    accelerator_run: Option<Rem6AcceleratorRunFileConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6AcceleratorRunFileConfig {
    engine: Option<u32>,
    lanes: Option<u32>,
    command_delay: Option<u64>,
    stats_format: Option<String>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    npu_inferences: Option<Vec<String>>,
    gpu_kernels: Option<Vec<String>>,
    #[serde(skip)]
    config_dir: Option<PathBuf>,
}

pub(crate) fn run_accelerator_run_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let config = Rem6AcceleratorRunConfig::parse_args(args)?;
    let artifact = run_accelerator_run_config(config)?;
    let stats_format = artifact.config.stats_format();
    let output = match stats_format {
        StatsFormat::Json => artifact.to_json(),
        StatsFormat::Text => artifact.stats_text.clone(),
    };
    cli_output::emit_cli_output(
        output,
        &artifact.stats_json,
        &artifact.stats_text,
        artifact.config.output(),
        artifact.config.stats_output(),
        stats_format,
        &[],
    )
}

pub fn run_accelerator_run_config(
    config: Rem6AcceleratorRunConfig,
) -> Result<Rem6AcceleratorRunArtifact, Rem6CliError> {
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(
        ACCELERATOR_RUN_PARTITIONS,
        config.command_delay,
    )
    .map_err(execute_error)?;
    let engine = AcceleratorEngine::new(
        AcceleratorEngineConfig::new(
            AcceleratorEngineId::new(config.engine),
            ACCELERATOR_RUN_ACCELERATOR_PARTITION,
            config.lanes,
        )
        .map_err(execute_error)?,
    );

    for command in config.commands.iter().cloned() {
        engine
            .submit_from_partition(
                &mut scheduler,
                ACCELERATOR_RUN_HOST_PARTITION,
                config.command_delay,
                command.into_accelerator_command()?,
            )
            .map_err(execute_error)?;
    }

    let summary = engine
        .run_until_idle_parallel_recorded(&mut scheduler)
        .map_err(execute_error)?;
    let completions = engine.completed();
    let execution = Rem6AcceleratorRunExecutionSummary {
        final_tick: summary.final_tick(),
        command_count: config.commands.len() as u64,
        gpu_kernel_command_count: config.gpu_kernel_command_count(),
        npu_inference_command_count: config.npu_inference_command_count(),
        completion_count: summary.command_completion_count() as u64,
        gpu_kernel_completion_count: completions
            .iter()
            .filter(|completion| {
                matches!(completion.kind(), AcceleratorCommandKind::GpuKernel { .. })
            })
            .count() as u64,
        npu_inference_completion_count: completions
            .iter()
            .filter(|completion| {
                matches!(
                    completion.kind(),
                    AcceleratorCommandKind::NpuInference { .. }
                )
            })
            .count() as u64,
        trace_event_count: summary.trace_event_count() as u64,
        scheduler_epoch_count: summary.epoch_count() as u64,
        scheduler_dispatch_count: summary.dispatch_count() as u64,
        scheduler_active_partition_count: summary.active_partition_count() as u64,
    };
    let stats = accelerator_run_stats_output(Rem6AcceleratorRunStatsInputs {
        config: &config,
        execution: &execution,
    })?;

    Ok(Rem6AcceleratorRunArtifact {
        schema: "rem6.cli.accelerator_run.v1",
        config,
        execution,
        stats_json: stats.json,
        stats_text: stats.text,
    })
}

impl Rem6AcceleratorRunConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "accelerator-run" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }
        let remaining_args = args.collect::<Vec<_>>();
        let file_config = accelerator_run_file_config_from_args(&remaining_args)?
            .map(|path| load_accelerator_run_file_config(&path))
            .transpose()?
            .unwrap_or_default();

        let mut engine = file_config
            .engine
            .map(|value| parse_positive_u32("--engine", value.to_string()))
            .transpose()?;
        let mut lanes = file_config
            .lanes
            .map(|value| parse_positive_u32("--lanes", value.to_string()))
            .transpose()?;
        let mut command_delay = file_config
            .command_delay
            .map(|value| parse_positive_u64("--command-delay", value.to_string()))
            .transpose()?
            .unwrap_or(2);
        let mut commands = file_config.commands()?;
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
        let mut commands_from_cli = false;

        let mut args = remaining_args.into_iter();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--config" => {
                    let _ = required_value(&flag, args.next())?;
                }
                "--engine" => {
                    engine = Some(parse_positive_u32(
                        &flag,
                        required_value(&flag, args.next())?,
                    )?);
                }
                "--lanes" => {
                    lanes = Some(parse_positive_u32(
                        &flag,
                        required_value(&flag, args.next())?,
                    )?);
                }
                "--command-delay" => {
                    command_delay = parse_positive_u64(&flag, required_value(&flag, args.next())?)?;
                }
                "--npu-inference" => {
                    if !commands_from_cli {
                        commands.clear();
                        commands_from_cli = true;
                    }
                    commands.push(Rem6AcceleratorRunCommand::parse_npu_inference(
                        &required_value(&flag, args.next())?,
                    )?);
                }
                "--gpu-kernel" => {
                    if !commands_from_cli {
                        commands.clear();
                        commands_from_cli = true;
                    }
                    commands.push(Rem6AcceleratorRunCommand::parse_gpu_kernel(
                        &required_value(&flag, args.next())?,
                    )?);
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
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

        if commands.is_empty() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--gpu-kernel",
            });
        }
        if let (Some(output), Some(stats_output)) = (&output, &stats_output) {
            if output == stats_output {
                return Err(Rem6CliError::ConflictingOutputPaths {
                    path: output.to_path_buf(),
                });
            }
        }

        Ok(Self {
            engine: engine.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--engine" })?,
            lanes: lanes.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--lanes" })?,
            command_delay,
            commands,
            stats_format,
            output,
            stats_output,
        })
    }

    pub const fn engine(&self) -> u32 {
        self.engine
    }

    pub const fn lanes(&self) -> u32 {
        self.lanes
    }

    pub const fn command_delay(&self) -> u64 {
        self.command_delay
    }

    pub const fn stats_format(&self) -> StatsFormat {
        self.stats_format
    }

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
    }

    fn gpu_kernel_command_count(&self) -> u64 {
        self.commands
            .iter()
            .filter(|command| {
                matches!(
                    command.kind,
                    Rem6AcceleratorRunCommandKind::GpuKernel { .. }
                )
            })
            .count() as u64
    }

    fn npu_inference_command_count(&self) -> u64 {
        self.commands
            .iter()
            .filter(|command| {
                matches!(
                    command.kind,
                    Rem6AcceleratorRunCommandKind::NpuInference { .. }
                )
            })
            .count() as u64
    }
}

impl Rem6AcceleratorRunArtifact {
    pub(crate) const fn schema(&self) -> &'static str {
        self.schema
    }

    pub(crate) const fn execution(&self) -> &Rem6AcceleratorRunExecutionSummary {
        &self.execution
    }

    pub(crate) fn configured_output(&self) -> Option<&Path> {
        self.config.output()
    }

    pub(crate) fn configured_stats_output(&self) -> Option<&Path> {
        self.config.stats_output()
    }

    pub(crate) fn emit_configured_output(&self) -> Result<(), Rem6CliError> {
        let stats_format = self.config.stats_format();
        cli_output::emit_configured_artifact_output(
            || self.to_json(),
            &self.stats_json,
            &self.stats_text,
            self.config.output(),
            self.config.stats_output(),
            stats_format,
            &[],
        )
        .map(|_| ())
    }

    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"{}\",\"engine\":{},\"lanes\":{},\"command_delay\":{},\"command_count\":{},\"gpu_kernel_command_count\":{},\"npu_inference_command_count\":{},\"completion_count\":{},\"gpu_kernel_completion_count\":{},\"npu_inference_completion_count\":{},\"final_tick\":{},\"trace_event_count\":{},\"scheduler_epoch_count\":{},\"scheduler_dispatch_count\":{},\"scheduler_active_partition_count\":{},\"stats\":{}}}\n",
            self.schema,
            self.config.engine(),
            self.config.lanes(),
            self.config.command_delay(),
            self.execution.command_count(),
            self.execution.gpu_kernel_command_count(),
            self.execution.npu_inference_command_count(),
            self.execution.completion_count(),
            self.execution.gpu_kernel_completion_count(),
            self.execution.npu_inference_completion_count(),
            self.execution.final_tick(),
            self.execution.trace_event_count(),
            self.execution.scheduler_epoch_count(),
            self.execution.scheduler_dispatch_count(),
            self.execution.scheduler_active_partition_count(),
            self.stats_json,
        )
    }
}

impl Rem6AcceleratorRunExecutionSummary {
    pub const fn final_tick(&self) -> u64 {
        self.final_tick
    }

    pub const fn command_count(&self) -> u64 {
        self.command_count
    }

    pub const fn gpu_kernel_command_count(&self) -> u64 {
        self.gpu_kernel_command_count
    }

    pub const fn npu_inference_command_count(&self) -> u64 {
        self.npu_inference_command_count
    }

    pub const fn completion_count(&self) -> u64 {
        self.completion_count
    }

    pub const fn gpu_kernel_completion_count(&self) -> u64 {
        self.gpu_kernel_completion_count
    }

    pub const fn npu_inference_completion_count(&self) -> u64 {
        self.npu_inference_completion_count
    }

    pub const fn trace_event_count(&self) -> u64 {
        self.trace_event_count
    }

    pub const fn scheduler_epoch_count(&self) -> u64 {
        self.scheduler_epoch_count
    }

    pub const fn scheduler_dispatch_count(&self) -> u64 {
        self.scheduler_dispatch_count
    }

    pub const fn scheduler_active_partition_count(&self) -> u64 {
        self.scheduler_active_partition_count
    }
}

impl Rem6AcceleratorRunFileConfig {
    fn resolve_path(&self, path: &Path) -> PathBuf {
        resolve_config_path(self.config_dir.as_deref(), path)
    }

    fn commands(&self) -> Result<Vec<Rem6AcceleratorRunCommand>, Rem6CliError> {
        let mut commands = Vec::new();
        for value in self.npu_inferences.as_deref().unwrap_or_default() {
            commands.push(Rem6AcceleratorRunCommand::parse_npu_inference(value)?);
        }
        for value in self.gpu_kernels.as_deref().unwrap_or_default() {
            commands.push(Rem6AcceleratorRunCommand::parse_gpu_kernel(value)?);
        }
        Ok(commands)
    }
}

impl Rem6AcceleratorRunCommand {
    fn parse_gpu_kernel(value: &str) -> Result<Self, Rem6CliError> {
        let [id, workgroups, latency] = parse_command_fields("--gpu-kernel", value)?;
        Ok(Self {
            id,
            kind: Rem6AcceleratorRunCommandKind::GpuKernel {
                workgroups: parse_positive_u32("--gpu-kernel workgroups", workgroups.to_string())?,
            },
            latency: parse_positive_u64("--gpu-kernel latency", latency.to_string())?,
        })
    }

    fn parse_npu_inference(value: &str) -> Result<Self, Rem6CliError> {
        let [id, tiles, latency] = parse_command_fields("--npu-inference", value)?;
        Ok(Self {
            id,
            kind: Rem6AcceleratorRunCommandKind::NpuInference {
                tiles: parse_positive_u32("--npu-inference tiles", tiles.to_string())?,
            },
            latency: parse_positive_u64("--npu-inference latency", latency.to_string())?,
        })
    }

    fn into_accelerator_command(self) -> Result<AcceleratorCommand, Rem6CliError> {
        AcceleratorCommand::new(
            AcceleratorCommandId::new(self.id),
            self.kind.into_accelerator_kind(),
            self.latency,
        )
        .map_err(execute_error)
    }
}

impl Rem6AcceleratorRunCommandKind {
    fn into_accelerator_kind(self) -> AcceleratorCommandKind {
        match self {
            Self::GpuKernel { workgroups } => AcceleratorCommandKind::GpuKernel { workgroups },
            Self::NpuInference { tiles } => AcceleratorCommandKind::NpuInference { tiles },
        }
    }
}

fn parse_command_fields(flag: &'static str, value: &str) -> Result<[u64; 3], Rem6CliError> {
    let fields = value.split(':').collect::<Vec<_>>();
    if fields.len() != 3 {
        return Err(execute_error(format!(
            "{flag} must use id:work:latency, got {value}"
        )));
    }
    Ok([
        parse_positive_u64(&format!("{flag} id"), fields[0].to_string())?,
        parse_positive_u64(&format!("{flag} work"), fields[1].to_string())?,
        parse_positive_u64(&format!("{flag} latency"), fields[2].to_string())?,
    ])
}

fn load_accelerator_run_file_config(
    path: &Path,
) -> Result<Rem6AcceleratorRunFileConfig, Rem6CliError> {
    let mut config = read_toml_config::<Rem6AcceleratorRunFileRoot>(path)?
        .accelerator_run
        .unwrap_or_default();
    config.config_dir = path.parent().map(Path::to_path_buf);
    Ok(config)
}

fn parse_positive_u32(name: &str, value: String) -> Result<u32, Rem6CliError> {
    let parsed = parse_positive_u64(name, value.clone())?;
    u32::try_from(parsed).map_err(|_| execute_error(format!("{name} is too large: {value}")))
}

fn parse_positive_u64(name: &str, value: String) -> Result<u64, Rem6CliError> {
    let parsed = parse_u64_value(name, value.clone())?;
    if parsed == 0 {
        return Err(execute_error(format!(
            "{name} must be positive, got {value}"
        )));
    }
    Ok(parsed)
}

fn parse_u64_value(name: &str, value: String) -> Result<u64, Rem6CliError> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        return u64::from_str_radix(hex, 16)
            .map_err(|_| execute_error(format!("invalid {name} {value}")));
    }
    value
        .parse()
        .map_err(|_| execute_error(format!("invalid {name} {value}")))
}
