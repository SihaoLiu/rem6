use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfOperatingSystem, BootImage,
};
use rem6_stats::{StatResetPolicy, StatsRegistry};

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
                _ => return Err(Rem6CliError::UnknownFlag { flag }),
            }
        }

        Ok(Self {
            isa: isa.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--isa" })?,
            binary: binary.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--binary" })?,
            max_tick: max_tick.ok_or(Rem6CliError::MissingRequiredFlag { flag: "--max-tick" })?,
            stats_format,
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6RunArtifact {
    schema: &'static str,
    config: Rem6RunConfig,
    binary_bytes: u64,
    entry: u64,
    metadata: rem6_boot::BootElfMetadata,
    load_segments: u64,
    stats_json: String,
}

impl Rem6RunArtifact {
    pub fn to_json(&self) -> String {
        format!(
            "{{\"schema\":\"{}\",\"isa\":\"{}\",\"binary\":\"{}\",\"entry\":\"0x{:x}\",\"elf\":{{\"class\":\"{}\",\"endian\":\"{}\",\"architecture\":\"{}\",\"os\":\"{}\",\"machine\":{},\"flags\":{}}},\"simulation\":{{\"status\":\"loaded\",\"max_tick\":{},\"executed_ticks\":0}},\"stats\":{}}}\n",
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
            self.config.max_tick(),
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
    Stats {
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
            Self::Stats { error } => write!(formatter, "failed to build run stats: {error}"),
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
    match artifact.config.stats_format() {
        StatsFormat::Json => Ok(artifact.to_json()),
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

    let stats_json = run_stats_json(
        bytes.len() as u64,
        image.segments().len() as u64,
        config.max_tick(),
    )?;
    Ok(Rem6RunArtifact {
        schema: "rem6.cli.run.v1",
        binary_bytes: bytes.len() as u64,
        entry: image.entry().get(),
        load_segments: image.segments().len() as u64,
        metadata,
        config,
        stats_json,
    })
}

fn run_stats_json(
    binary_bytes: u64,
    load_segments: u64,
    max_tick: u64,
) -> Result<String, Rem6CliError> {
    let mut stats = StatsRegistry::new();
    let binary_bytes_stat = stats
        .register_counter_with_reset_policy("sim.binary.bytes", "Byte", StatResetPolicy::Constant)
        .map_err(stats_error)?;
    let load_segments_stat = stats
        .register_counter_with_reset_policy(
            "sim.elf.load_segments",
            "Count",
            StatResetPolicy::Constant,
        )
        .map_err(stats_error)?;
    let max_tick_stat = stats
        .register_counter_with_reset_policy("sim.max_tick", "Tick", StatResetPolicy::Constant)
        .map_err(stats_error)?;

    stats
        .increment(binary_bytes_stat, binary_bytes)
        .map_err(stats_error)?;
    stats
        .increment(load_segments_stat, load_segments)
        .map_err(stats_error)?;
    stats
        .increment(max_tick_stat, max_tick)
        .map_err(stats_error)?;

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
