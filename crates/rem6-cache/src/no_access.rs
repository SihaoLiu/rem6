use rem6_memory::{MemoryRequest, MemoryResponse};
use rem6_protocol_chi::ChiState;
use rem6_protocol_mesi::MesiState;
use rem6_protocol_moesi::MoesiState;
use rem6_protocol_msi::MsiState;
use rem6_transport::TargetOutcome;

use crate::{
    CacheControllerError, CacheControllerResult, CacheControllerResultKind, ChiCacheBankError,
    ChiCacheController, ChiCacheControllerError, ChiCacheControllerResult,
    ChiCacheControllerResultKind, MesiCacheBankError, MesiCacheController,
    MesiCacheControllerError, MesiCacheControllerResult, MesiCacheControllerResultKind,
    MoesiCacheBankError, MoesiCacheController, MoesiCacheControllerError,
    MoesiCacheControllerResult, MoesiCacheControllerResultKind, MsiCacheBankError,
    MsiCacheController,
};

pub(crate) fn msi(
    request: &MemoryRequest,
    controller: Option<&MsiCacheController>,
) -> Result<CacheControllerResult, MsiCacheBankError> {
    let state = controller.map_or(MsiState::Invalid, MsiCacheController::state);
    let response =
        MemoryResponse::completed(request, None).map_err(CacheControllerError::Memory)?;
    Ok(CacheControllerResult::new(
        CacheControllerResultKind::Hit,
        state,
        None,
        None,
        Some(TargetOutcome::Respond(response)),
    ))
}

pub(crate) fn mesi(
    request: &MemoryRequest,
    controller: Option<&MesiCacheController>,
) -> Result<MesiCacheControllerResult, MesiCacheBankError> {
    let state = controller.map_or(MesiState::Invalid, MesiCacheController::state);
    let response =
        MemoryResponse::completed(request, None).map_err(MesiCacheControllerError::Memory)?;
    Ok(MesiCacheControllerResult::new(
        MesiCacheControllerResultKind::Hit,
        state,
        None,
        None,
        Some(TargetOutcome::Respond(response)),
    ))
}

pub(crate) fn moesi(
    request: &MemoryRequest,
    controller: Option<&MoesiCacheController>,
) -> Result<MoesiCacheControllerResult, MoesiCacheBankError> {
    let state = controller.map_or(MoesiState::Invalid, MoesiCacheController::state);
    let response =
        MemoryResponse::completed(request, None).map_err(MoesiCacheControllerError::Memory)?;
    Ok(MoesiCacheControllerResult::new(
        MoesiCacheControllerResultKind::Hit,
        state,
        None,
        None,
        Some(TargetOutcome::Respond(response)),
    ))
}

pub(crate) fn chi(
    request: &MemoryRequest,
    controller: Option<&ChiCacheController>,
) -> Result<ChiCacheControllerResult, ChiCacheBankError> {
    let state = controller.map_or(ChiState::Invalid, ChiCacheController::state);
    let response =
        MemoryResponse::completed(request, None).map_err(ChiCacheControllerError::Memory)?;
    Ok(ChiCacheControllerResult::new(
        ChiCacheControllerResultKind::Hit,
        state,
        None,
        None,
        Some(TargetOutcome::Respond(response)),
    ))
}
