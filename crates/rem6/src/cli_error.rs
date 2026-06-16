use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use rem6_boot::BootElfArchitecture;
use rem6_system::RiscvDataCacheProtocol;

use crate::config::RequestedIsa;
use crate::formatting::elf_architecture_name;

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
    ReadConfig {
        path: PathBuf,
        error: String,
    },
    ParseConfig {
        path: PathBuf,
        error: String,
    },
    UnsupportedIsa {
        isa: String,
    },
    UnsupportedStatsFormat {
        format: String,
    },
    UnsupportedPowerAnalysisFormat {
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
    InvalidGupsMemoryStart {
        value: String,
    },
    InvalidGupsMemorySize {
        value: String,
    },
    InvalidGupsUpdates {
        value: String,
    },
    InvalidGupsRngState {
        value: String,
    },
    InvalidTraceReplayMemoryStart {
        value: String,
    },
    InvalidTraceReplayMemorySize {
        value: String,
    },
    InvalidTraceReplayTickFrequency {
        value: String,
    },
    InvalidTraceReplayLineBytes {
        value: String,
    },
    InvalidTraceReplayAgent {
        value: String,
    },
    InvalidTraceReplayControlPartition {
        value: String,
    },
    InvalidTraceReplayDataCacheProtocol {
        value: String,
    },
    InvalidTraceReplayFabricBandwidth {
        value: String,
    },
    InvalidResourceKind {
        value: String,
    },
    InvalidResourceAcquisitionKind {
        value: String,
    },
    MissingRemoteResourceArtifactDigest {
        resource: String,
    },
    InvalidRemoteResourceArtifactDigest {
        resource: String,
        value: String,
    },
    InvalidRunDataCacheProtocol {
        value: String,
    },
    InvalidRunInstructionCacheProtocol {
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
    InvalidRiscvSeFile {
        value: String,
    },
    EmptyLoadBlob {
        path: PathBuf,
    },
    DramMemoryRequiresExecution,
    InstructionLimitRequiresExecution,
    MemoryDumpRequiresExecution,
    RiscvSeRequiresExecution,
    DataCacheProtocolRequiresExecution,
    InstructionCacheProtocolRequiresExecution,
    PowerOutputRequiresExecution,
    RiscvSeInputRequiresRiscvSe {
        input: &'static str,
    },
    DataCacheProtocolRequiresRiscv,
    InstructionCacheProtocolRequiresRiscv,
    DataCacheProtocolLargeMulticoreRequiresMsi {
        protocol: RiscvDataCacheProtocol,
        cores: usize,
    },
    InstructionCacheProtocolLargeMulticoreRequiresMsi {
        protocol: RiscvDataCacheProtocol,
        cores: usize,
    },
    RiscvSeRequiresRiscv,
    RiscvSeRequiresSingleCore {
        cores: usize,
    },
    ReadBinary {
        path: PathBuf,
        error: String,
    },
    ReadLoadBlob {
        path: PathBuf,
        error: String,
    },
    ReadResourceArtifact {
        path: PathBuf,
        error: String,
    },
    ReadRiscvSeStdin {
        path: PathBuf,
        error: String,
    },
    ReadRiscvSeFile {
        guest_path: String,
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
    ConflictingRunOutputPaths {
        path: PathBuf,
    },
    ConflictingRunBinarySources,
    PowerAnalysis {
        error: String,
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
            Self::ReadConfig { path, error } => {
                write!(formatter, "failed to read config {}: {error}", path.display())
            }
            Self::ParseConfig { path, error } => {
                write!(
                    formatter,
                    "failed to parse config {}: {error}",
                    path.display()
                )
            }
            Self::UnsupportedIsa { isa } => write!(formatter, "unsupported ISA {isa}"),
            Self::UnsupportedStatsFormat { format } => {
                write!(formatter, "unsupported stats format {format}")
            }
            Self::UnsupportedPowerAnalysisFormat { format } => {
                write!(formatter, "unsupported power analysis format {format}")
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
            Self::InvalidGupsMemoryStart { value } => {
                write!(formatter, "invalid GUPS memory start {value}")
            }
            Self::InvalidGupsMemorySize { value } => {
                write!(formatter, "invalid GUPS memory size {value}")
            }
            Self::InvalidGupsUpdates { value } => {
                write!(formatter, "invalid GUPS updates {value}")
            }
            Self::InvalidGupsRngState { value } => {
                write!(formatter, "invalid GUPS rng state {value}")
            }
            Self::InvalidTraceReplayMemoryStart { value } => {
                write!(formatter, "invalid trace replay memory start {value}")
            }
            Self::InvalidTraceReplayMemorySize { value } => {
                write!(formatter, "invalid trace replay memory size {value}")
            }
            Self::InvalidTraceReplayTickFrequency { value } => {
                write!(formatter, "invalid trace replay tick frequency {value}")
            }
            Self::InvalidTraceReplayLineBytes { value } => {
                write!(formatter, "invalid trace replay line bytes {value}")
            }
            Self::InvalidTraceReplayAgent { value } => {
                write!(formatter, "invalid trace replay agent {value}")
            }
            Self::InvalidTraceReplayControlPartition { value } => {
                write!(formatter, "invalid trace replay control partition {value}")
            }
            Self::InvalidTraceReplayDataCacheProtocol { value } => {
                write!(formatter, "invalid trace replay data cache protocol {value}")
            }
            Self::InvalidTraceReplayFabricBandwidth { value } => {
                write!(formatter, "invalid trace replay fabric bandwidth {value}")
            }
            Self::InvalidResourceKind { value } => {
                write!(formatter, "invalid resource kind {value}")
            }
            Self::InvalidResourceAcquisitionKind { value } => {
                write!(formatter, "invalid resource acquisition kind {value}")
            }
            Self::MissingRemoteResourceArtifactDigest { resource } => write!(
                formatter,
                "remote-uri resource {resource} requires explicit artifact_digest"
            ),
            Self::InvalidRemoteResourceArtifactDigest { resource, value } => write!(
                formatter,
                "remote-uri resource {resource} requires artifact_digest sha256:<64 lowercase hex>; got {value}"
            ),
            Self::InvalidRunDataCacheProtocol { value } => {
                write!(formatter, "invalid run data cache protocol {value}")
            }
            Self::InvalidRunInstructionCacheProtocol { value } => {
                write!(formatter, "invalid run instruction cache protocol {value}")
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
            Self::InvalidRiscvSeFile { value } => {
                write!(formatter, "invalid RISC-V SE file mapping {value}")
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
            Self::RiscvSeRequiresExecution => {
                write!(formatter, "--riscv-se requires --execute")
            }
            Self::DataCacheProtocolRequiresExecution => {
                write!(formatter, "--data-cache-protocol requires --execute")
            }
            Self::InstructionCacheProtocolRequiresExecution => {
                write!(formatter, "--instruction-cache-protocol requires --execute")
            }
            Self::PowerOutputRequiresExecution => {
                write!(formatter, "--power-output requires --execute")
            }
            Self::RiscvSeInputRequiresRiscvSe { input } => {
                write!(formatter, "{input} requires --riscv-se")
            }
            Self::DataCacheProtocolRequiresRiscv => {
                write!(formatter, "--data-cache-protocol requires --isa riscv")
            }
            Self::InstructionCacheProtocolRequiresRiscv => {
                write!(formatter, "--instruction-cache-protocol requires --isa riscv")
            }
            Self::DataCacheProtocolLargeMulticoreRequiresMsi { protocol, cores } => {
                write!(
                    formatter,
                    "--data-cache-protocol with --cores > 3 requires msi, got {} with {cores} cores",
                    riscv_data_cache_protocol_name(*protocol)
                )
            }
            Self::InstructionCacheProtocolLargeMulticoreRequiresMsi { protocol, cores } => {
                write!(
                    formatter,
                    "--instruction-cache-protocol with --cores > 3 requires msi, got {} with {cores} cores",
                    riscv_data_cache_protocol_name(*protocol)
                )
            }
            Self::RiscvSeRequiresRiscv => {
                write!(formatter, "--riscv-se requires --isa riscv")
            }
            Self::RiscvSeRequiresSingleCore { cores } => {
                write!(formatter, "--riscv-se requires --cores 1, got {cores}")
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
            Self::ReadResourceArtifact { path, error } => {
                write!(
                    formatter,
                    "failed to read resource artifact {}: {error}",
                    path.display()
                )
            }
            Self::ReadRiscvSeStdin { path, error } => {
                write!(
                    formatter,
                    "failed to read RISC-V SE stdin {}: {error}",
                    path.display()
                )
            }
            Self::ReadRiscvSeFile {
                guest_path,
                path,
                error,
            } => {
                write!(
                    formatter,
                    "failed to read RISC-V SE file {guest_path} from {}: {error}",
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
            Self::ConflictingRunOutputPaths { path } => write!(
                formatter,
                "run output artifacts must use different paths: {}",
                path.display()
            ),
            Self::ConflictingRunBinarySources => {
                write!(formatter, "run binary sources conflict: use binary or resource_config")
            }
            Self::PowerAnalysis { error } => {
                write!(formatter, "failed to build power analysis export: {error}")
            }
            Self::WriteOutput { path, error } => {
                write!(formatter, "failed to write {}: {error}", path.display())
            }
        }
    }
}

const fn riscv_data_cache_protocol_name(protocol: RiscvDataCacheProtocol) -> &'static str {
    match protocol {
        RiscvDataCacheProtocol::Msi => "msi",
        RiscvDataCacheProtocol::Mesi => "mesi",
        RiscvDataCacheProtocol::Moesi => "moesi",
        RiscvDataCacheProtocol::Chi => "chi",
    }
}

impl Error for Rem6CliError {}
