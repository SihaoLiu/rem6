use rem6_fabric::{QosPriority, QosRequestorId};

use crate::WorkloadError;

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
}

impl WorkloadQosTurnaroundPolicyKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RequestOrder => "request_order",
            Self::PreferCurrentDirection => "prefer_current_direction",
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadQosPolicy {
    priority_levels: u8,
    default_priority: QosPriority,
    queue_policy: WorkloadQosQueuePolicyKind,
    turnaround_policy: WorkloadQosTurnaroundPolicyKind,
    priority_escalation: bool,
    requestor_priorities: Vec<WorkloadQosRequestorPriority>,
}

impl WorkloadQosPolicy {
    pub fn new(priority_levels: u8, default_priority: QosPriority) -> Result<Self, WorkloadError> {
        validate_priority(priority_levels, default_priority)?;
        Ok(Self {
            priority_levels,
            default_priority,
            queue_policy: WorkloadQosQueuePolicyKind::Fifo,
            turnaround_policy: WorkloadQosTurnaroundPolicyKind::RequestOrder,
            priority_escalation: false,
            requestor_priorities: Vec::new(),
        })
    }

    pub const fn priority_levels(&self) -> u8 {
        self.priority_levels
    }

    pub const fn default_priority(&self) -> QosPriority {
        self.default_priority
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
        &self.requestor_priorities
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
        if self
            .requestor_priorities
            .iter()
            .any(|existing| existing.requestor() == requestor)
        {
            return Err(WorkloadError::DuplicateQosRequestorPriority { requestor });
        }

        self.requestor_priorities
            .push(WorkloadQosRequestorPriority::new(requestor, priority));
        self.requestor_priorities
            .sort_by_key(|entry| entry.requestor());
        Ok(self)
    }

    pub fn priority_for(&self, requestor: QosRequestorId) -> QosPriority {
        self.requestor_priorities
            .iter()
            .find(|entry| entry.requestor() == requestor)
            .map(WorkloadQosRequestorPriority::priority)
            .unwrap_or(self.default_priority)
    }
}

fn validate_priority(priority_levels: u8, priority: QosPriority) -> Result<(), WorkloadError> {
    if priority_levels == 0 {
        return Err(WorkloadError::ZeroQosPriorityLevels);
    }
    if priority.get() >= priority_levels {
        return Err(WorkloadError::QosPriorityOutOfRange {
            priority,
            priority_levels,
        });
    }
    Ok(())
}
