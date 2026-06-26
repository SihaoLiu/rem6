use rem6_isa_riscv::MemoryAccessKind;
use rem6_memory::{AccessSize, Address, MemoryRequestId};

use crate::CpuTranslationFaultRecord;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingDataTranslation {
    pub(crate) request_id: MemoryRequestId,
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) access: MemoryAccessKind,
    pub(crate) size: AccessSize,
}

impl PendingDataTranslation {
    pub(crate) const fn fetch_request(&self) -> MemoryRequestId {
        self.fetch_request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum DataTranslationCompletion {
    Access(TranslatedDataAccess),
    Fault {
        fetch_request: MemoryRequestId,
        fault: CpuTranslationFaultRecord,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranslatedDataAccess {
    pub(crate) request_id: MemoryRequestId,
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) access: MemoryAccessKind,
    pub(crate) size: AccessSize,
    pub(crate) physical_address: Address,
}
