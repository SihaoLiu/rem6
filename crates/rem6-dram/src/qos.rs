use rem6_fabric::{
    QosError, QosPriority, QosQueueArbiter, QosQueuedRequest, QosRequestId, QosRequestorId,
};
use rem6_memory::MemoryRequest;

use crate::{DramAccess, DramAccessKind, DramController, DramError};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DramQosTurnaroundPolicy {
    RequestOrder,
    PreferCurrentDirection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramQosRequest<'a> {
    request: &'a MemoryRequest,
    requestor: QosRequestorId,
    priority: QosPriority,
    order: u64,
}

impl<'a> DramQosRequest<'a> {
    pub fn new(request: &'a MemoryRequest, priority: QosPriority, order: u64) -> Self {
        Self {
            request,
            requestor: QosRequestorId::new(request.id().agent().get()),
            priority,
            order,
        }
    }

    pub const fn with_requestor(mut self, requestor: QosRequestorId) -> Self {
        self.requestor = requestor;
        self
    }

    pub const fn request(&self) -> &'a MemoryRequest {
        self.request
    }

    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub const fn priority(&self) -> QosPriority {
        self.priority
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub(crate) fn queued_request(&self) -> Result<QosQueuedRequest, QosError> {
        QosQueuedRequest::new(
            QosRequestId::new(self.request.id().sequence()),
            self.requestor,
            self.priority,
            self.request.size().bytes(),
            self.order,
        )
    }
}

pub(crate) fn grant_index_for_candidates<'a>(
    pending: &[DramQosRequest<'a>],
    candidates: &[usize],
    arbiter: &mut QosQueueArbiter,
) -> Result<usize, QosError> {
    let queue = candidates
        .iter()
        .map(|index| pending[*index].queued_request())
        .collect::<Result<Vec<_>, _>>()?;
    let grant = arbiter
        .grant(&queue)
        .expect("nonempty DRAM QoS queue must produce a grant");
    Ok(candidates[grant.queue_index()])
}

pub(crate) fn order_requests<'a>(
    requests: Vec<DramQosRequest<'a>>,
    arbiter: &mut QosQueueArbiter,
) -> Result<Vec<DramQosRequest<'a>>, QosError> {
    let mut pending = requests;
    let mut ordered = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let candidates = (0..pending.len()).collect::<Vec<_>>();
        let grant_index = grant_index_for_candidates(&pending, &candidates, arbiter)?;
        ordered.push(pending.remove(grant_index));
    }
    Ok(ordered)
}

pub(crate) fn schedule_qos_batch<'a, I>(
    controller: &mut DramController,
    arrival_cycle: u64,
    requests: I,
    arbiter: &mut QosQueueArbiter,
    turnaround: DramQosTurnaroundPolicy,
) -> Result<Vec<DramAccess>, DramError>
where
    I: IntoIterator<Item = DramQosRequest<'a>>,
{
    let requests = requests.into_iter().collect();
    let ordered = match turnaround {
        DramQosTurnaroundPolicy::RequestOrder => {
            order_requests(requests, arbiter).map_err(|source| DramError::Qos { source })?
        }
        DramQosTurnaroundPolicy::PreferCurrentDirection => {
            order_requests_with_current_direction(controller, requests, arbiter)?
        }
    };
    ordered
        .into_iter()
        .map(|request| controller.schedule(arrival_cycle, request.request()))
        .collect()
}

fn order_requests_with_current_direction<'a>(
    controller: &DramController,
    requests: Vec<DramQosRequest<'a>>,
    arbiter: &mut QosQueueArbiter,
) -> Result<Vec<DramQosRequest<'a>>, DramError> {
    let mut pending = requests;
    let mut ordered = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let candidates = current_direction_candidates(controller, &pending)?;
        let grant_index = grant_index_for_candidates(&pending, &candidates, arbiter)
            .map_err(|source| DramError::Qos { source })?;
        ordered.push(pending.remove(grant_index));
    }
    Ok(ordered)
}

fn current_direction_candidates<'a>(
    controller: &DramController,
    pending: &[DramQosRequest<'a>],
) -> Result<Vec<usize>, DramError> {
    let highest_priority = pending
        .iter()
        .map(DramQosRequest::priority)
        .min()
        .expect("candidate selection is called only with pending requests");
    let mut highest = Vec::new();
    let mut matching_direction = Vec::new();
    for (index, request) in pending.iter().enumerate() {
        if request.priority() != highest_priority {
            continue;
        }
        highest.push(index);
        let kind = DramAccessKind::from_operation(request.request())?;
        let decoded = controller
            .geometry
            .decode_request(controller.parallel_port_count(), request.request())?;
        let port = controller.ports[decoded.parallel_port as usize];
        if port.last_access_kind() == Some(kind) {
            matching_direction.push(index);
        }
    }

    if matching_direction.is_empty() {
        Ok(highest)
    } else {
        Ok(matching_direction)
    }
}
