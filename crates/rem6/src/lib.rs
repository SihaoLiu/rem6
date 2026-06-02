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
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_stats::StatsRegistry;
use rem6_system::{
    GuestEventId, GuestSourceId, GuestTrapKind, HostEventPolicy, RiscvSystemRun,
    RiscvSystemRunDriver, RiscvSystemRunStopReason, RiscvTrapEventPort, SystemHostController,
    SystemHostEventPort,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    TransportEndpointId,
};

mod artifact_json;
mod config;
mod formatting;
mod guest_memory;
mod parallel_stats;
mod runtime_memory;
mod stats_output;
#[cfg(test)]
mod transport_summary_tests;

pub use config::{
    CliDramMemoryProfile, LoadBlobRequest, MemoryDumpRequest, Rem6RunConfig, RequestedIsa,
    StatsFormat,
};
use formatting::{elf_architecture_name, json_escape};
use guest_memory::{build_cli_memory_store, read_load_blobs, LoadedBlob};
use runtime_memory::{cli_memory_response, read_memory_dumps, CliMemoryRuntime};
use stats_output::{run_stats_output, Rem6StatsInputs};

const DEFAULT_CACHE_LINE_BYTES: u64 = 16;
const CLI_MEMORY_DUMP_AGENT: AgentId = AgentId::new(u32::MAX);
const RISCV_BOOT_A0_REGISTER: u8 = 10;
const RISCV_BOOT_A1_REGISTER: u8 = 11;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunArtifact {
    schema: &'static str,
    config: Rem6RunConfig,
    binary_bytes: u64,
    entry: u64,
    start_address: u64,
    metadata: rem6_boot::BootElfMetadata,
    load_segments: u64,
    load_blobs: Vec<Rem6LoadBlobSummary>,
    execution: Option<Rem6ExecutionSummary>,
    stats_json: String,
    stats_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6LoadBlobSummary {
    address: u64,
    path: PathBuf,
    bytes: u64,
}

impl Rem6LoadBlobSummary {
    fn new(address: u64, path: PathBuf, bytes: u64) -> Self {
        Self {
            address,
            path,
            bytes,
        }
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
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
    dram: Rem6DramSummary,
    cores: Vec<Rem6CoreSummary>,
    memory_dumps: Vec<Rem6MemoryDump>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rem6DramSummary {
    active_targets: u64,
    active_ports: u64,
    active_banks: u64,
    accesses: u64,
    reads: u64,
    writes: u64,
    row_hits: u64,
    row_misses: u64,
    commands: u64,
    turnarounds: u64,
    total_ready_latency_ticks: u64,
    max_ready_latency_ticks: u64,
    profiled_targets: u64,
    profile_technology: Option<&'static str>,
    profile_parallel_port_label: Option<&'static str>,
    profile_topology_unit_label: Option<&'static str>,
    profile_parallel_ports: u64,
    profile_topology_units: u64,
    profile_scheduler_banks: u64,
    profile_topology_banks: u64,
    profile_scheduler_bank_groups: u64,
    low_power_active_powerdown_entries: u64,
    low_power_active_powerdown_ticks: u64,
    low_power_precharge_powerdown_entries: u64,
    low_power_precharge_powerdown_ticks: u64,
    low_power_self_refresh_entries: u64,
    low_power_self_refresh_ticks: u64,
    low_power_exits: u64,
    low_power_exit_latency_ticks: u64,
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
    memory: &'a CliMemoryRuntime,
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
    UnsupportedDramMemoryProfile {
        profile: String,
    },
    DramMemoryProfileRequiresDramMemory,
    InvalidMaxTick {
        value: String,
    },
    InvalidMinRemoteDelay {
        value: String,
    },
    InvalidMemoryRouteDelay {
        value: String,
    },
    InvalidHostEventDelay {
        value: String,
    },
    InvalidStartAddress {
        value: String,
    },
    InvalidRiscvBootA0 {
        value: String,
    },
    InvalidRiscvBootA1 {
        value: String,
    },
    MemoryRouteDelayBelowMinRemoteDelay {
        memory_route_delay: u64,
        min_remote_delay: u64,
    },
    HostEventDelayBelowMinRemoteDelay {
        host_event_delay: u64,
        min_remote_delay: u64,
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
    InvalidLoadBlob {
        value: String,
    },
    EmptyLoadBlob {
        path: PathBuf,
    },
    DramMemoryRequiresExecution,
    InstructionLimitRequiresExecution,
    MemoryDumpRequiresExecution,
    ReadBinary {
        path: PathBuf,
        error: String,
    },
    ReadLoadBlob {
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
            Self::UnsupportedDramMemoryProfile { profile } => {
                write!(formatter, "unsupported DRAM memory profile {profile}")
            }
            Self::DramMemoryProfileRequiresDramMemory => {
                write!(formatter, "--dram-memory-profile requires --dram-memory")
            }
            Self::InvalidMaxTick { value } => write!(formatter, "invalid max tick {value}"),
            Self::InvalidMinRemoteDelay { value } => {
                write!(formatter, "invalid min remote delay {value}")
            }
            Self::InvalidMemoryRouteDelay { value } => {
                write!(formatter, "invalid memory route delay {value}")
            }
            Self::InvalidHostEventDelay { value } => {
                write!(formatter, "invalid host event delay {value}")
            }
            Self::InvalidStartAddress { value } => {
                write!(formatter, "invalid start address {value}")
            }
            Self::InvalidRiscvBootA0 { value } => {
                write!(formatter, "invalid RISC-V boot a0 {value}")
            }
            Self::InvalidRiscvBootA1 { value } => {
                write!(formatter, "invalid RISC-V boot a1 {value}")
            }
            Self::MemoryRouteDelayBelowMinRemoteDelay {
                memory_route_delay,
                min_remote_delay,
            } => write!(
                formatter,
                "memory route delay {memory_route_delay} is below min remote delay {min_remote_delay}"
            ),
            Self::HostEventDelayBelowMinRemoteDelay {
                host_event_delay,
                min_remote_delay,
            } => write!(
                formatter,
                "host event delay {host_event_delay} is below min remote delay {min_remote_delay}"
            ),
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
            Self::InvalidLoadBlob { value } => {
                write!(formatter, "invalid load blob {value}")
            }
            Self::EmptyLoadBlob { path } => {
                write!(formatter, "load blob {} is empty", path.display())
            }
            Self::DramMemoryRequiresExecution => {
                write!(formatter, "--dram-memory requires --execute")
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
            Self::ReadLoadBlob { path, error } => {
                write!(
                    formatter,
                    "failed to read load blob {}: {error}",
                    path.display()
                )
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
        if config.dram_memory() {
            return Err(Rem6CliError::DramMemoryRequiresExecution);
        }
        if config.max_instructions().is_some() {
            return Err(Rem6CliError::InstructionLimitRequiresExecution);
        }
        if !config.memory_dumps().is_empty() {
            return Err(Rem6CliError::MemoryDumpRequiresExecution);
        }
    }

    let load_blobs = read_load_blobs(config.load_blobs())?;
    let load_blob_summaries = load_blobs
        .iter()
        .map(|blob| blob.summary.clone())
        .collect::<Vec<_>>();
    let line_layout = CacheLineLayout::new(DEFAULT_CACHE_LINE_BYTES).map_err(execute_error)?;
    if !config.execute() {
        build_cli_memory_store(&image, &load_blobs, line_layout)?;
    }

    let start_address = config
        .start_address()
        .unwrap_or_else(|| image.entry().get());
    let execution = if config.execute() {
        Some(match config.isa() {
            RequestedIsa::Riscv => execute_riscv(
                &image,
                &config,
                &load_blobs,
                line_layout,
                Address::new(start_address),
            )?,
            isa => return Err(Rem6CliError::UnsupportedExecutionIsa { isa }),
        })
    } else {
        None
    };
    let stats = run_stats_output(Rem6StatsInputs {
        binary_bytes: bytes.len() as u64,
        load_segments: image.segments().len() as u64,
        load_blobs: &load_blob_summaries,
        start_address,
        config: &config,
        execution: execution.as_ref(),
    })?;
    Ok(Rem6RunArtifact {
        schema: "rem6.cli.run.v1",
        binary_bytes: bytes.len() as u64,
        entry: image.entry().get(),
        start_address,
        load_segments: image.segments().len() as u64,
        metadata,
        load_blobs: load_blob_summaries,
        config,
        execution,
        stats_json: stats.json,
        stats_text: stats.text,
    })
}

fn execute_riscv(
    image: &BootImage,
    config: &Rem6RunConfig,
    load_blobs: &[LoadedBlob],
    line_layout: CacheLineLayout,
    start_address: Address,
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
    let memory = CliMemoryRuntime::new(
        image,
        load_blobs,
        line_layout,
        config.dram_memory(),
        config.dram_memory_profile(),
    )?;

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
            config.memory_route_delay(),
        )?;
        let data_route = add_memory_route(
            &mut transport,
            format!("cpu{cpu_index}.dmem"),
            cpu_partition,
            memory_partition,
            config.memory_route_delay(),
        )?;
        let core = RiscvCore::with_data(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(cpu_index),
                    cpu_partition,
                    AgentId::new(cpu_index),
                    start_address,
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
        core.write_register(
            Register::new(RISCV_BOOT_A0_REGISTER).map_err(execute_error)?,
            config.riscv_boot_a0(),
        );
        core.write_register(
            Register::new(RISCV_BOOT_A1_REGISTER).map_err(execute_error)?,
            config.riscv_boot_a1(),
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
            config.host_event_delay(),
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
                    let memory = memory.clone();
                    move |delivery, _context| cli_memory_response(&memory, &delivery)
                },
                |_cpu| {
                    let memory = memory.clone();
                    move |delivery, _context| cli_memory_response(&memory, &delivery)
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
                    let memory = memory.clone();
                    move |delivery, _context| cli_memory_response(&memory, &delivery)
                },
                |_cpu| {
                    let memory = memory.clone();
                    move |delivery, _context| cli_memory_response(&memory, &delivery)
                },
                tick_limit,
                |cpu| GuestEventId::new(u64::from(cpu.get())),
            )
            .map_err(execute_error)?,
    };

    let summary_inputs = ExecutionSummaryInputs {
        core_count,
        memory: &memory,
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
        dram: inputs.memory.dram_summary_until(final_tick),
        cores,
        memory_dumps: read_memory_dumps(
            inputs.memory,
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

fn stats_error(error: rem6_stats::StatsError) -> Rem6CliError {
    Rem6CliError::Stats {
        error: error.to_string(),
    }
}
