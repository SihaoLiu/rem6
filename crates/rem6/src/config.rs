use std::path::{Path, PathBuf};

use rem6_boot::BootElfArchitecture;
use rem6_system::RiscvDataCacheProtocol;
use rem6_workload::WorkloadDataCacheProtocol;
use serde::Deserialize;

use crate::Rem6CliError;

mod trace_replay;

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

    pub(super) const fn matches_architecture(self, architecture: BootElfArchitecture) -> bool {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PowerAnalysisFormat {
    McpatXml,
    DsentCsv,
}

impl PowerAnalysisFormat {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "mcpat-xml" => Ok(Self::McpatXml),
            "dsent-csv" => Ok(Self::DsentCsv),
            _ => Err(Rem6CliError::UnsupportedPowerAnalysisFormat {
                format: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::McpatXml => "mcpat-xml",
            Self::DsentCsv => "dsent-csv",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliDramMemoryProfile {
    Ddr,
    Ddr4_2400_8Gb,
    Ddr5_4800_16Gb,
    Hbm,
    Hbm2_2000_2Gb,
    Lpddr,
    Nvm,
}

impl CliDramMemoryProfile {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "ddr" => Ok(Self::Ddr),
            "ddr4-2400-8gb" => Ok(Self::Ddr4_2400_8Gb),
            "ddr5-4800-16gb" => Ok(Self::Ddr5_4800_16Gb),
            "hbm" => Ok(Self::Hbm),
            "hbm2-2000-2gb" => Ok(Self::Hbm2_2000_2Gb),
            "lpddr" => Ok(Self::Lpddr),
            "nvm" => Ok(Self::Nvm),
            _ => Err(Rem6CliError::UnsupportedDramMemoryProfile {
                profile: value.to_string(),
            }),
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ddr => "ddr",
            Self::Ddr4_2400_8Gb => "ddr4-2400-8gb",
            Self::Ddr5_4800_16Gb => "ddr5-4800-16gb",
            Self::Hbm => "hbm",
            Self::Hbm2_2000_2Gb => "hbm2-2000-2gb",
            Self::Lpddr => "lpddr",
            Self::Nvm => "nvm",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CliCachePrefetcher {
    TaggedNextLine,
}

impl CliCachePrefetcher {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "tagged-next-line" => Some(Self::TaggedNextLine),
            _ => None,
        }
    }

    pub fn parse_data_cache(value: &str) -> Result<Self, Rem6CliError> {
        Self::parse(value).ok_or_else(|| Rem6CliError::InvalidRunDataCachePrefetcher {
            value: value.to_string(),
        })
    }

    pub fn parse_instruction_cache(value: &str) -> Result<Self, Rem6CliError> {
        Self::parse(value).ok_or_else(|| Rem6CliError::InvalidRunInstructionCachePrefetcher {
            value: value.to_string(),
        })
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TaggedNextLine => "tagged-next-line",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunConfig {
    isa: RequestedIsa,
    binary: PathBuf,
    resource_config: Option<PathBuf>,
    max_tick: u64,
    min_remote_delay: u64,
    memory_route_delay: u64,
    host_event_delay: u64,
    start_address: Option<u64>,
    riscv_boot_a0: u64,
    riscv_boot_a1: u64,
    riscv_se: bool,
    riscv_se_args: Vec<String>,
    riscv_se_env: Vec<String>,
    riscv_se_stdin: Option<PathBuf>,
    riscv_se_files: Vec<RiscvSeFileRequest>,
    max_instructions: Option<u64>,
    stats_format: StatsFormat,
    execute: bool,
    dram_memory: bool,
    dram_memory_profile: CliDramMemoryProfile,
    data_cache_protocol: Option<RiscvDataCacheProtocol>,
    data_cache_prefetcher: Option<CliCachePrefetcher>,
    instruction_cache_protocol: Option<RiscvDataCacheProtocol>,
    instruction_cache_prefetcher: Option<CliCachePrefetcher>,
    gdb_listen: Option<String>,
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
    fabric_link: Option<String>,
    fabric_bandwidth_bytes_per_tick: Option<u64>,
    fabric_request_virtual_network: u16,
    fabric_response_virtual_network: u16,
    fabric_credit_depth: Option<u32>,
    stats_format: StatsFormat,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
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
    max_tick: Option<u64>,
    min_remote_delay: Option<u64>,
    memory_route_delay: Option<u64>,
    host_event_delay: Option<u64>,
    start_address: Option<u64>,
    riscv_boot_a0: Option<u64>,
    riscv_boot_a1: Option<u64>,
    riscv_se: Option<bool>,
    riscv_se_args: Option<Vec<String>>,
    riscv_se_env: Option<Vec<String>>,
    riscv_se_stdin: Option<PathBuf>,
    riscv_se_files: Option<Vec<String>>,
    max_instructions: Option<u64>,
    stats_format: Option<String>,
    execute: Option<bool>,
    dram_memory: Option<bool>,
    dram_memory_profile: Option<String>,
    data_cache_protocol: Option<String>,
    data_cache_prefetcher: Option<String>,
    instruction_cache_protocol: Option<String>,
    instruction_cache_prefetcher: Option<String>,
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
    fabric_link: Option<String>,
    fabric_bandwidth_bytes_per_tick: Option<u64>,
    fabric_request_virtual_network: Option<u16>,
    fabric_response_virtual_network: Option<u16>,
    fabric_credit_depth: Option<u32>,
    stats_format: Option<String>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
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
        let mut riscv_se = file_config.riscv_se.unwrap_or(false);
        let mut riscv_se_args = file_config.riscv_se_args.clone().unwrap_or_default();
        let mut riscv_se_env = file_config.riscv_se_env.clone().unwrap_or_default();
        let mut riscv_se_stdin = file_config
            .riscv_se_stdin
            .as_deref()
            .map(|path| file_config.resolve_path(path));
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
        let mut max_instructions = file_config.max_instructions;
        if max_instructions == Some(0) {
            return Err(Rem6CliError::InvalidMaxInstructions {
                value: "0".to_string(),
            });
        }
        let mut stats_format = file_config
            .stats_format
            .as_deref()
            .map(StatsFormat::parse)
            .transpose()?
            .unwrap_or(StatsFormat::Json);
        let mut execute = file_config.execute.unwrap_or(false);
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
        let mut instruction_cache_prefetcher = file_config
            .instruction_cache_prefetcher
            .as_deref()
            .map(CliCachePrefetcher::parse_instruction_cache)
            .transpose()?;
        let mut gdb_listen = None;
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
                }
                "--resource-config" => {
                    resource_config = Some(PathBuf::from(required_value(&flag, args.next())?));
                    binary = None;
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
                    riscv_se_stdin = Some(PathBuf::from(required_value(&flag, args.next())?));
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
                "--instruction-cache-prefetcher" => {
                    instruction_cache_prefetcher =
                        Some(CliCachePrefetcher::parse_instruction_cache(
                            &required_value(&flag, args.next())?,
                        )?);
                }
                "--gdb-listen" => {
                    gdb_listen = Some(required_value(&flag, args.next())?);
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

        Ok(Self {
            isa: isa.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--isa" })?,
            binary,
            resource_config,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            min_remote_delay,
            memory_route_delay,
            host_event_delay,
            start_address,
            riscv_boot_a0,
            riscv_boot_a1,
            riscv_se,
            riscv_se_args,
            riscv_se_env,
            riscv_se_stdin,
            riscv_se_files,
            max_instructions,
            stats_format,
            execute,
            dram_memory,
            dram_memory_profile,
            data_cache_protocol,
            data_cache_prefetcher,
            instruction_cache_protocol,
            instruction_cache_prefetcher,
            gdb_listen,
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

    pub const fn riscv_se(&self) -> bool {
        self.riscv_se
    }

    pub fn riscv_se_args(&self) -> &[String] {
        &self.riscv_se_args
    }

    pub fn riscv_se_env(&self) -> &[String] {
        &self.riscv_se_env
    }

    pub fn riscv_se_stdin(&self) -> Option<&Path> {
        self.riscv_se_stdin.as_deref()
    }

    pub fn riscv_se_files(&self) -> &[RiscvSeFileRequest] {
        &self.riscv_se_files
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

    pub const fn dram_memory(&self) -> bool {
        self.dram_memory
    }

    pub const fn dram_memory_profile(&self) -> CliDramMemoryProfile {
        self.dram_memory_profile
    }

    pub const fn data_cache_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.data_cache_protocol
    }

    pub const fn data_cache_prefetcher(&self) -> Option<CliCachePrefetcher> {
        self.data_cache_prefetcher
    }

    pub const fn instruction_cache_protocol(&self) -> Option<RiscvDataCacheProtocol> {
        self.instruction_cache_protocol
    }

    pub const fn instruction_cache_prefetcher(&self) -> Option<CliCachePrefetcher> {
        self.instruction_cache_prefetcher
    }

    pub fn gdb_listen(&self) -> Option<&str> {
        self.gdb_listen.as_deref()
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
pub struct LoadBlobRequest {
    address: u64,
    source: LoadBlobSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuiteResourceSelector {
    workload_id: String,
    resource_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadBlobSource {
    Path(PathBuf),
    Resource(String),
    SuiteResource(SuiteResourceSelector),
}

impl SuiteResourceSelector {
    pub fn parse_source(value: &str) -> Option<Self> {
        value.strip_prefix("suite-resource:").and_then(Self::parse)
    }

    fn parse(value: &str) -> Option<Self> {
        let (workload_id, resource_id) = value.split_once('/')?;
        if workload_id.is_empty() || resource_id.is_empty() {
            return None;
        }
        Some(Self {
            workload_id: workload_id.to_string(),
            resource_id: resource_id.to_string(),
        })
    }

    pub fn workload_id(&self) -> &str {
        &self.workload_id
    }

    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }

    pub fn qualified_id(&self) -> String {
        format!("{}/{}", self.workload_id, self.resource_id)
    }

    fn source_name(&self) -> String {
        format!("suite-resource:{}", self.qualified_id())
    }
}

impl LoadBlobRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let Some((address, path)) = value.split_once(':') else {
            return Err(Rem6CliError::InvalidLoadBlob {
                value: value.to_string(),
            });
        };
        let address = parse_number(address).ok_or_else(|| Rem6CliError::InvalidLoadBlob {
            value: value.to_string(),
        })?;
        if path.is_empty() {
            return Err(Rem6CliError::InvalidLoadBlob {
                value: value.to_string(),
            });
        }
        let source = if let Some(selector) = path.strip_prefix("suite-resource:") {
            SuiteResourceSelector::parse(selector)
                .map(LoadBlobSource::SuiteResource)
                .ok_or_else(|| Rem6CliError::InvalidLoadBlob {
                    value: value.to_string(),
                })?
        } else {
            path.strip_prefix("resource:")
                .map(|resource| {
                    if resource.is_empty() {
                        return Err(Rem6CliError::InvalidLoadBlob {
                            value: value.to_string(),
                        });
                    }
                    Ok(LoadBlobSource::Resource(resource.to_string()))
                })
                .unwrap_or_else(|| Ok(LoadBlobSource::Path(PathBuf::from(path))))?
        };
        Ok(Self { address, source })
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn source(&self) -> &LoadBlobSource {
        &self.source
    }

    pub fn source_name(&self) -> String {
        match &self.source {
            LoadBlobSource::Path(path) => path.display().to_string(),
            LoadBlobSource::Resource(resource) => format!("resource:{resource}"),
            LoadBlobSource::SuiteResource(selector) => selector.source_name(),
        }
    }

    fn resolve_path(&mut self, base: &Path) {
        let LoadBlobSource::Path(path) = &mut self.source else {
            return;
        };
        if path.is_relative() {
            *path = base.join(&*path);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadfileRequest {
    base: u64,
    size: u64,
    source: ReadfileSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadfileSource {
    Path(PathBuf),
    Resource(String),
    SuiteResource(SuiteResourceSelector),
}

impl ReadfileRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let mut parts = value.splitn(3, ':');
        let Some(base) = parts.next() else {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        };
        let Some(size) = parts.next() else {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        };
        let Some(path) = parts.next() else {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        };
        let base = parse_number(base).ok_or_else(|| Rem6CliError::InvalidReadfile {
            value: value.to_string(),
        })?;
        let size = parse_number(size)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            })?;
        if path.is_empty() {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        }
        let source = if let Some(selector) = path.strip_prefix("suite-resource:") {
            SuiteResourceSelector::parse(selector)
                .map(ReadfileSource::SuiteResource)
                .ok_or_else(|| Rem6CliError::InvalidReadfile {
                    value: value.to_string(),
                })?
        } else {
            path.strip_prefix("resource:")
                .map(|resource| {
                    if resource.is_empty() {
                        return Err(Rem6CliError::InvalidReadfile {
                            value: value.to_string(),
                        });
                    }
                    Ok(ReadfileSource::Resource(resource.to_string()))
                })
                .unwrap_or_else(|| Ok(ReadfileSource::Path(PathBuf::from(path))))?
        };
        Ok(Self { base, size, source })
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn source(&self) -> &ReadfileSource {
        &self.source
    }

    pub fn source_name(&self) -> String {
        match &self.source {
            ReadfileSource::Path(path) => path.display().to_string(),
            ReadfileSource::Resource(resource) => format!("resource:{resource}"),
            ReadfileSource::SuiteResource(selector) => selector.source_name(),
        }
    }

    fn resolve_path(&mut self, base: &Path) {
        let ReadfileSource::Path(path) = &mut self.source else {
            return;
        };
        if path.is_relative() {
            *path = base.join(&path);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSeFileRequest {
    guest_path: String,
    host_path: PathBuf,
}

impl RiscvSeFileRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let Some((guest_path, host_path)) = value.split_once('=') else {
            return Err(Rem6CliError::InvalidRiscvSeFile {
                value: value.to_string(),
            });
        };
        if guest_path.is_empty() || guest_path.as_bytes().contains(&0) || host_path.is_empty() {
            return Err(Rem6CliError::InvalidRiscvSeFile {
                value: value.to_string(),
            });
        }
        Ok(Self {
            guest_path: guest_path.to_string(),
            host_path: PathBuf::from(host_path),
        })
    }

    pub fn guest_path(&self) -> &str {
        &self.guest_path
    }

    pub fn host_path(&self) -> &Path {
        &self.host_path
    }

    fn resolve_host_path(&mut self, base: &Path) {
        if self.host_path.is_relative() {
            self.host_path = base.join(&self.host_path);
        }
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

fn parse_positive_u64(value: &str) -> Option<u64> {
    value.parse().ok().filter(|value| *value > 0)
}

fn parse_data_cache_protocol(value: &str) -> Option<WorkloadDataCacheProtocol> {
    match value {
        "msi" => Some(WorkloadDataCacheProtocol::Msi),
        "mesi" => Some(WorkloadDataCacheProtocol::Mesi),
        "moesi" => Some(WorkloadDataCacheProtocol::Moesi),
        "chi" => Some(WorkloadDataCacheProtocol::Chi),
        _ => None,
    }
}

fn parse_run_data_cache_protocol(value: &str) -> Option<RiscvDataCacheProtocol> {
    match value {
        "msi" => Some(RiscvDataCacheProtocol::Msi),
        "mesi" => Some(RiscvDataCacheProtocol::Mesi),
        "moesi" => Some(RiscvDataCacheProtocol::Moesi),
        "chi" => Some(RiscvDataCacheProtocol::Chi),
        _ => None,
    }
}

fn run_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        &[
            "--isa",
            "--binary",
            "--resource-config",
            "--max-tick",
            "--min-remote-delay",
            "--memory-route-delay",
            "--host-event-delay",
            "--start-address",
            "--riscv-boot-a0",
            "--riscv-boot-a1",
            "--riscv-se-arg",
            "--riscv-se-env",
            "--riscv-se-stdin",
            "--riscv-se-file",
            "--max-instructions",
            "--stats-format",
            "--dram-memory-profile",
            "--data-cache-protocol",
            "--data-cache-prefetcher",
            "--instruction-cache-protocol",
            "--instruction-cache-prefetcher",
            "--cores",
            "--parallel-workers",
            "--dump-memory",
            "--load-blob",
            "--readfile",
            "--output",
            "--stats-output",
            "--power-format",
            "--power-output",
        ],
        &["--execute", "--dram-memory", "--riscv-se"],
    )
}

fn gups_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        &[
            "--memory-start",
            "--memory-size",
            "--updates",
            "--max-tick",
            "--min-remote-delay",
            "--memory-route-delay",
            "--stats-format",
            "--rng-state",
            "--dump-memory",
            "--output",
            "--stats-output",
        ],
        &[],
    )
}

fn trace_replay_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        &[
            "--trace",
            "--resource-config",
            "--route",
            "--memory-start",
            "--memory-size",
            "--max-tick",
            "--min-remote-delay",
            "--memory-route-delay",
            "--tick-frequency",
            "--line-bytes",
            "--agent",
            "--control-partition",
            "--data-cache-protocol",
            "--fabric-link",
            "--fabric-bandwidth-bytes-per-tick",
            "--fabric-request-virtual-network",
            "--fabric-response-virtual-network",
            "--fabric-credit-depth",
            "--stats-format",
            "--output",
            "--stats-output",
        ],
        &[],
    )
}

fn config_path_from_args(
    args: &[String],
    value_flags: &[&str],
    bool_flags: &[&str],
) -> Result<Option<PathBuf>, Rem6CliError> {
    let mut path = None;
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--config" => {
                let value = args
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| Rem6CliError::MissingFlagValue { flag: flag.clone() })?;
                path = Some(PathBuf::from(value));
                index += 2;
            }
            flag if bool_flags.contains(&flag) => {
                index += 1;
            }
            flag if value_flags.contains(&flag) => {
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(path)
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

fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
}
