use rem6_fabric::{
    QosError, QosPriority, QosQueueArbiter, QosQueuedRequest, QosRequestId, QosRequestorId,
};
use rem6_memory::MemoryRequest;

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

    fn queued_request(&self) -> Result<QosQueuedRequest, QosError> {
        QosQueuedRequest::new(
            QosRequestId::new(self.request.id().sequence()),
            self.requestor,
            self.priority,
            self.request.size().bytes(),
            self.order,
        )
    }
}

pub(crate) fn order_requests<'a>(
    requests: Vec<DramQosRequest<'a>>,
    arbiter: &mut QosQueueArbiter,
) -> Result<Vec<DramQosRequest<'a>>, QosError> {
    let mut pending = requests;
    let mut ordered = Vec::with_capacity(pending.len());
    while !pending.is_empty() {
        let queue = pending
            .iter()
            .map(DramQosRequest::queued_request)
            .collect::<Result<Vec<_>, _>>()?;
        let grant = arbiter
            .grant(&queue)
            .expect("nonempty DRAM QoS queue must produce a grant");
        ordered.push(pending.remove(grant.queue_index()));
    }
    Ok(ordered)
}
