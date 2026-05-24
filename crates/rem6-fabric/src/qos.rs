use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QosRequestorId(u32);

impl QosRequestorId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QosRequestId(u64);

impl QosRequestId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct QosPriority(u8);

impl QosPriority {
    pub const fn new(value: u8) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum QosError {
    ZeroPriorityLevels,
    PriorityOutOfRange {
        priority: QosPriority,
        priority_levels: u8,
    },
    DuplicateRequestorPriority {
        requestor: QosRequestorId,
    },
    ZeroRequestBytes,
}

impl fmt::Display for QosError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPriorityLevels => write!(formatter, "QoS priority levels must be nonzero"),
            Self::PriorityOutOfRange {
                priority,
                priority_levels,
            } => write!(
                formatter,
                "QoS priority {} is outside {} configured levels",
                priority.get(),
                priority_levels
            ),
            Self::DuplicateRequestorPriority { requestor } => write!(
                formatter,
                "QoS requestor {} has more than one fixed priority",
                requestor.get()
            ),
            Self::ZeroRequestBytes => write!(formatter, "QoS request bytes must be nonzero"),
        }
    }
}

impl Error for QosError {}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QosFixedPriorityPolicy {
    priority_levels: u8,
    default_priority: QosPriority,
    requestor_priorities: BTreeMap<QosRequestorId, QosPriority>,
}

impl QosFixedPriorityPolicy {
    pub fn new(priority_levels: u8, default_priority: QosPriority) -> Result<Self, QosError> {
        validate_priority_levels(priority_levels)?;
        validate_priority(priority_levels, default_priority)?;

        Ok(Self {
            priority_levels,
            default_priority,
            requestor_priorities: BTreeMap::new(),
        })
    }

    pub const fn priority_levels(&self) -> u8 {
        self.priority_levels
    }

    pub const fn default_priority(&self) -> QosPriority {
        self.default_priority
    }

    pub fn with_requestor_priority(
        mut self,
        requestor: QosRequestorId,
        priority: QosPriority,
    ) -> Result<Self, QosError> {
        validate_priority(self.priority_levels, priority)?;
        if self.requestor_priorities.contains_key(&requestor) {
            return Err(QosError::DuplicateRequestorPriority { requestor });
        }

        self.requestor_priorities.insert(requestor, priority);
        Ok(self)
    }

    pub fn priority_for(&self, requestor: QosRequestorId, _bytes: u64) -> QosPriority {
        self.requestor_priorities
            .get(&requestor)
            .copied()
            .unwrap_or(self.default_priority)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QosQueuePolicyKind {
    Fifo,
    Lifo,
    LeastRecentlyGranted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QosQueuedRequest {
    request_id: QosRequestId,
    requestor: QosRequestorId,
    priority: QosPriority,
    bytes: u64,
    order: u64,
}

impl QosQueuedRequest {
    pub fn new(
        request_id: QosRequestId,
        requestor: QosRequestorId,
        priority: QosPriority,
        bytes: u64,
        order: u64,
    ) -> Result<Self, QosError> {
        if bytes == 0 {
            return Err(QosError::ZeroRequestBytes);
        }

        Ok(Self {
            request_id,
            requestor,
            priority,
            bytes,
            order,
        })
    }

    pub const fn request_id(&self) -> QosRequestId {
        self.request_id
    }

    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub const fn priority(&self) -> QosPriority {
        self.priority
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }

    pub const fn order(&self) -> u64 {
        self.order
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QosGrant {
    queue_index: usize,
    request_id: QosRequestId,
    requestor: QosRequestorId,
    priority: QosPriority,
    bytes: u64,
}

impl QosGrant {
    fn from_request(queue_index: usize, request: &QosQueuedRequest) -> Self {
        Self {
            queue_index,
            request_id: request.request_id,
            requestor: request.requestor,
            priority: request.priority,
            bytes: request.bytes,
        }
    }

    pub const fn queue_index(&self) -> usize {
        self.queue_index
    }

    pub const fn request_id(&self) -> QosRequestId {
        self.request_id
    }

    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub const fn priority(&self) -> QosPriority {
        self.priority
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QosQueueArbiterSnapshot {
    policy: QosQueuePolicyKind,
    lrg_requestors: Vec<QosRequestorId>,
}

impl QosQueueArbiterSnapshot {
    pub const fn policy(&self) -> QosQueuePolicyKind {
        self.policy
    }

    pub fn lrg_requestors(&self) -> &[QosRequestorId] {
        &self.lrg_requestors
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QosQueueArbiter {
    policy: QosQueuePolicyKind,
    lrg_requestors: VecDeque<QosRequestorId>,
}

impl QosQueueArbiter {
    pub fn new(policy: QosQueuePolicyKind) -> Self {
        Self {
            policy,
            lrg_requestors: VecDeque::new(),
        }
    }

    pub const fn policy(&self) -> QosQueuePolicyKind {
        self.policy
    }

    pub fn grant(&mut self, queue: &[QosQueuedRequest]) -> Option<QosGrant> {
        let highest_priority = queue.iter().map(QosQueuedRequest::priority).min()?;

        match self.policy {
            QosQueuePolicyKind::Fifo => fifo_grant(queue, highest_priority),
            QosQueuePolicyKind::Lifo => lifo_grant(queue, highest_priority),
            QosQueuePolicyKind::LeastRecentlyGranted => self.lrg_grant(queue, highest_priority),
        }
    }

    pub fn snapshot(&self) -> QosQueueArbiterSnapshot {
        QosQueueArbiterSnapshot {
            policy: self.policy,
            lrg_requestors: self.lrg_requestors.iter().copied().collect(),
        }
    }

    pub fn restore(&mut self, snapshot: QosQueueArbiterSnapshot) {
        self.policy = snapshot.policy;
        self.lrg_requestors = snapshot.lrg_requestors.into();
    }

    fn lrg_grant(
        &mut self,
        queue: &[QosQueuedRequest],
        highest_priority: QosPriority,
    ) -> Option<QosGrant> {
        self.register_lrg_requestors(queue);
        let first_by_requestor = first_eligible_by_requestor(queue, highest_priority);

        for requestor in self.lrg_requestors.iter().copied() {
            if let Some(index) = first_by_requestor.get(&requestor) {
                let grant = QosGrant::from_request(*index, &queue[*index]);
                self.rotate_lrg_after_grant(requestor);
                return Some(grant);
            }
        }

        None
    }

    fn register_lrg_requestors(&mut self, queue: &[QosQueuedRequest]) {
        for request in queue {
            if !self.lrg_requestors.contains(&request.requestor) {
                self.lrg_requestors.push_back(request.requestor);
            }
        }
    }

    fn rotate_lrg_after_grant(&mut self, requestor: QosRequestorId) {
        let Some(index) = self
            .lrg_requestors
            .iter()
            .position(|stored| *stored == requestor)
        else {
            return;
        };
        self.lrg_requestors.remove(index);
        self.lrg_requestors.push_back(requestor);
    }
}

fn fifo_grant(queue: &[QosQueuedRequest], priority: QosPriority) -> Option<QosGrant> {
    queue
        .iter()
        .enumerate()
        .filter(|(_, request)| request.priority == priority)
        .min_by_key(|(index, request)| (request.order, *index))
        .map(|(index, request)| QosGrant::from_request(index, request))
}

fn lifo_grant(queue: &[QosQueuedRequest], priority: QosPriority) -> Option<QosGrant> {
    queue
        .iter()
        .enumerate()
        .filter(|(_, request)| request.priority == priority)
        .max_by_key(|(index, request)| (request.order, *index))
        .map(|(index, request)| QosGrant::from_request(index, request))
}

fn first_eligible_by_requestor(
    queue: &[QosQueuedRequest],
    priority: QosPriority,
) -> BTreeMap<QosRequestorId, usize> {
    let mut first_by_requestor = BTreeMap::new();
    for (index, request) in queue.iter().enumerate() {
        if request.priority == priority {
            first_by_requestor.entry(request.requestor).or_insert(index);
        }
    }
    first_by_requestor
}

fn validate_priority_levels(priority_levels: u8) -> Result<(), QosError> {
    if priority_levels == 0 {
        return Err(QosError::ZeroPriorityLevels);
    }
    Ok(())
}

fn validate_priority(priority_levels: u8, priority: QosPriority) -> Result<(), QosError> {
    if priority.get() >= priority_levels {
        return Err(QosError::PriorityOutOfRange {
            priority,
            priority_levels,
        });
    }
    Ok(())
}
