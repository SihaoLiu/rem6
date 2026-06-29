use rem6_system::RiscvDataCacheProtocol;

use super::fabric::RunFabricConfigParts;
use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunMemorySystem {
    Direct,
    CacheFabricDram,
}

impl RunMemorySystem {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "direct" => Some(Self::Direct),
            "cache-fabric-dram" => Some(Self::CacheFabricDram),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Direct => "direct",
            Self::CacheFabricDram => "cache-fabric-dram",
        }
    }
}

pub(super) const fn default_run_memory_system_for_execution(
    memory_system: Option<RunMemorySystem>,
) -> RunMemorySystem {
    match memory_system {
        Some(memory_system) => memory_system,
        None => RunMemorySystem::CacheFabricDram,
    }
}

pub(super) fn apply_run_memory_system_preset(
    memory_system: RunMemorySystem,
    dram_memory_disabled: bool,
    dram_memory: &mut bool,
    data_cache_protocol: &mut Option<RiscvDataCacheProtocol>,
    data_cache_l2_protocol: &mut Option<RiscvDataCacheProtocol>,
    data_cache_l3_protocol: &mut Option<RiscvDataCacheProtocol>,
    instruction_cache_protocol: &mut Option<RiscvDataCacheProtocol>,
    instruction_cache_l2_protocol: &mut Option<RiscvDataCacheProtocol>,
    instruction_cache_l3_protocol: &mut Option<RiscvDataCacheProtocol>,
    fabric: &mut RunFabricConfigParts,
) -> Result<(), Rem6CliError> {
    match memory_system {
        RunMemorySystem::Direct => {}
        RunMemorySystem::CacheFabricDram => {
            if dram_memory_disabled {
                return Err(Rem6CliError::RunMemorySystemConflictsWithDisabledDram {
                    memory_system: RunMemorySystem::CacheFabricDram.as_str().to_string(),
                });
            }
            *dram_memory = true;
            data_cache_protocol.get_or_insert(RiscvDataCacheProtocol::Msi);
            data_cache_l2_protocol.get_or_insert(RiscvDataCacheProtocol::Msi);
            data_cache_l3_protocol.get_or_insert(RiscvDataCacheProtocol::Msi);
            instruction_cache_protocol.get_or_insert(RiscvDataCacheProtocol::Msi);
            instruction_cache_l2_protocol.get_or_insert(RiscvDataCacheProtocol::Msi);
            instruction_cache_l3_protocol.get_or_insert(RiscvDataCacheProtocol::Msi);
            fabric.apply_cache_fabric_dram_defaults();
        }
    }
    Ok(())
}
