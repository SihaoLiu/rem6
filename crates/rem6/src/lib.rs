use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfOperatingSystem, BootImage,
};
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvCoreDriveAction, RiscvDataAccessEventKind,
};
use rem6_isa_riscv::Register;
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest, MemoryRequestId,
    MemoryTargetId, PartitionedMemoryStore,
};
use rem6_stats::{StatResetPolicy, StatsRegistry};
use rem6_system::{
    GuestEventId, GuestSourceId, GuestTrapKind, HostEventPolicy, RiscvSystemRun,
    RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTrapEventPort, SystemHostController,
    SystemHostEventPort,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    RequestDelivery, TargetOutcome, TransportEndpointId,
};

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
}

impl StatsFormat {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "json" => Ok(Self::Json),
            _ => Err(Rem6CliError::UnsupportedStatsFormat {
                format: value.to_string(),
            }),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunConfig {
    isa: RequestedIsa,
    binary: PathBuf,
    max_tick: u64,
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

        Ok(Self {
            isa: isa.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--isa" })?,
            binary: binary.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--binary" })?,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
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
}

impl Rem6RunArtifact {
    pub fn to_json(&self) -> String {
        let simulation = match &self.execution {
            Some(execution) => execution.to_simulation_json(self.config.max_tick()),
            None => format!(
                "{{\"status\":\"loaded\",\"max_tick\":{},\"executed_ticks\":0,\"cores\":{}}}",
                self.config.max_tick(),
                self.config.cores(),
            ),
        };
        let cores = match &self.execution {
            Some(execution) => execution.to_cores_json(),
            None => "[]".to_string(),
        };
        let memory = match &self.execution {
            Some(execution) => execution.to_memory_json(),
            None => "[]".to_string(),
        };
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"entry\":\"0x{:x}\",\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{}}},\"simulation\":{},\"cores\":{},\"memory\":{},\"stats\":{}}}\n",
            self.schema,
            self.config.isa().as_str(),
            json_escape(&self.config.binary().display().to_string()),
            self.entry,
            elf_class_name(self.metadata.class()),
            elf_endian_name(self.metadata.endian()),
            elf_architecture_name(self.metadata.architecture()),
            elf_os_name(self.metadata.operating_system()),
            self.metadata.machine(),
            self.metadata.flags(),
            simulation,
            cores,
            memory,
            self.stats_json,
        )
    }

    pub const fn binary_bytes(&self) -> u64 {
        self.binary_bytes
    }

    pub const fn load_segments(&self) -> u64 {
        self.load_segments
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ExecutionSummary {
    final_tick: u64,
    stop_code: i32,
    trap: &'static str,
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
    fetch_transport: Rem6MemoryTransportSummary,
    data_transport: Rem6MemoryTransportSummary,
    cores: Vec<Rem6CoreSummary>,
    memory_dumps: Vec<Rem6MemoryDump>,
}

impl Rem6ExecutionSummary {
    fn to_simulation_json(&self, max_tick: u64) -> String {
        format!(
            "{{\"status\":\"executed_until_trap\",\"max_tick\":{},\"executed_ticks\":{},\"final_tick\":{},\"cores\":{},\"stop_code\":{},\"trap\":\"{}\"}}",
            max_tick,
            self.final_tick,
            self.final_tick,
            self.cores.len(),
            self.stop_code,
            self.trap,
        )
    }

    fn to_cores_json(&self) -> String {
        let cores = self
            .cores
            .iter()
            .map(Rem6CoreSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{cores}]")
    }

    fn to_memory_json(&self) -> String {
        let dumps = self
            .memory_dumps
            .iter()
            .map(Rem6MemoryDump::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!("[{dumps}]")
    }
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

impl Rem6CoreSummary {
    fn to_json(&self) -> String {
        let registers = self
            .registers
            .iter()
            .map(|(register, value)| format!("\"x{}\":\"0x{:x}\"", register, value))
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"cpu\":{},\"pc\":\"0x{:x}\",\"committed_instructions\":{},\"data_loads\":{},\"data_stores\":{},\"data_atomics\":{},\"data_load_bytes\":{},\"data_store_bytes\":{},\"data_atomic_bytes\":{},\"registers\":{{{}}}}}",
            self.cpu,
            self.pc,
            self.committed_instructions,
            self.data_loads,
            self.data_stores,
            self.data_atomics,
            self.data_load_bytes,
            self.data_store_bytes,
            self.data_atomic_bytes,
            registers
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6MemoryDump {
    address: u64,
    data: Vec<u8>,
}

impl Rem6MemoryDump {
    fn to_json(&self) -> String {
        format!(
            "{{\"address\":\"0x{:x}\",\"bytes\":{},\"hex\":\"{}\"}}",
            self.address,
            self.data.len(),
            bytes_to_hex(&self.data),
        )
    }
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
    InvalidCoreCount {
        value: String,
    },
    InvalidParallelWorkerCount {
        value: String,
    },
    InvalidMemoryDump {
        value: String,
    },
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
            Self::InvalidCoreCount { value } => write!(formatter, "invalid core count {value}"),
            Self::InvalidParallelWorkerCount { value } => {
                write!(formatter, "invalid parallel worker count {value}")
            }
            Self::InvalidMemoryDump { value } => {
                write!(formatter, "invalid memory dump request {value}")
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
    let output = match artifact.config.stats_format() {
        StatsFormat::Json => artifact.to_json(),
    };
    if let Some(path) = artifact.config.stats_output() {
        std::fs::write(path, format!("{}\n", artifact.stats_json)).map_err(|error| {
            Rem6CliError::WriteOutput {
                path: path.to_path_buf(),
                error: error.to_string(),
            }
        })?;
    }
    if let Some(path) = artifact.config.output() {
        std::fs::write(path, output).map_err(|error| Rem6CliError::WriteOutput {
            path: path.to_path_buf(),
            error: error.to_string(),
        })?;
        return Ok(format!(
            "{{\"schema\":\"rem6.cli.output.v1\",\"format\":\"json\",\"artifact\":\"{}\"}}\n",
            json_escape(&path.display().to_string())
        ));
    }
    Ok(output)
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

    if !config.execute() && !config.memory_dumps().is_empty() {
        return Err(Rem6CliError::MemoryDumpRequiresExecution);
    }

    let execution = if config.execute() {
        Some(match config.isa() {
            RequestedIsa::Riscv => execute_riscv(&image, &config)?,
            isa => return Err(Rem6CliError::UnsupportedExecutionIsa { isa }),
        })
    } else {
        None
    };
    let stats_json = run_stats_json(
        bytes.len() as u64,
        image.segments().len() as u64,
        config.max_tick(),
        config.cores() as u64,
        config.parallel_workers() as u64,
        execution.as_ref(),
    )?;
    Ok(Rem6RunArtifact {
        schema: "rem6.cli.run.v1",
        binary_bytes: bytes.len() as u64,
        entry: image.entry().get(),
        load_segments: image.segments().len() as u64,
        metadata,
        config,
        execution,
        stats_json,
    })
}

fn run_stats_json(
    binary_bytes: u64,
    load_segments: u64,
    max_tick: u64,
    cores: u64,
    parallel_workers: u64,
    execution: Option<&Rem6ExecutionSummary>,
) -> Result<String, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    increment_stat(
        &mut stats,
        "sim.binary.bytes",
        "Byte",
        StatResetPolicy::Constant,
        binary_bytes,
    )?;
    increment_stat(
        &mut stats,
        "sim.elf.load_segments",
        "Count",
        StatResetPolicy::Constant,
        load_segments,
    )?;
    increment_stat(
        &mut stats,
        "sim.max_tick",
        "Tick",
        StatResetPolicy::Constant,
        max_tick,
    )?;
    increment_stat(
        &mut stats,
        "sim.cores",
        "Count",
        StatResetPolicy::Constant,
        cores,
    )?;
    increment_stat(
        &mut stats,
        "sim.parallel.scheduler.worker_limit",
        "Count",
        StatResetPolicy::Constant,
        parallel_workers,
    )?;

    if let Some(execution) = execution {
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
        increment_stat(
            &mut stats,
            "sim.stop_code",
            "Count",
            StatResetPolicy::Constant,
            execution.stop_code as u64,
        )?;
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
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.epochs",
            "Count",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_epochs,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.max_workers",
            "Count",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_max_workers,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.dispatches",
            "Count",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_dispatches,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.batches",
            "Count",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_batches,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.total_workers",
            "Count",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_total_workers,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.active_partitions",
            "Count",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_active_partitions,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.remote_sends",
            "Count",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_remote_sends,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.batch.worker_ticks",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_batch_worker_ticks,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.batch.worker_capacity_ticks",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_batch_worker_capacity_ticks,
        )?;
        increment_stat(
            &mut stats,
            "sim.parallel.scheduler.batch.idle_worker_ticks",
            "Tick",
            StatResetPolicy::Monotonic,
            execution.parallel_scheduler_batch_idle_worker_ticks,
        )?;
        for slot in &execution.parallel_scheduler_worker_slots {
            increment_stat(
                &mut stats,
                &format!("sim.parallel.scheduler.worker{}.active_ticks", slot.slot),
                "Tick",
                StatResetPolicy::Monotonic,
                slot.active_ticks,
            )?;
            increment_stat(
                &mut stats,
                &format!("sim.parallel.scheduler.worker{}.idle_ticks", slot.slot),
                "Tick",
                StatResetPolicy::Monotonic,
                slot.idle_ticks,
            )?;
        }
        for lane in &execution.parallel_scheduler_worker_lanes {
            increment_stat(
                &mut stats,
                &format!(
                    "sim.parallel.scheduler.worker{}.partition{}.active_ticks",
                    lane.lane, lane.partition
                ),
                "Tick",
                StatResetPolicy::Monotonic,
                lane.active_ticks,
            )?;
        }
        for partition in &execution.parallel_scheduler_partitions {
            increment_stat(
                &mut stats,
                &format!(
                    "sim.parallel.partition{}.scheduler.workers",
                    partition.partition
                ),
                "Count",
                StatResetPolicy::Monotonic,
                partition.workers,
            )?;
            increment_stat(
                &mut stats,
                &format!(
                    "sim.parallel.partition{}.scheduler.dispatches",
                    partition.partition
                ),
                "Count",
                StatResetPolicy::Monotonic,
                partition.dispatches,
            )?;
            increment_stat(
                &mut stats,
                &format!(
                    "sim.parallel.partition{}.scheduler.remote_sends",
                    partition.partition
                ),
                "Count",
                StatResetPolicy::Monotonic,
                partition.remote_sends,
            )?;
            increment_stat(
                &mut stats,
                &format!(
                    "sim.parallel.partition{}.scheduler.remote_receives",
                    partition.partition
                ),
                "Count",
                StatResetPolicy::Monotonic,
                partition.remote_receives,
            )?;
            increment_stat(
                &mut stats,
                &format!(
                    "sim.parallel.partition{}.scheduler.max_pending_events",
                    partition.partition
                ),
                "Count",
                StatResetPolicy::Monotonic,
                partition.max_pending_events,
            )?;
        }
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
    Ok(format!("[{samples}]"))
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
    let max_turns =
        usize::try_from(config.max_tick()).map_err(|_| Rem6CliError::InvalidMaxTick {
            value: config.max_tick().to_string(),
        })?;
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
        1,
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
        )?;
        let data_route = add_memory_route(
            &mut transport,
            format!("cpu{cpu_index}.dmem"),
            cpu_partition,
            memory_partition,
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
        SystemHostEventPort::with_controller(host_partition, 1, Arc::clone(&controller))
            .map_err(execute_error)?,
        GuestSourceId::new(1),
    );
    let driver = RiscvSystemRunDriver::new(trap_port);
    let fetch_trace = MemoryTrace::new();
    let data_trace = MemoryTrace::new();
    let run = driver
        .drive_until_host_stop_parallel(
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
            max_turns,
            |cpu| GuestEventId::new(u64::from(cpu.get())),
        )
        .map_err(execute_error)?;

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
) -> Result<MemoryRouteId, Rem6CliError> {
    transport
        .add_route(
            MemoryRoute::new(
                transport_endpoint(source)?,
                cpu_partition,
                transport_endpoint("memory".to_string())?,
                memory_partition,
                1,
                1,
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
    let RiscvSystemRunStopReason::HostStop(stop) = run.stop_reason() else {
        return Err(Rem6CliError::Execute {
            error: "RISC-V execution stopped without a host trap".to_string(),
        });
    };
    let scheduled_trap = run
        .scheduled_traps()
        .first()
        .ok_or_else(|| Rem6CliError::Execute {
            error: "RISC-V execution reached host stop without a scheduled trap".to_string(),
        })?;
    let committed_by_cpu = committed_instructions_by_cpu(run);
    let committed_instructions = committed_by_cpu.values().sum();
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
        final_tick: stop.tick(),
        stop_code: stop.code(),
        trap: guest_trap_name(scheduled_trap.trap().kind()),
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
                if event.endpoint().as_str() != source {
                    continue;
                }
                let latency = event.tick().saturating_sub(*sent_tick);
                summary.counters.responses += 1;
                summary.counters.round_trip_ticks =
                    summary.counters.round_trip_ticks.saturating_add(latency);
                summary.counters.max_round_trip_ticks =
                    summary.counters.max_round_trip_ticks.max(latency);
                let route = route_summary(&mut routes, event.route(), source);
                route.counters.response_arrivals += 1;
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

fn bytes_to_hex(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<Vec<_>>()
        .join("")
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

fn elf_class_name(class: BootElfClass) -> &'static str {
    match class {
        BootElfClass::Class32 => "ELF32",
        BootElfClass::Class64 => "ELF64",
    }
}

fn elf_endian_name(endian: BootElfEndian) -> &'static str {
    match endian {
        BootElfEndian::Little => "little",
        BootElfEndian::Big => "big",
    }
}

fn elf_architecture_name(architecture: BootElfArchitecture) -> &'static str {
    match architecture {
        BootElfArchitecture::Sparc32 => "sparc32",
        BootElfArchitecture::Sparc64 => "sparc64",
        BootElfArchitecture::Mips => "mips",
        BootElfArchitecture::I386 => "i386",
        BootElfArchitecture::X8664 => "x86_64",
        BootElfArchitecture::Arm => "arm",
        BootElfArchitecture::Thumb => "thumb",
        BootElfArchitecture::Arm64 => "arm64",
        BootElfArchitecture::Riscv32 => "riscv32",
        BootElfArchitecture::Riscv64 => "riscv64",
        BootElfArchitecture::Power => "power",
        BootElfArchitecture::Power64 => "power64",
        BootElfArchitecture::Unknown { .. } => "unknown",
    }
}

fn elf_os_name(os: BootElfOperatingSystem) -> String {
    match os {
        BootElfOperatingSystem::Linux => "linux".to_string(),
        BootElfOperatingSystem::Solaris => "solaris".to_string(),
        BootElfOperatingSystem::Tru64 => "tru64".to_string(),
        BootElfOperatingSystem::LinuxArmOabi => "linux-arm-oabi".to_string(),
        BootElfOperatingSystem::LinuxPower64AbiV1 => "linux-power64-abi-v1".to_string(),
        BootElfOperatingSystem::LinuxPower64AbiV2 => "linux-power64-abi-v2".to_string(),
        BootElfOperatingSystem::FreeBsd => "freebsd".to_string(),
        BootElfOperatingSystem::Unknown { os_abi } => format!("unknown:{os_abi}"),
    }
}

fn json_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            c if c.is_control() => escaped.push_str(&format!("\\u{:04x}", c as u32)),
            c => escaped.push(c),
        }
    }
    escaped
}
