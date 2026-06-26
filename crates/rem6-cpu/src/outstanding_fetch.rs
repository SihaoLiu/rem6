use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{AccessSize, Address, CacheLineLayout, MemoryRequestId};
use rem6_transport::{MemoryRouteId, TransportEndpointId};

use crate::fetch_event::CpuFetchRecord;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OutstandingFetch {
    pub(crate) tick: Tick,
    pub(crate) partition: PartitionId,
    pub(crate) route: MemoryRouteId,
    pub(crate) endpoint: TransportEndpointId,
    pub(crate) request_id: MemoryRequestId,
    pub(crate) pc: Address,
    pub(crate) size: AccessSize,
    pub(crate) line_layout: CacheLineLayout,
}

impl OutstandingFetch {
    pub(crate) fn clone_without_layout(&self) -> IssuedFetch {
        IssuedFetch {
            partition: self.partition,
            request: self.request_id,
            pc: self.pc,
            size: self.size,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IssuedFetch {
    pub(crate) partition: PartitionId,
    pub(crate) request: MemoryRequestId,
    pub(crate) pc: Address,
    pub(crate) size: AccessSize,
}

impl IssuedFetch {
    pub(crate) fn record(
        &self,
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
    ) -> CpuFetchRecord {
        CpuFetchRecord::new(
            tick,
            self.partition,
            route,
            endpoint,
            self.request,
            self.pc,
            self.size,
        )
    }
}
