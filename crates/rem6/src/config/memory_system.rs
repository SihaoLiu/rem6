use rem6_system::RiscvDataCacheProtocol;

use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RunMemorySystem {
    CacheFabricDram,
}

impl RunMemorySystem {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "cache-fabric-dram" => Some(Self::CacheFabricDram),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CacheFabricDram => "cache-fabric-dram",
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn apply_run_memory_system_preset(
    memory_system: Option<RunMemorySystem>,
    dram_memory_disabled: bool,
    dram_memory: &mut bool,
    data_cache_protocol: &mut Option<RiscvDataCacheProtocol>,
    data_cache_l2_protocol: &mut Option<RiscvDataCacheProtocol>,
    data_cache_l3_protocol: &mut Option<RiscvDataCacheProtocol>,
    instruction_cache_protocol: &mut Option<RiscvDataCacheProtocol>,
    instruction_cache_l2_protocol: &mut Option<RiscvDataCacheProtocol>,
    instruction_cache_l3_protocol: &mut Option<RiscvDataCacheProtocol>,
    fabric_link: &mut Option<String>,
    fabric_bandwidth_bytes_per_tick: &mut Option<u64>,
    fabric_request_virtual_network: &mut Option<u16>,
    fabric_response_virtual_network: &mut Option<u16>,
    fabric_credit_depth: &mut Option<u32>,
) -> Result<(), Rem6CliError> {
    let Some(RunMemorySystem::CacheFabricDram) = memory_system else {
        return Ok(());
    };

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
    fabric_link.get_or_insert_with(|| "cpu_mem".to_string());
    fabric_bandwidth_bytes_per_tick.get_or_insert(64);
    fabric_request_virtual_network.get_or_insert(1);
    fabric_response_virtual_network.get_or_insert(2);
    fabric_credit_depth.get_or_insert(4);
    Ok(())
}
