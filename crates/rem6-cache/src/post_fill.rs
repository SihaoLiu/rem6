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
    let mut queued_requests = 0;
    for request in requests {
        validate_request(request)?;
        if is_queue_backed(request)? {
            queued_requests += 1;
        }
    }
    if queued_requests == 0 {
        return Ok(());
    }
    let Some(queue) = queue else {
        return Err(disabled_error);
    };
    if queue.allocated_count() + queued_requests > queue.config().entries() {
        return Err(CacheWriteQueueError::EntrySlotsFull {
            entries: queue.config().entries(),
            reserve: queue.config().reserve(),
        }
        .into());
    }
    Ok(())
}

fn is_queue_backed<E>(request: &MemoryRequest) -> Result<bool, E>
where
    E: From<CacheWriteQueueError>,
{
    match request.operation() {
        MemoryOperation::WriteClean
        | MemoryOperation::WritebackClean
        | MemoryOperation::WritebackDirty
        | MemoryOperation::CleanEvict => Ok(true),
        MemoryOperation::Invalidate => Ok(false),
        operation => Err(CacheWriteQueueError::WritebackOperationRequired { operation }.into()),
    }
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
    let mut queue = queue;
    for request in requests {
        if is_queue_backed(request)? {
            let Some(queue) = queue.as_deref_mut() else {
                return Err(disabled_error);
            };
            queue
                .enqueue_writeback(request.clone(), false, 0)
                .map_err(E::from)?;
        }
    }
    Ok(())
}
