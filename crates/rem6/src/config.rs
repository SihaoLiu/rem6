use std::path::{Path, PathBuf};

use rem6_cpu::RiscvBranchPredictorKind;
use rem6_fabric::QosPriority;
use rem6_stats::PcCountPair;
use rem6_system::{ExecutionMode, RiscvDataCacheProtocol};
use rem6_workload::WorkloadDataCacheProtocol;
use serde::Deserialize;

use crate::Rem6CliError;

mod accessors;
mod cache;
mod debug;
mod dram;
mod fabric;
mod file_scan;
mod guest_host_call;
mod isa;
mod memory_system;
mod output_format;
mod parse;
mod request;
mod riscv_branch;
mod riscv_se_input;
mod riscv_timing;
mod trace_replay;

pub use cache::CliCachePrefetcher;
use cache::{parse_data_cache_protocol, parse_run_data_cache_protocol};
pub use debug::CliDebugFlag;
use debug::{parse_debug_flag_list, parse_debug_flags};
use dram::{
    apply_dram_option_flag, validate_dram_timing_options, CliDramLowPowerTimingOptions,
    CliDramRefreshTimingOptions, CliDramTimingOptions,
};
pub use dram::{CliDramLowPowerTiming, CliDramMemoryProfile, CliDramRefreshTiming, CliDramTiming};
pub use fabric::RunFabricConfig;
pub(crate) use fabric::RunFabricRouterStageConfig;
use fabric::{run_fabric_config_from_parts, RunFabricConfigParts};
use file_scan::{
    gups_file_config_from_args, run_file_config_from_args, trace_replay_file_config_from_args,
};
use guest_host_call::parse_guest_host_call_response;
pub(crate) use guest_host_call::GuestHostCallResponseConfig;
pub use isa::RequestedIsa;
pub use memory_system::RunMemorySystem;
use memory_system::{apply_run_memory_system_preset, default_run_memory_system_for_execution};
pub use output_format::{PowerAnalysisFormat, StatsFormat};
use parse::{parse_number, parse_positive_u64, required_value};
pub use request::{
    KernelResourceSelector, LoadBlobRequest, LoadBlobSource, MemoryDumpRequest, ReadfileRequest,
    ReadfileSource, SuiteResourceSelector,
};
use riscv_branch::{
    parse_riscv_branch_predictor, parse_riscv_pc_count_target, valid_riscv_branch_lookahead,
};
use riscv_se_input::reject_conflicting_riscv_se_output_paths;
pub use riscv_se_input::{RiscvSeFileRequest, RiscvSeInputSource};
pub(crate) use riscv_timing::DEFAULT_RISCV_IN_ORDER_WIDTH;
use riscv_timing::{
    parse_execution_mode, parse_riscv_in_order_width, validate_riscv_in_order_width,
};
pub use trace_replay::{TraceReplayExternalAdapterKind, TraceReplayFabricRouterStageConfig};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunConfig {
    isa: RequestedIsa,
    binary: PathBuf,
    resource_config: Option<PathBuf>,
    kernel_resource: Option<KernelResourceSelector>,
    max_tick: u64,
    min_remote_delay: u64,
    memory_route_delay: u64,
    host_event_delay: u64,
    host_checkpoints: Vec<TraceReplayHostEventSpec>,
    host_checkpoint_restores: Vec<TraceReplayHostEventSpec>,
    start_address: Option<u64>,
    riscv_boot_a0: u64,
    riscv_boot_a1: u64,
    riscv_sbi: bool,
    riscv_sbi_console_input: Option<RiscvSeInputSource>,
    riscv_se: bool,
    riscv_se_args: Vec<String>,
    riscv_se_env: Vec<String>,
    riscv_se_stdin: Option<RiscvSeInputSource>,
    riscv_se_files: Vec<RiscvSeFileRequest>,
    riscv_pc_count_targets: Vec<PcCountPair>,
    riscv_branch_lookahead: usize,
    riscv_branch_predictor: RiscvBranchPredictorKind,
    riscv_in_order_width: Option<usize>,
    riscv_execution_mode: Option<ExecutionMode>,
    m5_switch_cpu_mode: Option<ExecutionMode>,
    guest_host_call_responses: Vec<GuestHostCallResponseConfig>,
    max_instructions: Option<u64>,
    stats_format: StatsFormat,
    execute: bool,
    checker_cpu: bool,
    memory_system: Option<RunMemorySystem>,
    dram_memory: bool,
    dram_memory_profile: CliDramMemoryProfile,
    dram_timing: CliDramTiming,
    dram_low_power_timing: CliDramLowPowerTiming,
    dram_refresh_timing: Option<CliDramRefreshTiming>,
    data_cache_protocol: Option<RiscvDataCacheProtocol>,
    data_cache_l2_protocol: Option<RiscvDataCacheProtocol>,
    data_cache_l3_protocol: Option<RiscvDataCacheProtocol>,
    data_cache_prefetcher: Option<CliCachePrefetcher>,
    instruction_cache_protocol: Option<RiscvDataCacheProtocol>,
    instruction_cache_l2_protocol: Option<RiscvDataCacheProtocol>,
    instruction_cache_l3_protocol: Option<RiscvDataCacheProtocol>,
    instruction_cache_prefetcher: Option<CliCachePrefetcher>,
    fabric: Option<RunFabricConfig>,
    gdb_listen: Option<String>,
    debug_flags: Vec<CliDebugFlag>,
    cores: usize,
    parallel_workers: usize,
    memory_dumps: Vec<MemoryDumpRequest>,
    load_blobs: Vec<LoadBlobRequest>,
    readfiles: Vec<ReadfileRequest>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    power_format: PowerAnalysisFormat,
    power_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6GupsConfig {
    memory_start: u64,
    memory_size: u64,
    updates: u64,
    max_tick: u64,
    min_remote_delay: u64,
    memory_route_delay: u64,
    stats_format: StatsFormat,
    rng_state: u64,
    memory_dumps: Vec<MemoryDumpRequest>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6TraceReplayConfig {
    trace: PathBuf,
    resource_config: Option<PathBuf>,
    trace_resource: Option<SuiteResourceSelector>,
    route: String,
    memory_start: u64,
    memory_size: u64,
    max_tick: u64,
    min_remote_delay: u64,
    memory_route_delay: u64,
    tick_frequency: u64,
    line_bytes: u64,
    agent: u32,
    control_partition: u32,
    data_cache_protocol: Option<WorkloadDataCacheProtocol>,
    data_cache_dram_memory_profile: Option<CliDramMemoryProfile>,
    data_cache_dram_qos_priority_levels: Option<u8>,
    data_cache_dram_qos_default_priority: Option<QosPriority>,
    fabric_link: Option<String>,
    fabric_bandwidth_bytes_per_tick: Option<u64>,
    fabric_request_virtual_network: u16,
    fabric_response_virtual_network: u16,
    fabric_credit_depth: Option<u32>,
    fabric_router_stage: Option<TraceReplayFabricRouterStageConfig>,
    external_adapter_kind: Option<TraceReplayExternalAdapterKind>,
    external_adapter_endpoint: Option<String>,
    external_adapter_checkpoint_after_events: Option<usize>,
    host_checkpoints: Vec<TraceReplayHostEventSpec>,
    host_checkpoint_restores: Vec<TraceReplayHostEventSpec>,
    stats_format: StatsFormat,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    power_format: PowerAnalysisFormat,
    power_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TraceReplayHostEventSpec {
    tick: u64,
    label: String,
}

impl TraceReplayHostEventSpec {
    pub(crate) fn new(tick: u64, label: impl Into<String>) -> Self {
        Self {
            tick,
            label: label.into(),
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6FileConfig {
    run: Option<Rem6RunFileConfig>,
    gups: Option<Rem6GupsFileConfig>,
    trace_replay: Option<Rem6TraceReplayFileConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6RunFileConfig {
    isa: Option<String>,
    binary: Option<PathBuf>,
    resource_config: Option<PathBuf>,
    kernel_resource: Option<String>,
    max_tick: Option<u64>,
    min_remote_delay: Option<u64>,
    memory_route_delay: Option<u64>,
    host_event_delay: Option<u64>,
    host_checkpoints: Option<Vec<String>>,
    host_checkpoint_restores: Option<Vec<String>>,
    start_address: Option<u64>,
    riscv_boot_a0: Option<u64>,
    riscv_boot_a1: Option<u64>,
    riscv_sbi: Option<bool>,
    riscv_sbi_console_input: Option<String>,
    riscv_se: Option<bool>,
    riscv_se_args: Option<Vec<String>>,
    riscv_se_env: Option<Vec<String>>,
    riscv_se_stdin: Option<String>,
    riscv_se_files: Option<Vec<String>>,
    riscv_pc_count_targets: Option<Vec<String>>,
    riscv_branch_lookahead: Option<usize>,
    riscv_branch_predictor: Option<String>,
    riscv_in_order_width: Option<usize>,
    riscv_execution_mode: Option<String>,
    m5_switch_cpu_mode: Option<String>,
    guest_host_call_responses: Option<Vec<String>>,
    max_instructions: Option<u64>,
    stats_format: Option<String>,
    execute: Option<bool>,
    checker_cpu: Option<bool>,
    memory_system: Option<String>,
    dram_memory: Option<bool>,
    dram_memory_profile: Option<String>,
    dram_activate_latency: Option<u64>,
    dram_read_latency: Option<u64>,
    dram_write_latency: Option<u64>,
    dram_precharge_latency: Option<u64>,
    dram_bus_turnaround: Option<u64>,
    dram_burst_spacing: Option<u64>,
    dram_same_bank_group_burst_spacing: Option<u64>,
    dram_command_window_cycles: Option<u64>,
    dram_command_window_max_commands: Option<u32>,
    dram_refresh_policy: Option<String>,
    dram_low_power_precharge_powerdown_entry_delay: Option<u64>,
    dram_low_power_self_refresh_entry_delay: Option<u64>,
    dram_low_power_exit_latency: Option<u64>,
    dram_low_power_self_refresh_exit_latency: Option<u64>,
    dram_refresh_interval: Option<u64>,
    dram_refresh_recovery: Option<u64>,
    data_cache_protocol: Option<String>,
    data_cache_l2_protocol: Option<String>,
    data_cache_l3_protocol: Option<String>,
    data_cache_prefetcher: Option<String>,
    instruction_cache_protocol: Option<String>,
    instruction_cache_l2_protocol: Option<String>,
    instruction_cache_l3_protocol: Option<String>,
    instruction_cache_prefetcher: Option<String>,
    fabric_link: Option<String>,
    fabric_bandwidth_bytes_per_tick: Option<u64>,
    fabric_request_virtual_network: Option<u16>,
    fabric_response_virtual_network: Option<u16>,
    fabric_credit_depth: Option<u32>,
    fabric_router: Option<String>,
    fabric_router_input_port: Option<u32>,
    fabric_router_output_port: Option<u32>,
    fabric_router_virtual_channel: Option<u16>,
    fabric_request_router_virtual_channel: Option<u16>,
    fabric_response_router_virtual_channel: Option<u16>,
    fabric_router_latency: Option<u64>,
    fabric_qos_queue_policy: Option<String>,
    debug_flags: Option<Vec<String>>,
    cores: Option<usize>,
    parallel_workers: Option<usize>,
    memory_dumps: Option<Vec<String>>,
    load_blobs: Option<Vec<String>>,
    readfiles: Option<Vec<String>>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    power_format: Option<String>,
    power_output: Option<PathBuf>,
    #[serde(skip)]
    config_dir: Option<PathBuf>,
}

impl Rem6RunFileConfig {
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
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6GupsFileConfig {
    memory_start: Option<u64>,
    memory_size: Option<u64>,
    updates: Option<u64>,
    max_tick: Option<u64>,
    min_remote_delay: Option<u64>,
    memory_route_delay: Option<u64>,
    stats_format: Option<String>,
    rng_state: Option<u64>,
    memory_dumps: Option<Vec<String>>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    #[serde(skip)]
    config_dir: Option<PathBuf>,
}

impl Rem6GupsFileConfig {
    fn resolve_path(&self, path: &Path) -> PathBuf {
        resolve_config_path(self.config_dir.as_deref(), path)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6TraceReplayFileConfig {
    trace: Option<PathBuf>,
    resource_config: Option<PathBuf>,
    trace_resource: Option<String>,
    route: Option<String>,
    memory_start: Option<u64>,
    memory_size: Option<u64>,
    max_tick: Option<u64>,
    min_remote_delay: Option<u64>,
    memory_route_delay: Option<u64>,
    tick_frequency: Option<u64>,
    line_bytes: Option<u64>,
    agent: Option<u32>,
    control_partition: Option<u32>,
    data_cache_protocol: Option<String>,
    data_cache_dram_memory_profile: Option<String>,
    data_cache_dram_qos_priority_levels: Option<u8>,
    data_cache_dram_qos_default_priority: Option<u8>,
    fabric_link: Option<String>,
    fabric_bandwidth_bytes_per_tick: Option<u64>,
    fabric_request_virtual_network: Option<u16>,
    fabric_response_virtual_network: Option<u16>,
    fabric_credit_depth: Option<u32>,
    fabric_router: Option<String>,
    fabric_router_input_port: Option<u32>,
    fabric_router_output_port: Option<u32>,
    fabric_router_virtual_channel: Option<u16>,
    fabric_router_latency: Option<u64>,
    external_adapter_kind: Option<String>,
    external_adapter_endpoint: Option<String>,
    external_adapter_checkpoint_after_events: Option<u64>,
    host_checkpoints: Option<Vec<String>>,
    host_checkpoint_restores: Option<Vec<String>>,
    stats_format: Option<String>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    power_format: Option<String>,
    power_output: Option<PathBuf>,
    #[serde(skip)]
    config_dir: Option<PathBuf>,
}

impl Rem6TraceReplayFileConfig {
    fn resolve_path(&self, path: &Path) -> PathBuf {
        resolve_config_path(self.config_dir.as_deref(), path)
    }
}

fn resolve_config_path(config_dir: Option<&Path>, path: &Path) -> PathBuf {
    if path.is_relative() {
        config_dir
            .map(|dir| dir.join(path))
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn run_host_events_from_file(
    values: Option<&[String]>,
) -> Result<Vec<TraceReplayHostEventSpec>, Rem6CliError> {
    values
        .unwrap_or_default()
        .iter()
        .map(|value| parse_run_host_event(value))
        .collect()
}

fn parse_run_host_event(value: &str) -> Result<TraceReplayHostEventSpec, Rem6CliError> {
    let Some((tick, label)) = value.split_once(':') else {
        return Err(Rem6CliError::InvalidRunHostCheckpointEvent {
            value: value.to_string(),
        });
    };
    let tick = tick
        .parse::<u64>()
        .map_err(|_| Rem6CliError::InvalidRunHostCheckpointEvent {
            value: value.to_string(),
        })?;
    if label.is_empty() {
        return Err(Rem6CliError::InvalidRunHostCheckpointEvent {
            value: value.to_string(),
        });
    }
    Ok(TraceReplayHostEventSpec::new(tick, label))
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
        let remaining_args = args.collect::<Vec<_>>();
        let file_config = run_file_config_from_args(&remaining_args)?
            .map(|path| load_run_file_config(&path))
            .transpose()?
            .unwrap_or_default();

        let mut isa = file_config
            .isa
            .as_deref()
            .map(RequestedIsa::parse)
            .transpose()?;
        let mut binary = file_config
            .binary
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut resource_config = file_config
            .resource_config
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut kernel_resource = file_config
            .kernel_resource
            .as_deref()
            .map(|value| {
                KernelResourceSelector::parse(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunKernelResource {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut max_tick = file_config.max_tick;
        let mut min_remote_delay = file_config.min_remote_delay.unwrap_or(1);
        if min_remote_delay == 0 {
            return Err(Rem6CliError::InvalidMinRemoteDelay {
                value: min_remote_delay.to_string(),
            });
        }
        let mut memory_route_delay = file_config.memory_route_delay;
        if memory_route_delay == Some(0) {
            return Err(Rem6CliError::InvalidMemoryRouteDelay {
                value: "0".to_string(),
            });
        }
        let mut host_event_delay = file_config.host_event_delay;
        if host_event_delay == Some(0) {
            return Err(Rem6CliError::InvalidHostEventDelay {
                value: "0".to_string(),
            });
        }
        let mut host_checkpoints =
            run_host_events_from_file(file_config.host_checkpoints.as_deref())?;
        let mut host_checkpoint_restores =
            run_host_events_from_file(file_config.host_checkpoint_restores.as_deref())?;
        let mut start_address = file_config.start_address;
        let mut riscv_boot_a0 = file_config.riscv_boot_a0.unwrap_or(0);
        let mut riscv_boot_a1 = file_config.riscv_boot_a1.unwrap_or(0);
        let mut riscv_sbi = file_config.riscv_sbi.unwrap_or(false);
        let mut riscv_sbi_console_input = file_config
            .riscv_sbi_console_input
            .as_deref()
            .map(|source| {
                RiscvSeInputSource::parse(source).ok_or_else(|| {
                    Rem6CliError::InvalidRiscvSbiConsoleInput {
                        value: source.to_string(),
                    }
                })
            })
            .transpose()?;
        if let Some(source) = riscv_sbi_console_input.as_mut() {
            if let Some(config_dir) = file_config.config_dir.as_deref() {
                source.resolve_path(config_dir);
            }
        }
        let mut riscv_se = file_config.riscv_se.unwrap_or(false);
        let mut riscv_se_args = file_config.riscv_se_args.clone().unwrap_or_default();
        let mut riscv_se_env = file_config.riscv_se_env.clone().unwrap_or_default();
        let mut riscv_se_stdin = file_config
            .riscv_se_stdin
            .as_deref()
            .map(|source| {
                RiscvSeInputSource::parse(source).ok_or_else(|| Rem6CliError::InvalidRiscvSeStdin {
                    value: source.to_string(),
                })
            })
            .transpose()?;
        if let Some(source) = riscv_se_stdin.as_mut() {
            if let Some(config_dir) = file_config.config_dir.as_deref() {
                source.resolve_path(config_dir);
            }
        }
        let mut riscv_se_files = file_config
            .riscv_se_files
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|request| {
                let mut request = RiscvSeFileRequest::parse(request)?;
                if let Some(config_dir) = file_config.config_dir.as_deref() {
                    request.resolve_host_path(config_dir);
                }
                Ok(request)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut riscv_pc_count_targets = file_config
            .riscv_pc_count_targets
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|target| parse_riscv_pc_count_target(target))
            .collect::<Result<Vec<_>, _>>()?;
        let mut max_instructions = file_config.max_instructions;
        if max_instructions == Some(0) {
            return Err(Rem6CliError::InvalidMaxInstructions {
                value: "0".to_string(),
            });
        }
        let mut riscv_branch_lookahead = file_config.riscv_branch_lookahead.unwrap_or(1);
        if !valid_riscv_branch_lookahead(riscv_branch_lookahead) {
            return Err(Rem6CliError::InvalidRiscvBranchLookahead {
                value: riscv_branch_lookahead.to_string(),
            });
        }
        let mut riscv_branch_predictor = file_config
            .riscv_branch_predictor
            .as_deref()
            .map(|value| {
                parse_riscv_branch_predictor(value).ok_or_else(|| {
                    Rem6CliError::InvalidRiscvBranchPredictor {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?
            .unwrap_or_default();
        let mut riscv_in_order_width = file_config
            .riscv_in_order_width
            .map(|width| validate_riscv_in_order_width(width, width.to_string()))
            .transpose()?;
        let mut riscv_execution_mode = file_config
            .riscv_execution_mode
            .as_deref()
            .map(|value| {
                parse_execution_mode(value).ok_or_else(|| Rem6CliError::InvalidRiscvExecutionMode {
                    value: value.to_string(),
                })
            })
            .transpose()?;
        let mut m5_switch_cpu_mode = file_config
            .m5_switch_cpu_mode
            .as_deref()
            .map(|value| {
                parse_execution_mode(value).ok_or_else(|| Rem6CliError::InvalidM5SwitchCpuMode {
                    value: value.to_string(),
                })
            })
            .transpose()?;
        let guest_host_call_responses = file_config
            .guest_host_call_responses
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|response| parse_guest_host_call_response(response))
            .collect::<Result<Vec<_>, _>>()?;
        let mut stats_format = file_config
            .stats_format
            .as_deref()
            .map(StatsFormat::parse)
            .transpose()?
            .unwrap_or(StatsFormat::Json);
        let mut execute = file_config.execute.unwrap_or(false);
        let mut checker_cpu = file_config.checker_cpu.unwrap_or(false);
        let mut memory_system = file_config
            .memory_system
            .as_deref()
            .map(|value| {
                RunMemorySystem::parse(value).ok_or_else(|| Rem6CliError::InvalidRunMemorySystem {
                    value: value.to_string(),
                })
            })
            .transpose()?;
        let dram_memory_disabled_by_config = file_config.dram_memory == Some(false);
        let mut dram_memory = file_config.dram_memory.unwrap_or(false);
        let mut dram_memory_profile = file_config
            .dram_memory_profile
            .as_deref()
            .map(CliDramMemoryProfile::parse)
            .transpose()?
            .unwrap_or(CliDramMemoryProfile::Ddr);
        let mut dram_memory_profile_was_set = file_config.dram_memory_profile.is_some();
        let mut dram_timing_options = CliDramTimingOptions::new(
            file_config.dram_activate_latency,
            file_config.dram_read_latency,
            file_config.dram_write_latency,
            file_config.dram_precharge_latency,
            file_config.dram_bus_turnaround,
            file_config.dram_burst_spacing,
            file_config.dram_same_bank_group_burst_spacing,
            file_config.dram_command_window_cycles,
            file_config.dram_command_window_max_commands,
            file_config.dram_refresh_policy.as_deref(),
        )?;
        let mut dram_low_power_timing_options = CliDramLowPowerTimingOptions::new(
            file_config.dram_low_power_precharge_powerdown_entry_delay,
            file_config.dram_low_power_self_refresh_entry_delay,
            file_config.dram_low_power_exit_latency,
            file_config.dram_low_power_self_refresh_exit_latency,
        );
        let mut dram_refresh_timing_options = CliDramRefreshTimingOptions::new(
            file_config.dram_refresh_interval,
            file_config.dram_refresh_recovery,
        );
        let mut data_cache_protocol = file_config
            .data_cache_protocol
            .as_deref()
            .map(|value| {
                parse_run_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunDataCacheProtocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut data_cache_l2_protocol = file_config
            .data_cache_l2_protocol
            .as_deref()
            .map(|value| {
                parse_run_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunDataCacheL2Protocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut data_cache_l3_protocol = file_config
            .data_cache_l3_protocol
            .as_deref()
            .map(|value| {
                parse_run_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunDataCacheL3Protocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut data_cache_prefetcher = file_config
            .data_cache_prefetcher
            .as_deref()
            .map(CliCachePrefetcher::parse_data_cache)
            .transpose()?;
        let mut instruction_cache_protocol = file_config
            .instruction_cache_protocol
            .as_deref()
            .map(|value| {
                parse_run_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunInstructionCacheProtocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut instruction_cache_l2_protocol = file_config
            .instruction_cache_l2_protocol
            .as_deref()
            .map(|value| {
                parse_run_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunInstructionCacheL2Protocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut instruction_cache_l3_protocol = file_config
            .instruction_cache_l3_protocol
            .as_deref()
            .map(|value| {
                parse_run_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunInstructionCacheL3Protocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut instruction_cache_prefetcher = file_config
            .instruction_cache_prefetcher
            .as_deref()
            .map(CliCachePrefetcher::parse_instruction_cache)
            .transpose()?;
        let mut fabric_parts = RunFabricConfigParts::new(
            file_config.fabric_link.clone(),
            file_config.fabric_bandwidth_bytes_per_tick,
            file_config.fabric_request_virtual_network,
            file_config.fabric_response_virtual_network,
            file_config.fabric_credit_depth,
            file_config.fabric_router.clone(),
            file_config.fabric_router_input_port,
            file_config.fabric_router_output_port,
            file_config.fabric_router_virtual_channel,
            file_config.fabric_request_router_virtual_channel,
            file_config.fabric_response_router_virtual_channel,
            file_config.fabric_router_latency,
            file_config.fabric_qos_queue_policy.clone(),
        )?;
        let mut gdb_listen = None;
        let mut debug_flags =
            parse_debug_flags(file_config.debug_flags.as_deref().unwrap_or_default())?;
        let mut cores = file_config.cores.unwrap_or(1);
        if cores == 0 {
            return Err(Rem6CliError::InvalidCoreCount {
                value: cores.to_string(),
            });
        }
        let mut parallel_workers = file_config.parallel_workers;
        if parallel_workers == Some(0) {
            return Err(Rem6CliError::InvalidParallelWorkerCount {
                value: "0".to_string(),
            });
        }
        let mut memory_dumps = file_config
            .memory_dumps
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|request| MemoryDumpRequest::parse(request))
            .collect::<Result<Vec<_>, _>>()?;
        let mut load_blobs = file_config
            .load_blobs
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|request| {
                let mut request = LoadBlobRequest::parse(request)?;
                if let Some(config_dir) = file_config.config_dir.as_deref() {
                    request.resolve_path(config_dir);
                }
                Ok(request)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut readfiles = file_config
            .readfiles
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|request| {
                let mut request = ReadfileRequest::parse(request)?;
                if let Some(config_dir) = file_config.config_dir.as_deref() {
                    request.resolve_path(config_dir);
                }
                Ok(request)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut memory_dumps_from_cli = false;
        let mut load_blobs_from_cli = false;
        let mut readfiles_from_cli = false;
        let mut riscv_se_args_from_cli = false;
        let mut riscv_se_env_from_cli = false;
        let mut riscv_sbi_console_input_from_cli = false;
        let mut riscv_se_stdin_from_cli = false;
        let mut riscv_se_files_from_cli = false;
        let mut riscv_pc_count_targets_from_cli = false;
        let mut debug_flags_from_cli = false;
        let mut output = file_config
            .output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut stats_output = file_config
            .stats_output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut power_format = file_config
            .power_format
            .as_deref()
            .map(PowerAnalysisFormat::parse)
            .transpose()?
            .unwrap_or(PowerAnalysisFormat::McpatXml);
        let mut power_output = file_config
            .power_output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut args = remaining_args.into_iter();
        while let Some(flag) = args.next() {
            if apply_dram_option_flag(
                &flag,
                &mut args,
                &mut dram_timing_options,
                &mut dram_low_power_timing_options,
                &mut dram_refresh_timing_options,
            )? {
                continue;
            }
            match flag.as_str() {
                "--config" => {
                    let _ = required_value(&flag, args.next())?;
                }
                "--isa" => {
                    isa = Some(RequestedIsa::parse(&required_value(&flag, args.next())?)?);
                }
                "--binary" => {
                    binary = Some(PathBuf::from(required_value(&flag, args.next())?));
                    resource_config = None;
                    kernel_resource = None;
                }
                "--resource-config" => {
                    resource_config = Some(PathBuf::from(required_value(&flag, args.next())?));
                    binary = None;
                }
                "--kernel-resource" => {
                    let value = required_value(&flag, args.next())?;
                    kernel_resource = Some(
                        KernelResourceSelector::parse(&value)
                            .ok_or(Rem6CliError::InvalidRunKernelResource { value })?,
                    );
                }
                "--max-tick" => {
                    let value = required_value(&flag, args.next())?;
                    max_tick = Some(value.parse().map_err(|_| Rem6CliError::InvalidMaxTick {
                        value: value.clone(),
                    })?);
                }
                "--min-remote-delay" => {
                    let value = required_value(&flag, args.next())?;
                    min_remote_delay = parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidMinRemoteDelay {
                            value: value.clone(),
                        }
                    })?;
                }
                "--memory-route-delay" => {
                    let value = required_value(&flag, args.next())?;
                    memory_route_delay = Some(parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidMemoryRouteDelay {
                            value: value.clone(),
                        }
                    })?);
                }
                "--host-event-delay" => {
                    let value = required_value(&flag, args.next())?;
                    host_event_delay = Some(parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidHostEventDelay {
                            value: value.clone(),
                        }
                    })?);
                }
                "--host-checkpoint" => {
                    let value = required_value(&flag, args.next())?;
                    host_checkpoints.push(parse_run_host_event(&value)?);
                }
                "--host-restore-checkpoint" => {
                    let value = required_value(&flag, args.next())?;
                    host_checkpoint_restores.push(parse_run_host_event(&value)?);
                }
                "--start-address" => {
                    let value = required_value(&flag, args.next())?;
                    start_address = Some(parse_number(&value).ok_or_else(|| {
                        Rem6CliError::InvalidStartAddress {
                            value: value.clone(),
                        }
                    })?);
                }
                "--riscv-boot-a0" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_boot_a0 =
                        parse_number(&value).ok_or_else(|| Rem6CliError::InvalidRiscvBootA0 {
                            value: value.clone(),
                        })?;
                }
                "--riscv-boot-a1" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_boot_a1 =
                        parse_number(&value).ok_or_else(|| Rem6CliError::InvalidRiscvBootA1 {
                            value: value.clone(),
                        })?;
                }
                "--riscv-se" => {
                    riscv_se = true;
                }
                "--riscv-sbi" => {
                    riscv_sbi = true;
                }
                "--riscv-sbi-console-input" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_sbi_console_input = Some(
                        RiscvSeInputSource::parse(&value)
                            .ok_or(Rem6CliError::InvalidRiscvSbiConsoleInput { value })?,
                    );
                    riscv_sbi_console_input_from_cli = true;
                }
                "--riscv-se-arg" => {
                    let value = required_value(&flag, args.next())?;
                    if !riscv_se_args_from_cli {
                        riscv_se_args.clear();
                        riscv_se_args_from_cli = true;
                    }
                    riscv_se_args.push(value);
                }
                "--riscv-se-env" => {
                    let value = required_value(&flag, args.next())?;
                    if !riscv_se_env_from_cli {
                        riscv_se_env.clear();
                        riscv_se_env_from_cli = true;
                    }
                    riscv_se_env.push(value);
                }
                "--riscv-se-stdin" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_se_stdin = Some(
                        RiscvSeInputSource::parse(&value)
                            .ok_or(Rem6CliError::InvalidRiscvSeStdin { value })?,
                    );
                    riscv_se_stdin_from_cli = true;
                }
                "--riscv-se-file" => {
                    let value = required_value(&flag, args.next())?;
                    if !riscv_se_files_from_cli {
                        riscv_se_files.clear();
                        riscv_se_files_from_cli = true;
                    }
                    riscv_se_files.push(RiscvSeFileRequest::parse(&value)?);
                }
                "--riscv-pc-count-target" => {
                    let value = required_value(&flag, args.next())?;
                    if !riscv_pc_count_targets_from_cli {
                        riscv_pc_count_targets.clear();
                        riscv_pc_count_targets_from_cli = true;
                    }
                    riscv_pc_count_targets.push(parse_riscv_pc_count_target(&value)?);
                }
                "--riscv-branch-lookahead" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_branch_lookahead = value
                        .parse()
                        .ok()
                        .filter(|lookahead| valid_riscv_branch_lookahead(*lookahead))
                        .ok_or_else(|| Rem6CliError::InvalidRiscvBranchLookahead {
                            value: value.clone(),
                        })?;
                }
                "--riscv-branch-predictor" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_branch_predictor =
                        parse_riscv_branch_predictor(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRiscvBranchPredictor {
                                value: value.clone(),
                            }
                        })?;
                }
                "--riscv-in-order-width" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_in_order_width = Some(parse_riscv_in_order_width(&value)?);
                }
                "--riscv-execution-mode" => {
                    let value = required_value(&flag, args.next())?;
                    riscv_execution_mode = Some(parse_execution_mode(&value).ok_or_else(|| {
                        Rem6CliError::InvalidRiscvExecutionMode {
                            value: value.clone(),
                        }
                    })?);
                }
                "--m5-switch-cpu-mode" => {
                    let value = required_value(&flag, args.next())?;
                    m5_switch_cpu_mode = Some(parse_execution_mode(&value).ok_or_else(|| {
                        Rem6CliError::InvalidM5SwitchCpuMode {
                            value: value.clone(),
                        }
                    })?);
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
                "--checker-cpu" => {
                    checker_cpu = true;
                }
                "--memory-system" => {
                    let value = required_value(&flag, args.next())?;
                    memory_system = Some(RunMemorySystem::parse(&value).ok_or_else(|| {
                        Rem6CliError::InvalidRunMemorySystem {
                            value: value.clone(),
                        }
                    })?);
                }
                "--dram-memory" => {
                    dram_memory = true;
                }
                "--dram-memory-profile" => {
                    dram_memory_profile_was_set = true;
                    dram_memory_profile =
                        CliDramMemoryProfile::parse(&required_value(&flag, args.next())?)?;
                }
                "--data-cache-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    data_cache_protocol =
                        Some(parse_run_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunDataCacheProtocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--data-cache-l2-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    data_cache_l2_protocol =
                        Some(parse_run_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunDataCacheL2Protocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--data-cache-l3-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    data_cache_l3_protocol =
                        Some(parse_run_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunDataCacheL3Protocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--data-cache-prefetcher" => {
                    data_cache_prefetcher = Some(CliCachePrefetcher::parse_data_cache(
                        &required_value(&flag, args.next())?,
                    )?);
                }
                "--instruction-cache-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    instruction_cache_protocol =
                        Some(parse_run_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunInstructionCacheProtocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--instruction-cache-l2-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    instruction_cache_l2_protocol =
                        Some(parse_run_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunInstructionCacheL2Protocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--instruction-cache-l3-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    instruction_cache_l3_protocol =
                        Some(parse_run_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunInstructionCacheL3Protocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--instruction-cache-prefetcher" => {
                    instruction_cache_prefetcher =
                        Some(CliCachePrefetcher::parse_instruction_cache(
                            &required_value(&flag, args.next())?,
                        )?);
                }
                "--fabric-link" => {
                    fabric_parts.set_link(required_value(&flag, args.next())?);
                }
                "--fabric-bandwidth-bytes-per-tick" => {
                    fabric_parts.set_bandwidth(&required_value(&flag, args.next())?)?;
                }
                "--fabric-request-virtual-network" => {
                    fabric_parts
                        .set_request_virtual_network(&required_value(&flag, args.next())?)?;
                }
                "--fabric-response-virtual-network" => {
                    fabric_parts
                        .set_response_virtual_network(&required_value(&flag, args.next())?)?;
                }
                "--fabric-credit-depth" => {
                    fabric_parts.set_credit_depth(&required_value(&flag, args.next())?)?;
                }
                "--fabric-router" => {
                    fabric_parts.set_router(&required_value(&flag, args.next())?)?;
                }
                "--fabric-router-input-port" => {
                    fabric_parts.set_router_input_port(&required_value(&flag, args.next())?)?;
                }
                "--fabric-router-output-port" => {
                    fabric_parts.set_router_output_port(&required_value(&flag, args.next())?)?;
                }
                "--fabric-router-virtual-channel" => {
                    fabric_parts
                        .set_router_virtual_channel(&required_value(&flag, args.next())?)?;
                }
                "--fabric-request-router-virtual-channel" => {
                    fabric_parts
                        .set_request_router_virtual_channel(&required_value(&flag, args.next())?)?;
                }
                "--fabric-response-router-virtual-channel" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_parts.set_response_router_virtual_channel(&value)?;
                }
                "--fabric-router-latency" => {
                    fabric_parts.set_router_latency(&required_value(&flag, args.next())?)?;
                }
                "--fabric-qos-queue-policy" => {
                    fabric_parts.set_qos_queue_policy(&required_value(&flag, args.next())?)?;
                }
                "--gdb-listen" => {
                    gdb_listen = Some(required_value(&flag, args.next())?);
                }
                "--debug-flags" => {
                    let value = required_value(&flag, args.next())?;
                    if !debug_flags_from_cli {
                        debug_flags.clear();
                        debug_flags_from_cli = true;
                    }
                    debug_flags.extend(parse_debug_flag_list(&value)?);
                    debug_flags.sort_unstable();
                    debug_flags.dedup();
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
                    if !memory_dumps_from_cli {
                        memory_dumps.clear();
                        memory_dumps_from_cli = true;
                    }
                    memory_dumps.push(MemoryDumpRequest::parse(&value)?);
                }
                "--load-blob" => {
                    let value = required_value(&flag, args.next())?;
                    if !load_blobs_from_cli {
                        load_blobs.clear();
                        load_blobs_from_cli = true;
                    }
                    load_blobs.push(LoadBlobRequest::parse(&value)?);
                }
                "--readfile" => {
                    let value = required_value(&flag, args.next())?;
                    if !readfiles_from_cli {
                        readfiles.clear();
                        readfiles_from_cli = true;
                    }
                    readfiles.push(ReadfileRequest::parse(&value)?);
                }
                "--output" => {
                    output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--stats-output" => {
                    stats_output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--power-format" => {
                    power_format =
                        PowerAnalysisFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--power-output" => {
                    power_output = Some(PathBuf::from(required_value(&flag, args.next())?));
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
        if let Some(power_output) = &power_output {
            if output.as_ref() == Some(power_output) || stats_output.as_ref() == Some(power_output)
            {
                return Err(Rem6CliError::ConflictingRunOutputPaths {
                    path: power_output.to_path_buf(),
                });
            }
        }
        reject_conflicting_riscv_se_output_paths(
            &riscv_se_files,
            output.as_deref(),
            stats_output.as_deref(),
            power_output.as_deref(),
        )?;
        let memory_route_delay = memory_route_delay.unwrap_or(min_remote_delay);
        let host_event_delay = host_event_delay.unwrap_or(min_remote_delay);
        if memory_route_delay < min_remote_delay {
            return Err(Rem6CliError::MemoryRouteDelayBelowMinRemoteDelay {
                memory_route_delay,
                min_remote_delay,
            });
        }
        if host_event_delay < min_remote_delay {
            return Err(Rem6CliError::HostEventDelayBelowMinRemoteDelay {
                host_event_delay,
                min_remote_delay,
            });
        }
        if binary.is_some() && resource_config.is_some() {
            return Err(Rem6CliError::ConflictingRunBinarySources);
        }
        if kernel_resource.is_some() && resource_config.is_none() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--resource-config",
            });
        }
        let explicit_memory_system = memory_system.is_some();
        let dram_timing_was_set = dram_timing_options.was_set();
        let dram_low_power_timing_was_set = dram_low_power_timing_options.was_set();
        let dram_refresh_timing_was_set = dram_refresh_timing_options.was_set();
        let explicit_enabled_memory_hierarchy = dram_memory
            || dram_memory_profile_was_set
            || dram_timing_was_set
            || dram_low_power_timing_was_set
            || dram_refresh_timing_was_set
            || data_cache_protocol.is_some()
            || data_cache_l2_protocol.is_some()
            || data_cache_l3_protocol.is_some()
            || data_cache_prefetcher.is_some()
            || instruction_cache_protocol.is_some()
            || instruction_cache_l2_protocol.is_some()
            || instruction_cache_l3_protocol.is_some()
            || instruction_cache_prefetcher.is_some()
            || fabric_parts.is_set();
        let explicit_memory_hierarchy =
            dram_memory_disabled_by_config || explicit_enabled_memory_hierarchy;
        if execute
            && matches!(memory_system, Some(RunMemorySystem::Direct))
            && explicit_enabled_memory_hierarchy
        {
            return Err(Rem6CliError::RunMemorySystemConflictsWithMemoryHierarchy {
                memory_system: RunMemorySystem::Direct.as_str().to_string(),
            });
        }
        if execute
            && (explicit_memory_system
                || (isa == Some(RequestedIsa::Riscv)
                    && !explicit_memory_hierarchy
                    && gdb_listen.is_none()))
        {
            let selected_memory_system = default_run_memory_system_for_execution(memory_system);
            memory_system = Some(selected_memory_system);
            apply_run_memory_system_preset(
                selected_memory_system,
                dram_memory_disabled_by_config,
                &mut dram_memory,
                &mut data_cache_protocol,
                &mut data_cache_l2_protocol,
                &mut data_cache_l3_protocol,
                &mut instruction_cache_protocol,
                &mut instruction_cache_l2_protocol,
                &mut instruction_cache_l3_protocol,
                &mut fabric_parts,
            )?;
        }
        if !riscv_sbi && riscv_sbi_console_input.is_some() {
            let input = if riscv_sbi_console_input_from_cli {
                "--riscv-sbi-console-input"
            } else {
                "riscv_sbi_console_input"
            };
            return Err(Rem6CliError::RiscvSbiInputRequiresRiscvSbi { input });
        }
        if !riscv_se {
            if !riscv_se_args.is_empty() {
                let input = if riscv_se_args_from_cli {
                    "--riscv-se-arg"
                } else {
                    "riscv_se_args"
                };
                return Err(Rem6CliError::RiscvSeInputRequiresRiscvSe { input });
            }
            if !riscv_se_env.is_empty() {
                let input = if riscv_se_env_from_cli {
                    "--riscv-se-env"
                } else {
                    "riscv_se_env"
                };
                return Err(Rem6CliError::RiscvSeInputRequiresRiscvSe { input });
            }
            if riscv_se_stdin.is_some() {
                let input = if riscv_se_stdin_from_cli {
                    "--riscv-se-stdin"
                } else {
                    "riscv_se_stdin"
                };
                return Err(Rem6CliError::RiscvSeInputRequiresRiscvSe { input });
            }
            if !riscv_se_files.is_empty() {
                let input = if riscv_se_files_from_cli {
                    "--riscv-se-file"
                } else {
                    "riscv_se_files"
                };
                return Err(Rem6CliError::RiscvSeInputRequiresRiscvSe { input });
            }
        }

        let binary = binary
            .or_else(|| {
                resource_config
                    .as_ref()
                    .map(|path| PathBuf::from(format!("resource-config:{}", path.display())))
            })
            .ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--binary or --resource-config",
            })?;
        let fabric = run_fabric_config_from_parts(fabric_parts)?;
        let (dram_timing, dram_low_power_timing, dram_refresh_timing) =
            validate_dram_timing_options(
                dram_memory,
                dram_memory_profile,
                dram_memory_profile_was_set,
                dram_timing_was_set,
                dram_low_power_timing_was_set,
                dram_refresh_timing_was_set,
                dram_timing_options,
                dram_low_power_timing_options,
                dram_refresh_timing_options,
            )?;

        Ok(Self {
            isa: isa.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--isa" })?,
            binary,
            resource_config,
            kernel_resource,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            min_remote_delay,
            memory_route_delay,
            host_event_delay,
            host_checkpoints,
            host_checkpoint_restores,
            start_address,
            riscv_boot_a0,
            riscv_boot_a1,
            riscv_sbi,
            riscv_sbi_console_input,
            riscv_se,
            riscv_se_args,
            riscv_se_env,
            riscv_se_stdin,
            riscv_se_files,
            riscv_pc_count_targets,
            riscv_branch_lookahead,
            riscv_branch_predictor,
            riscv_in_order_width,
            riscv_execution_mode,
            m5_switch_cpu_mode,
            guest_host_call_responses,
            max_instructions,
            stats_format,
            execute,
            checker_cpu,
            memory_system,
            dram_memory,
            dram_memory_profile,
            dram_timing,
            dram_low_power_timing,
            dram_refresh_timing,
            data_cache_protocol,
            data_cache_l2_protocol,
            data_cache_l3_protocol,
            data_cache_prefetcher,
            instruction_cache_protocol,
            instruction_cache_l2_protocol,
            instruction_cache_l3_protocol,
            instruction_cache_prefetcher,
            fabric,
            gdb_listen,
            debug_flags,
            cores,
            parallel_workers: parallel_workers.unwrap_or(cores),
            memory_dumps,
            load_blobs,
            readfiles,
            output,
            stats_output,
            power_format,
            power_output,
        })
    }
}

impl Rem6GupsConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "gups" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }

        let remaining_args = args.collect::<Vec<_>>();
        let file_config = gups_file_config_from_args(&remaining_args)?
            .map(|path| load_gups_file_config(&path))
            .transpose()?
            .unwrap_or_default();

        let mut memory_start = file_config.memory_start;
        let mut memory_size = file_config.memory_size;
        let mut updates = file_config.updates;
        if updates == Some(0) {
            return Err(Rem6CliError::InvalidGupsUpdates {
                value: "0".to_string(),
            });
        }
        let mut max_tick = file_config.max_tick;
        let mut min_remote_delay = file_config.min_remote_delay.unwrap_or(1);
        if min_remote_delay == 0 {
            return Err(Rem6CliError::InvalidMinRemoteDelay {
                value: min_remote_delay.to_string(),
            });
        }
        let mut memory_route_delay = file_config.memory_route_delay;
        if memory_route_delay == Some(0) {
            return Err(Rem6CliError::InvalidMemoryRouteDelay {
                value: "0".to_string(),
            });
        }
        let mut stats_format = file_config
            .stats_format
            .as_deref()
            .map(StatsFormat::parse)
            .transpose()?
            .unwrap_or(StatsFormat::Json);
        let mut rng_state = file_config.rng_state.unwrap_or(0);
        let mut memory_dumps = file_config
            .memory_dumps
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|request| MemoryDumpRequest::parse(request))
            .collect::<Result<Vec<_>, _>>()?;
        let mut memory_dumps_from_cli = false;
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
                "--memory-start" => {
                    let value = required_value(&flag, args.next())?;
                    memory_start = Some(parse_number(&value).ok_or_else(|| {
                        Rem6CliError::InvalidGupsMemoryStart {
                            value: value.clone(),
                        }
                    })?);
                }
                "--memory-size" => {
                    let value = required_value(&flag, args.next())?;
                    memory_size = Some(parse_number(&value).ok_or_else(|| {
                        Rem6CliError::InvalidGupsMemorySize {
                            value: value.clone(),
                        }
                    })?);
                }
                "--updates" => {
                    let value = required_value(&flag, args.next())?;
                    updates = Some(parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidGupsUpdates {
                            value: value.clone(),
                        }
                    })?);
                }
                "--max-tick" => {
                    let value = required_value(&flag, args.next())?;
                    max_tick = Some(value.parse().map_err(|_| Rem6CliError::InvalidMaxTick {
                        value: value.clone(),
                    })?);
                }
                "--min-remote-delay" => {
                    let value = required_value(&flag, args.next())?;
                    min_remote_delay = parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidMinRemoteDelay {
                            value: value.clone(),
                        }
                    })?;
                }
                "--memory-route-delay" => {
                    let value = required_value(&flag, args.next())?;
                    memory_route_delay = Some(parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidMemoryRouteDelay {
                            value: value.clone(),
                        }
                    })?);
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--rng-state" => {
                    let value = required_value(&flag, args.next())?;
                    rng_state =
                        parse_number(&value).ok_or_else(|| Rem6CliError::InvalidGupsRngState {
                            value: value.clone(),
                        })?;
                }
                "--dump-memory" => {
                    let value = required_value(&flag, args.next())?;
                    if !memory_dumps_from_cli {
                        memory_dumps.clear();
                        memory_dumps_from_cli = true;
                    }
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
        let memory_route_delay = memory_route_delay.unwrap_or(min_remote_delay);
        if memory_route_delay < min_remote_delay {
            return Err(Rem6CliError::MemoryRouteDelayBelowMinRemoteDelay {
                memory_route_delay,
                min_remote_delay,
            });
        }

        Ok(Self {
            memory_start: memory_start.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-start",
            })?,
            memory_size: memory_size.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-size",
            })?,
            updates: updates.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--updates" })?,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            min_remote_delay,
            memory_route_delay,
            stats_format,
            rng_state,
            memory_dumps,
            output,
            stats_output,
        })
    }

    pub const fn memory_start(&self) -> u64 {
        self.memory_start
    }

    pub const fn memory_size(&self) -> u64 {
        self.memory_size
    }

    pub const fn updates(&self) -> u64 {
        self.updates
    }

    pub const fn max_tick(&self) -> u64 {
        self.max_tick
    }

    pub const fn min_remote_delay(&self) -> u64 {
        self.min_remote_delay
    }

    pub const fn memory_route_delay(&self) -> u64 {
        self.memory_route_delay
    }

    pub const fn stats_format(&self) -> StatsFormat {
        self.stats_format
    }

    pub const fn rng_state(&self) -> u64 {
        self.rng_state
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

fn load_run_file_config(path: &Path) -> Result<Rem6RunFileConfig, Rem6CliError> {
    let mut run = load_file_config(path)?.run.unwrap_or_default();
    run.config_dir = path.parent().map(Path::to_path_buf);
    Ok(run)
}

fn load_gups_file_config(path: &Path) -> Result<Rem6GupsFileConfig, Rem6CliError> {
    let mut gups = load_file_config(path)?.gups.unwrap_or_default();
    gups.config_dir = path.parent().map(Path::to_path_buf);
    Ok(gups)
}

fn load_trace_replay_file_config(path: &Path) -> Result<Rem6TraceReplayFileConfig, Rem6CliError> {
    let mut trace_replay = load_file_config(path)?.trace_replay.unwrap_or_default();
    trace_replay.config_dir = path.parent().map(Path::to_path_buf);
    Ok(trace_replay)
}

fn load_file_config(path: &Path) -> Result<Rem6FileConfig, Rem6CliError> {
    let text = std::fs::read_to_string(path).map_err(|error| Rem6CliError::ReadConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })?;
    toml::from_str::<Rem6FileConfig>(&text).map_err(|error| Rem6CliError::ParseConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })
}
