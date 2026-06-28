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
    EmptyDebugFlag,
    UnsupportedDebugFlag {
        flag: String,
    },
    UnsupportedPowerAnalysisFormat {
        format: String,
    },
    UnsupportedDramMemoryProfile {
        profile: String,
    },
    DramMemoryProfileRequiresDramMemory,
    DramLowPowerTimingRequiresDramMemory,
    DramLowPowerTimingRequiresLowPowerProfile {
        profile: String,
    },
    DramRefreshTimingRequiresDramMemory,
    DramRefreshTimingRequiresRefreshProfile {
        profile: String,
    },
    IncompleteDramRefreshTiming,
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
    InvalidDramLowPowerTiming {
        value: String,
    },
    InvalidDramRefreshTiming {
        value: String,
    },
    InvalidRiscvBranchLookahead {
        value: String,
    },
    InvalidRiscvBranchPredictor {
        value: String,
    },
    InvalidRiscvInOrderWidth {
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
    InvalidTraceReplayDataCacheDramQosPriorityLevels {
        value: String,
    },
    InvalidTraceReplayDataCacheDramQosDefaultPriority {
        value: String,
    },
    InvalidTraceReplayFabricBandwidth {
        value: String,
    },
    InvalidTraceReplayFabricVirtualNetwork {
        value: String,
    },
    InvalidTraceReplayFabricCreditDepth {
        value: String,
    },
    InvalidTraceReplayExternalAdapterKind {
        value: String,
    },
    InvalidTraceReplayExternalAdapterCheckpointAfterEvents {
        value: String,
    },
    InvalidTraceReplayHostEvent {
        value: String,
    },
    InvalidGpuRunFabricBandwidth {
        value: String,
    },
    InvalidGpuRunFabricVirtualNetwork {
        value: String,
    },
    InvalidGpuRunFabricCreditDepth {
        value: String,
    },
    InvalidTraceReplayResource {
        value: String,
    },
    InvalidRunKernelResource {
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
    InvalidRunMemorySystem {
        value: String,
    },
    RunMemorySystemConflictsWithDisabledDram {
        memory_system: String,
    },
    RunMemorySystemConflictsWithMemoryHierarchy {
        memory_system: String,
    },
    InvalidRunDataCacheProtocol {
        value: String,
    },
    InvalidRunDataCacheL2Protocol {
        value: String,
    },
    InvalidRunDataCacheL3Protocol {
        value: String,
    },
    InvalidRunDataCachePrefetcher {
        value: String,
    },
    InvalidRunInstructionCacheProtocol {
        value: String,
    },
    InvalidRunInstructionCacheL2Protocol {
        value: String,
    },
    InvalidRunInstructionCacheL3Protocol {
        value: String,
    },
    InvalidRunInstructionCachePrefetcher {
        value: String,
    },
    InvalidRunFabricBandwidth {
        value: String,
    },
    InvalidRunFabricVirtualNetwork {
        value: String,
    },
    InvalidRunFabricCreditDepth {
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
    DuplicateMultiRunId {
        id: String,
    },
    InvalidMemoryDump {
        value: String,
    },
    InvalidLoadBlob {
        value: String,
    },
    InvalidReadfile {
        value: String,
    },
    InvalidRiscvSeStdin {
        value: String,
    },
    InvalidRiscvSeFile {
        value: String,
    },
    DuplicateRiscvSeGuestFile {
        guest_path: String,
    },
    InvalidRiscvPcCountTarget {
        value: String,
    },
    EmptyLoadBlob {
        source: String,
    },
    DramMemoryRequiresExecution,
    InstructionLimitRequiresExecution,
    MemoryDumpRequiresExecution,
    ReadfileRequiresExecution,
    MemorySystemRequiresExecution,
    ReadfileRequiresRiscv,
    RiscvSbiRequiresExecution,
    RiscvSeRequiresExecution,
    CheckerCpuRequiresExecution,
    DataCacheProtocolRequiresExecution,
    DataCacheL2ProtocolRequiresExecution,
    DataCacheL3ProtocolRequiresExecution,
    DataCachePrefetcherRequiresExecution,
    InstructionCacheProtocolRequiresExecution,
    InstructionCacheL2ProtocolRequiresExecution,
    InstructionCacheL3ProtocolRequiresExecution,
    InstructionCachePrefetcherRequiresExecution,
    FabricRequiresExecution,
    RiscvPcCountTargetRequiresExecution,
    RiscvBranchLookaheadRequiresExecution,
    RiscvBranchPredictorRequiresExecution,
    RiscvInOrderWidthRequiresExecution,
    DebugFlagsRequireExecution,
    DebugFlagsRequireJsonStats,
    PowerOutputRequiresExecution,
    TraceReplayExternalAdapterEndpointRequiresKind,
    RiscvSeInputRequiresRiscvSe {
        input: &'static str,
    },
    DataCacheProtocolRequiresRiscv,
    DataCacheL2ProtocolRequiresRiscv,
    DataCacheL2ProtocolRequiresDataCacheProtocol,
    DataCacheL3ProtocolRequiresRiscv,
    DataCacheL3ProtocolRequiresDataCacheL2Protocol,
    DataCachePrefetcherRequiresRiscv,
    DataCachePrefetcherRequiresDataCacheProtocol,
    InstructionCacheProtocolRequiresRiscv,
    InstructionCacheL2ProtocolRequiresRiscv,
    InstructionCacheL2ProtocolRequiresInstructionCacheProtocol,
    InstructionCacheL3ProtocolRequiresRiscv,
    InstructionCacheL3ProtocolRequiresInstructionCacheL2Protocol,
    InstructionCachePrefetcherRequiresRiscv,
    InstructionCachePrefetcherRequiresInstructionCacheProtocol,
    FabricRequiresRiscv,
    RiscvPcCountTargetRequiresRiscv,
    RiscvBranchLookaheadRequiresRiscv,
    RiscvBranchPredictorRequiresRiscv,
    RiscvInOrderWidthRequiresRiscv,
    CheckerCpuRequiresRiscv,
    RiscvSbiRequiresRiscv,
    RiscvSbiConflictsWithRiscvSe,
    RiscvSbiRequiresBootA0Zero,
    DataCacheProtocolLargeMulticoreRequiresMsi {
        protocol: RiscvDataCacheProtocol,
        cores: usize,
    },
    DataCacheL2ProtocolLargeMulticoreRequiresMsi {
        protocol: RiscvDataCacheProtocol,
        cores: usize,
    },
    DataCacheL3ProtocolLargeMulticoreRequiresMsi {
        protocol: RiscvDataCacheProtocol,
        cores: usize,
    },
    InstructionCacheProtocolLargeMulticoreRequiresMsi {
        protocol: RiscvDataCacheProtocol,
        cores: usize,
    },
    InstructionCacheL2ProtocolLargeMulticoreRequiresMsi {
        protocol: RiscvDataCacheProtocol,
        cores: usize,
    },
    InstructionCacheL3ProtocolLargeMulticoreRequiresMsi {
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
    ReadReadfile {
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
    WriteRiscvSeFile {
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
    ConflictingTraceReplaySources,
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
            Self::EmptyDebugFlag => write!(formatter, "empty debug flag entry"),
            Self::UnsupportedDebugFlag { flag } => {
                write!(formatter, "unsupported debug flag {flag}")
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
            Self::DramLowPowerTimingRequiresDramMemory => {
                write!(formatter, "DRAM low-power timing requires --dram-memory")
            }
            Self::DramLowPowerTimingRequiresLowPowerProfile { profile } => write!(
                formatter,
                "DRAM low-power timing requires lpddr, lpddr4-3200-16gb, or nvm profile, got {profile}"
            ),
            Self::DramRefreshTimingRequiresDramMemory => {
                write!(formatter, "DRAM refresh timing requires --dram-memory")
            }
            Self::DramRefreshTimingRequiresRefreshProfile { profile } => write!(
                formatter,
                "DRAM refresh timing requires ddr, ddr4-2400-8gb, ddr5-4800-16gb, hbm, hbm2-2000-2gb, lpddr, or lpddr4-3200-16gb profile, got {profile}"
            ),
            Self::IncompleteDramRefreshTiming => write!(
                formatter,
                "DRAM refresh timing requires both --dram-refresh-interval and --dram-refresh-recovery"
            ),
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
            Self::InvalidDramLowPowerTiming { value } => {
                write!(formatter, "invalid DRAM low-power timing {value}")
            }
            Self::InvalidDramRefreshTiming { value } => {
                write!(formatter, "invalid DRAM refresh timing {value}")
            }
            Self::InvalidRiscvBranchLookahead { value } => {
                write!(formatter, "invalid RISC-V branch lookahead {value}")
            }
            Self::InvalidRiscvBranchPredictor { value } => {
                write!(formatter, "invalid RISC-V branch predictor {value}")
            }
            Self::InvalidRiscvInOrderWidth { value } => {
                write!(formatter, "invalid RISC-V in-order width {value}")
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
            Self::InvalidTraceReplayDataCacheDramQosPriorityLevels { value } => {
                write!(
                    formatter,
                    "invalid trace replay data cache DRAM QoS priority levels {value}"
                )
            }
            Self::InvalidTraceReplayDataCacheDramQosDefaultPriority { value } => {
                write!(
                    formatter,
                    "invalid trace replay data cache DRAM QoS default priority {value}"
                )
            }
            Self::InvalidTraceReplayFabricBandwidth { value } => {
                write!(formatter, "invalid trace replay fabric bandwidth {value}")
            }
            Self::InvalidTraceReplayFabricVirtualNetwork { value } => {
                write!(
                    formatter,
                    "invalid trace replay fabric virtual network {value}"
                )
            }
            Self::InvalidTraceReplayFabricCreditDepth { value } => {
                write!(formatter, "invalid trace replay fabric credit depth {value}")
            }
            Self::InvalidTraceReplayExternalAdapterKind { value } => {
                write!(
                    formatter,
                    "unsupported trace replay external adapter kind {value}; supported: systemc, tlm, sst"
                )
            }
            Self::InvalidTraceReplayExternalAdapterCheckpointAfterEvents { value } => {
                write!(
                    formatter,
                    "invalid trace replay external adapter checkpoint after events {value}"
                )
            }
            Self::InvalidTraceReplayHostEvent { value } => {
                write!(
                    formatter,
                    "invalid trace replay host event {value}; expected tick:label"
                )
            }
            Self::InvalidGpuRunFabricBandwidth { value } => {
                write!(formatter, "invalid gpu run fabric bandwidth {value}")
            }
            Self::InvalidGpuRunFabricVirtualNetwork { value } => {
                write!(formatter, "invalid gpu run fabric virtual network {value}")
            }
            Self::InvalidGpuRunFabricCreditDepth { value } => {
                write!(formatter, "invalid gpu run fabric credit depth {value}")
            }
            Self::InvalidTraceReplayResource { value } => {
                write!(
                    formatter,
                    "invalid trace replay resource {value}; expected suite-resource:<workload>/<resource>"
                )
            }
            Self::InvalidRunKernelResource { value } => {
                write!(
                    formatter,
                    "invalid run kernel resource {value}; expected resource:<resource> or suite-resource:<workload>/<resource>"
                )
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
            Self::InvalidRunMemorySystem { value } => {
                write!(formatter, "invalid run memory system {value}")
            }
            Self::RunMemorySystemConflictsWithDisabledDram { memory_system } => write!(
                formatter,
                "memory system {memory_system} conflicts with dram_memory = false"
            ),
            Self::RunMemorySystemConflictsWithMemoryHierarchy { memory_system } => write!(
                formatter,
                "memory system {memory_system} conflicts with memory hierarchy options"
            ),
            Self::InvalidRunDataCacheProtocol { value } => {
                write!(formatter, "invalid run data cache protocol {value}")
            }
            Self::InvalidRunDataCacheL2Protocol { value } => {
                write!(formatter, "invalid run data cache L2 protocol {value}")
            }
            Self::InvalidRunDataCacheL3Protocol { value } => {
                write!(formatter, "invalid run data cache L3 protocol {value}")
            }
            Self::InvalidRunDataCachePrefetcher { value } => {
                write!(formatter, "invalid run data cache prefetcher {value}")
            }
            Self::InvalidRunInstructionCacheProtocol { value } => {
                write!(formatter, "invalid run instruction cache protocol {value}")
            }
            Self::InvalidRunInstructionCacheL2Protocol { value } => {
                write!(formatter, "invalid run instruction cache L2 protocol {value}")
            }
            Self::InvalidRunInstructionCacheL3Protocol { value } => {
                write!(formatter, "invalid run instruction cache L3 protocol {value}")
            }
            Self::InvalidRunInstructionCachePrefetcher { value } => {
                write!(formatter, "invalid run instruction cache prefetcher {value}")
            }
            Self::InvalidRunFabricBandwidth { value } => {
                write!(formatter, "invalid run fabric bandwidth {value}")
            }
            Self::InvalidRunFabricVirtualNetwork { value } => {
                write!(formatter, "invalid run fabric virtual network {value}")
            }
            Self::InvalidRunFabricCreditDepth { value } => {
                write!(formatter, "invalid run fabric credit depth {value}")
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
            Self::DuplicateMultiRunId { id } => {
                write!(formatter, "multi-run ids must be unique: {id}")
            }
            Self::InvalidMemoryDump { value } => {
                write!(formatter, "invalid memory dump request {value}")
            }
            Self::InvalidLoadBlob { value } => {
                write!(formatter, "invalid load blob {value}")
            }
            Self::InvalidReadfile { value } => {
                write!(formatter, "invalid readfile {value}")
            }
            Self::InvalidRiscvSeStdin { value } => {
                write!(formatter, "invalid RISC-V SE stdin {value}")
            }
            Self::InvalidRiscvSeFile { value } => {
                write!(formatter, "invalid RISC-V SE file mapping {value}")
            }
            Self::DuplicateRiscvSeGuestFile { guest_path } => {
                write!(
                    formatter,
                    "RISC-V SE file guest paths must be unique: {guest_path}"
                )
            }
            Self::InvalidRiscvPcCountTarget { value } => {
                write!(
                    formatter,
                    "invalid RISC-V PC count target {value}; expected <pc>:<positive-count>"
                )
            }
            Self::EmptyLoadBlob { source } => {
                write!(formatter, "load blob {source} is empty")
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
            Self::ReadfileRequiresExecution => {
                write!(formatter, "--readfile requires --execute")
            }
            Self::MemorySystemRequiresExecution => {
                write!(formatter, "--memory-system requires --execute")
            }
            Self::ReadfileRequiresRiscv => {
                write!(formatter, "--readfile requires --isa riscv")
            }
            Self::RiscvSbiRequiresExecution => {
                write!(formatter, "--riscv-sbi requires --execute")
            }
            Self::RiscvSeRequiresExecution => {
                write!(formatter, "--riscv-se requires --execute")
            }
            Self::CheckerCpuRequiresExecution => {
                write!(formatter, "--checker-cpu requires --execute")
            }
            Self::DataCacheProtocolRequiresExecution => {
                write!(formatter, "--data-cache-protocol requires --execute")
            }
            Self::DataCacheL2ProtocolRequiresExecution => {
                write!(formatter, "--data-cache-l2-protocol requires --execute")
            }
            Self::DataCacheL3ProtocolRequiresExecution => {
                write!(formatter, "--data-cache-l3-protocol requires --execute")
            }
            Self::DataCachePrefetcherRequiresExecution => {
                write!(formatter, "--data-cache-prefetcher requires --execute")
            }
            Self::InstructionCacheProtocolRequiresExecution => {
                write!(formatter, "--instruction-cache-protocol requires --execute")
            }
            Self::InstructionCacheL2ProtocolRequiresExecution => {
                write!(formatter, "--instruction-cache-l2-protocol requires --execute")
            }
            Self::InstructionCacheL3ProtocolRequiresExecution => {
                write!(formatter, "--instruction-cache-l3-protocol requires --execute")
            }
            Self::InstructionCachePrefetcherRequiresExecution => {
                write!(formatter, "--instruction-cache-prefetcher requires --execute")
            }
            Self::FabricRequiresExecution => {
                write!(formatter, "--fabric-link requires --execute")
            }
            Self::RiscvPcCountTargetRequiresExecution => {
                write!(formatter, "--riscv-pc-count-target requires --execute")
            }
            Self::RiscvBranchLookaheadRequiresExecution => {
                write!(formatter, "--riscv-branch-lookahead requires --execute")
            }
            Self::RiscvBranchPredictorRequiresExecution => {
                write!(formatter, "--riscv-branch-predictor requires --execute")
            }
            Self::RiscvInOrderWidthRequiresExecution => {
                write!(formatter, "--riscv-in-order-width requires --execute")
            }
            Self::DebugFlagsRequireExecution => {
                write!(formatter, "--debug-flags requires --execute")
            }
            Self::DebugFlagsRequireJsonStats => {
                write!(formatter, "--debug-flags requires --stats-format json")
            }
            Self::PowerOutputRequiresExecution => {
                write!(formatter, "--power-output requires --execute")
            }
            Self::TraceReplayExternalAdapterEndpointRequiresKind => {
                write!(
                    formatter,
                    "--external-adapter-endpoint requires --external-adapter-kind"
                )
            }
            Self::RiscvSeInputRequiresRiscvSe { input } => {
                write!(formatter, "{input} requires --riscv-se")
            }
            Self::DataCacheProtocolRequiresRiscv => {
                write!(formatter, "--data-cache-protocol requires --isa riscv")
            }
            Self::DataCacheL2ProtocolRequiresRiscv => {
                write!(formatter, "--data-cache-l2-protocol requires --isa riscv")
            }
            Self::DataCacheL2ProtocolRequiresDataCacheProtocol => {
                write!(
                    formatter,
                    "--data-cache-l2-protocol requires --data-cache-protocol"
                )
            }
            Self::DataCacheL3ProtocolRequiresRiscv => {
                write!(formatter, "--data-cache-l3-protocol requires --isa riscv")
            }
            Self::DataCacheL3ProtocolRequiresDataCacheL2Protocol => {
                write!(
                    formatter,
                    "--data-cache-l3-protocol requires --data-cache-l2-protocol"
                )
            }
            Self::DataCachePrefetcherRequiresRiscv => {
                write!(formatter, "--data-cache-prefetcher requires --isa riscv")
            }
            Self::DataCachePrefetcherRequiresDataCacheProtocol => {
                write!(
                    formatter,
                    "--data-cache-prefetcher requires --data-cache-protocol"
                )
            }
            Self::InstructionCacheProtocolRequiresRiscv => {
                write!(formatter, "--instruction-cache-protocol requires --isa riscv")
            }
            Self::InstructionCacheL2ProtocolRequiresRiscv => {
                write!(formatter, "--instruction-cache-l2-protocol requires --isa riscv")
            }
            Self::InstructionCacheL2ProtocolRequiresInstructionCacheProtocol => {
                write!(
                    formatter,
                    "--instruction-cache-l2-protocol requires --instruction-cache-protocol"
                )
            }
            Self::InstructionCacheL3ProtocolRequiresRiscv => {
                write!(formatter, "--instruction-cache-l3-protocol requires --isa riscv")
            }
            Self::InstructionCacheL3ProtocolRequiresInstructionCacheL2Protocol => {
                write!(
                    formatter,
                    "--instruction-cache-l3-protocol requires --instruction-cache-l2-protocol"
                )
            }
            Self::InstructionCachePrefetcherRequiresRiscv => {
                write!(formatter, "--instruction-cache-prefetcher requires --isa riscv")
            }
            Self::InstructionCachePrefetcherRequiresInstructionCacheProtocol => {
                write!(
                    formatter,
                    "--instruction-cache-prefetcher requires --instruction-cache-protocol"
                )
            }
            Self::FabricRequiresRiscv => {
                write!(formatter, "--fabric-link requires --isa riscv")
            }
            Self::RiscvPcCountTargetRequiresRiscv => {
                write!(formatter, "--riscv-pc-count-target requires --isa riscv")
            }
            Self::RiscvBranchLookaheadRequiresRiscv => {
                write!(formatter, "--riscv-branch-lookahead requires --isa riscv")
            }
            Self::RiscvBranchPredictorRequiresRiscv => {
                write!(formatter, "--riscv-branch-predictor requires --isa riscv")
            }
            Self::RiscvInOrderWidthRequiresRiscv => {
                write!(formatter, "--riscv-in-order-width requires --isa riscv")
            }
            Self::CheckerCpuRequiresRiscv => {
                write!(formatter, "--checker-cpu requires --isa riscv")
            }
            Self::RiscvSbiRequiresRiscv => {
                write!(formatter, "--riscv-sbi requires --isa riscv")
            }
            Self::RiscvSbiConflictsWithRiscvSe => {
                write!(formatter, "--riscv-sbi cannot be combined with --riscv-se")
            }
            Self::RiscvSbiRequiresBootA0Zero => {
                write!(formatter, "--riscv-sbi requires --riscv-boot-a0 0")
            }
            Self::DataCacheProtocolLargeMulticoreRequiresMsi { protocol, cores } => {
                write!(
                    formatter,
                    "--data-cache-protocol with --cores > 3 requires msi, got {} with {cores} cores",
                    riscv_data_cache_protocol_name(*protocol)
                )
            }
            Self::DataCacheL2ProtocolLargeMulticoreRequiresMsi { protocol, cores } => {
                write!(
                    formatter,
                    "--data-cache-l2-protocol with --cores > 3 requires msi, got {} with {cores} cores",
                    riscv_data_cache_protocol_name(*protocol)
                )
            }
            Self::DataCacheL3ProtocolLargeMulticoreRequiresMsi { protocol, cores } => {
                write!(
                    formatter,
                    "--data-cache-l3-protocol with --cores > 3 requires msi, got {} with {cores} cores",
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
            Self::InstructionCacheL2ProtocolLargeMulticoreRequiresMsi { protocol, cores } => {
                write!(
                    formatter,
                    "--instruction-cache-l2-protocol with --cores > 3 requires msi, got {} with {cores} cores",
                    riscv_data_cache_protocol_name(*protocol)
                )
            }
            Self::InstructionCacheL3ProtocolLargeMulticoreRequiresMsi { protocol, cores } => {
                write!(
                    formatter,
                    "--instruction-cache-l3-protocol with --cores > 3 requires msi, got {} with {cores} cores",
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
            Self::ReadReadfile { path, error } => {
                write!(
                    formatter,
                    "failed to read readfile {}: {error}",
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
            Self::WriteRiscvSeFile {
                guest_path,
                path,
                error,
            } => {
                write!(
                    formatter,
                    "failed to write RISC-V SE file {guest_path} to {}: {error}",
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
            Self::ConflictingTraceReplaySources => {
                write!(
                    formatter,
                    "trace replay sources conflict: use trace or resource_config"
                )
            }
            Self::PowerAnalysis { error } => {
                write!(formatter, "failed to process power analysis artifact: {error}")
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
