use rem6_cpu::RiscvBranchPredictorKind;
use rem6_system::RiscvDataCacheProtocol;

use crate::config::{Rem6RunConfig, RequestedIsa, StatsFormat};
use crate::run_gdb::validate_run_gdb_listen_config;
use crate::Rem6CliError;

pub(super) fn validate_run_config_inputs(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if !config.execute() {
        validate_non_execution_inputs(config)?;
    }
    validate_debug_flag_inputs(config)?;
    validate_cache_inputs(config)?;
    validate_readfile_inputs(config)?;
    validate_riscv_se_inputs(config)?;
    validate_riscv_sbi_inputs(config)?;
    if config.gdb_listen().is_some() {
        validate_run_gdb_listen_config(config)?;
    }
    Ok(())
}

fn validate_non_execution_inputs(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if config.dram_memory() {
        return Err(Rem6CliError::DramMemoryRequiresExecution);
    }
    if config.max_instructions().is_some() {
        return Err(Rem6CliError::InstructionLimitRequiresExecution);
    }
    if !config.memory_dumps().is_empty() {
        return Err(Rem6CliError::MemoryDumpRequiresExecution);
    }
    if !config.readfiles().is_empty() {
        return Err(Rem6CliError::ReadfileRequiresExecution);
    }
    if config.riscv_se() {
        return Err(Rem6CliError::RiscvSeRequiresExecution);
    }
    if config.riscv_sbi() {
        return Err(Rem6CliError::RiscvSbiRequiresExecution);
    }
    if config.checker_cpu() {
        return Err(Rem6CliError::CheckerCpuRequiresExecution);
    }
    if config.data_cache_protocol().is_some() {
        return Err(Rem6CliError::DataCacheProtocolRequiresExecution);
    }
    if config.data_cache_l2_protocol().is_some() {
        return Err(Rem6CliError::DataCacheL2ProtocolRequiresExecution);
    }
    if config.data_cache_prefetcher().is_some() {
        return Err(Rem6CliError::DataCachePrefetcherRequiresExecution);
    }
    if config.instruction_cache_protocol().is_some() {
        return Err(Rem6CliError::InstructionCacheProtocolRequiresExecution);
    }
    if config.instruction_cache_prefetcher().is_some() {
        return Err(Rem6CliError::InstructionCachePrefetcherRequiresExecution);
    }
    if !config.riscv_pc_count_targets().is_empty() {
        return Err(Rem6CliError::RiscvPcCountTargetRequiresExecution);
    }
    if config.riscv_branch_lookahead() > 1 {
        return Err(Rem6CliError::RiscvBranchLookaheadRequiresExecution);
    }
    if config.riscv_branch_predictor() != RiscvBranchPredictorKind::Basic {
        return Err(Rem6CliError::RiscvBranchPredictorRequiresExecution);
    }
    if !config.debug_flags().is_empty() {
        return Err(Rem6CliError::DebugFlagsRequireExecution);
    }
    if config.power_output().is_some() {
        return Err(Rem6CliError::PowerOutputRequiresExecution);
    }
    Ok(())
}

fn validate_debug_flag_inputs(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if !config.debug_flags().is_empty() && config.stats_format() != StatsFormat::Json {
        return Err(Rem6CliError::DebugFlagsRequireJsonStats);
    }
    Ok(())
}

fn validate_readfile_inputs(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if !config.readfiles().is_empty() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::ReadfileRequiresRiscv);
    }
    Ok(())
}

fn validate_cache_inputs(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if config.data_cache_protocol().is_some() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::DataCacheProtocolRequiresRiscv);
    }
    if config.data_cache_l2_protocol().is_some() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::DataCacheL2ProtocolRequiresRiscv);
    }
    if config.data_cache_l2_protocol().is_some() && config.data_cache_protocol().is_none() {
        return Err(Rem6CliError::DataCacheL2ProtocolRequiresDataCacheProtocol);
    }
    if config.data_cache_prefetcher().is_some() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::DataCachePrefetcherRequiresRiscv);
    }
    if config.data_cache_prefetcher().is_some() && config.data_cache_protocol().is_none() {
        return Err(Rem6CliError::DataCachePrefetcherRequiresDataCacheProtocol);
    }
    if config.instruction_cache_protocol().is_some() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::InstructionCacheProtocolRequiresRiscv);
    }
    if config.instruction_cache_prefetcher().is_some() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::InstructionCachePrefetcherRequiresRiscv);
    }
    if !config.riscv_pc_count_targets().is_empty() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::RiscvPcCountTargetRequiresRiscv);
    }
    if config.riscv_branch_lookahead() > 1 && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::RiscvBranchLookaheadRequiresRiscv);
    }
    if config.riscv_branch_predictor() != RiscvBranchPredictorKind::Basic
        && config.isa() != RequestedIsa::Riscv
    {
        return Err(Rem6CliError::RiscvBranchPredictorRequiresRiscv);
    }
    if config.checker_cpu() && config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::CheckerCpuRequiresRiscv);
    }
    if config.instruction_cache_prefetcher().is_some()
        && config.instruction_cache_protocol().is_none()
    {
        return Err(Rem6CliError::InstructionCachePrefetcherRequiresInstructionCacheProtocol);
    }
    validate_large_multicore_cache_protocols(config)
}

fn validate_large_multicore_cache_protocols(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if config.cores() <= 3 {
        return Ok(());
    }
    if let Some(protocol) = config.data_cache_protocol() {
        if protocol != RiscvDataCacheProtocol::Msi {
            return Err(Rem6CliError::DataCacheProtocolLargeMulticoreRequiresMsi {
                protocol,
                cores: config.cores(),
            });
        }
    }
    if let Some(protocol) = config.data_cache_l2_protocol() {
        if protocol != RiscvDataCacheProtocol::Msi {
            return Err(Rem6CliError::DataCacheL2ProtocolLargeMulticoreRequiresMsi {
                protocol,
                cores: config.cores(),
            });
        }
    }
    if let Some(protocol) = config.instruction_cache_protocol() {
        if protocol != RiscvDataCacheProtocol::Msi {
            return Err(
                Rem6CliError::InstructionCacheProtocolLargeMulticoreRequiresMsi {
                    protocol,
                    cores: config.cores(),
                },
            );
        }
    }
    Ok(())
}

fn validate_riscv_se_inputs(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if !config.riscv_se() {
        return Ok(());
    }
    if config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::RiscvSeRequiresRiscv);
    }
    if config.cores() != 1 {
        return Err(Rem6CliError::RiscvSeRequiresSingleCore {
            cores: config.cores(),
        });
    }
    Ok(())
}

fn validate_riscv_sbi_inputs(config: &Rem6RunConfig) -> Result<(), Rem6CliError> {
    if !config.riscv_sbi() {
        return Ok(());
    }
    if config.isa() != RequestedIsa::Riscv {
        return Err(Rem6CliError::RiscvSbiRequiresRiscv);
    }
    if config.riscv_se() {
        return Err(Rem6CliError::RiscvSbiConflictsWithRiscvSe);
    }
    if config.riscv_boot_a0() != 0 {
        return Err(Rem6CliError::RiscvSbiRequiresBootA0Zero);
    }
    Ok(())
}
