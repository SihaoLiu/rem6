use std::cmp::Ordering;
use std::collections::{BTreeMap, VecDeque};
use std::error::Error;
use std::fmt;

use crate::{FabricPacket, FabricPath};

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
    DuplicateRequestorScore {
        requestor: QosRequestorId,
    },
    TooManyProportionalFairRequestors {
        requestor_count: usize,
        priority_levels: u8,
    },
    UnknownProportionalFairRequestor {
        requestor: QosRequestorId,
    },
    InvalidProportionalFairWeight {
        weight_bits: u64,
    },
    InvalidProportionalFairScore {
        requestor: QosRequestorId,
        score_bits: u64,
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
            Self::DuplicateRequestorScore { requestor } => write!(
                formatter,
                "QoS requestor {} has more than one proportional-fair score",
                requestor.get()
            ),
            Self::TooManyProportionalFairRequestors {
                requestor_count,
                priority_levels,
            } => write!(
                formatter,
                "QoS proportional-fair requestor count {} exceeds {} priority levels",
                requestor_count, priority_levels
            ),
            Self::UnknownProportionalFairRequestor { requestor } => write!(
                formatter,
                "QoS proportional-fair requestor {} is not registered",
                requestor.get()
            ),
            Self::InvalidProportionalFairWeight { weight_bits } => write!(
                formatter,
                "QoS proportional-fair weight {} must be finite and between 0 and 1",
                f64::from_bits(*weight_bits)
            ),
            Self::InvalidProportionalFairScore {
                requestor,
                score_bits,
            } => write!(
                formatter,
                "QoS proportional-fair requestor {} score {} must be finite",
                requestor.get(),
                f64::from_bits(*score_bits)
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

#[derive(Clone, Debug)]
pub enum QosPriorityPolicy {
    FixedPriority(QosFixedPriorityPolicy),
    ProportionalFair(QosProportionalFairPolicy),
}

impl QosPriorityPolicy {
    pub const fn fixed_priority(policy: QosFixedPriorityPolicy) -> Self {
        Self::FixedPriority(policy)
    }

    pub const fn proportional_fair(policy: QosProportionalFairPolicy) -> Self {
        Self::ProportionalFair(policy)
    }

    pub fn priority_for(
        &mut self,
        requestor: QosRequestorId,
        bytes: u64,
    ) -> Result<QosPriority, QosError> {
        match self {
            Self::FixedPriority(policy) => Ok(policy.priority_for(requestor, bytes)),
            Self::ProportionalFair(policy) => policy.priority_for(requestor, bytes),
        }
    }
}

impl From<QosFixedPriorityPolicy> for QosPriorityPolicy {
    fn from(policy: QosFixedPriorityPolicy) -> Self {
        Self::fixed_priority(policy)
    }
}

impl From<QosProportionalFairPolicy> for QosPriorityPolicy {
    fn from(policy: QosProportionalFairPolicy) -> Self {
        Self::proportional_fair(policy)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QosProportionalFairScoreSnapshot {
    requestor: QosRequestorId,
    score_bits: u64,
}

impl QosProportionalFairScoreSnapshot {
    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub fn score(&self) -> f64 {
        f64::from_bits(self.score_bits)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QosProportionalFairPolicySnapshot {
    priority_levels: u8,
    weight_bits: u64,
    scores: Vec<QosProportionalFairScoreSnapshot>,
}

impl QosProportionalFairPolicySnapshot {
    pub const fn priority_levels(&self) -> u8 {
        self.priority_levels
    }

    pub fn weight(&self) -> f64 {
        f64::from_bits(self.weight_bits)
    }

    pub fn scores(&self) -> &[QosProportionalFairScoreSnapshot] {
        &self.scores
    }
}

#[derive(Clone, Debug)]
pub struct QosProportionalFairPolicy {
    priority_levels: u8,
    weight: f64,
    scores: BTreeMap<QosRequestorId, f64>,
}

impl QosProportionalFairPolicy {
    pub fn new(priority_levels: u8, weight: f64) -> Result<Self, QosError> {
        validate_priority_levels(priority_levels)?;
        validate_proportional_fair_weight(weight)?;

        Ok(Self {
            priority_levels,
            weight,
            scores: BTreeMap::new(),
        })
    }

    pub const fn priority_levels(&self) -> u8 {
        self.priority_levels
    }

    pub const fn weight(&self) -> f64 {
        self.weight
    }

    pub fn with_requestor_score(
        mut self,
        requestor: QosRequestorId,
        score: f64,
    ) -> Result<Self, QosError> {
        validate_proportional_fair_score(requestor, score)?;
        if self.scores.contains_key(&requestor) {
            return Err(QosError::DuplicateRequestorScore { requestor });
        }
        let requestor_count = self.scores.len() + 1;
        if requestor_count > self.priority_levels as usize {
            return Err(QosError::TooManyProportionalFairRequestors {
                requestor_count,
                priority_levels: self.priority_levels,
            });
        }

        self.scores.insert(requestor, score);
        Ok(self)
    }

    pub fn score_for(&self, requestor: QosRequestorId) -> Option<f64> {
        self.scores.get(&requestor).copied()
    }

    pub fn priority_for(
        &mut self,
        requestor: QosRequestorId,
        bytes: u64,
    ) -> Result<QosPriority, QosError> {
        if !self.scores.contains_key(&requestor) {
            return Err(QosError::UnknownProportionalFairRequestor { requestor });
        }

        let sorted = self.sorted_requestors_by_gem5_score();
        let position = sorted
            .iter()
            .position(|(candidate, _)| *candidate == requestor)
            .expect("registered proportional-fair requestor must be present");
        // gem5 PF ranks high-score requestors at 0, while rem6 priority 0
        // is served first. Invert the rank and keep the score formula.
        let priority = QosPriority::new((sorted.len() - 1 - position) as u8);
        self.update_scores_after_service(requestor, bytes);
        Ok(priority)
    }

    pub fn snapshot(&self) -> QosProportionalFairPolicySnapshot {
        QosProportionalFairPolicySnapshot {
            priority_levels: self.priority_levels,
            weight_bits: self.weight.to_bits(),
            scores: self
                .scores
                .iter()
                .map(|(requestor, score)| QosProportionalFairScoreSnapshot {
                    requestor: *requestor,
                    score_bits: score.to_bits(),
                })
                .collect(),
        }
    }

    pub fn restore(&mut self, snapshot: QosProportionalFairPolicySnapshot) -> Result<(), QosError> {
        let weight = f64::from_bits(snapshot.weight_bits);
        validate_priority_levels(snapshot.priority_levels)?;
        validate_proportional_fair_weight(weight)?;

        let mut scores = BTreeMap::new();
        for score in snapshot.scores {
            let value = f64::from_bits(score.score_bits);
            validate_proportional_fair_score(score.requestor, value)?;
            if scores.insert(score.requestor, value).is_some() {
                return Err(QosError::DuplicateRequestorScore {
                    requestor: score.requestor,
                });
            }
        }
        if scores.len() > snapshot.priority_levels as usize {
            return Err(QosError::TooManyProportionalFairRequestors {
                requestor_count: scores.len(),
                priority_levels: snapshot.priority_levels,
            });
        }

        self.priority_levels = snapshot.priority_levels;
        self.weight = weight;
        self.scores = scores;
        Ok(())
    }

    fn sorted_requestors_by_gem5_score(&self) -> Vec<(QosRequestorId, f64)> {
        let mut sorted = self
            .scores
            .iter()
            .map(|(requestor, score)| (*requestor, *score))
            .collect::<Vec<_>>();
        sorted.sort_by(
            |(left_requestor, left_score), (right_requestor, right_score)| {
                right_score
                    .partial_cmp(left_score)
                    .unwrap_or(Ordering::Equal)
                    .then_with(|| left_requestor.cmp(right_requestor))
            },
        );
        sorted
    }

    fn update_scores_after_service(&mut self, served_requestor: QosRequestorId, bytes: u64) {
        let served_bytes = bytes as f64;
        for (requestor, score) in &mut self.scores {
            let bytes = if *requestor == served_requestor {
                served_bytes
            } else {
                0.0
            };
            *score = ((1.0 - self.weight) * *score) + (self.weight * bytes);
        }
    }
}

impl PartialEq for QosProportionalFairPolicy {
    fn eq(&self, other: &Self) -> bool {
        self.priority_levels == other.priority_levels
            && self.weight.to_bits() == other.weight.to_bits()
            && scores_equal_by_bits(&self.scores, &other.scores)
    }
}

impl Eq for QosProportionalFairPolicy {}

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
pub struct FabricQosRequest {
    requestor: QosRequestorId,
    priority: QosPriority,
    order: u64,
    packet: FabricPacket,
    path: FabricPath,
}

impl FabricQosRequest {
    pub fn new(
        requestor: QosRequestorId,
        priority: QosPriority,
        order: u64,
        packet: FabricPacket,
        path: FabricPath,
    ) -> Self {
        Self {
            requestor,
            priority,
            order,
            packet,
            path,
        }
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

    pub fn packet(&self) -> &FabricPacket {
        &self.packet
    }

    pub fn path(&self) -> &FabricPath {
        &self.path
    }

    pub(crate) fn queued_request(&self) -> QosQueuedRequest {
        QosQueuedRequest::new(
            QosRequestId::new(self.packet.id().get()),
            self.requestor,
            self.priority,
            self.packet.bytes(),
            self.order,
        )
        .expect("fabric packets always have nonzero bytes")
    }

    pub(crate) fn into_packet_path(self) -> (FabricPacket, FabricPath) {
        (self.packet, self.path)
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

fn validate_proportional_fair_weight(weight: f64) -> Result<(), QosError> {
    if !(0.0..=1.0).contains(&weight) {
        return Err(QosError::InvalidProportionalFairWeight {
            weight_bits: weight.to_bits(),
        });
    }
    Ok(())
}

fn validate_proportional_fair_score(requestor: QosRequestorId, score: f64) -> Result<(), QosError> {
    if !score.is_finite() {
        return Err(QosError::InvalidProportionalFairScore {
            requestor,
            score_bits: score.to_bits(),
        });
    }
    Ok(())
}

fn scores_equal_by_bits(
    left: &BTreeMap<QosRequestorId, f64>,
    right: &BTreeMap<QosRequestorId, f64>,
) -> bool {
    left.len() == right.len()
        && left.iter().all(|(requestor, score)| {
            right
                .get(requestor)
                .is_some_and(|other| score.to_bits() == other.to_bits())
        })
}
