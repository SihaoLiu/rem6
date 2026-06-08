use std::path::{Path, PathBuf};

use rem6_boot::BootElfArchitecture;

use crate::Rem6CliError;

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
pub enum CliDramMemoryProfile {
    Ddr,
    Hbm,
    Lpddr,
    Nvm,
}

impl CliDramMemoryProfile {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        match value {
            "ddr" => Ok(Self::Ddr),
            "hbm" => Ok(Self::Hbm),
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
            Self::Hbm => "hbm",
            Self::Lpddr => "lpddr",
            Self::Nvm => "nvm",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunConfig {
    isa: RequestedIsa,
    binary: PathBuf,
    max_tick: u64,
    min_remote_delay: u64,
    memory_route_delay: u64,
    host_event_delay: u64,
    start_address: Option<u64>,
    riscv_boot_a0: u64,
    riscv_boot_a1: u64,
    max_instructions: Option<u64>,
    stats_format: StatsFormat,
    execute: bool,
    dram_memory: bool,
    dram_memory_profile: CliDramMemoryProfile,
    cores: usize,
    parallel_workers: usize,
    memory_dumps: Vec<MemoryDumpRequest>,
    load_blobs: Vec<LoadBlobRequest>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
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
    stats_format: StatsFormat,
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
        let mut memory_route_delay = None;
        let mut host_event_delay = None;
        let mut start_address = None;
        let mut riscv_boot_a0 = 0u64;
        let mut riscv_boot_a1 = 0u64;
        let mut max_instructions = None;
        let mut stats_format = StatsFormat::Json;
        let mut execute = false;
        let mut dram_memory = false;
        let mut dram_memory_profile = CliDramMemoryProfile::Ddr;
        let mut dram_memory_profile_was_set = false;
        let mut cores = 1usize;
        let mut parallel_workers = None;
        let mut memory_dumps = Vec::new();
        let mut load_blobs = Vec::new();
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
                "--load-blob" => {
                    let value = required_value(&flag, args.next())?;
                    load_blobs.push(LoadBlobRequest::parse(&value)?);
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
        if dram_memory_profile_was_set && !dram_memory {
            return Err(Rem6CliError::DramMemoryProfileRequiresDramMemory);
        }

        Ok(Self {
            isa: isa.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--isa" })?,
            binary: binary.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--binary" })?,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            min_remote_delay,
            memory_route_delay,
            host_event_delay,
            start_address,
            riscv_boot_a0,
            riscv_boot_a1,
            max_instructions,
            stats_format,
            execute,
            dram_memory,
            dram_memory_profile,
            cores,
            parallel_workers: parallel_workers.unwrap_or(cores),
            memory_dumps,
            load_blobs,
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

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
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

        let mut memory_start = None;
        let mut memory_size = None;
        let mut updates = None;
        let mut max_tick = None;
        let mut min_remote_delay = 1u64;
        let mut memory_route_delay = None;
        let mut stats_format = StatsFormat::Json;
        let mut rng_state = 0u64;
        let mut memory_dumps = Vec::new();
        let mut output = None;
        let mut stats_output = None;
        while let Some(flag) = args.next() {
            match flag.as_str() {
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

impl Rem6TraceReplayConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "trace-replay" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }

        let mut trace = None;
        let mut route = None;
        let mut memory_start = None;
        let mut memory_size = None;
        let mut max_tick = None;
        let mut min_remote_delay = 1u64;
        let mut memory_route_delay = None;
        let mut tick_frequency = 1_000u64;
        let mut line_bytes = 64u64;
        let mut agent = 0u32;
        let mut control_partition = 2u32;
        let mut stats_format = StatsFormat::Json;
        let mut output = None;
        let mut stats_output = None;
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--trace" => {
                    trace = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--route" => {
                    route = Some(required_value(&flag, args.next())?);
                }
                "--memory-start" => {
                    let value = required_value(&flag, args.next())?;
                    memory_start = Some(parse_number(&value).ok_or_else(|| {
                        Rem6CliError::InvalidTraceReplayMemoryStart {
                            value: value.clone(),
                        }
                    })?);
                }
                "--memory-size" => {
                    let value = required_value(&flag, args.next())?;
                    memory_size = Some(parse_number(&value).filter(|size| *size > 0).ok_or_else(
                        || Rem6CliError::InvalidTraceReplayMemorySize {
                            value: value.clone(),
                        },
                    )?);
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
                "--tick-frequency" => {
                    let value = required_value(&flag, args.next())?;
                    tick_frequency = parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidTraceReplayTickFrequency {
                            value: value.clone(),
                        }
                    })?;
                }
                "--line-bytes" => {
                    let value = required_value(&flag, args.next())?;
                    line_bytes = parse_positive_u64(&value).ok_or_else(|| {
                        Rem6CliError::InvalidTraceReplayLineBytes {
                            value: value.clone(),
                        }
                    })?;
                }
                "--agent" => {
                    let value = required_value(&flag, args.next())?;
                    agent = value
                        .parse()
                        .map_err(|_| Rem6CliError::InvalidTraceReplayAgent {
                            value: value.clone(),
                        })?;
                }
                "--control-partition" => {
                    let value = required_value(&flag, args.next())?;
                    control_partition = value.parse().map_err(|_| {
                        Rem6CliError::InvalidTraceReplayControlPartition {
                            value: value.clone(),
                        }
                    })?;
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
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
            trace: trace.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--trace" })?,
            route: route.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--route" })?,
            memory_start: memory_start.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-start",
            })?,
            memory_size: memory_size.ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "--memory-size",
            })?,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            min_remote_delay,
            memory_route_delay,
            tick_frequency,
            line_bytes,
            agent,
            control_partition,
            stats_format,
            output,
            stats_output,
        })
    }

    pub fn trace(&self) -> &Path {
        &self.trace
    }

    pub fn route(&self) -> &str {
        &self.route
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

    pub const fn tick_frequency(&self) -> u64 {
        self.tick_frequency
    }

    pub const fn line_bytes(&self) -> u64 {
        self.line_bytes
    }

    pub const fn agent(&self) -> u32 {
        self.agent
    }

    pub const fn control_partition(&self) -> u32 {
        self.control_partition
    }

    pub const fn stats_format(&self) -> StatsFormat {
        self.stats_format
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
    path: PathBuf,
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
        Ok(Self {
            address,
            path: PathBuf::from(path),
        })
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub fn path(&self) -> &Path {
        &self.path
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

fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
}
