use std::fmt;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_cpu::{
    BranchTargetKindCounts, BranchTargetProviderCounts, CpuCore, CpuDataConfig, CpuFetchConfig,
    CpuId, CpuResetState, InOrderPipelineConfig, InOrderPipelineStage, InOrderPipelineStageWidth,
    RiscvCluster, RiscvCore,
};
use rem6_fabric::{FabricLinkId, FabricPath, FabricPathHop, VirtualNetworkId};
use rem6_isa_riscv::{Register, RiscvPrivilegeMode};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{AccessSize, Address, AgentId, CacheLineLayout};
use rem6_stats::{
    MemFootprintAddressRange, MemFootprintProbeConfig, PcCountPair, StackDistProbeConfig,
    StatsRegistry,
};
use rem6_system::{
    GuestSourceId, GuestTrapKind, HostEventPolicy, RiscvDataAccessStats, RiscvGuestFileIdentity,
    RiscvInstructionStats, RiscvSeAuxvEntry, RiscvSeStartupConfig, RiscvSystemRun,
    RiscvSystemRunDriver, RiscvTrapEventPort, SystemHostController, SystemHostEventPort,
    RISCV_LINUX_AT_ENTRY,
};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTransport, TransportEndpointId,
};

mod artifact_json;
mod branch_predictor_summary;
mod cli_error;
mod cli_output;
mod config;
mod core_summary_json;
mod data_access_counts;
mod data_cache_runtime;
mod debug_output;
mod formatting;
mod gpu_cli;
mod guest_memory;
mod gups_cli;
mod host_actions;
mod instruction_probe_summary;
mod memory_resource_summary;
mod multi_run_cli;
mod parallel_stats;
mod pipeline_stats;
mod power_output;
mod readfile_runtime;
mod resource_acquire_cli;
mod resource_acquire_config;
mod riscv_checkpoint_runtime;
mod riscv_guest_output;
mod riscv_run_driver;
mod riscv_sbi_runtime;
mod riscv_se_inputs;
mod run_execution_summary;
mod run_fabric;
mod run_gdb;
mod run_resource_config;
mod run_validation;
mod runtime_memory;
mod stats_output;
mod trace_replay_cli;
mod transport_summary;
#[cfg(test)]
mod transport_summary_tests;

pub(crate) use branch_predictor_summary::{
    Rem6BranchPredictorCounterSummary, Rem6MultiperspectivePerceptronCounterSummary,
    Rem6TageScLBranchPredictorCounterSummary,
};
pub use cli_error::Rem6CliError;
pub use config::{
    CliCachePrefetcher, CliDebugFlag, CliDramLowPowerTiming, CliDramMemoryProfile,
    CliDramRefreshTiming, KernelResourceSelector, LoadBlobRequest, LoadBlobSource,
    MemoryDumpRequest, PowerAnalysisFormat, ReadfileRequest, ReadfileSource, Rem6GupsConfig,
    Rem6RunConfig, Rem6TraceReplayConfig, RequestedIsa, RiscvSeFileRequest, RiscvSeInputSource,
    RunFabricConfig, RunMemorySystem, StatsFormat, SuiteResourceSelector,
    TraceReplayExternalAdapterKind,
};
use data_cache_runtime::{
    cli_cache_runtime_with_prefetcher, with_riscv_syscall_data_cache_memory_io, CliCacheHierarchy,
    CliDataCacheSummary,
};
use debug_output::Rem6DebugSummary;
pub use gpu_cli::{run_gpu_run_config, Rem6GpuRunArtifact, Rem6GpuRunConfig};
use guest_memory::{build_cli_memory_store, read_load_blobs, LoadedBlob, Rem6LoadBlobSummary};
pub use gups_cli::{run_gups_config, Rem6GupsArtifact, Rem6GupsExecutionSummary};
pub(crate) use host_actions::{
    Rem6GuestHostCallSummary, Rem6HostActionSummary, Rem6HostCheckpointChunkSummary,
    Rem6HostCheckpointComponentSummary, Rem6HostCheckpointSummary, Rem6HostInjectedCommandSummary,
    Rem6HostStatsDumpSummary, Rem6HostStatsResetSummary, Rem6HostStopActionSummary,
    Rem6HostWorkMarkerSummary,
};
pub(crate) use instruction_probe_summary::{
    instruction_probe_summary, Rem6InstructionProbeSummary, Rem6PcCountPairSummary,
    Rem6PcCountTrackerSummary,
};
pub(crate) use memory_resource_summary::{
    Rem6CacheResourceHierarchySummary, Rem6CacheResourceSummary, Rem6DramResourceSummary,
    Rem6FabricResourceSummary, Rem6MemoryResourceInputs, Rem6MemoryResourceSummary,
    Rem6TransportResourceSummary,
};
pub use multi_run_cli::{run_multi_run_config, Rem6MultiRunArtifact, Rem6MultiRunConfig};
use pipeline_stats::Rem6InOrderPipelineStageSummary;
use power_output::{run_power_analysis_artifact, Rem6PowerAnalysisArtifact};
use readfile_runtime::{read_readfiles, readfile_mmio_bus, LoadedReadfile, Rem6ReadfileSummary};
pub use resource_acquire_cli::{
    run_resource_acquire_config, Rem6ResourceAcquireArtifact, Rem6ResourceAcquireResourceSummary,
};
pub use resource_acquire_config::{Rem6ResourceAcquireConfig, Rem6ResourceAcquireResourceConfig};
use riscv_checkpoint_runtime::{
    attach_cli_memory_checkpoint_bank, attach_cli_riscv_checkpoint_bank,
};
pub(crate) use riscv_guest_output::{
    Rem6RiscvGuestWriteSummary, Rem6RiscvSbiConsoleSummary, Rem6RiscvSbiHsmSummary,
    Rem6RiscvSbiHsmWakeSummary, Rem6RiscvSbiIpiSummary, Rem6RiscvSbiResetSummary,
    Rem6RiscvSbiRfenceCompletionSummary, Rem6RiscvSbiRfenceSummary, Rem6RiscvSbiTimerSummary,
    Rem6RiscvUnknownSyscallSummary,
};
use riscv_run_driver::drive_cli_riscv_run;
use riscv_sbi_runtime::{attach_cli_riscv_sbi_firmware, configure_cli_riscv_sbi_core};
use riscv_se_inputs::{read_riscv_se_file, read_riscv_se_stdin};
use run_execution_summary::{execution_summary, ExecutionSummaryInputs};
use run_fabric::run_memory_transport;
pub(crate) use run_fabric::Rem6RunFabricSummary;
use run_gdb::{serve_riscv_gdb_with_run_control, RiscvGdbServeOutcome};
use run_resource_config::{run_resource_payloads_from_config, RunResourcePayloads};
use run_validation::validate_run_config_inputs;
use runtime_memory::CliMemoryRuntime;
use stats_output::{run_stats_output, Rem6StatsInputs};
pub use trace_replay_cli::{
    run_trace_replay_config, Rem6TraceReplayArtifact, Rem6TraceReplayExecutionSummary,
    Rem6TraceReplayExternalAdapterSummary,
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

struct RiscvSePathFileWriteback {
    guest_path: String,
    host_path: PathBuf,
    identity: RiscvGuestFileIdentity,
}
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
    instruction_cache_l2: CliDataCacheSummary,
    instruction_cache_l3: CliDataCacheSummary,
    data_cache: CliDataCacheSummary,
    data_cache_l2: CliDataCacheSummary,
    data_cache_l3: CliDataCacheSummary,
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
    fabric: Rem6RunFabricSummary,
    dram: Rem6DramSummary,
    memory_resources: Rem6MemoryResourceSummary,
    debug: Rem6DebugSummary,
    cores: Vec<Rem6CoreSummary>,
    memory_dumps: Vec<Rem6MemoryDump>,
    riscv_guest_writes: Vec<Rem6RiscvGuestWriteSummary>,
    riscv_unknown_syscalls: Vec<Rem6RiscvUnknownSyscallSummary>,
    riscv_sbi_console: Rem6RiscvSbiConsoleSummary,
    riscv_sbi_timers: Vec<Rem6RiscvSbiTimerSummary>,
    riscv_sbi_hsm_events: Vec<Rem6RiscvSbiHsmSummary>,
    riscv_sbi_hsm_wakes: Vec<Rem6RiscvSbiHsmWakeSummary>,
    riscv_sbi_ipis: Vec<Rem6RiscvSbiIpiSummary>,
    riscv_sbi_rfences: Vec<Rem6RiscvSbiRfenceSummary>,
    riscv_sbi_rfence_completions: Vec<Rem6RiscvSbiRfenceCompletionSummary>,
    riscv_sbi_resets: Vec<Rem6RiscvSbiResetSummary>,
    host_actions: Rem6HostActionSummary,
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

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Rem6DramSummary {
    active_targets: u64,
    active_ports: u64,
    active_banks: u64,
    accesses: u64,
    reads: u64,
    writes: u64,
    row_hits: u64,
    read_row_hits: u64,
    write_row_hits: u64,
    row_misses: u64,
    refreshes: u64,
    refresh_ticks: u64,
    commands: u64,
    turnarounds: u64,
    total_ready_latency_ticks: u64,
    read_ready_latency_ticks: u64,
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
    read_row_hits: u64,
    write_row_hits: u64,
    row_misses: u64,
    refreshes: u64,
    refresh_ticks: u64,
    commands: u64,
    turnarounds: u64,
    total_ready_latency_ticks: u64,
    max_ready_latency_ticks: u64,
    low_power_active_powerdown_entries: u64,
    low_power_active_powerdown_ticks: u64,
    low_power_precharge_powerdown_entries: u64,
    low_power_precharge_powerdown_ticks: u64,
    low_power_self_refresh_entries: u64,
    low_power_self_refresh_ticks: u64,
    low_power_exits: u64,
    low_power_exit_latency_ticks: u64,
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
    low_power_active_powerdown_entries: u64,
    low_power_active_powerdown_ticks: u64,
    low_power_precharge_powerdown_entries: u64,
    low_power_precharge_powerdown_ticks: u64,
    low_power_self_refresh_entries: u64,
    low_power_self_refresh_ticks: u64,
    low_power_exits: u64,
    low_power_exit_latency_ticks: u64,
    banks: Vec<Rem6DramBankSummary>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6DramBankSummary {
    bank: u32,
    accesses: u64,
    reads: u64,
    writes: u64,
    read_bytes: u64,
    write_bytes: u64,
    row_hits: u64,
    read_row_hits: u64,
    write_row_hits: u64,
    row_misses: u64,
    refreshes: u64,
    refresh_ticks: u64,
    commands: u64,
    total_ready_latency_ticks: u64,
    max_ready_latency_ticks: u64,
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
    Idle,
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
pub struct Rem6CoreSummary {
    cpu: u32,
    pc: u64,
    committed_instructions: u64,
    in_order_pipeline_cycles: u64,
    in_order_pipeline_in_flight: u64,
    in_order_pipeline_stage_widths: Rem6InOrderPipelineStageSummary,
    in_order_pipeline_stage_in_flight: Rem6InOrderPipelineStageSummary,
    in_order_pipeline_stage_max_in_flight: Rem6InOrderPipelineStageSummary,
    in_order_pipeline_stage_occupied_cycles: Rem6InOrderPipelineStageSummary,
    in_order_pipeline_stage_resource_blocked: Rem6InOrderPipelineStageSummary,
    in_order_pipeline_stage_ordering_blocked: Rem6InOrderPipelineStageSummary,
    in_order_pipeline_stage_flushed: Rem6InOrderPipelineStageSummary,
    in_order_pipeline_retired: u64,
    in_order_pipeline_advanced: u64,
    in_order_pipeline_flushed: u64,
    in_order_pipeline_resource_blocked: u64,
    in_order_pipeline_ordering_blocked: u64,
    in_order_pipeline_stall_cycles: u64,
    in_order_pipeline_fetch_wait_cycles: u64,
    in_order_pipeline_data_wait_cycles: u64,
    in_order_pipeline_branch_predictions: u64,
    in_order_pipeline_branch_mispredictions: u64,
    in_order_pipeline_conditional_branch_predictions: u64,
    in_order_pipeline_conditional_branch_predicted_taken: u64,
    in_order_pipeline_conditional_branch_mispredictions: u64,
    in_order_pipeline_branch_prediction_flushes: u64,
    in_order_pipeline_redirects: u64,
    in_order_pipeline_branch_speculation_predictions: u64,
    in_order_pipeline_branch_speculation_repairs: u64,
    in_order_pipeline_branch_speculation_removed_youngers: u64,
    in_order_pipeline_branch_speculation_max_pending: u64,
    branch_target_buffer_lookups: u64,
    branch_target_buffer_hits: u64,
    branch_target_buffer_misses: u64,
    branch_target_buffer_updates: u64,
    branch_target_buffer_evictions: u64,
    branch_target_buffer_mispredictions: u64,
    branch_target_buffer_predicted_taken_misses: u64,
    branch_target_buffer_mispredict_due_to_btb_miss: BranchTargetKindCounts,
    branch_predictor_lookups: BranchTargetKindCounts,
    branch_predictor_target_provider: BranchTargetProviderCounts,
    branch_predictor_committed: BranchTargetKindCounts,
    branch_predictor_mispredicted: BranchTargetKindCounts,
    branch_predictor_corrected: BranchTargetKindCounts,
    branch_predictor_target_wrong: BranchTargetKindCounts,
    branch_predictor_mispredict_due_to_predictor: BranchTargetKindCounts,
    branch_predictor_gshare: Rem6BranchPredictorCounterSummary,
    branch_predictor_bimode: Rem6BranchPredictorCounterSummary,
    branch_predictor_tournament: Rem6BranchPredictorCounterSummary,
    branch_predictor_tage_sc_l: Rem6TageScLBranchPredictorCounterSummary,
    branch_predictor_multiperspective_perceptron: Rem6MultiperspectivePerceptronCounterSummary,
    tournament_local_predictions: u64,
    tournament_global_predictions: u64,
    data_loads: u64,
    data_stores: u64,
    data_atomics: u64,
    data_load_bytes: u64,
    data_store_bytes: u64,
    data_atomic_bytes: u64,
    checker: Option<Rem6CheckerSummary>,
    registers: Vec<(u8, u64)>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6CheckerSummary {
    checked_instructions: u64,
    mismatches: u64,
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
        "multi-run" => multi_run_cli::run_multi_run_cli(args),
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
    cli_output::emit_cli_output(
        output,
        &artifact.stats_json,
        &artifact.stats_text,
        artifact.config.output(),
        artifact.config.stats_output(),
        stats_format,
        &extra_artifacts,
    )
}

impl Rem6RunArtifact {
    pub(crate) fn emit_configured_output(&self) -> Result<(), Rem6CliError> {
        let stats_format = self.config.stats_format();
        let extra_artifacts = self
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
        cli_output::emit_configured_artifact_output(
            || self.to_json(),
            &self.stats_json,
            &self.stats_text,
            self.config.output(),
            self.config.stats_output(),
            stats_format,
            &extra_artifacts,
        )
        .map(|_| ())
    }
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
                resource_payloads.as_ref(),
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
            .kernel_binary(config.kernel_resource());
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
    resource_payloads: Option<&RunResourcePayloads>,
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
        config.dram_low_power_timing(),
        config.dram_refresh_timing(),
    )?;
    let instruction_cache = cli_cache_runtime_with_prefetcher(
        config.instruction_cache_protocol(),
        line_layout,
        core_count,
        config.instruction_cache_prefetcher(),
    )?;
    let instruction_cache_l2 = cli_cache_runtime_with_prefetcher(
        config.instruction_cache_l2_protocol(),
        line_layout,
        core_count,
        None,
    )?;
    let instruction_cache_l3 = cli_cache_runtime_with_prefetcher(
        config.instruction_cache_l3_protocol(),
        line_layout,
        core_count,
        None,
    )?;
    let instruction_cache_hierarchy = CliCacheHierarchy::from_levels([
        instruction_cache.clone(),
        instruction_cache_l2.clone(),
        instruction_cache_l3.clone(),
    ]);
    let data_cache = cli_cache_runtime_with_prefetcher(
        config.data_cache_protocol(),
        line_layout,
        core_count,
        config.data_cache_prefetcher(),
    )?;
    let data_cache_l2 = cli_cache_runtime_with_prefetcher(
        config.data_cache_l2_protocol(),
        line_layout,
        core_count,
        None,
    )?;
    let data_cache_l3 = cli_cache_runtime_with_prefetcher(
        config.data_cache_l3_protocol(),
        line_layout,
        core_count,
        None,
    )?;
    let data_cache_hierarchy = CliCacheHierarchy::from_levels([
        data_cache.clone(),
        data_cache_l2.clone(),
        data_cache_l3.clone(),
    ]);
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
    let mut transport = run_memory_transport(config.fabric());
    let mut cores = Vec::new();
    let in_order_pipeline_config = cli_in_order_pipeline_config(config.riscv_in_order_width())?;
    for cpu_index in 0..core_count {
        let cpu_partition = PartitionId::new(cpu_index);
        let fetch_route = add_memory_route(
            &mut transport,
            format!("cpu{cpu_index}.ifetch"),
            cpu_partition,
            memory_partition,
            config.memory_route_delay(),
            config.fabric(),
        )?;
        let data_route = add_memory_route(
            &mut transport,
            format!("cpu{cpu_index}.dmem"),
            cpu_partition,
            memory_partition,
            config.memory_route_delay(),
            config.fabric(),
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
        configure_cli_riscv_sbi_core(config, cpu_index, &core, start_address);
        if config.checker_cpu() {
            core.enable_checker_cpu();
        }
        core.reset_in_order_pipeline_config(in_order_pipeline_config.clone());
        core.set_branch_lookahead(config.riscv_branch_lookahead());
        core.set_branch_predictor_kind(config.riscv_branch_predictor());
        cores.push(core);
    }
    let cluster = RiscvCluster::new(cores).map_err(execute_error)?;
    let instruction_stats = cli_instruction_stats(core_count, config.riscv_pc_count_targets());
    let controller = Arc::new(Mutex::new(SystemHostController::new(
        HostEventPolicy,
        StatsRegistry::new(),
    )));
    attach_cli_riscv_checkpoint_bank(&controller, &cluster)?;
    attach_cli_memory_checkpoint_bank(&controller, &memory)?;
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
    driver = attach_cli_riscv_sbi_firmware(
        config,
        driver,
        &memory,
        instruction_cache_hierarchy.clone(),
        data_cache_hierarchy.clone(),
        line_layout,
    );
    let mut riscv_se_path_file_writebacks = Vec::new();
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
        if let Some(stdin_source) = config.riscv_se_stdin() {
            let stdin = read_riscv_se_stdin(stdin_source, resource_payloads)?;
            driver
                .riscv_syscall_emulation()
                .expect("RISC-V SE syscall emulation was just installed")
                .push_stdin_bytes(&stdin);
        }
        for file in config.riscv_se_files() {
            let contents = read_riscv_se_file(file, resource_payloads)?;
            let identity = driver
                .riscv_syscall_emulation()
                .expect("RISC-V SE syscall emulation was just installed")
                .register_guest_file(file.guest_path().as_bytes(), contents);
            if let RiscvSeInputSource::Path(path) = file.source() {
                riscv_se_path_file_writebacks.push(RiscvSePathFileWriteback {
                    guest_path: file.guest_path().to_string(),
                    host_path: path.to_path_buf(),
                    identity,
                });
            }
        }
        driver = with_riscv_syscall_data_cache_memory_io(
            driver,
            memory.clone(),
            instruction_cache_hierarchy.clone(),
            data_cache_hierarchy.clone(),
            line_layout,
        );
        if config.debug_syscall_enabled() {
            driver
                .riscv_syscall_emulation()
                .expect("RISC-V SE syscall emulation was just installed")
                .enable_syscall_trace();
        }
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
            instruction_cache_hierarchy.clone(),
            data_cache_hierarchy.clone(),
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
            instruction_cache_hierarchy.clone(),
            data_cache_hierarchy.clone(),
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
    if let Some(error) = data_cache_hierarchy.take_error() {
        return Err(error);
    }
    if let Some(error) = instruction_cache_hierarchy.take_error() {
        return Err(error);
    }
    let mut run = run_result.map_err(execute_error)?;
    write_back_riscv_se_path_files(&driver, &riscv_se_path_file_writebacks)?;
    if let Some(data_cache) = data_cache.as_ref() {
        run = run.with_data_cache_run_records(data_cache.records());
    }
    let (riscv_guest_writes, riscv_unknown_syscalls, riscv_syscall_trace) = driver
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
            let syscall_trace = state.syscall_trace().to_vec();
            (guest_writes, unknown_syscalls, syscall_trace)
        })
        .unwrap_or_default();
    let riscv_sbi_output = riscv_sbi_runtime::collect_cli_riscv_sbi_output(&driver, core_count);
    let host_actions = {
        let controller = controller
            .lock()
            .map_err(|error| execute_error(format!("host controller lock poisoned: {error}")))?;
        Rem6HostActionSummary::from_outcomes(controller.run().action_outcomes())
    };

    let summary_inputs = ExecutionSummaryInputs {
        core_count,
        memory: &memory,
        line_layout,
        config,
        instruction_cache: CliDataCacheSummary::from_records(
            &instruction_cache_hierarchy.records(0),
        )
        .with_prefetch_summary(instruction_cache_hierarchy.top_prefetch_summary()),
        instruction_cache_l2: CliDataCacheSummary::from_records(
            &instruction_cache_hierarchy.records(1),
        ),
        instruction_cache_l3: CliDataCacheSummary::from_records(
            &instruction_cache_hierarchy.records(2),
        ),
        data_cache: CliDataCacheSummary::from_run(&run)
            .with_prefetch_summary(data_cache_hierarchy.top_prefetch_summary()),
        data_cache_l2: CliDataCacheSummary::from_records(&data_cache_hierarchy.records(1)),
        data_cache_l3: CliDataCacheSummary::from_records(&data_cache_hierarchy.records(2)),
        fetch_trace: &fetch_trace,
        data_trace: &data_trace,
        fabric: Rem6RunFabricSummary::from_transport(config.fabric(), &transport),
        riscv_guest_writes,
        riscv_unknown_syscalls,
        riscv_sbi_console: riscv_sbi_output.console,
        riscv_sbi_timers: riscv_sbi_output.timers,
        riscv_sbi_hsm_events: riscv_sbi_output.hsm_events,
        riscv_sbi_hsm_wakes: riscv_sbi_output.hsm_wakes,
        riscv_sbi_ipis: riscv_sbi_output.ipis,
        riscv_sbi_rfences: riscv_sbi_output.rfences,
        riscv_sbi_rfence_completions: riscv_sbi_output.rfence_completions,
        riscv_sbi_resets: riscv_sbi_output.resets,
        riscv_syscall_trace,
        host_actions,
        prior_committed_by_cpu: gdb_outcome.retired_by_cpu().clone(),
    };

    execution_summary(&cluster, &run, summary_inputs)
}

fn write_back_riscv_se_path_files(
    driver: &RiscvSystemRunDriver,
    files: &[RiscvSePathFileWriteback],
) -> Result<(), Rem6CliError> {
    let Some(emulation) = driver.riscv_syscall_emulation() else {
        return Ok(());
    };
    let state = emulation.state();
    for file in files {
        if !state.guest_file_contents_dirty_by_identity(file.identity) {
            continue;
        }
        let Some(contents) = state.guest_file_contents_by_identity(file.identity) else {
            continue;
        };
        std::fs::write(&file.host_path, contents).map_err(|error| {
            Rem6CliError::WriteRiscvSeFile {
                guest_path: file.guest_path.clone(),
                path: file.host_path.clone(),
                error: error.to_string(),
            }
        })?;
    }
    Ok(())
}

fn add_memory_route(
    transport: &mut MemoryTransport,
    source: String,
    cpu_partition: PartitionId,
    memory_partition: PartitionId,
    route_delay: u64,
    fabric: Option<&RunFabricConfig>,
) -> Result<MemoryRouteId, Rem6CliError> {
    let source = transport_endpoint(source)?;
    let target = transport_endpoint("memory".to_string())?;
    let route = match fabric {
        Some(fabric) => {
            let hop = MemoryRouteHop::new(target, memory_partition, route_delay, route_delay)
                .map_err(execute_error)?
                .with_request_fabric_path(run_fabric_path(
                    fabric,
                    route_delay,
                    fabric.request_virtual_network(),
                )?)
                .with_response_fabric_path(run_fabric_path(
                    fabric,
                    route_delay,
                    fabric.response_virtual_network(),
                )?);
            MemoryRoute::new_path(source, cpu_partition, [hop])
                .map_err(execute_error)?
                .with_virtual_networks(
                    VirtualNetworkId::new(fabric.request_virtual_network()),
                    VirtualNetworkId::new(fabric.response_virtual_network()),
                )
        }
        None => MemoryRoute::new(
            source,
            cpu_partition,
            target,
            memory_partition,
            route_delay,
            route_delay,
        )
        .map_err(execute_error)?,
    };

    transport.add_route(route).map_err(execute_error)
}

fn run_fabric_path(
    fabric: &RunFabricConfig,
    latency: u64,
    virtual_network: u16,
) -> Result<FabricPath, Rem6CliError> {
    let link = FabricLinkId::new(fabric.link()).map_err(execute_error)?;
    let hop = FabricPathHop::new(link, latency, fabric.bandwidth_bytes_per_tick())
        .map_err(execute_error)?
        .with_virtual_network(VirtualNetworkId::new(virtual_network));
    let hop = match fabric.credit_depth() {
        Some(credit_depth) => hop.with_credit_depth(credit_depth).map_err(execute_error)?,
        None => hop,
    };
    FabricPath::new([hop]).map_err(execute_error)
}

fn cli_in_order_pipeline_config(width: usize) -> Result<InOrderPipelineConfig, Rem6CliError> {
    InOrderPipelineConfig::new(InOrderPipelineStage::ALL.map(|stage| {
        InOrderPipelineStageWidth::new(stage, width)
            .expect("validated RISC-V in-order pipeline width is nonzero")
    }))
    .map_err(execute_error)
}

fn cli_instruction_stats(
    core_count: u32,
    pc_count_targets: &[PcCountPair],
) -> RiscvInstructionStats {
    RiscvInstructionStats::for_cpus((0..core_count).map(CpuId::new))
        .with_pc_count_targets(pc_count_targets.iter().copied())
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
