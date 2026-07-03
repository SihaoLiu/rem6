use rem6_isa_riscv::{MemoryAccessKind, RiscvFenceSet, RiscvMemoryOrdering};
use rem6_kernel::{PartitionId, SchedulerError, Tick};
use rem6_memory::{
    AccessSize, Address, MemoryAccessOrdering, MemoryBarrierSet, MemoryOperation, MemoryRequestId,
};
use rem6_mmio::MmioRoute;
use rem6_transport::{MemoryRouteId, TransportEndpointId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvDataAccessEventKind {
    Issued,
    Completed,
    Retry,
    ConditionalFailed,
    Failed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvDataAccessTarget {
    Memory {
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
    },
    Mmio {
        route: MmioRoute,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvLoadReservation {
    address: Address,
    size: AccessSize,
}

impl RiscvLoadReservation {
    pub const fn new(address: Address, size: AccessSize) -> Self {
        Self { address, size }
    }

    pub const fn address(self) -> Address {
        self.address
    }

    pub const fn size(self) -> AccessSize {
        self.size
    }

    pub fn overlaps(self, address: Address, size: AccessSize) -> bool {
        let reservation_start = self.address.get();
        let reservation_end = reservation_start.saturating_add(self.size.bytes());
        let access_start = address.get();
        let access_end = access_start.saturating_add(size.bytes());
        reservation_start < access_end && access_start < reservation_end
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataAccessRecord {
    tick: Tick,
    partition: PartitionId,
    target: RiscvDataAccessTarget,
    request: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    size: AccessSize,
    physical_address: Address,
}

impl RiscvDataAccessRecord {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tick: Tick,
        partition: PartitionId,
        target: RiscvDataAccessTarget,
        request: MemoryRequestId,
        fetch_request: MemoryRequestId,
        access: MemoryAccessKind,
        size: AccessSize,
        physical_address: Address,
    ) -> Self {
        Self {
            tick,
            partition,
            target,
            request,
            fetch_request,
            access,
            size,
            physical_address,
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn route(&self) -> Option<MemoryRouteId> {
        match &self.target {
            RiscvDataAccessTarget::Memory { route, .. } => Some(*route),
            RiscvDataAccessTarget::Mmio { .. } => None,
        }
    }

    pub fn endpoint(&self) -> Option<&TransportEndpointId> {
        match &self.target {
            RiscvDataAccessTarget::Memory { endpoint, .. } => Some(endpoint),
            RiscvDataAccessTarget::Mmio { .. } => None,
        }
    }

    pub fn target(&self) -> RiscvDataAccessTarget {
        self.target.clone()
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.request
    }

    pub fn fetch_request_id(&self) -> MemoryRequestId {
        self.fetch_request
    }

    pub fn access(&self) -> &MemoryAccessKind {
        &self.access
    }

    pub fn size(&self) -> AccessSize {
        self.size
    }

    pub fn physical_address(&self) -> Address {
        self.physical_address
    }

    pub fn operation(&self) -> MemoryOperation {
        match self.access {
            MemoryAccessKind::Load { .. }
            | MemoryAccessKind::FloatLoad { .. }
            | MemoryAccessKind::VectorLoadUnitStride { .. }
            | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
            | MemoryAccessKind::VectorLoadStrided { .. }
            | MemoryAccessKind::VectorLoadIndexed { .. } => MemoryOperation::ReadShared,
            MemoryAccessKind::LoadReserved { .. } => MemoryOperation::LoadLocked,
            MemoryAccessKind::StoreConditional { .. } => MemoryOperation::StoreConditional,
            MemoryAccessKind::AtomicMemory { .. } => MemoryOperation::Atomic,
            MemoryAccessKind::Store { .. }
            | MemoryAccessKind::FloatStore { .. }
            | MemoryAccessKind::VectorStoreUnitStride { .. }
            | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
            | MemoryAccessKind::VectorStoreStrided { .. }
            | MemoryAccessKind::VectorStoreIndexed { .. } => MemoryOperation::Write,
        }
    }

    pub fn memory_ordering(&self) -> RiscvMemoryOrdering {
        self.access.memory_ordering()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvDataAccessEvent {
    record: RiscvDataAccessRecord,
    kind: RiscvDataAccessEventKind,
    data: Option<Vec<u8>>,
}

impl RiscvDataAccessEvent {
    pub fn issued(record: RiscvDataAccessRecord) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::Issued,
            data: None,
        }
    }

    pub fn completed(record: RiscvDataAccessRecord, data: Option<Vec<u8>>) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::Completed,
            data,
        }
    }

    pub fn retry(record: RiscvDataAccessRecord) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::Retry,
            data: None,
        }
    }

    pub fn conditional_failed(record: RiscvDataAccessRecord) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::ConditionalFailed,
            data: None,
        }
    }

    pub fn failed(record: RiscvDataAccessRecord) -> Self {
        Self {
            record,
            kind: RiscvDataAccessEventKind::Failed,
            data: None,
        }
    }

    pub fn tick(&self) -> Tick {
        self.record.tick()
    }

    pub fn partition(&self) -> PartitionId {
        self.record.partition()
    }

    pub fn route(&self) -> Option<MemoryRouteId> {
        self.record.route()
    }

    pub fn endpoint(&self) -> Option<&TransportEndpointId> {
        self.record.endpoint()
    }

    pub fn target(&self) -> RiscvDataAccessTarget {
        self.record.target()
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.record.request_id()
    }

    pub fn fetch_request_id(&self) -> MemoryRequestId {
        self.record.fetch_request_id()
    }

    pub fn access(&self) -> &MemoryAccessKind {
        self.record.access()
    }

    pub fn size(&self) -> AccessSize {
        self.record.size()
    }

    pub fn physical_address(&self) -> Address {
        self.record.physical_address()
    }

    pub fn operation(&self) -> MemoryOperation {
        self.record.operation()
    }

    pub fn memory_ordering(&self) -> RiscvMemoryOrdering {
        self.record.memory_ordering()
    }

    pub fn kind(&self) -> RiscvDataAccessEventKind {
        self.kind
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

pub(crate) fn memory_request_ordering(access: &MemoryAccessKind) -> MemoryAccessOrdering {
    let ordering = access.memory_ordering();
    MemoryAccessOrdering::new(
        ordering.before().map(memory_barrier_set),
        ordering.after().map(memory_barrier_set),
    )
}

pub(crate) fn validate_parallel_mmio_route(
    route: MmioRoute,
    issue_tick: Tick,
    min_remote_delay: Tick,
    partition_count: u32,
) -> Result<(), SchedulerError> {
    if route.target_partition().index() >= partition_count {
        return Err(SchedulerError::UnknownPartition {
            partition: route.target_partition(),
            partitions: partition_count,
        });
    }

    let request_tick =
        issue_tick
            .checked_add(route.request_latency())
            .ok_or(SchedulerError::TickOverflow {
                now: issue_tick,
                delay: route.request_latency(),
            })?;
    validate_remote_delivery_boundary(
        route.source_partition(),
        route.target_partition(),
        issue_tick,
        request_tick,
        min_remote_delay,
    )?;

    let response_tick =
        request_tick
            .checked_add(route.response_latency())
            .ok_or(SchedulerError::TickOverflow {
                now: request_tick,
                delay: route.response_latency(),
            })?;
    validate_remote_delivery_boundary(
        route.target_partition(),
        route.source_partition(),
        request_tick,
        response_tick,
        min_remote_delay,
    )
}

fn validate_remote_delivery_boundary(
    source: PartitionId,
    target: PartitionId,
    source_tick: Tick,
    delivery_tick: Tick,
    min_remote_delay: Tick,
) -> Result<(), SchedulerError> {
    if target == source {
        return Ok(());
    }
    let minimum_delivery_tick =
        source_tick
            .checked_add(min_remote_delay)
            .ok_or(SchedulerError::TickOverflow {
                now: source_tick,
                delay: min_remote_delay,
            })?;
    if delivery_tick < minimum_delivery_tick {
        return Err(SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
            source,
            target,
            source_tick,
            delivery_tick,
            minimum_delivery_tick,
        });
    }
    Ok(())
}

fn memory_barrier_set(fence: RiscvFenceSet) -> MemoryBarrierSet {
    MemoryBarrierSet::new(fence.read(), fence.write())
}
