use std::collections::BTreeSet;

use crate::PreparedParallelTransaction;
use rem6_fabric::{
    FabricError, FabricModel, FabricPacket, FabricPath, FabricTransfer, QosPriority,
    QosQueueArbiter, QosQueuedRequest, QosRequestId, QosRequestorId,
};
use rem6_kernel::Tick;

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
    earlier.request.orders_before(&later.request)
}
