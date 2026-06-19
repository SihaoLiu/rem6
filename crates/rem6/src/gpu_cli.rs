use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use rem6_gpu::{
    GpuCoalescedMemoryAccess, GpuComputeConfig, GpuDevice, GpuDeviceId, GpuIsaInstruction,
    GpuIsaProgram, GpuKernelId, GpuKernelLaunch, GpuMemoryAccessKind, GpuWorkgroupCompletion,
};
use rem6_kernel::{PartitionId, PartitionedScheduler};
use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_system::RiscvDataCacheProtocol;
use rem6_transport::{MemoryRoute, MemoryTrace, MemoryTransport, ParallelMemoryTransaction};
use serde::Deserialize;

use crate::cli_output;
use crate::config::{CliDramMemoryProfile, MemoryDumpRequest, PowerAnalysisFormat, StatsFormat};
use crate::data_cache_runtime::{
    cli_cache_runtime_with_prefetcher, cli_data_memory_response, CliDataCacheSummary,
};
use crate::power_output::{gpu_run_power_analysis_artifact, Rem6PowerAnalysisArtifact};
use crate::runtime_memory::{read_memory_dumps, CliMemoryRuntime};
use crate::stats_output::{gpu_run_stats_output, Rem6GpuRunStatsInputs};
use crate::transport_summary::{memory_transport_summary, Rem6MemoryTransportSummary};
use crate::{
    execute_error, transport_endpoint, Rem6CliError, Rem6DramSummary, Rem6MemoryDump,
    DEFAULT_CACHE_LINE_BYTES,
};

const GPU_RUN_CPU_PARTITION: PartitionId = PartitionId::new(0);
const GPU_RUN_GPU_PARTITION: PartitionId = PartitionId::new(1);
const GPU_RUN_MEMORY_PARTITION: PartitionId = PartitionId::new(2);
const GPU_STORE_FILL_BYTE: u8 = 0xa5;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6GpuRunConfig {
    workgroups: u32,
    compute_units: u32,
    wave_slots_per_compute_unit: u32,
    workgroup_cycles: u64,
    memory_start: u64,
    memory_size: u64,
    max_tick: u64,
    min_remote_delay: u64,
    memory_route_delay: u64,
    stats_format: StatsFormat,
    power_format: PowerAnalysisFormat,
    power_output: Option<PathBuf>,
    dram_memory: bool,
    dram_memory_profile: CliDramMemoryProfile,
    data_cache_protocol: Option<RiscvDataCacheProtocol>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    memory_dumps: Vec<MemoryDumpRequest>,
    global_accesses: Vec<GpuGlobalAccessSpec>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6GpuRunRootFileConfig {
    gpu_run: Option<Rem6GpuRunFileConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6GpuRunFileConfig {
    workgroups: Option<u32>,
    compute_units: Option<u32>,
    wave_slots_per_compute_unit: Option<u32>,
    workgroup_cycles: Option<u64>,
    memory_start: Option<u64>,
    memory_size: Option<u64>,
    max_tick: Option<u64>,
    min_remote_delay: Option<u64>,
    memory_route_delay: Option<u64>,
    stats_format: Option<String>,
    power_format: Option<String>,
    power_output: Option<PathBuf>,
    dram_memory: Option<bool>,
    dram_memory_profile: Option<String>,
    data_cache_protocol: Option<String>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    memory_dumps: Option<Vec<String>>,
    global_loads: Option<Vec<String>>,
    global_stores: Option<Vec<String>>,
    #[serde(skip)]
    config_dir: Option<PathBuf>,
}

impl Rem6GpuRunFileConfig {
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

impl Rem6GpuRunConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "gpu-run" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }
        let remaining_args = args.collect::<Vec<_>>();
        let file_config = gpu_run_file_config_from_args(&remaining_args)?
            .map(|path| load_gpu_run_file_config(&path))
            .transpose()?
            .unwrap_or_default();

        let mut workgroups = file_config
            .workgroups
            .map(|value| validate_positive_u32("--workgroups", value))
            .transpose()?;
        let mut compute_units = file_config
            .compute_units
            .map(|value| validate_positive_u32("--compute-units", value))
            .transpose()?
            .unwrap_or(1);
        let mut wave_slots_per_compute_unit = file_config
            .wave_slots_per_compute_unit
            .map(|value| validate_positive_u32("--wave-slots-per-compute-unit", value))
            .transpose()?
            .unwrap_or(1);
        let mut workgroup_cycles = file_config
            .workgroup_cycles
            .map(|value| validate_positive_u64("--workgroup-cycles", value))
            .transpose()?
            .unwrap_or(3);
        let mut memory_start = file_config.memory_start;
        let mut memory_size = file_config
            .memory_size
            .map(|value| validate_positive_u64("--memory-size", value))
            .transpose()?;
        let mut max_tick = file_config
            .max_tick
            .map(|value| validate_positive_u64("--max-tick", value))
            .transpose()?
            .unwrap_or(100);
        let mut min_remote_delay = file_config
            .min_remote_delay
            .map(|value| validate_positive_u64("--min-remote-delay", value))
            .transpose()?
            .unwrap_or(1);
        let mut memory_route_delay = file_config
            .memory_route_delay
            .map(|value| validate_positive_u64("--memory-route-delay", value))
            .transpose()?
            .unwrap_or(1);
        let mut stats_format = file_config
            .stats_format
            .as_deref()
            .map(StatsFormat::parse)
            .transpose()?
            .unwrap_or(StatsFormat::Json);
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
                parse_data_cache_protocol(value).ok_or_else(|| {
                    Rem6CliError::InvalidRunDataCacheProtocol {
                        value: value.to_string(),
                    }
                })
            })
            .transpose()?;
        let mut output = file_config
            .output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut stats_output = file_config
            .stats_output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut memory_dumps = file_config
            .memory_dumps
            .as_deref()
            .unwrap_or(&[])
            .iter()
            .map(|request| MemoryDumpRequest::parse(request))
            .collect::<Result<Vec<_>, _>>()?;
        let mut global_accesses = gpu_global_accesses_from_file_config(&file_config)?;
        let mut memory_dumps_from_cli = false;
        let mut global_loads_from_cli = false;
        let mut global_stores_from_cli = false;

        let mut args = remaining_args.into_iter();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--config" => {
                    let _ = required_value(&flag, args.next())?;
                }
                "--workgroups" => {
                    workgroups = Some(parse_positive_u32(
                        "--workgroups",
                        required_value(&flag, args.next())?,
                    )?);
                }
                "--compute-units" => {
                    compute_units =
                        parse_positive_u32("--compute-units", required_value(&flag, args.next())?)?;
                }
                "--wave-slots-per-compute-unit" => {
                    wave_slots_per_compute_unit = parse_positive_u32(
                        "--wave-slots-per-compute-unit",
                        required_value(&flag, args.next())?,
                    )?;
                }
                "--workgroup-cycles" => {
                    workgroup_cycles = parse_positive_u64(
                        "--workgroup-cycles",
                        required_value(&flag, args.next())?,
                    )?;
                }
                "--memory-start" => {
                    memory_start = Some(parse_u64_value(
                        "--memory-start",
                        required_value(&flag, args.next())?,
                    )?);
                }
                "--memory-size" => {
                    memory_size = Some(parse_positive_u64(
                        "--memory-size",
                        required_value(&flag, args.next())?,
                    )?);
                }
                "--max-tick" => {
                    let value = required_value(&flag, args.next())?;
                    max_tick = parse_positive_u64(&flag, value.clone()).map_err(|_| {
                        Rem6CliError::InvalidMaxTick {
                            value: value.clone(),
                        }
                    })?;
                }
                "--min-remote-delay" => {
                    let value = required_value(&flag, args.next())?;
                    min_remote_delay = parse_positive_u64(&flag, value.clone()).map_err(|_| {
                        Rem6CliError::InvalidMinRemoteDelay {
                            value: value.clone(),
                        }
                    })?;
                }
                "--memory-route-delay" => {
                    let value = required_value(&flag, args.next())?;
                    memory_route_delay =
                        parse_positive_u64(&flag, value.clone()).map_err(|_| {
                            Rem6CliError::InvalidMemoryRouteDelay {
                                value: value.clone(),
                            }
                        })?;
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--power-format" => {
                    power_format =
                        PowerAnalysisFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--power-output" => {
                    power_output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--dram-memory" => {
                    dram_memory = true;
                }
                "--dram-memory-profile" => {
                    dram_memory_profile =
                        CliDramMemoryProfile::parse(&required_value(&flag, args.next())?)?;
                    dram_memory_profile_was_set = true;
                }
                "--data-cache-protocol" => {
                    let value = required_value(&flag, args.next())?;
                    data_cache_protocol =
                        Some(parse_data_cache_protocol(&value).ok_or_else(|| {
                            Rem6CliError::InvalidRunDataCacheProtocol {
                                value: value.clone(),
                            }
                        })?);
                }
                "--global-load" => {
                    if !global_loads_from_cli {
                        global_accesses.retain(|access| access.kind != GpuMemoryAccessKind::Read);
                        global_loads_from_cli = true;
                    }
                    global_accesses.push(GpuGlobalAccessSpec::parse(
                        GpuMemoryAccessKind::Read,
                        &required_value(&flag, args.next())?,
                    )?);
                }
                "--global-store" => {
                    if !global_stores_from_cli {
                        global_accesses.retain(|access| access.kind != GpuMemoryAccessKind::Write);
                        global_stores_from_cli = true;
                    }
                    global_accesses.push(GpuGlobalAccessSpec::parse(
                        GpuMemoryAccessKind::Write,
                        &required_value(&flag, args.next())?,
                    )?);
                }
                "--output" => {
                    output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--stats-output" => {
                    stats_output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--dump-memory" => {
                    if !memory_dumps_from_cli {
                        memory_dumps.clear();
                        memory_dumps_from_cli = true;
                    }
                    memory_dumps.push(MemoryDumpRequest::parse(&required_value(
                        &flag,
                        args.next(),
                    )?)?);
                }
                _ => return Err(Rem6CliError::UnknownFlag { flag }),
            }
        }

        if dram_memory_profile_was_set && !dram_memory {
            return Err(Rem6CliError::DramMemoryProfileRequiresDramMemory);
        }
        if memory_route_delay < min_remote_delay {
            return Err(Rem6CliError::MemoryRouteDelayBelowMinRemoteDelay {
                memory_route_delay,
                min_remote_delay,
            });
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
        if global_accesses.is_empty() {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "--global-load",
            });
        }

        let config = Self {
            workgroups: workgroups.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--workgroups",
            })?,
            compute_units,
            wave_slots_per_compute_unit,
            workgroup_cycles,
            memory_start: memory_start.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-start",
            })?,
            memory_size: memory_size.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-size",
            })?,
            max_tick,
            min_remote_delay,
            memory_route_delay,
            stats_format,
            power_format,
            power_output,
            dram_memory,
            dram_memory_profile,
            data_cache_protocol,
            output,
            stats_output,
            memory_dumps,
            global_accesses,
        };
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<(), Rem6CliError> {
        if !self.memory_start.is_multiple_of(DEFAULT_CACHE_LINE_BYTES) {
            return Err(execute_error(format!(
                "GPU memory start 0x{:x} is not aligned to {} bytes",
                self.memory_start, DEFAULT_CACHE_LINE_BYTES
            )));
        }
        if !self.memory_size.is_multiple_of(DEFAULT_CACHE_LINE_BYTES) {
            return Err(execute_error(format!(
                "GPU memory size {} is not a multiple of {} bytes",
                self.memory_size, DEFAULT_CACHE_LINE_BYTES
            )));
        }
        let memory_end = self.memory_end()?;
        for access in &self.global_accesses {
            access.validate_in_range(self.memory_start, memory_end)?;
        }
        Ok(())
    }

    fn memory_end(&self) -> Result<u64, Rem6CliError> {
        self.memory_start
            .checked_add(self.memory_size)
            .ok_or_else(|| execute_error("GPU memory range overflows u64"))
    }

    pub const fn workgroups(&self) -> u32 {
        self.workgroups
    }

    pub const fn compute_units(&self) -> u32 {
        self.compute_units
    }

    pub const fn wave_slots_per_compute_unit(&self) -> u32 {
        self.wave_slots_per_compute_unit
    }

    pub const fn workgroup_cycles(&self) -> u64 {
        self.workgroup_cycles
    }

    pub const fn memory_start(&self) -> u64 {
        self.memory_start
    }

    pub const fn memory_size(&self) -> u64 {
        self.memory_size
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

    pub const fn power_format(&self) -> PowerAnalysisFormat {
        self.power_format
    }

    pub fn power_output(&self) -> Option<&Path> {
        self.power_output.as_deref()
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

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
    }

    fn global_accesses(&self) -> &[GpuGlobalAccessSpec] {
        &self.global_accesses
    }

    pub fn memory_dumps(&self) -> &[MemoryDumpRequest] {
        &self.memory_dumps
    }
}

fn gpu_run_file_config_from_args(args: &[String]) -> Result<Option<PathBuf>, Rem6CliError> {
    const VALUE_FLAGS: &[&str] = &[
        "--workgroups",
        "--compute-units",
        "--wave-slots-per-compute-unit",
        "--workgroup-cycles",
        "--memory-start",
        "--memory-size",
        "--max-tick",
        "--min-remote-delay",
        "--memory-route-delay",
        "--stats-format",
        "--power-format",
        "--power-output",
        "--dram-memory-profile",
        "--data-cache-protocol",
        "--global-load",
        "--global-store",
        "--output",
        "--stats-output",
        "--dump-memory",
    ];
    const BOOL_FLAGS: &[&str] = &["--dram-memory"];

    let mut path = None;
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--config" => {
                path = Some(PathBuf::from(required_value(
                    flag,
                    args.get(index + 1).cloned(),
                )?));
                index += 2;
            }
            flag if VALUE_FLAGS.contains(&flag) => {
                index += 2;
            }
            flag if BOOL_FLAGS.contains(&flag) => {
                index += 1;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(path)
}

fn load_gpu_run_file_config(path: &Path) -> Result<Rem6GpuRunFileConfig, Rem6CliError> {
    let text = std::fs::read_to_string(path).map_err(|error| Rem6CliError::ReadConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })?;
    let root = toml::from_str::<Rem6GpuRunRootFileConfig>(&text).map_err(|error| {
        Rem6CliError::ParseConfig {
            path: path.to_path_buf(),
            error: error.to_string(),
        }
    })?;
    let mut config = root.gpu_run.unwrap_or_default();
    config.config_dir = path.parent().map(Path::to_path_buf);
    Ok(config)
}

fn gpu_global_accesses_from_file_config(
    config: &Rem6GpuRunFileConfig,
) -> Result<Vec<GpuGlobalAccessSpec>, Rem6CliError> {
    let loads = config
        .global_loads
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|value| GpuGlobalAccessSpec::parse(GpuMemoryAccessKind::Read, value));
    let stores = config
        .global_stores
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|value| GpuGlobalAccessSpec::parse(GpuMemoryAccessKind::Write, value));
    loads.chain(stores).collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GpuGlobalAccessSpec {
    kind: GpuMemoryAccessKind,
    base: u64,
    lane_count: u32,
    lane_stride: u64,
    access_bytes: u64,
}

impl GpuGlobalAccessSpec {
    fn parse(kind: GpuMemoryAccessKind, value: &str) -> Result<Self, Rem6CliError> {
        let fields = value.split(':').collect::<Vec<_>>();
        if fields.len() != 4 {
            return Err(execute_error(format!(
                "GPU global access {value} must be base:lanes:stride:bytes"
            )));
        }
        Ok(Self {
            kind,
            base: parse_u64_value("global access base", fields[0].to_string())?,
            lane_count: parse_positive_u32("global access lanes", fields[1].to_string())?,
            lane_stride: parse_u64_value("global access stride", fields[2].to_string())?,
            access_bytes: parse_positive_u64("global access bytes", fields[3].to_string())?,
        })
    }

    fn validate_in_range(&self, memory_start: u64, memory_end: u64) -> Result<(), Rem6CliError> {
        let last_lane_offset = self
            .lane_stride
            .checked_mul(u64::from(self.lane_count.saturating_sub(1)))
            .ok_or_else(|| execute_error("GPU global access lane offset overflows u64"))?;
        let access_start = self
            .base
            .checked_add(last_lane_offset)
            .ok_or_else(|| execute_error("GPU global access address overflows u64"))?;
        let access_end = access_start
            .checked_add(self.access_bytes)
            .ok_or_else(|| execute_error("GPU global access end overflows u64"))?;
        if self.base < memory_start || access_end > memory_end {
            return Err(execute_error(format!(
                "GPU global access 0x{:x}..0x{:x} is outside mapped memory 0x{:x}..0x{:x}",
                self.base, access_end, memory_start, memory_end
            )));
        }
        Ok(())
    }

    fn instruction(self, line_layout: CacheLineLayout) -> Result<GpuIsaInstruction, Rem6CliError> {
        let size = AccessSize::new(self.access_bytes).map_err(execute_error)?;
        Ok(match self.kind {
            GpuMemoryAccessKind::Read => GpuIsaInstruction::global_load(
                Address::new(self.base),
                self.lane_count,
                self.lane_stride,
                size,
                line_layout,
            ),
            GpuMemoryAccessKind::Write => GpuIsaInstruction::global_store(
                Address::new(self.base),
                self.lane_count,
                self.lane_stride,
                size,
                line_layout,
            ),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6GpuRunArtifact {
    schema: &'static str,
    config: Rem6GpuRunConfig,
    execution: Rem6GpuRunExecutionSummary,
    data_cache: CliDataCacheSummary,
    dram: Rem6DramSummary,
    transport: Rem6MemoryTransportSummary,
    memory_dumps: Vec<Rem6MemoryDump>,
    stats_json: String,
    stats_text: String,
    power_analysis: Option<Rem6PowerAnalysisArtifact>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6GpuRunExecutionSummary {
    final_tick: u64,
    workgroup_completions: u64,
    memory_accesses: u64,
    coalesced_memory_accesses: u64,
    global_memory_requests: u64,
    memory_responses: u64,
    scheduler_epochs: u64,
    scheduler_dispatches: u64,
    memory_scheduler_epochs: u64,
    memory_scheduler_dispatches: u64,
    compute_unit_activity: Vec<Rem6GpuComputeUnitActivity>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Rem6GpuComputeUnitActivity {
    compute_unit: u32,
    workgroup_completions: u64,
    busy_cycles: u64,
    coalesced_memory_accesses: u64,
    global_memory_reads: u64,
    global_memory_writes: u64,
    first_started_at: Option<u64>,
    last_completed_at: Option<u64>,
}

impl Rem6GpuComputeUnitActivity {
    const fn new(compute_unit: u32) -> Self {
        Self {
            compute_unit,
            workgroup_completions: 0,
            busy_cycles: 0,
            coalesced_memory_accesses: 0,
            global_memory_reads: 0,
            global_memory_writes: 0,
            first_started_at: None,
            last_completed_at: None,
        }
    }

    fn record_completion(&mut self, completion: &GpuWorkgroupCompletion) {
        self.workgroup_completions += 1;
        self.first_started_at = Some(
            self.first_started_at
                .map(|tick| tick.min(completion.started_at()))
                .unwrap_or(completion.started_at()),
        );
        self.last_completed_at = Some(
            self.last_completed_at
                .map(|tick| tick.max(completion.completed_at()))
                .unwrap_or(completion.completed_at()),
        );
    }

    fn record_memory_access(&mut self, access: &GpuCoalescedMemoryAccess) {
        self.coalesced_memory_accesses += 1;
        match access.kind() {
            GpuMemoryAccessKind::Read => self.global_memory_reads += 1,
            GpuMemoryAccessKind::Write => self.global_memory_writes += 1,
        }
    }

    pub(crate) const fn compute_unit(&self) -> u32 {
        self.compute_unit
    }

    pub(crate) const fn workgroup_completions(&self) -> u64 {
        self.workgroup_completions
    }

    pub(crate) const fn busy_cycles(&self) -> u64 {
        self.busy_cycles
    }

    pub(crate) const fn coalesced_memory_accesses(&self) -> u64 {
        self.coalesced_memory_accesses
    }

    pub(crate) const fn global_memory_reads(&self) -> u64 {
        self.global_memory_reads
    }

    pub(crate) const fn global_memory_writes(&self) -> u64 {
        self.global_memory_writes
    }

    pub(crate) const fn first_started_at(&self) -> Option<u64> {
        self.first_started_at
    }

    pub(crate) const fn last_completed_at(&self) -> Option<u64> {
        self.last_completed_at
    }

    fn to_json(&self) -> String {
        format!(
            "{{\"compute_unit\":{},\"workgroup_completions\":{},\"busy_cycles\":{},\"coalesced_memory_accesses\":{},\"global_memory_reads\":{},\"global_memory_writes\":{},\"first_started_at\":{},\"last_completed_at\":{}}}",
            self.compute_unit,
            self.workgroup_completions,
            self.busy_cycles,
            self.coalesced_memory_accesses,
            self.global_memory_reads,
            self.global_memory_writes,
            optional_tick_json(self.first_started_at),
            optional_tick_json(self.last_completed_at),
        )
    }
}

impl Rem6GpuRunExecutionSummary {
    pub(crate) const fn final_tick(&self) -> u64 {
        self.final_tick
    }

    pub(crate) const fn workgroup_completions(&self) -> u64 {
        self.workgroup_completions
    }

    pub(crate) const fn memory_accesses(&self) -> u64 {
        self.memory_accesses
    }

    pub(crate) const fn coalesced_memory_accesses(&self) -> u64 {
        self.coalesced_memory_accesses
    }

    pub(crate) const fn global_memory_requests(&self) -> u64 {
        self.global_memory_requests
    }

    pub(crate) const fn memory_responses(&self) -> u64 {
        self.memory_responses
    }

    pub(crate) const fn scheduler_epochs(&self) -> u64 {
        self.scheduler_epochs
    }

    pub(crate) const fn scheduler_dispatches(&self) -> u64 {
        self.scheduler_dispatches
    }

    pub(crate) const fn memory_scheduler_epochs(&self) -> u64 {
        self.memory_scheduler_epochs
    }

    pub(crate) const fn memory_scheduler_dispatches(&self) -> u64 {
        self.memory_scheduler_dispatches
    }

    pub(crate) fn compute_unit_activity(&self) -> &[Rem6GpuComputeUnitActivity] {
        &self.compute_unit_activity
    }
}

pub(crate) fn run_gpu_run_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let config = Rem6GpuRunConfig::parse_args(args)?;
    let artifact = run_gpu_run_config(config)?;
    let stats_format = artifact.config.stats_format();
    let output = match stats_format {
        StatsFormat::Json => artifact.to_json(),
        StatsFormat::Text => artifact.stats_text.clone(),
    };
    let power_artifact =
        artifact
            .power_analysis
            .as_ref()
            .map(|artifact| cli_output::ExtraCliArtifact {
                name: "power_artifact",
                path: artifact.output(),
                contents: artifact.contents(),
            });
    cli_output::emit_cli_output(
        output,
        &artifact.stats_json,
        &artifact.stats_text,
        artifact.config.output(),
        artifact.config.stats_output(),
        stats_format,
        power_artifact,
    )
}

pub fn run_gpu_run_config(config: Rem6GpuRunConfig) -> Result<Rem6GpuRunArtifact, Rem6CliError> {
    let line_layout = CacheLineLayout::new(DEFAULT_CACHE_LINE_BYTES).map_err(execute_error)?;
    let mut scheduler = PartitionedScheduler::with_min_remote_delay(3, config.min_remote_delay())
        .map_err(execute_error)?;
    let gpu = GpuDevice::new(
        GpuComputeConfig::new(
            GpuDeviceId::new(0),
            GPU_RUN_GPU_PARTITION,
            config.compute_units(),
            config.wave_slots_per_compute_unit(),
        )
        .map_err(execute_error)?,
    );
    let program = GpuIsaProgram::new(
        config
            .global_accesses()
            .iter()
            .map(|access| access.instruction(line_layout))
            .collect::<Result<Vec<_>, _>>()?,
    );
    let launch = GpuKernelLaunch::new(
        GpuKernelId::new(0),
        config.workgroups(),
        config.workgroup_cycles(),
    )
    .map_err(execute_error)?
    .with_isa_program(program);

    gpu.submit_kernel_from_partition(
        &mut scheduler,
        GPU_RUN_CPU_PARTITION,
        config.min_remote_delay(),
        launch,
    )
    .map_err(execute_error)?;
    let gpu_summary = gpu
        .run_until_idle_parallel_recorded(&mut scheduler)
        .map_err(execute_error)?;

    let memory = CliMemoryRuntime::new_mapped_zeroed(
        Address::new(config.memory_start()),
        config.memory_size(),
        line_layout,
        config.dram_memory(),
        config.dram_memory_profile(),
    )?;
    let data_cache = cli_cache_runtime_with_prefetcher(
        config.data_cache_protocol(),
        line_layout,
        config.compute_units(),
        None,
    )?;
    let mut transport = MemoryTransport::new();
    let memory_route = transport
        .add_route(
            MemoryRoute::new(
                transport_endpoint("gpu.global".to_string())?,
                GPU_RUN_GPU_PARTITION,
                transport_endpoint("memory".to_string())?,
                GPU_RUN_MEMORY_PARTITION,
                config.memory_route_delay(),
                config.memory_route_delay(),
            )
            .map_err(execute_error)?,
        )
        .map_err(execute_error)?;
    let trace = MemoryTrace::new();
    let memory_responses = Arc::new(Mutex::new(Vec::new()));
    let snapshot = gpu.snapshot();
    let accesses = snapshot.coalesced_memory_accesses().to_vec();
    let compute_unit_activity =
        gpu_compute_unit_activity(config.compute_units(), snapshot.completions(), &accesses)?;
    let memory_end = config.memory_end()?;
    let transactions = accesses
        .iter()
        .enumerate()
        .map(|(index, access)| {
            if access.line().get() < config.memory_start()
                || access.line().get().saturating_add(line_layout.bytes()) > memory_end
            {
                return Err(execute_error(format!(
                    "GPU coalesced line 0x{:x} is outside mapped memory",
                    access.line().get()
                )));
            }
            let request = gpu_memory_request(
                access.compute_unit(),
                index as u64,
                access.kind(),
                access.line(),
                line_layout,
            )?;
            let request_memory = memory.clone();
            let request_data_cache = data_cache.clone();
            let response_records = Arc::clone(&memory_responses);
            Ok(ParallelMemoryTransaction::new(
                memory_route,
                request,
                trace.clone(),
                move |delivery, _context| {
                    cli_data_memory_response(
                        request_data_cache.as_ref(),
                        &request_memory,
                        &delivery,
                    )
                },
                move |delivery| {
                    response_records
                        .lock()
                        .expect("GPU memory response record lock")
                        .push(delivery.response().status());
                },
            ))
        })
        .collect::<Result<Vec<_>, Rem6CliError>>()?;
    if !transactions.is_empty() {
        transport
            .submit_parallel_batch(&mut scheduler, transactions)
            .map_err(execute_error)?;
    }
    let memory_scheduler_run = scheduler
        .run_until_idle_parallel_recorded()
        .map_err(execute_error)?;
    if let Some(data_cache) = data_cache.as_ref() {
        if let Some(error) = data_cache.take_error() {
            return Err(error);
        }
    }
    let memory_responses = memory_responses
        .lock()
        .expect("GPU memory response record lock")
        .clone();
    if let Some(status) = memory_responses
        .iter()
        .copied()
        .find(|status| *status != ResponseStatus::Completed)
    {
        return Err(execute_error(format!(
            "GPU memory request completed with {status:?}"
        )));
    }

    let final_tick = gpu_summary
        .final_tick()
        .max(memory_scheduler_run.summary().final_tick());
    if final_tick > config.max_tick() {
        return Err(execute_error(format!(
            "GPU final tick {final_tick} exceeded max tick {}",
            config.max_tick()
        )));
    }
    let data_cache_summary = data_cache
        .as_ref()
        .map(|cache| CliDataCacheSummary::from_records(&cache.records()))
        .unwrap_or_default();
    let dram = memory.dram_summary_until(final_tick);
    let transport = memory_transport_summary(&trace);
    let memory_dumps = read_memory_dumps(&memory, line_layout, config.memory_dumps())?;
    let execution = Rem6GpuRunExecutionSummary {
        final_tick,
        workgroup_completions: gpu_summary.workgroup_completion_count() as u64,
        memory_accesses: gpu_summary.memory_access_count() as u64,
        coalesced_memory_accesses: gpu_summary.coalesced_memory_access_count() as u64,
        global_memory_requests: accesses.len() as u64,
        memory_responses: memory_responses.len() as u64,
        scheduler_epochs: gpu_summary.epoch_count() as u64,
        scheduler_dispatches: gpu_summary.dispatch_count() as u64,
        memory_scheduler_epochs: memory_scheduler_run.epoch_count() as u64,
        memory_scheduler_dispatches: memory_scheduler_run.dispatch_count() as u64,
        compute_unit_activity,
    };
    let stats = gpu_run_stats_output(Rem6GpuRunStatsInputs {
        config: &config,
        execution: &execution,
        data_cache: &data_cache_summary,
        dram: &dram,
        transport: &transport,
        memory_dumps: &memory_dumps,
    })?;
    let power_analysis = config
        .power_output()
        .map(|path| {
            gpu_run_power_analysis_artifact(
                config.power_format(),
                path.to_path_buf(),
                &execution,
                &data_cache_summary,
                &dram,
            )
        })
        .transpose()?;

    Ok(Rem6GpuRunArtifact {
        schema: "rem6.cli.gpu-run.v1",
        config,
        execution,
        data_cache: data_cache_summary,
        dram,
        transport,
        memory_dumps,
        stats_json: stats.json,
        stats_text: stats.text,
        power_analysis,
    })
}

impl Rem6GpuRunArtifact {
    pub fn to_json(&self) -> String {
        let data_cache_protocol = self
            .config
            .data_cache_protocol()
            .map(|protocol| format!("\"{}\"", data_cache_protocol_name(protocol)))
            .unwrap_or_else(|| "null".to_string());
        let memory_dumps = self
            .memory_dumps
            .iter()
            .map(Rem6MemoryDump::to_json)
            .collect::<Vec<_>>()
            .join(",");
        let power_analysis = self
            .power_analysis
            .as_ref()
            .map(Rem6PowerAnalysisArtifact::to_json)
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"schema\":\"{}\",\"status\":\"completed\",\"workgroups\":{},\"compute_units\":{},\"wave_slots_per_compute_unit\":{},\"workgroup_cycles\":{},\"memory_start\":\"0x{:x}\",\"memory_size\":{},\"dram_memory\":{},\"data_cache_protocol\":{},\"simulation\":{},\"data_cache\":{},\"dram\":{},\"transport\":{},\"memory\":[{}],\"power_analysis\":{},\"stats\":{}}}\n",
            self.schema,
            self.config.workgroups(),
            self.config.compute_units(),
            self.config.wave_slots_per_compute_unit(),
            self.config.workgroup_cycles(),
            self.config.memory_start(),
            self.config.memory_size(),
            self.config.dram_memory(),
            data_cache_protocol,
            self.execution.to_json(),
            data_cache_summary_json(&self.data_cache),
            self.dram.to_json(),
            self.transport.to_json(),
            memory_dumps,
            power_analysis,
            self.stats_json,
        )
    }
}

impl Rem6GpuRunExecutionSummary {
    fn to_json(&self) -> String {
        format!(
            "{{\"status\":\"completed\",\"final_tick\":{},\"workgroup_completions\":{},\"memory_accesses\":{},\"coalesced_memory_accesses\":{},\"global_memory_requests\":{},\"memory_responses\":{},\"scheduler_epochs\":{},\"scheduler_dispatches\":{},\"memory_scheduler_epochs\":{},\"memory_scheduler_dispatches\":{},\"compute_unit_activity\":[{}]}}",
            self.final_tick,
            self.workgroup_completions,
            self.memory_accesses,
            self.coalesced_memory_accesses,
            self.global_memory_requests,
            self.memory_responses,
            self.scheduler_epochs,
            self.scheduler_dispatches,
            self.memory_scheduler_epochs,
            self.memory_scheduler_dispatches,
            self.compute_unit_activity
                .iter()
                .map(Rem6GpuComputeUnitActivity::to_json)
                .collect::<Vec<_>>()
                .join(","),
        )
    }
}

fn gpu_compute_unit_activity(
    compute_units: u32,
    completions: &[GpuWorkgroupCompletion],
    accesses: &[GpuCoalescedMemoryAccess],
) -> Result<Vec<Rem6GpuComputeUnitActivity>, Rem6CliError> {
    let mut activity = (0..compute_units)
        .map(Rem6GpuComputeUnitActivity::new)
        .collect::<Vec<_>>();
    let mut active_intervals = vec![Vec::new(); activity.len()];
    for completion in completions {
        let compute_unit = usize::try_from(completion.compute_unit())
            .map_err(|_| execute_error("GPU compute unit index does not fit usize"))?;
        let Some(activity) = activity.get_mut(compute_unit) else {
            return Err(execute_error(format!(
                "GPU completion used compute unit {} outside configured count {}",
                completion.compute_unit(),
                compute_units
            )));
        };
        if completion.completed_at() < completion.started_at() {
            return Err(execute_error("GPU completion ended before it started"));
        }
        activity.record_completion(completion);
        active_intervals[compute_unit].push((completion.started_at(), completion.completed_at()));
    }
    for access in accesses {
        let compute_unit = usize::try_from(access.compute_unit()).map_err(|_| {
            execute_error("GPU memory access compute unit index does not fit usize")
        })?;
        let Some(activity) = activity.get_mut(compute_unit) else {
            return Err(execute_error(format!(
                "GPU memory access used compute unit {} outside configured count {}",
                access.compute_unit(),
                compute_units
            )));
        };
        activity.record_memory_access(access);
    }
    for (activity, intervals) in activity.iter_mut().zip(active_intervals) {
        activity.busy_cycles = merged_interval_cycles(intervals);
    }
    Ok(activity)
}

fn merged_interval_cycles(mut intervals: Vec<(u64, u64)>) -> u64 {
    intervals.sort_unstable_by_key(|(start, end)| (*start, *end));
    let mut merged_cycles = 0;
    let mut current: Option<(u64, u64)> = None;
    for (start, end) in intervals {
        match current {
            Some((current_start, current_end)) if start <= current_end => {
                current = Some((current_start, current_end.max(end)));
            }
            Some((current_start, current_end)) => {
                merged_cycles += current_end - current_start;
                current = Some((start, end));
            }
            None => current = Some((start, end)),
        }
    }
    if let Some((start, end)) = current {
        merged_cycles += end - start;
    }
    merged_cycles
}

fn optional_tick_json(tick: Option<u64>) -> String {
    tick.map(|tick| tick.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn gpu_memory_request(
    agent: u32,
    sequence: u64,
    kind: GpuMemoryAccessKind,
    line: Address,
    line_layout: CacheLineLayout,
) -> Result<MemoryRequest, Rem6CliError> {
    let size = AccessSize::new(line_layout.bytes()).map_err(execute_error)?;
    let request = MemoryRequestId::new(AgentId::new(agent), sequence);
    match kind {
        GpuMemoryAccessKind::Read => {
            MemoryRequest::read_shared(request, line, size, line_layout).map_err(execute_error)
        }
        GpuMemoryAccessKind::Write => MemoryRequest::write(
            request,
            line,
            size,
            vec![GPU_STORE_FILL_BYTE; line_layout.bytes() as usize],
            ByteMask::full(size).map_err(execute_error)?,
            line_layout,
        )
        .map_err(execute_error),
    }
}

fn data_cache_summary_json(summary: &CliDataCacheSummary) -> String {
    format!(
        "{{\"data_cache_runs\":{},\"data_cache_msi_runs\":{},\"data_cache_mesi_runs\":{},\"data_cache_moesi_runs\":{},\"data_cache_chi_runs\":{},\"data_cache_cpu_responses\":{},\"data_cache_directory_decisions\":{},\"data_cache_dram_accesses\":{}}}",
        summary.runs,
        summary.msi_runs,
        summary.mesi_runs,
        summary.moesi_runs,
        summary.chi_runs,
        summary.cpu_responses,
        summary.directory_decisions,
        summary.dram_accesses,
    )
}

fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
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

fn parse_positive_u64(name: &str, value: String) -> Result<u64, Rem6CliError> {
    let parsed = parse_u64_value(name, value.clone())?;
    if parsed == 0 {
        return Err(execute_error(format!(
            "{name} must be positive, got {value}"
        )));
    }
    Ok(parsed)
}

fn validate_positive_u64(name: &str, value: u64) -> Result<u64, Rem6CliError> {
    parse_positive_u64(name, value.to_string())
}

fn validate_positive_u32(name: &str, value: u32) -> Result<u32, Rem6CliError> {
    parse_positive_u32(name, value.to_string())
}

fn parse_positive_u32(name: &str, value: String) -> Result<u32, Rem6CliError> {
    let parsed = parse_positive_u64(name, value.clone())?;
    u32::try_from(parsed).map_err(|_| execute_error(format!("{name} is too large: {value}")))
}

fn parse_data_cache_protocol(value: &str) -> Option<RiscvDataCacheProtocol> {
    match value {
        "msi" => Some(RiscvDataCacheProtocol::Msi),
        "mesi" => Some(RiscvDataCacheProtocol::Mesi),
        "moesi" => Some(RiscvDataCacheProtocol::Moesi),
        "chi" => Some(RiscvDataCacheProtocol::Chi),
        _ => None,
    }
}

fn data_cache_protocol_name(protocol: RiscvDataCacheProtocol) -> &'static str {
    match protocol {
        RiscvDataCacheProtocol::Msi => "msi",
        RiscvDataCacheProtocol::Mesi => "mesi",
        RiscvDataCacheProtocol::Moesi => "moesi",
        RiscvDataCacheProtocol::Chi => "chi",
    }
}
