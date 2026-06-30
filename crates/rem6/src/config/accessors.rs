use std::path::Path;

use rem6_cpu::RiscvBranchPredictorKind;
use rem6_stats::PcCountPair;
use rem6_system::{ExecutionMode, RiscvDataCacheProtocol};

use super::{
    CliCachePrefetcher, CliDebugFlag, CliDramLowPowerTiming, CliDramMemoryProfile,
    CliDramRefreshTiming, CliDramTiming, GuestHostCallResponseConfig, KernelResourceSelector,
    LoadBlobRequest, MemoryDumpRequest, PowerAnalysisFormat, ReadfileRequest, Rem6RunConfig,
    RequestedIsa, RiscvSeFileRequest, RiscvSeInputSource, RunFabricConfig, RunMemorySystem,
    StatsFormat, DEFAULT_RISCV_IN_ORDER_WIDTH,
};

impl Rem6RunConfig {
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

    pub fn riscv_sbi_console_input(&self) -> Option<&RiscvSeInputSource> {
        self.riscv_sbi_console_input.as_ref()
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

    pub fn riscv_in_order_width(&self) -> usize {
        self.riscv_in_order_width
            .unwrap_or(DEFAULT_RISCV_IN_ORDER_WIDTH)
    }

    pub const fn riscv_in_order_width_is_explicit(&self) -> bool {
        self.riscv_in_order_width.is_some()
    }

    pub const fn m5_switch_cpu_mode(&self) -> ExecutionMode {
        match self.m5_switch_cpu_mode {
            Some(mode) => mode,
            None => ExecutionMode::Detailed,
        }
    }

    pub const fn m5_switch_cpu_mode_is_explicit(&self) -> bool {
        self.m5_switch_cpu_mode.is_some()
    }

    pub(crate) fn guest_host_call_responses(&self) -> &[GuestHostCallResponseConfig] {
        &self.guest_host_call_responses
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

    pub const fn dram_timing(&self) -> CliDramTiming {
        self.dram_timing
    }

    pub const fn dram_low_power_timing(&self) -> CliDramLowPowerTiming {
        self.dram_low_power_timing
    }

    pub const fn dram_refresh_timing(&self) -> Option<CliDramRefreshTiming> {
        self.dram_refresh_timing
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

    pub fn debug_branch_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Branch)
    }

    pub fn debug_cache_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Cache)
    }

    pub fn debug_exec_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Exec)
    }

    pub fn debug_fabric_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Fabric)
    }

    pub fn debug_fetch_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Fetch)
    }

    pub fn debug_data_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Data)
    }

    pub fn debug_dram_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Dram)
    }

    pub fn debug_memory_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Memory)
    }

    pub fn debug_pipeline_enabled(&self) -> bool {
        self.debug_flags.contains(&CliDebugFlag::Pipeline)
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
