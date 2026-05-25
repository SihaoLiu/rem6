use std::collections::BTreeSet;

use rem6_fabric::{
    FabricError, FabricModel, FabricPacket, FabricPath, FabricTransfer, QosPriority,
    QosQueueArbiter, QosQueuedRequest, QosRequestId, QosRequestorId,
};
use rem6_kernel::Tick;
use rem6_memory::{MemoryBarrierSet, MemoryOperation};

use crate::PreparedParallelTransaction;

pub(crate) struct OrderedFabricQosRequest {
    transaction_index: usize,
    packet: FabricPacket,
    path: FabricPath,
    requestor: QosRequestorId,
    priority: QosPriority,
}

impl OrderedFabricQosRequest {
    pub(crate) fn new(
        transaction_index: usize,
        packet: FabricPacket,
        path: FabricPath,
        requestor: QosRequestorId,
        priority: QosPriority,
    ) -> Self {
        Self {
            transaction_index,
            packet,
            path,
            requestor,
            priority,
        }
    }

    pub(crate) const fn transaction_index(&self) -> usize {
        self.transaction_index
    }

    pub(crate) fn packet(&self) -> &FabricPacket {
        &self.packet
    }

    pub(crate) fn path(&self) -> &FabricPath {
        &self.path
    }
}

pub(crate) fn transmit_ordered_qos_fabric_batch(
    now: Tick,
    transactions: &[&PreparedParallelTransaction],
    requests: &[OrderedFabricQosRequest],
    fabric: &mut FabricModel,
    arbiter: &mut QosQueueArbiter,
) -> Result<Vec<FabricTransfer>, FabricError> {
    reject_duplicate_packets(requests)?;

    let mut pending = (0..requests.len()).collect::<Vec<_>>();
    let mut transfers = Vec::with_capacity(requests.len());
    while !pending.is_empty() {
        let eligible_indexes = eligible_fabric_qos_requests(&pending, requests, transactions);
        let queue = pending
            .iter()
            .enumerate()
            .filter(|(index, _)| eligible_indexes.contains(index))
            .map(|(_, request_index)| {
                let request = &requests[*request_index];
                QosQueuedRequest::new(
                    QosRequestId::new(request.packet.id().get()),
                    request.requestor,
                    request.priority,
                    request.packet.bytes(),
                    request.transaction_index as u64,
                )
                .expect("fabric packets always have nonzero bytes")
            })
            .collect::<Vec<_>>();
        let Some(grant) = arbiter.grant(&queue) else {
            return Err(FabricError::QosNoGrant);
        };
        let request_index = pending.remove(eligible_indexes[grant.queue_index()]);
        let request = &requests[request_index];
        transfers.push(fabric.transmit(now, request.packet.clone(), request.path.clone())?);
    }

    Ok(transfers)
}

fn reject_duplicate_packets(requests: &[OrderedFabricQosRequest]) -> Result<(), FabricError> {
    let mut seen = BTreeSet::new();
    for request in requests {
        if !seen.insert(request.packet().id()) {
            return Err(FabricError::DuplicatePacketInBatch {
                packet: request.packet().id(),
            });
        }
    }
    Ok(())
}

fn eligible_fabric_qos_requests(
    pending: &[usize],
    requests: &[OrderedFabricQosRequest],
    transactions: &[&PreparedParallelTransaction],
) -> Vec<usize> {
    let eligible = pending
        .iter()
        .enumerate()
        .filter_map(|(candidate_pending_index, request_index)| {
            let candidate = &requests[*request_index];
            let candidate_transaction = transactions[candidate.transaction_index()];
            let blocked = pending.iter().any(|other_request_index| {
                let other = &requests[*other_request_index];
                other.transaction_index() < candidate.transaction_index()
                    && transaction_orders_before(
                        transactions[other.transaction_index()],
                        candidate_transaction,
                    )
            });
            (!blocked).then_some(candidate_pending_index)
        })
        .collect::<Vec<_>>();
    debug_assert!(
        !eligible.is_empty(),
        "oldest fabric QoS request is always ordering-eligible"
    );
    eligible
}

pub(crate) fn transaction_orders_before(
    earlier: &PreparedParallelTransaction,
    later: &PreparedParallelTransaction,
) -> bool {
    same_request_agent(earlier, later)
        && (barrier_orders_before(
            later.request.ordering().before(),
            earlier.request.operation(),
        ) || barrier_orders_after(
            earlier.request.ordering().after(),
            later.request.operation(),
        ))
}

fn same_request_agent(
    first: &PreparedParallelTransaction,
    second: &PreparedParallelTransaction,
) -> bool {
    first.request.id().agent() == second.request.id().agent()
}

fn barrier_orders_before(barrier: Option<MemoryBarrierSet>, operation: MemoryOperation) -> bool {
    barrier.is_some_and(|barrier| barrier_matches_operation(barrier, operation))
}

fn barrier_orders_after(barrier: Option<MemoryBarrierSet>, operation: MemoryOperation) -> bool {
    barrier.is_some_and(|barrier| barrier_matches_operation(barrier, operation))
}

fn barrier_matches_operation(barrier: MemoryBarrierSet, operation: MemoryOperation) -> bool {
    (barrier.read() && operation_reads_for_ordering(operation))
        || (barrier.write() && operation_writes_for_ordering(operation))
}

fn operation_reads_for_ordering(operation: MemoryOperation) -> bool {
    matches!(
        operation,
        MemoryOperation::InstructionFetch
            | MemoryOperation::ReadShared
            | MemoryOperation::ReadUnique
            | MemoryOperation::Atomic
            | MemoryOperation::PrefetchRead
    )
}

fn operation_writes_for_ordering(operation: MemoryOperation) -> bool {
    matches!(
        operation,
        MemoryOperation::ReadUnique
            | MemoryOperation::Write
            | MemoryOperation::Upgrade
            | MemoryOperation::Atomic
            | MemoryOperation::PrefetchWrite
            | MemoryOperation::WritebackClean
            | MemoryOperation::WritebackDirty
            | MemoryOperation::CleanEvict
            | MemoryOperation::Invalidate
    )
}
