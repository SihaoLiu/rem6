use std::collections::BTreeMap;

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
pub struct DramQosSchedulingPolicy {
    turnaround: DramQosTurnaroundPolicy,
    priority_escalation: bool,
}

impl DramQosSchedulingPolicy {
    pub const fn new() -> Self {
        Self {
            turnaround: DramQosTurnaroundPolicy::RequestOrder,
            priority_escalation: false,
        }
    }

    pub const fn with_turnaround(mut self, turnaround: DramQosTurnaroundPolicy) -> Self {
        self.turnaround = turnaround;
        self
    }

    pub const fn with_priority_escalation(mut self) -> Self {
        self.priority_escalation = true;
        self
    }

    pub const fn turnaround(self) -> DramQosTurnaroundPolicy {
        self.turnaround
    }

    pub const fn priority_escalation(self) -> bool {
        self.priority_escalation
    }
}

impl Default for DramQosSchedulingPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramQosRequest<'a> {
    request: &'a MemoryRequest,
    requestor: QosRequestorId,
    assigned_priority: QosPriority,
    effective_priority: QosPriority,
    order: u64,
}

impl<'a> DramQosRequest<'a> {
    pub fn new(request: &'a MemoryRequest, priority: QosPriority, order: u64) -> Self {
        Self {
            request,
            requestor: QosRequestorId::new(request.id().agent().get()),
            assigned_priority: priority,
            effective_priority: priority,
            order,
        }
    }

    pub const fn with_requestor(mut self, requestor: QosRequestorId) -> Self {
        self.requestor = requestor;
        self
    }

    pub const fn with_priority(mut self, priority: QosPriority) -> Self {
        self.effective_priority = priority;
        self
    }

    pub const fn request(&self) -> &'a MemoryRequest {
        self.request
    }

    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub const fn priority(&self) -> QosPriority {
        self.effective_priority
    }

    pub const fn assigned_priority(&self) -> QosPriority {
        self.assigned_priority
    }

    pub const fn effective_priority(&self) -> QosPriority {
        self.effective_priority
    }

    pub const fn order(&self) -> u64 {
        self.order
    }

    pub(crate) fn queued_request(&self) -> Result<QosQueuedRequest, QosError> {
        QosQueuedRequest::new(
            QosRequestId::new(self.request.id().sequence()),
            self.requestor,
            self.effective_priority,
            self.request.size().bytes(),
            self.order,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramQosAccess {
    requestor: QosRequestorId,
    assigned_priority: QosPriority,
    effective_priority: QosPriority,
    bytes: u64,
}

impl DramQosAccess {
    pub(crate) fn from_request(request: &DramQosRequest<'_>) -> Self {
        Self {
            requestor: request.requestor(),
            assigned_priority: request.assigned_priority(),
            effective_priority: request.effective_priority(),
            bytes: request.request().size().bytes(),
        }
    }

    pub const fn requestor(self) -> QosRequestorId {
        self.requestor
    }

    pub const fn assigned_priority(self) -> QosPriority {
        self.assigned_priority
    }

    pub const fn effective_priority(self) -> QosPriority {
        self.effective_priority
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }

    pub const fn escalated(self) -> bool {
        self.assigned_priority.get() > self.effective_priority.get()
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
    schedule_qos_batch_with_policy(
        controller,
        arrival_cycle,
        requests,
        arbiter,
        DramQosSchedulingPolicy::new().with_turnaround(turnaround),
    )
}

pub(crate) fn schedule_qos_batch_with_policy<'a, I>(
    controller: &mut DramController,
    arrival_cycle: u64,
    requests: I,
    arbiter: &mut QosQueueArbiter,
    policy: DramQosSchedulingPolicy,
) -> Result<Vec<DramAccess>, DramError>
where
    I: IntoIterator<Item = DramQosRequest<'a>>,
{
    let mut requests: Vec<DramQosRequest<'a>> = requests.into_iter().collect();
    if policy.priority_escalation() {
        escalate_requestor_priorities(&mut requests);
    }
    let ordered = match policy.turnaround() {
        DramQosTurnaroundPolicy::RequestOrder => {
            order_requests(requests, arbiter).map_err(|source| DramError::Qos { source })?
        }
        DramQosTurnaroundPolicy::PreferCurrentDirection => {
            order_requests_with_current_direction(controller, requests, arbiter)?
        }
    };
    ordered
        .into_iter()
        .map(|request| {
            controller.schedule_with_qos(
                arrival_cycle,
                request.request(),
                Some(DramQosAccess::from_request(&request)),
            )
        })
        .collect()
}

fn escalate_requestor_priorities(requests: &mut [DramQosRequest<'_>]) {
    let mut highest_by_requestor = BTreeMap::<QosRequestorId, QosPriority>::new();
    for request in requests.iter() {
        highest_by_requestor
            .entry(request.requestor())
            .and_modify(|priority| *priority = (*priority).min(request.priority()))
            .or_insert(request.priority());
    }
    for request in requests.iter_mut() {
        if let Some(priority) = highest_by_requestor.get(&request.requestor()).copied() {
            *request = request.with_priority(priority);
        }
    }
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
