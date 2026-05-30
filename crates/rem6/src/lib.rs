use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rem6_boot::{BootElfArchitecture, BootImage};
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvCoreDriveAction, RiscvDataAccessEventKind,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionFrontier, PartitionId, PartitionedScheduler, ReadyPartition};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::{StatResetPolicy, StatSnapshot, StatsRegistry};
use rem6_system::{
    GuestEventId, GuestSourceId, GuestTrapKind, HostEventPolicy, RiscvSystemRun,
    RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTrapEventPort, SystemHostController,
    SystemHostEventPort,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    RequestDelivery, TargetOutcome, TransportEndpointId,
};

mod artifact_json;
mod formatting;
mod parallel_stats;
#[cfg(test)]
mod transport_summary_tests;

use formatting::{elf_architecture_name, json_escape};

const DEFAULT_CACHE_LINE_BYTES: u64 = 16;
const CLI_MEMORY_TARGET: MemoryTargetId = MemoryTargetId::new(0);
const CLI_MEMORY_DUMP_AGENT: AgentId = AgentId::new(u32::MAX);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestedIsa {
    Riscv,
    X86,
}

impl RequestedIsa {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "riscv" => Ok(Self::Riscv),
            "x86" => Ok(Self::X86),
            _ => Err(Rem6CliError::UnsupportedIsa {
                isa: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Riscv => "riscv",
            Self::X86 => "x86",
        }
    }

    const fn matches_architecture(self, architecture: BootElfArchitecture) -> bool {
        matches!(
            (self, architecture),
            (
                Self::Riscv,
                BootElfArchitecture::Riscv32 | BootElfArchitecture::Riscv64
            ) | (
                Self::X86,
                BootElfArchitecture::I386 | BootElfArchitecture::X8664
            )
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatsFormat {
    Json,
    Text,
}

impl StatsFormat {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "json" => Ok(Self::Json),
            "text" => Ok(Self::Text),
            _ => Err(Rem6CliError::UnsupportedStatsFormat {
                format: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Text => "text",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunConfig {
    isa: RequestedIsa,
    binary: PathBuf,
    max_tick: u64,
    min_remote_delay: u64,
    max_instructions: Option<u64>,
    stats_format: StatsFormat,
    execute: bool,
    cores: usize,
    parallel_workers: usize,
    memory_dumps: Vec<MemoryDumpRequest>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
}

impl Rem6RunConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "run" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }

        let mut isa = None;
        let mut binary = None;
        let mut max_tick = None;
        let mut min_remote_delay = 1u64;
        let mut max_instructions = None;
        let mut stats_format = StatsFormat::Json;
        let mut execute = false;
        let mut cores = 1usize;
        let mut parallel_workers = None;
        let mut memory_dumps = Vec::new();
        let mut output = None;
        let mut stats_output = None;
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--isa" => {
                    isa = Some(RequestedIsa::parse(&required_value(&flag, args.next())?)?);
                }
                "--binary" => {
                    binary = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--max-tick" => {
                    let value = required_value(&flag, args.next())?;
                    max_tick = Some(value.parse().map_err(|_| Rem6CliError::InvalidMaxTick {
                        value: value.clone(),
                    })?);
                }
                "--min-remote-delay" => {
                    let value = required_value(&flag, args.next())?;
                    min_remote_delay =
                        value
                            .parse()
                            .ok()
                            .filter(|delay| *delay > 0)
                            .ok_or_else(|| Rem6CliError::InvalidMinRemoteDelay {
                                value: value.clone(),
                            })?;
                }
                "--max-instructions" => {
                    let value = required_value(&flag, args.next())?;
                    max_instructions = Some(
                        value
                            .parse()
                            .ok()
                            .filter(|instructions| *instructions > 0)
                            .ok_or_else(|| Rem6CliError::InvalidMaxInstructions {
                                value: value.clone(),
                            })?,
                    );
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--execute" => {
                    execute = true;
                }
                "--cores" => {
                    let value = required_value(&flag, args.next())?;
                    cores = value
                        .parse()
                        .ok()
                        .filter(|cores| *cores > 0)
                        .ok_or_else(|| Rem6CliError::InvalidCoreCount {
                            value: value.clone(),
                        })?;
                }
                "--parallel-workers" => {
                    let value = required_value(&flag, args.next())?;
                    parallel_workers = Some(
                        value
                            .parse()
                            .ok()
                            .filter(|workers| *workers > 0)
                            .ok_or_else(|| Rem6CliError::InvalidParallelWorkerCount {
                                value: value.clone(),
                            })?,
                    );
                }
                "--dump-memory" => {
                    let value = required_value(&flag, args.next())?;
                    memory_dumps.push(MemoryDumpRequest::parse(&value)?);
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

        Ok(Self {
            isa: isa.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--isa" })?,
            binary: binary.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--binary" })?,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            min_remote_delay,
            max_instructions,
            stats_format,
            execute,
            cores,
            parallel_workers: parallel_workers.unwrap_or(cores),
            memory_dumps,
            output,
            stats_output,
        })
    }

    pub const fn isa(&self) -> RequestedIsa {
        self.isa
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub const fn max_tick(&self) -> u64 {
        self.max_tick
    }

    pub const fn min_remote_delay(&self) -> u64 {
        self.min_remote_delay
    }

    pub const fn max_instructions(&self) -> Option<u64> {
        self.max_instructions
    }

    pub const fn stats_format(&self) -> StatsFormat {
        self.stats_format
    }

    pub const fn execute(&self) -> bool {
        self.execute
    }

    pub const fn cores(&self) -> usize {
        self.cores
    }

    pub const fn parallel_workers(&self) -> usize {
        self.parallel_workers
    }

    pub fn memory_dumps(&self) -> &[MemoryDumpRequest] {
        &self.memory_dumps
    }

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryDumpRequest {
    address: u64,
    bytes: u64,
}

impl MemoryDumpRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let Some((address, bytes)) = value.split_once(':') else {
            return Err(Rem6CliError::InvalidMemoryDump {
                value: value.to_string(),
            });
        };
        let address = parse_number(address).ok_or_else(|| Rem6CliError::InvalidMemoryDump {
            value: value.to_string(),
        })?;
        let bytes = parse_number(bytes)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| Rem6CliError::InvalidMemoryDump {
                value: value.to_string(),
            })?;
        Ok(Self { address, bytes })
    }

    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunArtifact {
    schema: &'static str,
    config: Rem6RunConfig,
    binary_bytes: u64,
    entry: u64,
    metadata: rem6_boot::BootElfMetadata,
    load_segments: u64,
    execution: Option<Rem6ExecutionSummary>,
    stats_json: String,
    stats_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ExecutionSummary {
    final_tick: u64,
    stop: Rem6ExecutionStop,
    committed_instructions: u64,
    data_loads: u64,
    data_stores: u64,
    data_atomics: u64,
    data_load_bytes: u64,
    data_store_bytes: u64,
    data_atomic_bytes: u64,
    parallel_scheduler_epochs: u64,
    parallel_scheduler_dispatches: u64,
    parallel_scheduler_batches: u64,
    parallel_scheduler_max_workers: u64,
    parallel_scheduler_total_workers: u64,
    parallel_scheduler_active_partitions: u64,
    parallel_scheduler_remote_sends: u64,
    parallel_scheduler_batch_worker_ticks: u64,
    parallel_scheduler_batch_worker_capacity_ticks: u64,
    parallel_scheduler_batch_idle_worker_ticks: u64,
    parallel_scheduler_worker_slots: Vec<Rem6ParallelWorkerSlotSummary>,
    parallel_scheduler_worker_lanes: Vec<Rem6ParallelWorkerLaneSummary>,
    parallel_scheduler_partitions: Vec<Rem6ParallelPartitionSummary>,
    parallel_scheduler_frontiers: Vec<Rem6ParallelFrontierSummary>,
    parallel_scheduler_final_frontiers: Vec<Rem6ParallelFrontierSummary>,
    parallel_scheduler_ready_partitions: Vec<Rem6ParallelReadyPartitionSummary>,
    fetch_transport: Rem6MemoryTransportSummary,
    data_transport: Rem6MemoryTransportSummary,
    cores: Vec<Rem6CoreSummary>,
    memory_dumps: Vec<Rem6MemoryDump>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Rem6ExecutionStop {
    HostTrap { stop_code: i32, trap: &'static str },
    TickLimit { tick_limit: u64 },
    InstructionLimit { instruction_limit: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ParallelWorkerSlotSummary {
    slot: usize,
    active_ticks: u64,
    idle_ticks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ParallelWorkerLaneSummary {
    lane: usize,
    partition: u32,
    active_ticks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ParallelPartitionSummary {
    partition: u32,
    workers: u64,
    dispatches: u64,
    remote_sends: u64,
    remote_receives: u64,
    max_pending_events: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ParallelFrontierSummary {
    partition: u32,
    now: u64,
    safe_until: u64,
    next_tick: Option<u64>,
    pending_events: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ParallelReadyPartitionSummary {
    partition: u32,
    next_tick: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6MemoryTransportSummary {
    counters: Rem6MemoryTransportCounters,
    routes: Vec<Rem6MemoryTransportRouteSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6MemoryTransportRouteSummary {
    route: MemoryRouteId,
    source: String,
    counters: Rem6MemoryTransportCounters,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Rem6MemoryTransportCounters {
    requests: u64,
    request_arrivals: u64,
    responses: u64,
    response_arrivals: u64,
    round_trip_ticks: u64,
    max_round_trip_ticks: u64,
}

struct ExecutionSummaryInputs<'a> {
    core_count: u32,
    store: &'a Arc<Mutex<PartitionedMemoryStore>>,
    line_layout: CacheLineLayout,
    config: &'a Rem6RunConfig,
    fetch_trace: &'a MemoryTrace,
    data_trace: &'a MemoryTrace,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6CoreSummary {
    cpu: u32,
    pc: u64,
    committed_instructions: u64,
    data_loads: u64,
    data_stores: u64,
    data_atomics: u64,
    data_load_bytes: u64,
    data_store_bytes: u64,
    data_atomic_bytes: u64,
    registers: Vec<(u8, u64)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6MemoryDump {
    address: u64,
    data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Rem6CliError {
    MissingCommand,
    UnsupportedCommand {
        command: String,
    },
    UnknownFlag {
        flag: String,
    },
    MissingFlagValue {
        flag: String,
    },
    MissingRequiredFlag {
        flag: &'static str,
    },
    UnsupportedIsa {
        isa: String,
    },
    UnsupportedStatsFormat {
        format: String,
    },
    InvalidMaxTick {
        value: String,
    },
    InvalidMinRemoteDelay {
        value: String,
    },
    InvalidMaxInstructions {
        value: String,
    },
    InvalidCoreCount {
        value: String,
    },
    InvalidParallelWorkerCount {
        value: String,
    },
    InvalidMemoryDump {
        value: String,
    },
    InstructionLimitRequiresExecution,
    MemoryDumpRequiresExecution,
    ReadBinary {
        path: PathBuf,
        error: String,
    },
    LoadBinary {
        path: PathBuf,
        error: String,
    },
    MissingElfMetadata {
        path: PathBuf,
    },
    IsaMismatch {
        requested: RequestedIsa,
        architecture: BootElfArchitecture,
    },
    UnsupportedExecutionIsa {
        isa: RequestedIsa,
    },
    Execute {
        error: String,
    },
    Stats {
        error: String,
    },
    ConflictingOutputPaths {
        path: PathBuf,
    },
    WriteOutput {
        path: PathBuf,
        error: String,
    },
}

impl fmt::Display for Rem6CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingCommand => write!(formatter, "missing command"),
            Self::UnsupportedCommand { command } => {
                write!(formatter, "unsupported command {command}")
            }
            Self::UnknownFlag { flag } => write!(formatter, "unknown flag {flag}"),
            Self::MissingFlagValue { flag } => {
                write!(formatter, "missing value for flag {flag}")
            }
            Self::MissingRequiredFlag { flag } => {
                write!(formatter, "missing required flag {flag}")
            }
            Self::UnsupportedIsa { isa } => write!(formatter, "unsupported ISA {isa}"),
            Self::UnsupportedStatsFormat { format } => {
                write!(formatter, "unsupported stats format {format}")
            }
            Self::InvalidMaxTick { value } => write!(formatter, "invalid max tick {value}"),
            Self::InvalidMinRemoteDelay { value } => {
                write!(formatter, "invalid min remote delay {value}")
            }
            Self::InvalidMaxInstructions { value } => {
                write!(formatter, "invalid max instructions {value}")
            }
            Self::InvalidCoreCount { value } => write!(formatter, "invalid core count {value}"),
            Self::InvalidParallelWorkerCount { value } => {
                write!(formatter, "invalid parallel worker count {value}")
            }
            Self::InvalidMemoryDump { value } => {
                write!(formatter, "invalid memory dump request {value}")
            }
            Self::InstructionLimitRequiresExecution => {
                write!(formatter, "--max-instructions requires --execute")
            }
            Self::MemoryDumpRequiresExecution => {
                write!(formatter, "--dump-memory requires --execute")
            }
            Self::ReadBinary { path, error } => {
                write!(formatter, "failed to read {}: {error}", path.display())
            }
            Self::LoadBinary { path, error } => {
                write!(
                    formatter,
                    "failed to load {} as ELF: {error}",
                    path.display()
                )
            }
            Self::MissingElfMetadata { path } => {
                write!(formatter, "{} did not produce ELF metadata", path.display())
            }
            Self::IsaMismatch {
                requested,
                architecture,
            } => write!(
                formatter,
                "requested ISA {} does not match ELF architecture {}",
                requested.as_str(),
                elf_architecture_name(*architecture)
            ),
            Self::UnsupportedExecutionIsa { isa } => {
                write!(
                    formatter,
                    "execution is not implemented for ISA {}",
                    isa.as_str()
                )
            }
            Self::Execute { error } => write!(formatter, "failed to execute run: {error}"),
            Self::Stats { error } => write!(formatter, "failed to build run stats: {error}"),
            Self::ConflictingOutputPaths { path } => write!(
                formatter,
                "--output and --stats-output must use different paths: {}",
                path.display()
            ),
            Self::WriteOutput { path, error } => {
                write!(formatter, "failed to write {}: {error}", path.display())
            }
        }
    }
}

impl Error for Rem6CliError {}

pub fn run_cli<I, S>(args: I) -> Result<String, Rem6CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let config = Rem6RunConfig::parse_args(args)?;
    let artifact = run_config(config)?;
    let stats_format = artifact.config.stats_format();
    let output = match stats_format {
        StatsFormat::Json => artifact.to_json(),
        StatsFormat::Text => artifact.stats_text.clone(),
    };
    if let Some(path) = artifact.config.stats_output() {
        let stats_output = match stats_format {
            StatsFormat::Json => format!("{}\n", artifact.stats_json),
            StatsFormat::Text => artifact.stats_text.clone(),
        };
        std::fs::write(path, stats_output).map_err(|error| Rem6CliError::WriteOutput {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;
    }
    if let Some(path) = artifact.config.output() {
        std::fs::write(path, output).map_err(|error| Rem6CliError::WriteOutput {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;
        return Ok(output_envelope_json(
            path,
            artifact.config.stats_output(),
            stats_format,
        ));
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

pub fn run_config(config: Rem6RunConfig) -> Result<Rem6RunArtifact, Rem6CliError> {
    let bytes = std::fs::read(config.binary()).map_err(|error| Rem6CliError::ReadBinary {
        path: config.binary().to_path_buf(),
        error: error.to_string(),
    })?;
    let image = BootImage::from_elf(&bytes).map_err(|error| Rem6CliError::LoadBinary {
        path: config.binary().to_path_buf(),
        error: error.to_string(),
    })?;
    let metadata = image
        .elf_metadata()
        .ok_or_else(|| Rem6CliError::MissingElfMetadata {
            path: config.binary().to_path_buf(),
        })?;
    if !config.isa().matches_architecture(metadata.architecture()) {
        return Err(Rem6CliError::IsaMismatch {
            requested: config.isa(),
            architecture: metadata.architecture(),
        });
    }

    if !config.execute() {
        if config.max_instructions().is_some() {
            return Err(Rem6CliError::InstructionLimitRequiresExecution);
        }
        if !config.memory_dumps().is_empty() {
            return Err(Rem6CliError::MemoryDumpRequiresExecution);
        }
    }

    let execution = if config.execute() {
        Some(match config.isa() {
            RequestedIsa::Riscv => execute_riscv(&image, &config)?,
            isa => return Err(Rem6CliError::UnsupportedExecutionIsa { isa }),
        })
    } else {
        None
    };
    let stats = run_stats_output(Rem6StatsInputs {
        binary_bytes: bytes.len() as u64,
        load_segments: image.segments().len() as u64,
        config: &config,
        execution: execution.as_ref(),
    })?;
    Ok(Rem6RunArtifact {
        schema: "rem6.cli.run.v1",
        binary_bytes: bytes.len() as u64,
        entry: image.entry().get(),
        load_segments: image.segments().len() as u64,
        metadata,
        config,
        execution,
        stats_json: stats.json,
        stats_text: stats.text,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Rem6StatsOutput {
    json: String,
    text: String,
}

struct Rem6StatsInputs<'a> {
    binary_bytes: u64,
    load_segments: u64,
    config: &'a Rem6RunConfig,
    execution: Option<&'a Rem6ExecutionSummary>,
}

fn run_stats_output(inputs: Rem6StatsInputs<'_>) -> Result<Rem6StatsOutput, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.binary.bytes",
        "Byte",
        StatResetPolicy::Constant,
        inputs.binary_bytes,
    )?;
    increment_stat(
        &mut stats,
        "sim.elf.load_segments",
        "Count",
        StatResetPolicy::Constant,
        inputs.load_segments,
    )?;
    increment_stat(
        &mut stats,
        "sim.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.max_tick(),
    )?;
    increment_stat(
        &mut stats,
        "sim.parallel.scheduler.min_remote_delay",
        "Tick",
        StatResetPolicy::Constant,
        inputs.config.min_remote_delay(),
    )?;
    if let Some(max_instructions) = inputs.config.max_instructions() {
        increment_stat(
            &mut stats,
            "sim.instructions.limit",
            "Count",
            StatResetPolicy::Constant,
            max_instructions,
        )?;
    }
    increment_stat(
        &mut stats,
        "sim.cores",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.cores() as u64,
    )?;
    increment_stat(
        &mut stats,
        "sim.parallel.scheduler.worker_limit",
        "Count",
        StatResetPolicy::Constant,
        inputs.config.parallel_workers() as u64,
    )?;

    if let Some(execution) = inputs.execution {
        increment_stat(
            &mut stats,
            "sim.instructions.committed",
            "Count",
            StatResetPolicy::Monotonic,
            execution.committed_instructions,
        )?;
        increment_stat(
            &mut stats,
            "sim.final_tick",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.final_tick,
        )?;
        match execution.stop {
            Rem6ExecutionStop::HostTrap { stop_code, .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.host_trap",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
                increment_stat(
                    &mut stats,
                    "sim.stop_code",
                    "Count",
                    StatResetPolicy::Constant,
                    stop_code as u64,
                )?;
            }
            Rem6ExecutionStop::TickLimit { .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.tick_limit",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
            }
            Rem6ExecutionStop::InstructionLimit { .. } => {
                increment_stat(
                    &mut stats,
                    "sim.stop.instruction_limit",
                    "Count",
                    StatResetPolicy::Constant,
                    1,
                )?;
            }
        }
        increment_stat(
            &mut stats,
            "sim.memory.dumps",
            "Count",
            StatResetPolicy::Constant,
            execution.memory_dumps.len() as u64,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.loads",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_loads,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.stores",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_stores,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.atomics",
            "Count",
            StatResetPolicy::Monotonic,
            execution.data_atomics,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.load_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_load_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.store_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_store_bytes,
        )?;
        increment_stat(
            &mut stats,
            "sim.data.atomic_bytes",
            "Byte",
            StatResetPolicy::Monotonic,
            execution.data_atomic_bytes,
        )?;
        parallel_stats::emit_scheduler_stats(&mut stats, execution)?;
        emit_transport_stats(&mut stats, "sim.memory.fetch", &execution.fetch_transport)?;
        emit_transport_stats(&mut stats, "sim.memory.data", &execution.data_transport)?;
        for core in &execution.cores {
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.instructions.committed", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.committed_instructions,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.loads", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_loads,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.stores", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_stores,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.atomics", core.cpu),
                "Count",
                StatResetPolicy::Monotonic,
                core.data_atomics,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.load_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_load_bytes,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.store_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_store_bytes,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.cpu{}.data.atomic_bytes", core.cpu),
                "Byte",
                StatResetPolicy::Monotonic,
                core.data_atomic_bytes,
            )?;
        }
    }

    let snapshot = stats.snapshot(0);
    Ok(Rem6StatsOutput {
        json: stats_snapshot_json(&snapshot),
        text: stats_snapshot_text(&snapshot),
    })
}

fn stats_snapshot_json(snapshot: &StatSnapshot) -> String {
    let samples = snapshot
        .samples()
        .iter()
        .map(|sample| {
            format!(
                "{{\"path\":\"{}\",\"unit\":\"{}\",\"value\":{},\"reset_policy\":\"{}\"}}",
                json_escape(sample.path()),
                json_escape(sample.unit()),
                sample.value(),
                sample.reset_policy()
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{samples}]")
}

fn stats_snapshot_text(snapshot: &StatSnapshot) -> String {
    let mut output = "\n---------- Begin Simulation Statistics ----------\n".to_string();
    for sample in snapshot.samples() {
        output.push_str(&format!(
            "{:<64} {:>20} # unit={} reset_policy={}\n",
            sample.path(),
            sample.value(),
            sample.unit(),
            sample.reset_policy()
        ));
    }
    output.push_str("\n---------- End Simulation Statistics   ----------\n");
    output
}

fn increment_stat(
    stats: &mut StatsRegistry,
    path: &str,
    unit: &str,
    reset_policy: StatResetPolicy,
    value: u64,
) -> Result<(), Rem6CliError> {
    let stat = stats
        .register_counter_with_reset_policy(path, unit, reset_policy)
        .map_err(stats_error)?;
    stats.increment(stat, value).map_err(stats_error)
}

fn emit_transport_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    summary: &Rem6MemoryTransportSummary,
) -> Result<(), Rem6CliError> {
    emit_transport_counters(stats, prefix, &summary.counters)?;
    for route in &summary.routes {
        let route_prefix = format!(
            "{prefix}.route{}.source.{}",
            route.route.get(),
            endpoint_stat_path(&route.source)
        );
        emit_transport_counters(stats, &route_prefix, &route.counters)?;
    }
    Ok(())
}

fn emit_transport_counters(
    stats: &mut StatsRegistry,
    prefix: &str,
    counters: &Rem6MemoryTransportCounters,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        &format!("{prefix}.requests"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.requests,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.request_arrivals"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.request_arrivals,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.responses"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.responses,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.response_arrivals"),
        "Count",
        StatResetPolicy::Monotonic,
        counters.response_arrivals,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.round_trip_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        counters.round_trip_ticks,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.max_round_trip_ticks"),
        "Tick",
        StatResetPolicy::Monotonic,
        counters.max_round_trip_ticks,
    )
}

fn endpoint_stat_path(endpoint: &str) -> String {
    endpoint
        .split('.')
        .map(stat_path_segment)
        .collect::<Vec<_>>()
        .join(".")
}

fn stat_path_segment(segment: &str) -> String {
    let mut output = String::new();
    for (index, character) in segment.chars().enumerate() {
        if index == 0 {
            if character.is_ascii_alphabetic() || character == '_' {
                output.push(character);
            } else {
                output.push('_');
                if character.is_ascii_alphanumeric() {
                    output.push(character);
                }
            }
        } else if character.is_ascii_alphanumeric() || character == '_' {
            output.push(character);
        } else {
            output.push('_');
        }
    }
    if output.is_empty() {
        "_".to_string()
    } else {
        output
    }
}

fn execute_riscv(
    image: &BootImage,
    config: &Rem6RunConfig,
) -> Result<Rem6ExecutionSummary, Rem6CliError> {
    let core_count = u32::try_from(config.cores()).map_err(|_| Rem6CliError::InvalidCoreCount {
        value: config.cores().to_string(),
    })?;
    let partition_count =
        core_count
            .checked_add(2)
            .ok_or_else(|| Rem6CliError::InvalidCoreCount {
                value: config.cores().to_string(),
            })?;
    let tick_limit = config.max_tick();
    let memory_partition = PartitionId::new(core_count);
    let host_partition = PartitionId::new(core_count + 1);
    let line_layout = CacheLineLayout::new(DEFAULT_CACHE_LINE_BYTES).map_err(execute_error)?;
    let mut store = PartitionedMemoryStore::new();
    store
        .add_partition(CLI_MEMORY_TARGET, line_layout)
        .map_err(execute_error)?;
    for segment in image.segments() {
        store
            .map_region(
                CLI_MEMORY_TARGET,
                segment.range().start(),
                segment.range().size(),
            )
            .map_err(execute_error)?;
    }
    image
        .load_into_partitioned_store(&mut store, CLI_MEMORY_TARGET)
        .map_err(execute_error)?;
    let store = Arc::new(Mutex::new(store));

    let mut scheduler = PartitionedScheduler::with_parallel_worker_limit(
        partition_count,
        config.min_remote_delay(),
        config.parallel_workers(),
    )
    .map_err(execute_error)?;
    let mut transport = MemoryTransport::new();
    let mut cores = Vec::new();
    for cpu_index in 0..core_count {
        let cpu_partition = PartitionId::new(cpu_index);
        let fetch_route = add_memory_route(
            &mut transport,
            format!("cpu{cpu_index}.ifetch"),
            cpu_partition,
            memory_partition,
            config.min_remote_delay(),
        )?;
        let data_route = add_memory_route(
            &mut transport,
            format!("cpu{cpu_index}.dmem"),
            cpu_partition,
            memory_partition,
            config.min_remote_delay(),
        )?;
        let core = RiscvCore::with_data(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(cpu_index),
                    cpu_partition,
                    AgentId::new(cpu_index),
                    image.entry(),
                ),
                CpuFetchConfig::new(
                    transport_endpoint(format!("cpu{cpu_index}.ifetch"))?,
                    fetch_route,
                    line_layout,
                    AccessSize::new(4).map_err(execute_error)?,
                ),
            )
            .map_err(execute_error)?,
            CpuDataConfig::new(
                transport_endpoint(format!("cpu{cpu_index}.dmem"))?,
                data_route,
                line_layout,
            ),
        );
        cores.push(core);
    }
    let cluster = RiscvCluster::new(cores).map_err(execute_error)?;
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    let trap_port = RiscvTrapEventPort::new(
        SystemHostEventPort::with_controller(
            host_partition,
            config.min_remote_delay(),
            Arc::clone(&controller),
        )
        .map_err(execute_error)?,
        GuestSourceId::new(1),
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let fetch_trace = MemoryTrace::new();
    let data_trace = MemoryTrace::new();
    let run = match config.max_instructions() {
        Some(max_instructions) => driver
            .drive_until_host_stop_or_instruction_limit_parallel(
                &cluster,
                &mut scheduler,
                &transport,
                fetch_trace.clone(),
                data_trace.clone(),
                |_cpu| {
                    let store = Arc::clone(&store);
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = Arc::clone(&store);
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                tick_limit,
                max_instructions,
                |cpu| GuestEventId::new(u64::from(cpu.get())),
            )
            .map_err(execute_error)?,
        None => driver
            .drive_until_host_stop_or_tick_limit_parallel(
                &cluster,
                &mut scheduler,
                &transport,
                fetch_trace.clone(),
                data_trace.clone(),
                |_cpu| {
                    let store = Arc::clone(&store);
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                |_cpu| {
                    let store = Arc::clone(&store);
                    move |delivery, _context| memory_response(&store, &delivery)
                },
                tick_limit,
                |cpu| GuestEventId::new(u64::from(cpu.get())),
            )
            .map_err(execute_error)?,
    };

    let summary_inputs = ExecutionSummaryInputs {
        core_count,
        store: &store,
        line_layout,
        config,
        fetch_trace: &fetch_trace,
        data_trace: &data_trace,
    };

    execution_summary(&cluster, &run, summary_inputs)
}

fn add_memory_route(
    transport: &mut MemoryTransport,
    source: String,
    cpu_partition: PartitionId,
    memory_partition: PartitionId,
    route_delay: u64,
) -> Result<MemoryRouteId, Rem6CliError> {
    transport
        .add_route(
            MemoryRoute::new(
                transport_endpoint(source)?,
                cpu_partition,
                transport_endpoint("memory".to_string())?,
                memory_partition,
                route_delay,
                route_delay,
            )
            .map_err(execute_error)?,
        )
        .map_err(execute_error)
}

fn memory_response(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    let outcome = store
        .lock()
        .expect("CLI memory store lock")
        .respond(delivery.request())
        .expect("CLI memory response");
    match outcome.response().cloned() {
        Some(response) => TargetOutcome::Respond(response),
        None => TargetOutcome::NoResponse,
    }
}

fn execution_summary(
    cluster: &RiscvCluster,
    run: &RiscvSystemRun,
    inputs: ExecutionSummaryInputs<'_>,
) -> Result<Rem6ExecutionSummary, Rem6CliError> {
    let committed_by_cpu = committed_instructions_by_cpu(run);
    let committed_instructions = committed_by_cpu.values().sum();
    let final_tick = run.final_tick().ok_or_else(|| Rem6CliError::Execute {
        error: "RISC-V execution stopped without a final tick".to_string(),
    })?;
    let stop = match run.stop_reason() {
        RiscvSystemRunStopReason::HostStop(stop) => {
            let scheduled_trap =
                run.scheduled_traps()
                    .first()
                    .ok_or_else(|| Rem6CliError::Execute {
                        error: "RISC-V execution reached host stop without a scheduled trap"
                            .to_string(),
                    })?;
            Rem6ExecutionStop::HostTrap {
                stop_code: stop.code(),
                trap: guest_trap_name(scheduled_trap.trap().kind()),
            }
        }
        RiscvSystemRunStopReason::InstructionLimit { limit, .. } => {
            Rem6ExecutionStop::InstructionLimit {
                instruction_limit: limit,
            }
        }
        RiscvSystemRunStopReason::TickLimit { limit, .. } => {
            Rem6ExecutionStop::TickLimit { tick_limit: limit }
        }
        RiscvSystemRunStopReason::Idle { .. } => {
            return Err(Rem6CliError::Execute {
                error: "RISC-V execution stopped without a host trap".to_string(),
            });
        }
    };
    let mut cores = Vec::new();
    let mut data_loads = 0;
    let mut data_stores = 0;
    let mut data_atomics = 0;
    let mut data_load_bytes = 0;
    let mut data_store_bytes = 0;
    let mut data_atomic_bytes = 0;
    for cpu_index in 0..inputs.core_count {
        let cpu = CpuId::new(cpu_index);
        let core = cluster.core(cpu).map_err(execute_error)?;
        let data = core_data_access_counts(&core);
        data_loads += data.loads;
        data_stores += data.stores;
        data_atomics += data.atomics;
        data_load_bytes += data.load_bytes;
        data_store_bytes += data.store_bytes;
        data_atomic_bytes += data.atomic_bytes;
        let mut registers = Vec::new();
        for register_index in 1..32 {
            let register = Register::new(register_index).map_err(execute_error)?;
            let value = core.read_register(register);
            if value != 0 {
                registers.push((register_index, value));
            }
        }
        cores.push(Rem6CoreSummary {
            cpu: cpu_index,
            pc: core.pc().get(),
            committed_instructions: committed_by_cpu.get(&cpu).copied().unwrap_or(0),
            data_loads: data.loads,
            data_stores: data.stores,
            data_atomics: data.atomics,
            data_load_bytes: data.load_bytes,
            data_store_bytes: data.store_bytes,
            data_atomic_bytes: data.atomic_bytes,
            registers,
        });
    }

    Ok(Rem6ExecutionSummary {
        final_tick,
        stop,
        committed_instructions,
        data_loads,
        data_stores,
        data_atomics,
        data_load_bytes,
        data_store_bytes,
        data_atomic_bytes,
        parallel_scheduler_epochs: run.parallel_scheduler_epochs().len() as u64,
        parallel_scheduler_dispatches: run.parallel_scheduler_dispatches().len() as u64,
        parallel_scheduler_batches: run.parallel_scheduler_batches().len() as u64,
        parallel_scheduler_max_workers: run.max_parallel_scheduler_workers() as u64,
        parallel_scheduler_total_workers: run.parallel_scheduler_workers().len() as u64,
        parallel_scheduler_active_partitions: run.active_parallel_scheduler_partition_count()
            as u64,
        parallel_scheduler_remote_sends: run.parallel_scheduler_total_remote_send_count() as u64,
        parallel_scheduler_batch_worker_ticks: run.parallel_scheduler_batch_worker_ticks(),
        parallel_scheduler_batch_worker_capacity_ticks: run
            .parallel_scheduler_batch_worker_capacity_ticks(),
        parallel_scheduler_batch_idle_worker_ticks: run
            .parallel_scheduler_batch_idle_worker_ticks(),
        parallel_scheduler_worker_slots: parallel_worker_slot_summaries(run),
        parallel_scheduler_worker_lanes: parallel_worker_lane_summaries(run),
        parallel_scheduler_partitions: parallel_partition_summaries(run),
        parallel_scheduler_frontiers: parallel_frontier_summaries(
            run.parallel_scheduler_frontiers(),
        ),
        parallel_scheduler_final_frontiers: parallel_frontier_summaries(
            run.parallel_scheduler_final_frontiers(),
        ),
        parallel_scheduler_ready_partitions: parallel_ready_partition_summaries(
            run.parallel_scheduler_ready_partitions(),
        ),
        fetch_transport: memory_transport_summary(inputs.fetch_trace),
        data_transport: memory_transport_summary(inputs.data_trace),
        cores,
        memory_dumps: read_memory_dumps(
            inputs.store,
            inputs.line_layout,
            inputs.config.memory_dumps(),
        )?,
    })
}

fn parallel_worker_slot_summaries(run: &RiscvSystemRun) -> Vec<Rem6ParallelWorkerSlotSummary> {
    run.parallel_scheduler_batch_worker_slot_tick_summaries()
        .into_iter()
        .map(
            |(slot, active_ticks, idle_ticks)| Rem6ParallelWorkerSlotSummary {
                slot,
                active_ticks,
                idle_ticks,
            },
        )
        .collect()
}

fn parallel_worker_lane_summaries(run: &RiscvSystemRun) -> Vec<Rem6ParallelWorkerLaneSummary> {
    let mut summaries = BTreeMap::<(usize, u32), u64>::new();
    for lane in run.parallel_scheduler_worker_lanes() {
        let key = (lane.lane(), lane.partition().index());
        let ticks = summaries.entry(key).or_default();
        *ticks = ticks.saturating_add(lane.duration_ticks());
    }
    summaries
        .into_iter()
        .map(
            |((lane, partition), active_ticks)| Rem6ParallelWorkerLaneSummary {
                lane,
                partition,
                active_ticks,
            },
        )
        .collect()
}

fn parallel_partition_summaries(run: &RiscvSystemRun) -> Vec<Rem6ParallelPartitionSummary> {
    run.parallel_scheduler_partition_activities()
        .into_iter()
        .map(|(partition, activity)| Rem6ParallelPartitionSummary {
            partition: partition.index(),
            workers: activity.worker_count() as u64,
            dispatches: activity.dispatch_count() as u64,
            remote_sends: activity.remote_send_count() as u64,
            remote_receives: activity.remote_receive_count() as u64,
            max_pending_events: activity.max_pending_events() as u64,
        })
        .collect()
}

fn parallel_frontier_summaries(
    frontiers: Vec<PartitionFrontier>,
) -> Vec<Rem6ParallelFrontierSummary> {
    frontiers
        .into_iter()
        .map(|frontier| Rem6ParallelFrontierSummary {
            partition: frontier.partition().index(),
            now: frontier.now(),
            safe_until: frontier.safe_until(),
            next_tick: frontier.next_tick(),
            pending_events: frontier.pending_events() as u64,
        })
        .collect()
}

fn parallel_ready_partition_summaries(
    ready_partitions: Vec<ReadyPartition>,
) -> Vec<Rem6ParallelReadyPartitionSummary> {
    ready_partitions
        .into_iter()
        .map(|ready| Rem6ParallelReadyPartitionSummary {
            partition: ready.partition.index(),
            next_tick: ready.next_tick,
        })
        .collect()
}

fn memory_transport_summary(trace: &MemoryTrace) -> Rem6MemoryTransportSummary {
    let events = trace.snapshot();
    let mut request_sources: BTreeMap<(MemoryRouteId, MemoryRequestId), (u64, String)> =
        BTreeMap::new();
    let mut routes: BTreeMap<(MemoryRouteId, String), Rem6MemoryTransportRouteSummary> =
        BTreeMap::new();
    let mut summary = Rem6MemoryTransportSummary {
        counters: Rem6MemoryTransportCounters::default(),
        routes: Vec::new(),
    };

    for event in &events {
        match event.kind() {
            MemoryTraceKind::RequestSent => {
                summary.counters.requests += 1;
                let source = event.endpoint().as_str().to_string();
                route_summary(&mut routes, event.route(), &source)
                    .counters
                    .requests += 1;
                request_sources.insert(trace_key(event), (event.tick(), source));
            }
            MemoryTraceKind::RequestArrived => {
                summary.counters.request_arrivals += 1;
                if let Some((_, source)) = request_sources.get(&trace_key(event)) {
                    route_summary(&mut routes, event.route(), source)
                        .counters
                        .request_arrivals += 1;
                }
            }
            MemoryTraceKind::ResponseArrived => {
                summary.counters.response_arrivals += 1;
                let Some((sent_tick, source)) = request_sources.get(&trace_key(event)) else {
                    continue;
                };
                let route = route_summary(&mut routes, event.route(), source);
                route.counters.response_arrivals += 1;
                if event.endpoint().as_str() != source {
                    continue;
                }
                let latency = event.tick().saturating_sub(*sent_tick);
                summary.counters.responses += 1;
                summary.counters.round_trip_ticks =
                    summary.counters.round_trip_ticks.saturating_add(latency);
                summary.counters.max_round_trip_ticks =
                    summary.counters.max_round_trip_ticks.max(latency);
                route.counters.responses += 1;
                route.counters.round_trip_ticks =
                    route.counters.round_trip_ticks.saturating_add(latency);
                route.counters.max_round_trip_ticks =
                    route.counters.max_round_trip_ticks.max(latency);
            }
        }
    }

    summary.routes = routes.into_values().collect();
    summary
}

fn route_summary<'a>(
    routes: &'a mut BTreeMap<(MemoryRouteId, String), Rem6MemoryTransportRouteSummary>,
    route: MemoryRouteId,
    source: &str,
) -> &'a mut Rem6MemoryTransportRouteSummary {
    routes
        .entry((route, source.to_string()))
        .or_insert_with(|| Rem6MemoryTransportRouteSummary {
            route,
            source: source.to_string(),
            counters: Rem6MemoryTransportCounters::default(),
        })
}

fn trace_key(event: &MemoryTraceEvent) -> (MemoryRouteId, MemoryRequestId) {
    (event.route(), event.request_id())
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct DataAccessCounts {
    loads: u64,
    stores: u64,
    atomics: u64,
    load_bytes: u64,
    store_bytes: u64,
    atomic_bytes: u64,
}

fn core_data_access_counts(core: &RiscvCore) -> DataAccessCounts {
    let mut counts = DataAccessCounts::default();
    for event in core.data_access_events() {
        if event.kind() != RiscvDataAccessEventKind::Completed {
            continue;
        }
        let bytes = event.size().bytes();
        match event.operation() {
            MemoryOperation::ReadShared | MemoryOperation::ReadUnique => {
                counts.loads += 1;
                counts.load_bytes += bytes;
            }
            MemoryOperation::Write => {
                counts.stores += 1;
                counts.store_bytes += bytes;
            }
            MemoryOperation::Atomic => {
                counts.atomics += 1;
                counts.atomic_bytes += bytes;
            }
            _ => {}
        }
    }
    counts
}

fn read_memory_dumps(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    line_layout: CacheLineLayout,
    requests: &[MemoryDumpRequest],
) -> Result<Vec<Rem6MemoryDump>, Rem6CliError> {
    requests
        .iter()
        .enumerate()
        .map(|(index, request)| read_memory_dump(store, line_layout, index as u64, *request))
        .collect()
}

fn read_memory_dump(
    store: &Arc<Mutex<PartitionedMemoryStore>>,
    line_layout: CacheLineLayout,
    sequence: u64,
    dump: MemoryDumpRequest,
) -> Result<Rem6MemoryDump, Rem6CliError> {
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(CLI_MEMORY_DUMP_AGENT, sequence),
        Address::new(dump.address()),
        AccessSize::new(dump.bytes()).map_err(execute_error)?,
        line_layout,
    )
    .map_err(execute_error)?;
    let outcome = store
        .lock()
        .expect("CLI memory store lock")
        .respond(&request)
        .map_err(execute_error)?;
    let data = outcome
        .response()
        .and_then(|response| response.data())
        .ok_or_else(|| Rem6CliError::Execute {
            error: format!("memory dump at 0x{:x} returned no data", dump.address()),
        })?
        .to_vec();
    Ok(Rem6MemoryDump {
        address: dump.address(),
        data,
    })
}

fn committed_instructions_by_cpu(run: &RiscvSystemRun) -> BTreeMap<CpuId, u64> {
    let mut committed = BTreeMap::new();
    for event in run.turns().iter().flat_map(|turn| turn.core_events()) {
        if matches!(event.action(), RiscvCoreDriveAction::InstructionExecuted(_)) {
            *committed.entry(event.cpu()).or_insert(0) += 1;
        }
    }
    committed
}

fn guest_trap_name(kind: GuestTrapKind) -> &'static str {
    match kind {
        GuestTrapKind::EnvironmentCall => "environment_call",
        GuestTrapKind::Breakpoint => "breakpoint",
    }
}

fn transport_endpoint(value: String) -> Result<TransportEndpointId, Rem6CliError> {
    TransportEndpointId::new(value).map_err(execute_error)
}

fn execute_error(error: impl fmt::Display) -> Rem6CliError {
    Rem6CliError::Execute {
        error: error.to_string(),
    }
}

fn parse_number(value: &str) -> Option<u64> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
}

fn stats_error(error: rem6_stats::StatsError) -> Rem6CliError {
    Rem6CliError::Stats {
        error: error.to_string(),
    }
}
