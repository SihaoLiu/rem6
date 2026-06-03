use std::fmt;

use rem6_fabric::{QosPriority, QosRequestorId};

use crate::{WorkloadError, WorkloadQosPriorityPolicyKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadQosError {
    ZeroPriorityLevels,
    PriorityOutOfRange {
        priority: QosPriority,
        priority_levels: u8,
    },
    PriorityPolicyMismatch {
        expected: WorkloadQosPriorityPolicyKind,
        actual: WorkloadQosPriorityPolicyKind,
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
    InvalidProportionalFairWeight {
        weight_bits: u64,
    },
    InvalidProportionalFairScore {
        requestor: QosRequestorId,
        score_bits: u64,
    },
}

impl fmt::Display for WorkloadQosError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroPriorityLevels => write!(formatter, "QoS priority levels must be positive"),
            Self::PriorityOutOfRange {
                priority,
                priority_levels,
            } => write!(
                formatter,
                "QoS priority {} is outside {priority_levels} configured levels",
                priority.get()
            ),
            Self::PriorityPolicyMismatch { expected, actual } => write!(
                formatter,
                "QoS priority policy {} cannot be used as {}",
                actual.as_str(),
                expected.as_str()
            ),
            Self::DuplicateRequestorPriority { requestor } => write!(
                formatter,
                "QoS requestor {} has more than one priority declaration",
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
                "QoS proportional-fair requestor count {requestor_count} exceeds {priority_levels} priority levels"
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
        }
    }
}

impl From<WorkloadQosError> for WorkloadError {
    fn from(error: WorkloadQosError) -> Self {
        Self::Qos(error)
    }
}
