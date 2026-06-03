use rem6_fabric::{QosPriority, QosRequestorId};

use crate::{WorkloadError, WorkloadQosError};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadQosPriorityPolicyKind {
    FixedPriority,
    ProportionalFair,
}

impl WorkloadQosPriorityPolicyKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::FixedPriority => "fixed_priority",
            Self::ProportionalFair => "proportional_fair",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadQosQueuePolicyKind {
    Fifo,
    Lifo,
    LeastRecentlyGranted,
}

impl WorkloadQosQueuePolicyKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Fifo => "fifo",
            Self::Lifo => "lifo",
            Self::LeastRecentlyGranted => "least_recently_granted",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadQosTurnaroundPolicyKind {
    RequestOrder,
    PreferCurrentDirection,
    HighestPriorityOppositeOnTie,
}

impl WorkloadQosTurnaroundPolicyKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestOrder => "request_order",
            Self::PreferCurrentDirection => "prefer_current_direction",
            Self::HighestPriorityOppositeOnTie => "highest_priority_opposite_on_tie",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadQosRequestorPriority {
    requestor: QosRequestorId,
    priority: QosPriority,
}

impl WorkloadQosRequestorPriority {
    pub const fn new(requestor: QosRequestorId, priority: QosPriority) -> Self {
        Self {
            requestor,
            priority,
        }
    }

    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub const fn priority(&self) -> QosPriority {
        self.priority
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadQosRequestorScore {
    requestor: QosRequestorId,
    score_bits: u64,
}

impl WorkloadQosRequestorScore {
    pub fn new(requestor: QosRequestorId, score: f64) -> Self {
        Self {
            requestor,
            score_bits: score.to_bits(),
        }
    }

    pub const fn requestor(&self) -> QosRequestorId {
        self.requestor
    }

    pub fn score(&self) -> f64 {
        f64::from_bits(self.score_bits)
    }

    pub const fn score_bits(&self) -> u64 {
        self.score_bits
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum WorkloadQosPriorityPolicy {
    FixedPriority {
        default_priority: QosPriority,
        requestor_priorities: Vec<WorkloadQosRequestorPriority>,
    },
    ProportionalFair {
        weight_bits: u64,
        requestor_scores: Vec<WorkloadQosRequestorScore>,
    },
}

impl WorkloadQosPriorityPolicy {
    const fn kind(&self) -> WorkloadQosPriorityPolicyKind {
        match self {
            Self::FixedPriority { .. } => WorkloadQosPriorityPolicyKind::FixedPriority,
            Self::ProportionalFair { .. } => WorkloadQosPriorityPolicyKind::ProportionalFair,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadQosPolicy {
    priority_levels: u8,
    priority_policy: WorkloadQosPriorityPolicy,
    queue_policy: WorkloadQosQueuePolicyKind,
    turnaround_policy: WorkloadQosTurnaroundPolicyKind,
    priority_escalation: bool,
}

impl WorkloadQosPolicy {
    pub fn new(priority_levels: u8, default_priority: QosPriority) -> Result<Self, WorkloadError> {
        validate_priority(priority_levels, default_priority)?;
        Ok(Self {
            priority_levels,
            priority_policy: WorkloadQosPriorityPolicy::FixedPriority {
                default_priority,
                requestor_priorities: Vec::new(),
            },
            queue_policy: WorkloadQosQueuePolicyKind::Fifo,
            turnaround_policy: WorkloadQosTurnaroundPolicyKind::RequestOrder,
            priority_escalation: false,
        })
    }

    pub fn proportional_fair(priority_levels: u8, weight: f64) -> Result<Self, WorkloadError> {
        validate_priority_levels(priority_levels)?;
        validate_proportional_fair_weight(weight)?;
        Ok(Self {
            priority_levels,
            priority_policy: WorkloadQosPriorityPolicy::ProportionalFair {
                weight_bits: weight.to_bits(),
                requestor_scores: Vec::new(),
            },
            queue_policy: WorkloadQosQueuePolicyKind::Fifo,
            turnaround_policy: WorkloadQosTurnaroundPolicyKind::RequestOrder,
            priority_escalation: false,
        })
    }

    pub const fn priority_levels(&self) -> u8 {
        self.priority_levels
    }

    pub fn priority_policy_kind(&self) -> WorkloadQosPriorityPolicyKind {
        self.priority_policy.kind()
    }

    pub fn default_priority(&self) -> QosPriority {
        match &self.priority_policy {
            WorkloadQosPriorityPolicy::FixedPriority {
                default_priority, ..
            } => *default_priority,
            WorkloadQosPriorityPolicy::ProportionalFair { .. } => QosPriority::new(0),
        }
    }

    pub const fn queue_policy(&self) -> WorkloadQosQueuePolicyKind {
        self.queue_policy
    }

    pub const fn turnaround_policy(&self) -> WorkloadQosTurnaroundPolicyKind {
        self.turnaround_policy
    }

    pub const fn priority_escalation_enabled(&self) -> bool {
        self.priority_escalation
    }

    pub fn requestor_priorities(&self) -> &[WorkloadQosRequestorPriority] {
        match &self.priority_policy {
            WorkloadQosPriorityPolicy::FixedPriority {
                requestor_priorities,
                ..
            } => requestor_priorities,
            WorkloadQosPriorityPolicy::ProportionalFair { .. } => &[],
        }
    }

    pub fn proportional_fair_weight(&self) -> Option<f64> {
        match &self.priority_policy {
            WorkloadQosPriorityPolicy::FixedPriority { .. } => None,
            WorkloadQosPriorityPolicy::ProportionalFair { weight_bits, .. } => {
                Some(f64::from_bits(*weight_bits))
            }
        }
    }

    pub fn requestor_scores(&self) -> &[WorkloadQosRequestorScore] {
        match &self.priority_policy {
            WorkloadQosPriorityPolicy::FixedPriority { .. } => &[],
            WorkloadQosPriorityPolicy::ProportionalFair {
                requestor_scores, ..
            } => requestor_scores,
        }
    }

    pub const fn with_queue_policy(mut self, queue_policy: WorkloadQosQueuePolicyKind) -> Self {
        self.queue_policy = queue_policy;
        self
    }

    pub const fn with_turnaround_policy(
        mut self,
        turnaround_policy: WorkloadQosTurnaroundPolicyKind,
    ) -> Self {
        self.turnaround_policy = turnaround_policy;
        self
    }

    pub const fn with_priority_escalation(mut self) -> Self {
        self.priority_escalation = true;
        self
    }

    pub fn with_requestor_priority(
        mut self,
        requestor: QosRequestorId,
        priority: QosPriority,
    ) -> Result<Self, WorkloadError> {
        validate_priority(self.priority_levels, priority)?;
        let WorkloadQosPriorityPolicy::FixedPriority {
            requestor_priorities,
            ..
        } = &mut self.priority_policy
        else {
            return Err(WorkloadQosError::PriorityPolicyMismatch {
                expected: WorkloadQosPriorityPolicyKind::FixedPriority,
                actual: self.priority_policy.kind(),
            }
            .into());
        };
        if requestor_priorities
            .iter()
            .any(|existing| existing.requestor() == requestor)
        {
            return Err(WorkloadQosError::DuplicateRequestorPriority { requestor }.into());
        }

        requestor_priorities.push(WorkloadQosRequestorPriority::new(requestor, priority));
        requestor_priorities.sort_by_key(|entry| entry.requestor());
        Ok(self)
    }

    pub fn with_requestor_score(
        mut self,
        requestor: QosRequestorId,
        score: f64,
    ) -> Result<Self, WorkloadError> {
        validate_proportional_fair_score(requestor, score)?;
        let WorkloadQosPriorityPolicy::ProportionalFair {
            requestor_scores, ..
        } = &mut self.priority_policy
        else {
            return Err(WorkloadQosError::PriorityPolicyMismatch {
                expected: WorkloadQosPriorityPolicyKind::ProportionalFair,
                actual: self.priority_policy.kind(),
            }
            .into());
        };
        if requestor_scores
            .iter()
            .any(|existing| existing.requestor() == requestor)
        {
            return Err(WorkloadQosError::DuplicateRequestorScore { requestor }.into());
        }
        let requestor_count = requestor_scores.len() + 1;
        if requestor_count > self.priority_levels as usize {
            return Err(WorkloadQosError::TooManyProportionalFairRequestors {
                requestor_count,
                priority_levels: self.priority_levels,
            }
            .into());
        }

        requestor_scores.push(WorkloadQosRequestorScore::new(requestor, score));
        requestor_scores.sort_by_key(|entry| entry.requestor());
        Ok(self)
    }

    pub fn priority_for(&self, requestor: QosRequestorId) -> QosPriority {
        match &self.priority_policy {
            WorkloadQosPriorityPolicy::FixedPriority {
                default_priority,
                requestor_priorities,
            } => requestor_priorities
                .iter()
                .find(|entry| entry.requestor() == requestor)
                .map(WorkloadQosRequestorPriority::priority)
                .unwrap_or(*default_priority),
            WorkloadQosPriorityPolicy::ProportionalFair { .. } => QosPriority::new(0),
        }
    }
}

fn validate_priority_levels(priority_levels: u8) -> Result<(), WorkloadError> {
    if priority_levels == 0 {
        return Err(WorkloadQosError::ZeroPriorityLevels.into());
    }
    Ok(())
}

fn validate_priority(priority_levels: u8, priority: QosPriority) -> Result<(), WorkloadError> {
    validate_priority_levels(priority_levels)?;
    if priority.get() >= priority_levels {
        return Err(WorkloadQosError::PriorityOutOfRange {
            priority,
            priority_levels,
        }
        .into());
    }
    Ok(())
}

fn validate_proportional_fair_weight(weight: f64) -> Result<(), WorkloadError> {
    if !weight.is_finite() || !(0.0..=1.0).contains(&weight) {
        return Err(WorkloadQosError::InvalidProportionalFairWeight {
            weight_bits: weight.to_bits(),
        }
        .into());
    }
    Ok(())
}

fn validate_proportional_fair_score(
    requestor: QosRequestorId,
    score: f64,
) -> Result<(), WorkloadError> {
    if !score.is_finite() {
        return Err(WorkloadQosError::InvalidProportionalFairScore {
            requestor,
            score_bits: score.to_bits(),
        }
        .into());
    }
    Ok(())
}
