use crate::{
    CacheWriteQueue, CacheWriteQueueError, MshrHandle, MshrQueue, MshrQueueError,
    MshrTargetPostFillAction,
};
use rem6_memory::{MemoryOperation, MemoryRequest};

pub(crate) fn downstream_requests_for_mshr(
    mshr: &MshrQueue,
    handle: MshrHandle,
) -> Result<Vec<MemoryRequest>, MshrQueueError> {
    let entry = mshr.entry(handle)?;
    Ok(entry
        .targets()
        .iter()
        .filter(|target| target.post_fill_action() == MshrTargetPostFillAction::ForwardDownstream)
        .map(|target| target.request().clone())
        .collect())
}

pub(crate) fn preflight_downstream_requests<E, V>(
    queue: Option<&CacheWriteQueue>,
    requests: &[MemoryRequest],
    disabled_error: E,
    mut validate_request: V,
) -> Result<(), E>
where
    E: From<CacheWriteQueueError>,
    V: FnMut(&MemoryRequest) -> Result<(), E>,
{
    if requests.is_empty() {
        return Ok(());
    }
    let Some(queue) = queue else {
        return Err(disabled_error);
    };
    for request in requests {
        validate_request(request)?;
        if !matches!(
            request.operation(),
            MemoryOperation::WritebackClean
                | MemoryOperation::WritebackDirty
                | MemoryOperation::CleanEvict
        ) {
            return Err(CacheWriteQueueError::WritebackOperationRequired {
                operation: request.operation(),
            }
            .into());
        }
    }
    if queue.allocated_count() + requests.len() > queue.config().entries() {
        return Err(CacheWriteQueueError::EntrySlotsFull {
            entries: queue.config().entries(),
            reserve: queue.config().reserve(),
        }
        .into());
    }
    Ok(())
}

pub(crate) fn enqueue_downstream_requests<E>(
    queue: Option<&mut CacheWriteQueue>,
    requests: &[MemoryRequest],
    disabled_error: E,
) -> Result<(), E>
where
    E: From<CacheWriteQueueError>,
{
    if requests.is_empty() {
        return Ok(());
    }
    let Some(queue) = queue else {
        return Err(disabled_error);
    };
    for request in requests {
        queue
            .enqueue_writeback(request.clone(), false, 0)
            .map_err(E::from)?;
    }
    Ok(())
}
