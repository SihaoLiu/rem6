use std::collections::BTreeMap;
use std::fmt;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvCoreDriveAction, RiscvDataAccessEventKind,
};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation};
use rem6_stats::{
    MemFootprintAddressRange, MemFootprintProbeConfig, ProbePayload, StackDistProbeConfig,
    StatsRegistry,
};
use rem6_system::{
    GuestSourceId, GuestTrapKind, HostEventPolicy, RiscvDataAccessStats, RiscvGuestWriteRecord,
    RiscvInstructionStats, RiscvRetiredInstructionProbeSnapshot, RiscvSeAuxvEntry,
    RiscvSeStartupConfig, RiscvSystemRun, RiscvSystemRunDriver, RiscvSystemRunStopReason,
    RiscvTrapEventPort, RiscvUnknownSyscallRecord, SystemHostController, SystemHostEventPort,
    RISCV_LINUX_AT_ENTRY,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTransport, TransportEndpointId,
};

mod artifact_json;
mod cli_error;
mod cli_output;
mod config;
mod data_cache_runtime;
mod debug_output;
mod formatting;
mod gpu_cli;
mod guest_memory;
mod gups_cli;
mod parallel_stats;
mod pipeline_stats;
mod power_output;
mod readfile_runtime;
mod resource_acquire_cli;
mod resource_acquire_config;
mod riscv_run_driver;
mod run_gdb;
mod run_resource_config;
mod run_validation;
mod runtime_memory;
mod stats_output;
mod trace_replay_cli;
mod transport_summary;
#[cfg(test)]
mod transport_summary_tests;

pub use cli_error::Rem6CliError;
pub use config::{
    CliCachePrefetcher, CliDebugFlag, CliDramMemoryProfile, LoadBlobRequest, LoadBlobSource,
    MemoryDumpRequest, PowerAnalysisFormat, ReadfileRequest, ReadfileSource, Rem6GupsConfig,
    Rem6RunConfig, Rem6TraceReplayConfig, RequestedIsa, RiscvSeFileRequest, StatsFormat,
    SuiteResourceSelector,
};
use data_cache_runtime::{
    cli_cache_runtime_with_prefetcher, with_riscv_syscall_data_cache_memory_io,
    CliDataCacheRuntime, CliDataCacheSummary,
};
use debug_output::Rem6DebugSummary;
pub use gpu_cli::{run_gpu_run_config, Rem6GpuRunArtifact, Rem6GpuRunConfig};
use guest_memory::{build_cli_memory_store, read_load_blobs, LoadedBlob};
pub use gups_cli::{run_gups_config, Rem6GupsArtifact, Rem6GupsExecutionSummary};
use parallel_stats::{
    parallel_frontier_summaries, parallel_partition_summaries, parallel_ready_partition_summaries,
    parallel_worker_lane_summaries, parallel_worker_slot_summaries,
};
use pipeline_stats::{
    in_order_pipeline_data_wait_cycles, in_order_pipeline_fetch_wait_cycles,
    in_order_pipeline_run_summary,
};
use power_output::{run_power_analysis_artifact, Rem6PowerAnalysisArtifact};
use readfile_runtime::{read_readfiles, readfile_mmio_bus, LoadedReadfile, Rem6ReadfileSummary};
pub use resource_acquire_cli::{
    run_resource_acquire_config, Rem6ResourceAcquireArtifact, Rem6ResourceAcquireResourceSummary,
};
pub use resource_acquire_config::{Rem6ResourceAcquireConfig, Rem6ResourceAcquireResourceConfig};
use riscv_run_driver::drive_cli_riscv_run;
use run_gdb::{serve_riscv_gdb_with_run_control, RiscvGdbServeOutcome};
use run_resource_config::{run_resource_payloads_from_config, RunResourcePayloads};
use run_validation::validate_run_config_inputs;
use runtime_memory::{read_memory_dumps, CliMemoryRuntime};
use stats_output::{run_stats_output, Rem6StatsInputs};
pub use trace_replay_cli::{
    run_trace_replay_config, Rem6TraceReplayArtifact, Rem6TraceReplayExecutionSummary,
};
pub(crate) use transport_summary::{
    memory_transport_summary, Rem6MemoryTransportCounters, Rem6MemoryTransportRouteSummary,
    Rem6MemoryTransportSummary,
};

const DEFAULT_CACHE_LINE_BYTES: u64 = 16;
const RISCV_DATA_PROBE_PAGE_BYTES: u64 = 4096;
const CLI_MEMORY_DUMP_AGENT: AgentId = AgentId::new(u32::MAX);
const RISCV_BOOT_A0_REGISTER: u8 = 10;
const RISCV_BOOT_A1_REGISTER: u8 = 11;
const RISCV_STACK_POINTER_REGISTER: u8 = 2;
const RISCV64_SE_STACK_TOP: u64 = 0x0000_7fff_ffff_f000;

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
    readfiles: Vec<Rem6ReadfileSummary>,
    execution: Option<Rem6ExecutionSummary>,
    stats_json: String,
    stats_text: String,
    power_analysis: Option<Rem6PowerAnalysisArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6LoadBlobSummary {
    address: u64,
    source: String,
    bytes: u64,
}

impl Rem6LoadBlobSummary {
    fn new(address: u64, source: impl Into<String>, bytes: u64) -> Self {
        Self {
            address,
            source: source.into(),
            bytes,
        }
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub fn source(&self) -> &str {
        &self.source
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
    instruction_cache: CliDataCacheSummary,
    data_cache: CliDataCacheSummary,
    instruction_probes: Rem6InstructionProbeSummary,
    data_access_probes: Rem6DataAccessProbeSummary,
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
    debug: Rem6DebugSummary,
    cores: Vec<Rem6CoreSummary>,
    memory_dumps: Vec<Rem6MemoryDump>,
    riscv_guest_writes: Vec<Rem6RiscvGuestWriteSummary>,
    riscv_unknown_syscalls: Vec<Rem6RiscvUnknownSyscallSummary>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rem6InstructionProbeSummary {
    event_count: u64,
    retired_instruction_events: u64,
    tracked_instructions: u64,
    pc_sample_events: u64,
    pc_target_counters: u64,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rem6DataAccessProbeSummary {
    sample_count: u64,
    stack_distance_infinite_samples: u64,
    stack_distance_finite_samples: u64,
    stack_distance_stack_depth: u64,
    stack_distance_read_linear: Vec<(u64, u64)>,
    stack_distance_write_linear: Vec<(u64, u64)>,
    stack_distance_read_log: Vec<(u64, u64)>,
    stack_distance_write_log: Vec<(u64, u64)>,
    memory_footprint_cache_line_bytes: u64,
    memory_footprint_cache_line_total_bytes: u64,
    memory_footprint_page_bytes: u64,
    memory_footprint_page_total_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RiscvGuestWriteSummary {
    fd: u32,
    address: u64,
    tick: u64,
    bytes: Vec<u8>,
}

impl Rem6RiscvGuestWriteSummary {
    fn from_record(record: &RiscvGuestWriteRecord) -> Self {
        Self {
            fd: record.fd().get(),
            address: record.address(),
            tick: record.tick(),
            bytes: record.bytes().to_vec(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RiscvUnknownSyscallSummary {
    pc: u64,
    number: u64,
    arguments: [u64; 6],
    tick: u64,
}

impl Rem6RiscvUnknownSyscallSummary {
    fn from_record(record: &RiscvUnknownSyscallRecord) -> Self {
        Self {
            pc: record.pc(),
            number: record.number(),
            arguments: record.arguments(),
            tick: record.tick(),
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rem6DramSummary {
    active_targets: u64,
    active_ports: u64,
    active_banks: u64,
    accesses: u64,
    reads: u64,
    writes: u64,
    row_hits: u64,
    row_misses: u64,
    refreshes: u64,
    refresh_ticks: u64,
    commands: u64,
    turnarounds: u64,
    total_ready_latency_ticks: u64,
    max_ready_latency_ticks: u64,
    profiled_targets: u64,
    profile_technology: Option<&'static str>,
    profile_parallel_port_label: Option<&'static str>,
    profile_topology_unit_label: Option<&'static str>,
    profile_geometry_bank_count: u64,
    profile_geometry_row_size: u64,
    profile_geometry_line_size: u64,
    profile_geometry_lines_per_row: u64,
    profile_geometry_bank_group_count: u64,
    profile_timing_activate_latency: u64,
    profile_timing_read_latency: u64,
    profile_timing_write_latency: u64,
    profile_timing_precharge_latency: u64,
    profile_timing_bus_turnaround: u64,
    profile_timing_burst_spacing: u64,
    profile_timing_same_bank_group_burst_spacing: u64,
    profile_timing_refresh_interval: u64,
    profile_timing_refresh_recovery: u64,
    profile_timing_command_window_cycles: u64,
    profile_timing_command_window_max_commands: u64,
    profile_low_power_precharge_powerdown_entry_delay: u64,
    profile_low_power_self_refresh_entry_delay: u64,
    profile_low_power_exit_latency: u64,
    profile_low_power_self_refresh_exit_latency: u64,
    profile_nvm_media_read_latency: u64,
    profile_nvm_media_write_latency: u64,
    profile_nvm_media_send_latency: u64,
    profile_nvm_media_max_pending_reads: u64,
    profile_nvm_media_max_pending_writes: u64,
    profile_parallel_ports: u64,
    profile_topology_units: u64,
    profile_scheduler_banks: u64,
    profile_topology_banks: u64,
    profile_scheduler_bank_groups: u64,
    nvm_persistent_writes: u64,
    nvm_persistent_write_bytes: u64,
    nvm_max_pending_reads: u64,
    nvm_max_pending_persistent_writes: u64,
    low_power_active_powerdown_entries: u64,
    low_power_active_powerdown_ticks: u64,
    low_power_precharge_powerdown_entries: u64,
    low_power_precharge_powerdown_ticks: u64,
    low_power_self_refresh_entries: u64,
    low_power_self_refresh_ticks: u64,
    low_power_exits: u64,
    low_power_exit_latency_ticks: u64,
    targets: Vec<Rem6DramTargetSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6DramTargetSummary {
    target: u32,
    active_ports: u64,
    active_banks: u64,
    accesses: u64,
    reads: u64,
    writes: u64,
    row_hits: u64,
    row_misses: u64,
    refreshes: u64,
    refresh_ticks: u64,
    commands: u64,
    turnarounds: u64,
    total_ready_latency_ticks: u64,
    max_ready_latency_ticks: u64,
    ports: Vec<Rem6DramPortSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6DramPortSummary {
    port: u32,
    accesses: u64,
    reads: u64,
    writes: u64,
    turnarounds: u64,
    commands: u64,
    banks: Vec<Rem6DramBankSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6DramBankSummary {
    bank: u32,
    accesses: u64,
    read_bytes: u64,
    write_bytes: u64,
    row_hits: u64,
    row_misses: u64,
    refreshes: u64,
    refresh_ticks: u64,
    commands: u64,
    total_ready_latency_ticks: u64,
    max_ready_latency_ticks: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Rem6ExecutionStop {
    HostTrap {
        stop_code: i32,
        trap: &'static str,
        trap_pc: u64,
    },
    HostStop {
        stop_code: i32,
    },
    TickLimit {
        tick_limit: u64,
    },
    InstructionLimit {
        instruction_limit: u64,
    },
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

struct ExecutionSummaryInputs<'a> {
    core_count: u32,
    memory: &'a CliMemoryRuntime,
    line_layout: CacheLineLayout,
    config: &'a Rem6RunConfig,
    instruction_cache: CliDataCacheSummary,
    data_cache: CliDataCacheSummary,
    fetch_trace: &'a MemoryTrace,
    data_trace: &'a MemoryTrace,
    riscv_guest_writes: Vec<Rem6RiscvGuestWriteSummary>,
    riscv_unknown_syscalls: Vec<Rem6RiscvUnknownSyscallSummary>,
    prior_committed_by_cpu: BTreeMap<CpuId, u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6CoreSummary {
    cpu: u32,
    pc: u64,
    committed_instructions: u64,
    in_order_pipeline_cycles: u64,
    in_order_pipeline_in_flight: u64,
    in_order_pipeline_retired: u64,
    in_order_pipeline_advanced: u64,
    in_order_pipeline_flushed: u64,
    in_order_pipeline_resource_blocked: u64,
    in_order_pipeline_ordering_blocked: u64,
    in_order_pipeline_fetch_wait_cycles: u64,
    in_order_pipeline_data_wait_cycles: u64,
    in_order_pipeline_branch_predictions: u64,
    in_order_pipeline_branch_mispredictions: u64,
    in_order_pipeline_branch_prediction_flushes: u64,
    in_order_pipeline_redirects: u64,
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

pub fn run_cli<I, S>(args: I) -> Result<String, Rem6CliError>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    let Some(command) = args.first() else {
        return Err(Rem6CliError::MissingCommand);
    };
    match command.as_str() {
        "run" => run_run_cli(args),
        "gpu-run" => gpu_cli::run_gpu_run_cli(args),
        "gups" => gups_cli::run_gups_cli(args),
        "trace-replay" => trace_replay_cli::run_trace_replay_cli(args),
        "resource-acquire" => resource_acquire_cli::run_resource_acquire_cli(args),
        _ => Err(Rem6CliError::UnsupportedCommand {
            command: command.clone(),
        }),
    }
}

fn run_run_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let config = Rem6RunConfig::parse_args(args)?;
    let artifact = run_config(config)?;
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
        artifact
            .power_analysis
            .as_ref()
            .map(|artifact| cli_output::ExtraCliArtifact {
                name: "power_artifact",
                path: artifact.output(),
                contents: artifact.contents(),
            }),
    )
}

pub fn run_config(config: Rem6RunConfig) -> Result<Rem6RunArtifact, Rem6CliError> {
    let resource_payloads = config
        .resource_config()
        .map(run_resource_payloads_from_config)
        .transpose()?;
    let bytes = run_binary_bytes(&config, resource_payloads.as_ref())?;
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

    validate_run_config_inputs(&config)?;

    let load_blobs = read_load_blobs(config.load_blobs(), resource_payloads.as_ref())?;
    let load_blob_summaries = load_blobs
        .iter()
        .map(|blob| blob.summary.clone())
        .collect::<Vec<_>>();
    let readfiles = read_readfiles(config.readfiles(), resource_payloads.as_ref())?;
    let readfile_summaries = readfiles
        .iter()
        .map(|readfile| readfile.summary().clone())
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
                &readfiles,
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
        readfiles: &readfile_summaries,
        start_address,
        config: &config,
        execution: execution.as_ref(),
    })?;
    let power_analysis = match (execution.as_ref(), config.power_output()) {
        (Some(execution), Some(path)) => Some(run_power_analysis_artifact(
            config.power_format(),
            path.to_path_buf(),
            execution,
        )?),
        _ => None,
    };
    Ok(Rem6RunArtifact {
        schema: "rem6.cli.run.v1",
        binary_bytes: bytes.len() as u64,
        entry: image.entry().get(),
        start_address,
        load_segments: image.segments().len() as u64,
        metadata,
        load_blobs: load_blob_summaries,
        readfiles: readfile_summaries,
        config,
        execution,
        stats_json: stats.json,
        stats_text: stats.text,
        power_analysis,
    })
}

fn run_binary_bytes(
    config: &Rem6RunConfig,
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<u8>, Rem6CliError> {
    if config.resource_config().is_some() {
        return resource_payloads
            .ok_or_else(|| Rem6CliError::Execute {
                error: "run resource config was not loaded".to_string(),
            })?
            .kernel_binary();
    }

    std::fs::read(config.binary()).map_err(|error| Rem6CliError::ReadBinary {
        path: config.binary().to_path_buf(),
        error: error.to_string(),
    })
}

fn execute_riscv(
    image: &BootImage,
    config: &Rem6RunConfig,
    load_blobs: &[LoadedBlob],
    readfiles: &[LoadedReadfile],
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
    let instruction_cache = cli_cache_runtime_with_prefetcher(
        config.instruction_cache_protocol(),
        line_layout,
        core_count,
        config.instruction_cache_prefetcher(),
    )?;
    let data_cache = cli_cache_runtime_with_prefetcher(
        config.data_cache_protocol(),
        line_layout,
        core_count,
        config.data_cache_prefetcher(),
    )?;
    let readfile_bus = readfile_mmio_bus(
        readfiles,
        core_count,
        memory_partition,
        config.memory_route_delay(),
    )?;
    let riscv_se_startup = if config.riscv_se() {
        let mut startup_config = RiscvSeStartupConfig::new(Address::new(RISCV64_SE_STACK_TOP));
        if config.riscv_se_args().is_empty() {
            startup_config = startup_config.with_arg(config.binary().display().to_string());
        } else {
            for arg in config.riscv_se_args() {
                startup_config = startup_config.with_arg(arg);
            }
        }
        for entry in config.riscv_se_env() {
            startup_config = startup_config.with_env(entry);
        }
        if let Some(metadata) = image.elf_metadata() {
            startup_config = startup_config.with_elf_auxv(metadata);
        }
        let startup = startup_config
            .with_auxv_entry(RiscvSeAuxvEntry::new(
                RISCV_LINUX_AT_ENTRY,
                image.entry().get(),
            ))
            .build()
            .map_err(execute_error)?;
        memory.install_riscv_se_startup(&startup, line_layout)?;
        Some(startup)
    } else {
        None
    };

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
        if let Some(startup) = &riscv_se_startup {
            core.set_privilege_mode(RiscvPrivilegeMode::User);
            core.write_register(
                Register::new(RISCV_STACK_POINTER_REGISTER).map_err(execute_error)?,
                startup.initial_stack_pointer().get(),
            );
        }
        cores.push(core);
    }
    let cluster = RiscvCluster::new(cores).map_err(execute_error)?;
    let instruction_stats = cli_instruction_stats(core_count);
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
    let probe_config = StackDistProbeConfig::builder(line_layout.bytes(), line_layout.bytes())
        .build()
        .map_err(stats_error)?;
    let footprint_range =
        MemFootprintAddressRange::new(0, u64::MAX - (RISCV_DATA_PROBE_PAGE_BYTES - 1))
            .map_err(stats_error)?;
    let footprint_config = MemFootprintProbeConfig::new(
        line_layout.bytes(),
        RISCV_DATA_PROBE_PAGE_BYTES,
        vec![footprint_range],
    )
    .map_err(stats_error)?;
    let mut driver = RiscvSystemRunDriver::with_instruction_stats(trap_port, instruction_stats)
        .with_data_access_stats(
            RiscvDataAccessStats::with_stack_distance(probe_config)
                .with_mem_footprint(footprint_config),
        );
    if config.riscv_se() {
        driver = driver.with_riscv_syscall_emulation_for_boot_image(image);
        let proc_self_exe_target = std::fs::canonicalize(config.binary())
            .unwrap_or_else(|_| config.binary().to_path_buf());
        #[cfg(unix)]
        let proc_self_exe_target = proc_self_exe_target.as_os_str().as_bytes().to_vec();
        #[cfg(not(unix))]
        let proc_self_exe_target = proc_self_exe_target
            .to_string_lossy()
            .into_owned()
            .into_bytes();
        driver
            .riscv_syscall_emulation()
            .expect("RISC-V SE syscall emulation was just installed")
            .register_guest_symlink(b"/proc/self/exe", proc_self_exe_target);
        if let Some(stdin_path) = config.riscv_se_stdin() {
            let stdin =
                std::fs::read(stdin_path).map_err(|error| Rem6CliError::ReadRiscvSeStdin {
                    path: stdin_path.to_path_buf(),
                    error: error.to_string(),
                })?;
            driver
                .riscv_syscall_emulation()
                .expect("RISC-V SE syscall emulation was just installed")
                .push_stdin_bytes(&stdin);
        }
        for file in config.riscv_se_files() {
            let contents =
                std::fs::read(file.host_path()).map_err(|error| Rem6CliError::ReadRiscvSeFile {
                    guest_path: file.guest_path().to_string(),
                    path: file.host_path().to_path_buf(),
                    error: error.to_string(),
                })?;
            driver
                .riscv_syscall_emulation()
                .expect("RISC-V SE syscall emulation was just installed")
                .register_guest_file(file.guest_path().as_bytes(), contents);
        }
        driver = with_riscv_syscall_data_cache_memory_io(
            driver,
            memory.clone(),
            data_cache.clone(),
            line_layout,
        );
    }
    let fetch_trace = MemoryTrace::new();
    let data_trace = MemoryTrace::new();
    let mut gdb_outcome = if let Some(listen) = config.gdb_listen() {
        serve_riscv_gdb_with_run_control(
            listen,
            &cluster,
            &memory,
            &driver,
            &mut scheduler,
            &transport,
            instruction_cache.clone(),
            data_cache.clone(),
            fetch_trace.clone(),
            data_trace.clone(),
            tick_limit,
            config.max_instructions(),
        )?
    } else {
        RiscvGdbServeOutcome::default()
    };
    let run_result = if let Some(run) = gdb_outcome.take_completed_run() {
        Ok(run)
    } else {
        drive_cli_riscv_run(
            &driver,
            &cluster,
            &mut scheduler,
            &transport,
            readfile_bus.as_ref(),
            &memory,
            instruction_cache.clone(),
            data_cache.clone(),
            fetch_trace.clone(),
            data_trace.clone(),
            tick_limit,
            config.max_instructions(),
            gdb_outcome.retired_instruction_count(),
        )
    };
    if let Some(bus) = readfile_bus.as_ref() {
        if let Some(error) = bus.response_errors().into_iter().next() {
            return Err(execute_error(error));
        }
    }
    if let Some(data_cache) = data_cache.as_ref() {
        if let Some(error) = data_cache.take_error() {
            return Err(error);
        }
    }
    if let Some(instruction_cache) = instruction_cache.as_ref() {
        if let Some(error) = instruction_cache.take_error() {
            return Err(error);
        }
    }
    let mut run = run_result.map_err(execute_error)?;
    if let Some(data_cache) = data_cache.as_ref() {
        run = run.with_data_cache_run_records(data_cache.records());
    }
    let instruction_cache_records = instruction_cache
        .as_ref()
        .map(CliDataCacheRuntime::records)
        .unwrap_or_default();
    let (riscv_guest_writes, riscv_unknown_syscalls) = driver
        .riscv_syscall_emulation()
        .map(|emulation| {
            let state = emulation.state();
            let guest_writes = state
                .guest_writes()
                .iter()
                .map(Rem6RiscvGuestWriteSummary::from_record)
                .collect();
            let unknown_syscalls = state
                .unknown_syscalls()
                .iter()
                .map(Rem6RiscvUnknownSyscallSummary::from_record)
                .collect();
            (guest_writes, unknown_syscalls)
        })
        .unwrap_or_default();

    let summary_inputs = ExecutionSummaryInputs {
        core_count,
        memory: &memory,
        line_layout,
        config,
        instruction_cache: CliDataCacheSummary::from_records(&instruction_cache_records)
            .with_prefetch_summary(
                instruction_cache
                    .as_ref()
                    .map(CliDataCacheRuntime::prefetch_summary)
                    .unwrap_or_default(),
            ),
        data_cache: CliDataCacheSummary::from_run(&run).with_prefetch_summary(
            data_cache
                .as_ref()
                .map(CliDataCacheRuntime::prefetch_summary)
                .unwrap_or_default(),
        ),
        fetch_trace: &fetch_trace,
        data_trace: &data_trace,
        riscv_guest_writes,
        riscv_unknown_syscalls,
        prior_committed_by_cpu: gdb_outcome.retired_by_cpu().clone(),
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
    let mut committed_by_cpu = inputs.prior_committed_by_cpu.clone();
    for (cpu, committed) in committed_instructions_by_cpu(run) {
        *committed_by_cpu.entry(cpu).or_insert(0) += committed;
    }
    let committed_instructions = committed_by_cpu.values().sum();
    let final_tick = run.final_tick().ok_or_else(|| Rem6CliError::Execute {
        error: "RISC-V execution stopped without a final tick".to_string(),
    })?;
    let stop = match run.stop_reason() {
        RiscvSystemRunStopReason::HostStop(stop) => {
            if let Some(scheduled_trap) = run.scheduled_traps().first() {
                Rem6ExecutionStop::HostTrap {
                    stop_code: stop.code(),
                    trap: guest_trap_name(scheduled_trap.trap().kind()),
                    trap_pc: scheduled_trap.trap().pc(),
                }
            } else {
                Rem6ExecutionStop::HostStop {
                    stop_code: stop.code(),
                }
            }
        }
        RiscvSystemRunStopReason::DebugStop { .. } => {
            return Err(Rem6CliError::Execute {
                error: "RISC-V execution stopped at a debugger watchpoint".to_string(),
            });
        }
        RiscvSystemRunStopReason::InstructionLimit { limit, .. } => {
            Rem6ExecutionStop::InstructionLimit {
                instruction_limit: inputs.config.max_instructions().unwrap_or(limit),
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
        let pipeline_summary = in_order_pipeline_run_summary(&core);
        let pipeline_snapshot = core.in_order_pipeline_snapshot();
        cores.push(Rem6CoreSummary {
            cpu: cpu_index,
            pc: core.pc().get(),
            committed_instructions: committed_by_cpu.get(&cpu).copied().unwrap_or(0),
            in_order_pipeline_cycles: pipeline_snapshot.cycle(),
            in_order_pipeline_in_flight: pipeline_snapshot.in_flight().len() as u64,
            in_order_pipeline_retired: pipeline_summary.retired_count() as u64,
            in_order_pipeline_advanced: pipeline_summary.advanced_count() as u64,
            in_order_pipeline_flushed: pipeline_summary.flushed_count() as u64,
            in_order_pipeline_resource_blocked: pipeline_summary.resource_blocked_count() as u64,
            in_order_pipeline_ordering_blocked: pipeline_summary.ordering_blocked_count() as u64,
            in_order_pipeline_fetch_wait_cycles: in_order_pipeline_fetch_wait_cycles(&core),
            in_order_pipeline_data_wait_cycles: in_order_pipeline_data_wait_cycles(&core),
            in_order_pipeline_branch_predictions: pipeline_summary.branch_prediction_count() as u64,
            in_order_pipeline_branch_mispredictions: pipeline_summary.branch_misprediction_count()
                as u64,
            in_order_pipeline_branch_prediction_flushes: pipeline_summary
                .branch_prediction_flushed_count()
                as u64,
            in_order_pipeline_redirects: pipeline_summary.redirect_count() as u64,
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
        instruction_cache: inputs.instruction_cache,
        data_cache: inputs.data_cache,
        instruction_probes: instruction_probe_summary(run),
        data_access_probes: data_access_probe_summary(
            run,
            inputs.line_layout,
            RISCV_DATA_PROBE_PAGE_BYTES,
        ),
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
        debug: Rem6DebugSummary::from_run(inputs.config, run),
        cores,
        memory_dumps: read_memory_dumps(
            inputs.memory,
            inputs.line_layout,
            inputs.config.memory_dumps(),
        )?,
        riscv_guest_writes: inputs.riscv_guest_writes,
        riscv_unknown_syscalls: inputs.riscv_unknown_syscalls,
    })
}

fn cli_instruction_stats(core_count: u32) -> RiscvInstructionStats {
    RiscvInstructionStats::for_cpus((0..core_count).map(CpuId::new))
}

fn instruction_probe_summary(run: &RiscvSystemRun) -> Rem6InstructionProbeSummary {
    run.retired_instruction_probes()
        .map(instruction_probe_snapshot_summary)
        .unwrap_or_default()
}

fn instruction_probe_snapshot_summary(
    probes: &RiscvRetiredInstructionProbeSnapshot,
) -> Rem6InstructionProbeSummary {
    let mut retired_instruction_events = 0_u64;
    let mut pc_sample_events = 0_u64;
    for event in probes.probes().events() {
        match event.payload() {
            ProbePayload::Counter { .. } => {
                retired_instruction_events = retired_instruction_events.saturating_add(1);
            }
            ProbePayload::ProgramCounter { .. } => {
                pc_sample_events = pc_sample_events.saturating_add(1);
            }
            ProbePayload::Unit | ProbePayload::MemoryPacket(_) => {}
        }
    }

    Rem6InstructionProbeSummary {
        event_count: probes.probes().events().len() as u64,
        retired_instruction_events,
        tracked_instructions: probes.tracker().counter(),
        pc_sample_events,
        pc_target_counters: probes
            .pc_count()
            .map(|pc_count| pc_count.counters().len() as u64)
            .unwrap_or(0),
    }
}

fn data_access_probe_summary(
    run: &RiscvSystemRun,
    line_layout: CacheLineLayout,
    page_bytes: u64,
) -> Rem6DataAccessProbeSummary {
    run.data_access_probes()
        .map(|probes| {
            let stack_distance = probes.stack_distance();
            let histograms = stack_distance.histograms();
            let infinite_samples = stack_distance.infinite_samples();
            let finite_samples = stack_distance.finite_samples();
            let footprint = probes.memory_footprint();
            Rem6DataAccessProbeSummary {
                sample_count: infinite_samples.saturating_add(finite_samples),
                stack_distance_infinite_samples: infinite_samples,
                stack_distance_finite_samples: finite_samples,
                stack_distance_stack_depth: stack_distance.stack().len() as u64,
                stack_distance_read_linear: histograms.read_linear().to_vec(),
                stack_distance_write_linear: histograms.write_linear().to_vec(),
                stack_distance_read_log: histograms.read_log().to_vec(),
                stack_distance_write_log: histograms.write_log().to_vec(),
                memory_footprint_cache_line_bytes: footprint
                    .map(|snapshot| {
                        (snapshot.cache_lines().len() as u64).saturating_mul(line_layout.bytes())
                    })
                    .unwrap_or(0),
                memory_footprint_cache_line_total_bytes: footprint
                    .map(|snapshot| {
                        (snapshot.cache_lines_total().len() as u64)
                            .saturating_mul(line_layout.bytes())
                    })
                    .unwrap_or(0),
                memory_footprint_page_bytes: footprint
                    .map(|snapshot| (snapshot.pages().len() as u64).saturating_mul(page_bytes))
                    .unwrap_or(0),
                memory_footprint_page_total_bytes: footprint
                    .map(|snapshot| {
                        (snapshot.pages_total().len() as u64).saturating_mul(page_bytes)
                    })
                    .unwrap_or(0),
            }
        })
        .unwrap_or_default()
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
            MemoryOperation::Atomic | MemoryOperation::AtomicNoReturn => {
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
        let RiscvCoreDriveAction::InstructionExecuted(instruction) = event.action() else {
            continue;
        };
        if instruction.counts_as_retired_instruction() {
            *committed.entry(event.cpu()).or_insert(0) += 1;
        }
    }
    committed
}

fn guest_trap_name(kind: GuestTrapKind) -> &'static str {
    match kind {
        GuestTrapKind::EnvironmentCall => "environment_call",
        GuestTrapKind::Breakpoint => "breakpoint",
        GuestTrapKind::IllegalInstruction => "illegal_instruction",
        GuestTrapKind::InstructionPageFault => "instruction_page_fault",
        GuestTrapKind::LoadPageFault => "load_page_fault",
        GuestTrapKind::StorePageFault => "store_page_fault",
        GuestTrapKind::Interrupt { .. } => "interrupt",
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
