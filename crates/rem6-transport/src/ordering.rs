use std::collections::BTreeSet;

use crate::{
    FabricQosGrantActivity, FabricQosGrantDirection, FabricQosSuppressedRequest,
    FabricQosSuppressionReason,
};
use rem6_fabric::{
    FabricError, FabricPacket, FabricPath, FabricTransaction, FabricTransfer, QosPriority,
    QosQueueArbiter, QosQueuedRequest, QosRequestId, QosRequestorId,
};
use rem6_kernel::Tick;
use rem6_memory::MemoryRequest;

pub(crate) struct OrderedFabricQosRequest {
    transaction_index: usize,
    packet: FabricPacket,
    path: FabricPath,
    memory_request: MemoryRequest,
    requestor: QosRequestorId,
    priority: QosPriority,
}

impl OrderedFabricQosRequest {
    pub(crate) fn new(
        transaction_index: usize,
        packet: FabricPacket,
        path: FabricPath,
        memory_request: MemoryRequest,
        requestor: QosRequestorId,
        priority: QosPriority,
    ) -> Self {
        Self {
            transaction_index,
            packet,
            path,
            memory_request,
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

pub(crate) struct FabricQosTransfer {
    packet: FabricPacket,
    path: FabricPath,
    requestor: QosRequestorId,
    priority: QosPriority,
    order: u64,
}

impl FabricQosTransfer {
    pub(crate) fn new(
        packet: FabricPacket,
        path: FabricPath,
        requestor: QosRequestorId,
        priority: QosPriority,
        order: u64,
    ) -> Self {
        Self {
            packet,
            path,
            requestor,
            priority,
            order,
        }
    }

    pub(crate) fn packet(&self) -> &FabricPacket {
        &self.packet
    }
}

pub(crate) fn transmit_qos_fabric_batch(
    direction: FabricQosGrantDirection,
    now: Tick,
    batch: u64,
    requests: &[FabricQosTransfer],
    fabric: &mut FabricTransaction<'_>,
    arbiter: &mut QosQueueArbiter,
) -> Result<(Vec<FabricTransfer>, Vec<FabricQosGrantActivity>), FabricError> {
    reject_duplicate_transfer_packets(requests)?;
    let mut pending = (0..requests.len()).collect::<Vec<_>>();
    let mut transfers = Vec::with_capacity(requests.len());
    let mut activities = Vec::with_capacity(requests.len());
    while !pending.is_empty() {
        let queue = pending
            .iter()
            .map(|request_index| qos_queued_transfer(&requests[*request_index]))
            .collect::<Vec<_>>();
        let before = arbiter.snapshot();
        let Some(grant) = arbiter.grant(&queue) else {
            return Err(FabricError::QosNoGrant);
        };
        let after = arbiter.snapshot();
        let request_index = pending.remove(grant.queue_index());
        let request = &requests[request_index];
        let transfer = fabric.transmit(now, request.packet.clone(), request.path.clone())?;
        activities.push(FabricQosGrantActivity::new(
            direction,
            now,
            batch,
            activities.len(),
            before.policy(),
            queue,
            Vec::new(),
            grant.queue_index(),
            before.lrg_requestors().to_vec(),
            after.lrg_requestors().to_vec(),
        ));
        transfers.push(transfer);
    }

    Ok((transfers, activities))
}

pub(crate) fn transmit_ordered_qos_fabric_batch(
    now: Tick,
    batch: u64,
    requests: &[OrderedFabricQosRequest],
    fabric: &mut FabricTransaction<'_>,
    arbiter: &mut QosQueueArbiter,
) -> Result<(Vec<FabricTransfer>, Vec<FabricQosGrantActivity>), FabricError> {
    reject_duplicate_packets(requests)?;
    let mut pending = (0..requests.len()).collect::<Vec<_>>();
    let mut transfers = Vec::with_capacity(requests.len());
    let mut activities = Vec::with_capacity(requests.len());
    while !pending.is_empty() {
        let eligible_indexes = eligible_fabric_qos_requests(&pending, requests);
        let queue = pending
            .iter()
            .enumerate()
            .filter(|(index, _)| eligible_indexes.contains(index))
            .map(|(_, request_index)| qos_queued_request(&requests[*request_index]))
            .collect::<Vec<_>>();
        let suppressed = pending
            .iter()
            .enumerate()
            .filter(|(pending_index, _)| !eligible_indexes.contains(pending_index))
            .map(|(_, request_index)| {
                FabricQosSuppressedRequest::new(
                    qos_queued_request(&requests[*request_index]),
                    FabricQosSuppressionReason::MemoryOrder,
                )
            })
            .collect::<Vec<_>>();
        let before = arbiter.snapshot();
        let Some(grant) = arbiter.grant(&queue) else {
            return Err(FabricError::QosNoGrant);
        };
        let after = arbiter.snapshot();
        let request_index = pending.remove(eligible_indexes[grant.queue_index()]);
        let request = &requests[request_index];
        let transfer = fabric.transmit(now, request.packet.clone(), request.path.clone())?;
        activities.push(FabricQosGrantActivity::new(
            FabricQosGrantDirection::Request,
            now,
            batch,
            activities.len(),
            before.policy(),
            queue,
            suppressed,
            grant.queue_index(),
            before.lrg_requestors().to_vec(),
            after.lrg_requestors().to_vec(),
        ));
        transfers.push(transfer);
    }

    Ok((transfers, activities))
}

fn qos_queued_request(request: &OrderedFabricQosRequest) -> QosQueuedRequest {
    QosQueuedRequest::new(
        QosRequestId::new(request.packet.id().get()),
        request.requestor,
        request.priority,
        request.packet.bytes(),
        request.transaction_index as u64,
    )
    .expect("fabric packets always have nonzero bytes")
}

fn qos_queued_transfer(request: &FabricQosTransfer) -> QosQueuedRequest {
    QosQueuedRequest::new(
        QosRequestId::new(request.packet.id().get()),
        request.requestor,
        request.priority,
        request.packet.bytes(),
        request.order,
    )
    .expect("fabric packets always have nonzero bytes")
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

fn reject_duplicate_transfer_packets(requests: &[FabricQosTransfer]) -> Result<(), FabricError> {
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
) -> Vec<usize> {
    let eligible = pending
        .iter()
        .enumerate()
        .filter_map(|(candidate_pending_index, request_index)| {
            let candidate = &requests[*request_index];
            let blocked = pending.iter().any(|other_request_index| {
                let other = &requests[*other_request_index];
                other.transaction_index() < candidate.transaction_index()
                    && other
                        .memory_request
                        .orders_before(&candidate.memory_request)
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
    earlier: &crate::PreparedParallelTransaction,
    later: &crate::PreparedParallelTransaction,
) -> bool {
    earlier.request.orders_before(&later.request)
}
