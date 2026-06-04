use crate::{
    bank::MsiCacheBank, chi::ChiCacheControllerResult, chi::ChiCacheControllerResultKind,
    chi_bank::ChiCacheBank, mesi::MesiCacheControllerResult, mesi::MesiCacheControllerResultKind,
    mesi_bank::MesiCacheBank, moesi::MoesiCacheControllerResult,
    moesi::MoesiCacheControllerResultKind, moesi_bank::MoesiCacheBank, msi::CacheControllerResult,
    msi::CacheControllerResultKind,
};
use rem6_memory::{Address, MemoryOperation, MemoryRequest};
use rem6_protocol_chi::ChiState;
use rem6_protocol_mesi::MesiState;
use rem6_protocol_moesi::MoesiState;
use rem6_protocol_msi::MsiState;

pub(crate) fn is_clean_maintenance(request: &MemoryRequest) -> bool {
    matches!(
        request.operation(),
        MemoryOperation::CleanShared | MemoryOperation::Invalidate
    )
}

pub(crate) fn msi_result(
    bank: &MsiCacheBank,
    request: MemoryRequest,
    line: Address,
) -> CacheControllerResult {
    CacheControllerResult::new(
        CacheControllerResultKind::Miss,
        bank.state(line).unwrap_or(MsiState::Invalid),
        None,
        Some(request),
        None,
    )
}

pub(crate) fn mesi_result(
    bank: &MesiCacheBank,
    request: MemoryRequest,
    line: Address,
) -> MesiCacheControllerResult {
    MesiCacheControllerResult::new(
        MesiCacheControllerResultKind::Miss,
        bank.state(line).unwrap_or(MesiState::Invalid),
        None,
        Some(request),
        None,
    )
}

pub(crate) fn moesi_result(
    bank: &MoesiCacheBank,
    request: MemoryRequest,
    line: Address,
) -> MoesiCacheControllerResult {
    MoesiCacheControllerResult::new(
        MoesiCacheControllerResultKind::Miss,
        bank.state(line).unwrap_or(MoesiState::Invalid),
        None,
        Some(request),
        None,
    )
}

pub(crate) fn chi_result(
    bank: &ChiCacheBank,
    request: MemoryRequest,
    line: Address,
) -> ChiCacheControllerResult {
    ChiCacheControllerResult::new(
        ChiCacheControllerResultKind::Miss,
        bank.state(line).unwrap_or(ChiState::Invalid),
        None,
        Some(request),
        None,
    )
}
