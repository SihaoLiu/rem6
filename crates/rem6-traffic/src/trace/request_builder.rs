use rem6_memory::{
    AccessSize, Address, AgentId, ByteMask, CacheLineLayout, MemoryAtomicOp, MemoryRequest,
    MemoryRequestId,
};

use crate::TrafficGeneratorError;

use super::TrafficTraceCommand;

pub(super) fn build_compare_swap_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let compare = vec![0; data_len];
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::compare_swap(id, address, size, compare, data, mask, layout).map_err(Into::into)
}

pub(super) fn build_compare_swap_no_return_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let compare = vec![0; data_len];
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::compare_swap_no_return(id, address, size, compare, data, mask, layout)
        .map_err(Into::into)
}

pub(super) fn build_atomic_swap_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::atomic_with_op(id, address, size, MemoryAtomicOp::Swap, data, mask, layout)
        .map_err(Into::into)
}

pub(super) fn build_atomic_no_return_swap_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::atomic_no_return(id, address, size, MemoryAtomicOp::Swap, data, mask, layout)
        .map_err(Into::into)
}

pub(super) fn build_write_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::write(id, address, size, data, mask, layout).map_err(Into::into)
}

pub(super) fn build_locked_rmw_write_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::locked_rmw_write(id, address, size, data, mask, layout).map_err(Into::into)
}

pub(super) fn build_store_conditional_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::store_conditional(id, address, size, data, mask, layout).map_err(Into::into)
}

pub(super) fn build_store_conditional_fail_request(
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let mask = ByteMask::full(size)?;
    let data_len =
        usize::try_from(mask.len()).expect("byte mask length fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    MemoryRequest::store_conditional_fail(id, address, size, data, mask, layout).map_err(Into::into)
}

pub(super) fn build_writeback_request(
    command: TrafficTraceCommand,
    agent: AgentId,
    id: MemoryRequestId,
    address: Address,
    size: AccessSize,
    layout: CacheLineLayout,
) -> Result<MemoryRequest, TrafficGeneratorError> {
    let data_len =
        usize::try_from(size.bytes()).expect("access size fits usize after construction");
    let data = vec![agent.get() as u8; data_len];
    match command {
        TrafficTraceCommand::WritebackDirty => {
            MemoryRequest::writeback_dirty(id, address, data, layout).map_err(Into::into)
        }
        TrafficTraceCommand::WritebackClean => {
            MemoryRequest::writeback_clean(id, address, data, layout).map_err(Into::into)
        }
        TrafficTraceCommand::WriteClean => {
            MemoryRequest::write_clean(id, address, data, layout).map_err(Into::into)
        }
        _ => unreachable!("writeback builder is only called for writeback trace commands"),
    }
}
