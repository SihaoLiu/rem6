use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    CpuCore, CpuDataConfig, CpuFetchConfig, CpuId, CpuResetState, RiscvCluster, RiscvCore,
    RiscvCoreDriveAction, RiscvDataAccessEventKind,
};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode};
use rem6_kernel::{PartitionFrontier, PartitionId, PartitionedScheduler, ReadyPartition};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryOperation, MemoryRequestId,
};
use rem6_stats::{StackDistProbeConfig, StatsRegistry};
use rem6_system::{
    GuestEventId, GuestSourceId, GuestTrapKind, HostEventPolicy, RiscvDataAccessStats,
    RiscvSeAuxvEntry, RiscvSeStartupConfig, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvSystemRunStopReason, RiscvTrapEventPort, SystemHostController, SystemHostEventPort,
    RISCV_LINUX_AT_ENTRY,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    TransportEndpointId,
};

mod artifact_json;
mod cli_error;
mod cli_output;
mod config;
mod formatting;
mod guest_memory;
mod gups_cli;
mod parallel_stats;
mod runtime_memory;
mod stats_output;
mod trace_replay_cli;
#[cfg(test)]
mod transport_summary_tests;

pub use cli_error::Rem6CliError;
pub use config::{
    CliDramMemoryProfile, LoadBlobRequest, MemoryDumpRequest, Rem6GupsConfig, Rem6RunConfig,
    Rem6TraceReplayConfig, RequestedIsa, StatsFormat,
};
use guest_memory::{build_cli_memory_store, read_load_blobs, LoadedBlob};
pub use gups_cli::{run_gups_config, Rem6GupsArtifact, Rem6GupsExecutionSummary};
use runtime_memory::{cli_memory_response, read_memory_dumps, CliMemoryRuntime};
use stats_output::{run_stats_output, Rem6StatsInputs};
pub use trace_replay_cli::{
    run_trace_replay_config, Rem6TraceReplayArtifact, Rem6TraceReplayExecutionSummary,
};

const DEFAULT_CACHE_LINE_BYTES: u64 = 16;
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
    cores: Vec<Rem6CoreSummary>,
    memory_dumps: Vec<Rem6MemoryDump>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Rem6DataAccessProbeSummary {
    sample_count: u64,
    stack_distance_infinite_samples: u64,
    stack_distance_finite_samples: u64,
    stack_distance_stack_depth: u64,
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
        "gups" => gups_cli::run_gups_cli(args),
        "trace-replay" => trace_replay_cli::run_trace_replay_cli(args),
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
    )
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
        if config.riscv_se() {
            return Err(Rem6CliError::RiscvSeRequiresExecution);
        }
    }
    if config.riscv_se() {
        if config.isa() != RequestedIsa::Riscv {
            return Err(Rem6CliError::RiscvSeRequiresRiscv);
        }
        if config.cores() != 1 {
            return Err(Rem6CliError::RiscvSeRequiresSingleCore {
                cores: config.cores(),
            });
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
    let riscv_se_startup = if config.riscv_se() {
        let mut startup_config = RiscvSeStartupConfig::new(Address::new(RISCV64_SE_STACK_TOP))
            .with_arg(config.binary().display().to_string());
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
    let mut driver = RiscvSystemRunDriver::new(trap_port)
        .with_data_access_stats(RiscvDataAccessStats::with_stack_distance(probe_config));
    if config.riscv_se() {
        driver = driver.with_riscv_syscall_emulation_for_boot_image(image);
        let read_memory = memory.clone();
        let write_memory = memory.clone();
        let map_memory = memory.clone();
        driver = driver.with_riscv_syscall_emulation_and_guest_memory_io_map_handler(
            move |address, bytes| read_memory.read_guest_memory(address, bytes, line_layout),
            move |address, bytes| write_memory.write_guest_memory(address, bytes, line_layout),
            move |request| {
                map_memory.map_guest_memory(
                    request.address(),
                    request.bytes(),
                    line_layout,
                    request.replace_existing(),
                )
            },
        );
    }
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
        data_access_probes: data_access_probe_summary(run),
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

fn data_access_probe_summary(run: &RiscvSystemRun) -> Rem6DataAccessProbeSummary {
    run.data_access_probes()
        .map(|probes| {
            let infinite_samples = probes.stack_distance().infinite_samples();
            let finite_samples = probes.stack_distance().finite_samples();
            Rem6DataAccessProbeSummary {
                sample_count: infinite_samples.saturating_add(finite_samples),
                stack_distance_infinite_samples: infinite_samples,
                stack_distance_finite_samples: finite_samples,
                stack_distance_stack_depth: probes.stack_distance().stack().len() as u64,
            }
        })
        .unwrap_or_default()
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
        GuestTrapKind::IllegalInstruction => "illegal_instruction",
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
