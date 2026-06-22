use crate::Rem6CliError;
use rem6_system::RiscvDataCacheProtocol;
use rem6_workload::WorkloadDataCacheProtocol;

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

pub(super) fn parse_data_cache_protocol(value: &str) -> Option<WorkloadDataCacheProtocol> {
    match value {
        "msi" => Some(WorkloadDataCacheProtocol::Msi),
        "mesi" => Some(WorkloadDataCacheProtocol::Mesi),
        "moesi" => Some(WorkloadDataCacheProtocol::Moesi),
        "chi" => Some(WorkloadDataCacheProtocol::Chi),
        _ => None,
    }
}

pub(super) fn parse_run_data_cache_protocol(value: &str) -> Option<RiscvDataCacheProtocol> {
    match value {
        "msi" => Some(RiscvDataCacheProtocol::Msi),
        "mesi" => Some(RiscvDataCacheProtocol::Mesi),
        "moesi" => Some(RiscvDataCacheProtocol::Moesi),
        "chi" => Some(RiscvDataCacheProtocol::Chi),
        _ => None,
    }
}
