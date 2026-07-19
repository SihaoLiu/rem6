use std::sync::{Arc, Mutex};

use rem6_fabric::{QosQueueArbiter, QosQueuePolicyKind, QosQueuedRequest, QosRequestorId};
use rem6_kernel::Tick;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FabricQosGrantDirection {
    Request,
    Response,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FabricQosSuppressionReason {
    MemoryOrder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricQosSuppressedRequest {
    request: QosQueuedRequest,
    reason: FabricQosSuppressionReason,
}

impl FabricQosSuppressedRequest {
    pub(crate) fn new(request: QosQueuedRequest, reason: FabricQosSuppressionReason) -> Self {
        Self { request, reason }
    }

    pub fn request(&self) -> &QosQueuedRequest {
        &self.request
    }

    pub const fn reason(&self) -> FabricQosSuppressionReason {
        self.reason
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricQosGrantActivity {
    direction: FabricQosGrantDirection,
    tick: Tick,
    batch: u64,
    grant_index: usize,
    policy: QosQueuePolicyKind,
    candidates: Vec<QosQueuedRequest>,
    suppressed: Vec<FabricQosSuppressedRequest>,
    selected_queue_index: usize,
    lrg_requestors_before: Vec<QosRequestorId>,
    lrg_requestors_after: Vec<QosRequestorId>,
}

#[derive(Clone, Debug)]
pub struct SharedFabricQosState {
    pub(crate) inner: Arc<Mutex<FabricQosState>>,
    pub(crate) response_batches: crate::response_qos::ResponseQosBatches,
}

impl SharedFabricQosState {
    pub fn new(arbiter: QosQueueArbiter) -> Self {
        let response_arbiter = QosQueueArbiter::new(arbiter.policy());
        Self {
            inner: Arc::new(Mutex::new(FabricQosState {
                request_arbiter: arbiter,
                response_arbiter,
                activity: FabricQosActivityLog::default(),
            })),
            response_batches: crate::response_qos::ResponseQosBatches::default(),
        }
    }
}

#[derive(Debug)]
pub(crate) struct FabricQosState {
    pub(crate) request_arbiter: QosQueueArbiter,
    pub(crate) response_arbiter: QosQueueArbiter,
    pub(crate) activity: FabricQosActivityLog,
}

impl FabricQosGrantActivity {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        direction: FabricQosGrantDirection,
        tick: Tick,
        batch: u64,
        grant_index: usize,
        policy: QosQueuePolicyKind,
        candidates: Vec<QosQueuedRequest>,
        suppressed: Vec<FabricQosSuppressedRequest>,
        selected_queue_index: usize,
        lrg_requestors_before: Vec<QosRequestorId>,
        lrg_requestors_after: Vec<QosRequestorId>,
    ) -> Self {
        assert!(
            selected_queue_index < candidates.len(),
            "fabric QoS selected queue index must identify a candidate"
        );
        Self {
            direction,
            tick,
            batch,
            grant_index,
            policy,
            candidates,
            suppressed,
            selected_queue_index,
            lrg_requestors_before,
            lrg_requestors_after,
        }
    }

    pub const fn direction(&self) -> FabricQosGrantDirection {
        self.direction
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn batch(&self) -> u64 {
        self.batch
    }

    pub const fn grant_index(&self) -> usize {
        self.grant_index
    }

    pub const fn policy(&self) -> QosQueuePolicyKind {
        self.policy
    }

    pub fn candidates(&self) -> &[QosQueuedRequest] {
        &self.candidates
    }

    pub fn suppressed(&self) -> &[FabricQosSuppressedRequest] {
        &self.suppressed
    }

    pub const fn selected_queue_index(&self) -> usize {
        self.selected_queue_index
    }

    pub fn grant(&self) -> &QosQueuedRequest {
        &self.candidates[self.selected_queue_index]
    }

    pub fn lrg_requestors_before(&self) -> &[QosRequestorId] {
        &self.lrg_requestors_before
    }

    pub fn lrg_requestors_after(&self) -> &[QosRequestorId] {
        &self.lrg_requestors_after
    }
}

#[derive(Debug, Default)]
pub(crate) struct FabricQosActivityLog {
    next_batch: u64,
    grants: Vec<FabricQosGrantActivity>,
}

impl FabricQosActivityLog {
    pub(crate) const fn next_batch(&self) -> u64 {
        self.next_batch
    }

    pub(crate) fn commit_batch(&mut self, batch: u64, grants: Vec<FabricQosGrantActivity>) {
        assert_eq!(batch, self.next_batch, "fabric QoS batches commit in order");
        self.next_batch = self
            .next_batch
            .checked_add(1)
            .expect("fabric QoS batch id overflow");
        self.grants.extend(grants);
    }

    pub(crate) fn grants(&self) -> &[FabricQosGrantActivity] {
        &self.grants
    }
}
