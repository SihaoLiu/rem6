use std::path::{Path, PathBuf};

use rem6_cpu::RiscvBranchPredictorKind;
use rem6_stats::PcCountPair;
use rem6_system::RiscvDataCacheProtocol;
use rem6_workload::WorkloadDataCacheProtocol;
use serde::Deserialize;

use crate::Rem6CliError;

mod cache;
mod debug;
mod dram;
mod fabric;
mod file_scan;
mod isa;
mod memory_system;
mod output_format;
mod parse;
mod request;
mod riscv_branch;
mod riscv_se_input;
mod trace_replay;

pub use cache::CliCachePrefetcher;
use cache::{parse_data_cache_protocol, parse_run_data_cache_protocol};
pub use debug::CliDebugFlag;
use debug::{parse_debug_flag_list, parse_debug_flags};
pub use dram::CliDramMemoryProfile;
pub use fabric::RunFabricConfig;
use fabric::{
    parse_run_fabric_credit_depth, parse_run_fabric_virtual_network, run_fabric_config_from_parts,
};
use file_scan::{
    gups_file_config_from_args, run_file_config_from_args, trace_replay_file_config_from_args,
};
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
pub use trace_replay::TraceReplayExternalAdapterKind;

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
    start_address: Option<u64>,
    riscv_boot_a0: u64,
    riscv_boot_a1: u64,
    riscv_sbi: bool,
    riscv_se: bool,
    riscv_se_args: Vec<String>,
    riscv_se_env: Vec<String>,
    riscv_se_stdin: Option<RiscvSeInputSource>,
    riscv_se_files: Vec<RiscvSeFileRequest>,
    riscv_pc_count_targets: Vec<PcCountPair>,
    riscv_branch_lookahead: usize,
    riscv_branch_predictor: RiscvBranchPredictorKind,
    max_instructions: Option<u64>,
    stats_format: StatsFormat,
    execute: bool,
    checker_cpu: bool,
    memory_system: Option<RunMemorySystem>,
    dram_memory: bool,
    dram_memory_profile: CliDramMemoryProfile,
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
    fabric_link: Option<String>,
    fabric_bandwidth_bytes_per_tick: Option<u64>,
    fabric_request_virtual_network: u16,
    fabric_response_virtual_network: u16,
    fabric_credit_depth: Option<u32>,
    external_adapter_kind: Option<TraceReplayExternalAdapterKind>,
    external_adapter_endpoint: Option<String>,
    stats_format: StatsFormat,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    power_format: PowerAnalysisFormat,
    power_output: Option<PathBuf>,
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
    start_address: Option<u64>,
    riscv_boot_a0: Option<u64>,
    riscv_boot_a1: Option<u64>,
    riscv_sbi: Option<bool>,
    riscv_se: Option<bool>,
    riscv_se_args: Option<Vec<String>>,
    riscv_se_env: Option<Vec<String>>,
    riscv_se_stdin: Option<String>,
    riscv_se_files: Option<Vec<String>>,
    riscv_pc_count_targets: Option<Vec<String>>,
    riscv_branch_lookahead: Option<usize>,
    riscv_branch_predictor: Option<String>,
    max_instructions: Option<u64>,
    stats_format: Option<String>,
    execute: Option<bool>,
    checker_cpu: Option<bool>,
    memory_system: Option<String>,
    dram_memory: Option<bool>,
    dram_memory_profile: Option<String>,
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
    fabric_link: Option<String>,
    fabric_bandwidth_bytes_per_tick: Option<u64>,
    fabric_request_virtual_network: Option<u16>,
    fabric_response_virtual_network: Option<u16>,
    fabric_credit_depth: Option<u32>,
    external_adapter_kind: Option<String>,
    external_adapter_endpoint: Option<String>,
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
        let mut start_address = file_config.start_address;
        let mut riscv_boot_a0 = file_config.riscv_boot_a0.unwrap_or(0);
        let mut riscv_boot_a1 = file_config.riscv_boot_a1.unwrap_or(0);
        let mut riscv_sbi = file_config.riscv_sbi.unwrap_or(false);
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
        let mut fabric_link = file_config.fabric_link.clone();
        let mut fabric_bandwidth_bytes_per_tick = file_config.fabric_bandwidth_bytes_per_tick;
        if fabric_bandwidth_bytes_per_tick == Some(0) {
            return Err(Rem6CliError::InvalidRunFabricBandwidth {
                value: "0".to_string(),
            });
        }
        let mut fabric_request_virtual_network = file_config.fabric_request_virtual_network;
        let mut fabric_response_virtual_network = file_config.fabric_response_virtual_network;
        let mut fabric_credit_depth = file_config.fabric_credit_depth;
        if fabric_credit_depth == Some(0) {
            return Err(Rem6CliError::InvalidRunFabricCreditDepth {
                value: "0".to_string(),
            });
        }
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
                    fabric_link = Some(required_value(&flag, args.next())?);
                }
                "--fabric-bandwidth-bytes-per-tick" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_bandwidth_bytes_per_tick =
                        Some(parse_positive_u64(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunFabricBandwidth {
                                value: value.clone(),
                            }
                        })?);
                }
                "--fabric-request-virtual-network" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_request_virtual_network =
                        Some(parse_run_fabric_virtual_network(&value)?);
                }
                "--fabric-response-virtual-network" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_response_virtual_network =
                        Some(parse_run_fabric_virtual_network(&value)?);
                }
                "--fabric-credit-depth" => {
                    let value = required_value(&flag, args.next())?;
                    fabric_credit_depth = Some(parse_run_fabric_credit_depth(&value)?);
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
        let explicit_enabled_memory_hierarchy = dram_memory
            || dram_memory_profile_was_set
            || data_cache_protocol.is_some()
            || data_cache_l2_protocol.is_some()
            || data_cache_l3_protocol.is_some()
            || data_cache_prefetcher.is_some()
            || instruction_cache_protocol.is_some()
            || instruction_cache_l2_protocol.is_some()
            || instruction_cache_l3_protocol.is_some()
            || instruction_cache_prefetcher.is_some()
            || fabric_link.is_some()
            || fabric_bandwidth_bytes_per_tick.is_some()
            || fabric_request_virtual_network.is_some()
            || fabric_response_virtual_network.is_some()
            || fabric_credit_depth.is_some();
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
                &mut fabric_link,
                &mut fabric_bandwidth_bytes_per_tick,
                &mut fabric_request_virtual_network,
                &mut fabric_response_virtual_network,
                &mut fabric_credit_depth,
            )?;
        }
        if dram_memory_profile_was_set && !dram_memory {
            return Err(Rem6CliError::DramMemoryProfileRequiresDramMemory);
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
        let fabric = run_fabric_config_from_parts(
            fabric_link,
            fabric_bandwidth_bytes_per_tick,
            fabric_request_virtual_network,
            fabric_response_virtual_network,
            fabric_credit_depth,
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
            start_address,
            riscv_boot_a0,
            riscv_boot_a1,
            riscv_sbi,
            riscv_se,
            riscv_se_args,
            riscv_se_env,
            riscv_se_stdin,
            riscv_se_files,
            riscv_pc_count_targets,
            riscv_branch_lookahead,
            riscv_branch_predictor,
            max_instructions,
            stats_format,
            execute,
            checker_cpu,
            memory_system,
            dram_memory,
            dram_memory_profile,
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

    pub const fn isa(&self) -> RequestedIsa {
        self.isa
    }

    pub fn binary(&self) -> &Path {
        &self.binary
    }

    pub fn resource_config(&self) -> Option<&Path> {
        self.resource_config.as_deref()
    }

    pub fn kernel_resource(&self) -> Option<&KernelResourceSelector> {
        self.kernel_resource.as_ref()
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

    pub const fn host_event_delay(&self) -> u64 {
        self.host_event_delay
    }

    pub const fn start_address(&self) -> Option<u64> {
        self.start_address
    }

    pub const fn riscv_boot_a0(&self) -> u64 {
        self.riscv_boot_a0
    }

    pub const fn riscv_boot_a1(&self) -> u64 {
        self.riscv_boot_a1
    }

    pub const fn riscv_sbi(&self) -> bool {
        self.riscv_sbi
    }

    pub const fn riscv_se(&self) -> bool {
        self.riscv_se
    }

    pub fn riscv_se_args(&self) -> &[String] {
        &self.riscv_se_args
    }

    pub fn riscv_se_env(&self) -> &[String] {
        &self.riscv_se_env
    }

    pub fn riscv_se_stdin(&self) -> Option<&RiscvSeInputSource> {
        self.riscv_se_stdin.as_ref()
    }

    pub fn riscv_se_files(&self) -> &[RiscvSeFileRequest] {
        &self.riscv_se_files
    }

    pub fn riscv_pc_count_targets(&self) -> &[PcCountPair] {
        &self.riscv_pc_count_targets
    }

    pub const fn riscv_branch_lookahead(&self) -> usize {
        self.riscv_branch_lookahead
    }

    pub const fn riscv_branch_predictor(&self) -> RiscvBranchPredictorKind {
        self.riscv_branch_predictor
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

    pub const fn checker_cpu(&self) -> bool {
        self.checker_cpu
    }

    pub const fn memory_system(&self) -> Option<RunMemorySystem> {
        self.memory_system
    }

    pub const fn dram_memory(&self) -> bool {
        self.dram_memory
    }

    pub const fn dram_memory_profile(&self) -> CliDramMemoryProfile {
        self.dram_memory_profile
    }

    pub const fn data_cache_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.data_cache_protocol
    }

    pub const fn data_cache_l2_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.data_cache_l2_protocol
    }

    pub const fn data_cache_l3_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.data_cache_l3_protocol
    }

    pub const fn data_cache_prefetcher(&self) -> Option<CliCachePrefetcher> {
        self.data_cache_prefetcher
    }

    pub const fn instruction_cache_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.instruction_cache_protocol
    }

    pub const fn instruction_cache_l2_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.instruction_cache_l2_protocol
    }

    pub const fn instruction_cache_l3_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.instruction_cache_l3_protocol
    }

    pub const fn instruction_cache_prefetcher(&self) -> Option<CliCachePrefetcher> {
        self.instruction_cache_prefetcher
    }

    pub fn fabric(&self) -> Option<&RunFabricConfig> {
        self.fabric.as_ref()
    }

    pub fn gdb_listen(&self) -> Option<&str> {
        self.gdb_listen.as_deref()
    }

    pub fn debug_flags(&self) -> &[CliDebugFlag] {
        &self.debug_flags
    }

    pub fn debug_exec_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Exec)
    }

    pub fn debug_fetch_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Fetch)
    }

    pub fn debug_data_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Data)
    }

    pub fn debug_memory_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Memory)
    }

    pub fn debug_power_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Power)
    }

    pub fn debug_syscall_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Syscall)
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

    pub fn load_blobs(&self) -> &[LoadBlobRequest] {
        &self.load_blobs
    }

    pub fn readfiles(&self) -> &[ReadfileRequest] {
        &self.readfiles
    }

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
    }

    pub const fn power_format(&self) -> PowerAnalysisFormat {
        self.power_format
    }

    pub fn power_output(&self) -> Option<&Path> {
        self.power_output.as_deref()
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
