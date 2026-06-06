use rem6_memory::{MemoryError, MemoryRequest, MemoryResponse, ResponseStatus};
use rem6_transport::TargetOutcome;

pub(crate) fn with_source_attributes(
    downstream: MemoryRequest,
    source: &MemoryRequest,
) -> MemoryRequest {
    let downstream = downstream.with_ordering(source.ordering());
    if source.is_uncacheable() {
        downstream.with_uncacheable_strict_order()
    } else {
        downstream
    }
}

pub(crate) fn uncacheable_fill_outcome(
    original: &MemoryRequest,
    response: MemoryResponse,
) -> Result<TargetOutcome, MemoryError> {
    let response = match response.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(original, response.data().map(<[u8]>::to_vec))?
        }
        ResponseStatus::Retry => MemoryResponse::retry(original),
        ResponseStatus::StoreConditionalFailed => {
            MemoryResponse::store_conditional_failed(original)?
        }
    };
    Ok(TargetOutcome::Respond(response))
}

pub(crate) fn uncacheable_write_response_outcome(
    original: &MemoryRequest,
    response: MemoryResponse,
) -> Result<TargetOutcome, MemoryError> {
    let response = match response.status() {
        ResponseStatus::Completed => {
            MemoryResponse::completed(original, response.data().map(<[u8]>::to_vec))?
        }
        ResponseStatus::Retry => MemoryResponse::retry(original),
        ResponseStatus::StoreConditionalFailed => {
            MemoryResponse::store_conditional_failed(original)?
        }
    };
    Ok(TargetOutcome::Respond(response))
}
