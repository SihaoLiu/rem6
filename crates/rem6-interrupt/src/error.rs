use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, SchedulerError};

use crate::{
    InterruptEventKind, InterruptLineId, InterruptRoute, InterruptSourceId, InterruptTargetId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterruptError {
    ZeroSignalLatency,
    DuplicateLine {
        line: InterruptLineId,
    },
    DuplicateSnapshotPriority {
        line: InterruptLineId,
    },
    DuplicateSnapshotPending {
        line: InterruptLineId,
    },
    DuplicateSnapshotClaim {
        target: InterruptTargetId,
        target_partition: PartitionId,
    },
    MissingSnapshotPriority {
        line: InterruptLineId,
    },
    UnknownLine {
        line: InterruptLineId,
    },
    AlreadyPending {
        line: InterruptLineId,
        source: InterruptSourceId,
    },
    NotPending {
        line: InterruptLineId,
    },
    SourceMismatch {
        line: InterruptLineId,
        expected: InterruptSourceId,
        actual: InterruptSourceId,
    },
    RouteMismatch {
        line: InterruptLineId,
        expected: InterruptRoute,
        actual: InterruptRoute,
    },
    NoClaimedInterrupt {
        target: InterruptTargetId,
        target_partition: PartitionId,
    },
    ClaimMismatch {
        target: InterruptTargetId,
        target_partition: PartitionId,
        expected: InterruptLineId,
        actual: InterruptLineId,
    },
    NonSignalDelivery {
        kind: InterruptEventKind,
    },
    Scheduler(SchedulerError),
}

impl fmt::Display for InterruptError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroSignalLatency => {
                write!(formatter, "interrupt signal latency must be positive")
            }
            Self::DuplicateLine { line } => {
                write!(
                    formatter,
                    "interrupt line {} is already registered",
                    line.get()
                )
            }
            Self::DuplicateSnapshotPriority { line } => {
                write!(
                    formatter,
                    "interrupt snapshot has duplicate priority for line {}",
                    line.get()
                )
            }
            Self::DuplicateSnapshotPending { line } => {
                write!(
                    formatter,
                    "interrupt snapshot has duplicate pending line {}",
                    line.get()
                )
            }
            Self::DuplicateSnapshotClaim {
                target,
                target_partition,
            } => write!(
                formatter,
                "interrupt snapshot has duplicate claim for target {} partition {}",
                target.get(),
                target_partition.index()
            ),
            Self::MissingSnapshotPriority { line } => {
                write!(
                    formatter,
                    "interrupt snapshot is missing priority for line {}",
                    line.get()
                )
            }
            Self::UnknownLine { line } => {
                write!(formatter, "unknown interrupt line {}", line.get())
            }
            Self::AlreadyPending { line, source } => write!(
                formatter,
                "interrupt line {} is already pending from source {}",
                line.get(),
                source.get()
            ),
            Self::NotPending { line } => {
                write!(formatter, "interrupt line {} is not pending", line.get())
            }
            Self::SourceMismatch {
                line,
                expected,
                actual,
            } => write!(
                formatter,
                "interrupt line {} is pending from source {}, not source {}",
                line.get(),
                expected.get(),
                actual.get()
            ),
            Self::RouteMismatch {
                line,
                expected,
                actual,
            } => write!(
                formatter,
                "interrupt line {} delivery route targets partition {} target {}, \
                 expected partition {} target {}",
                line.get(),
                actual.target_partition().index(),
                actual.target().get(),
                expected.target_partition().index(),
                expected.target().get()
            ),
            Self::NoClaimedInterrupt {
                target,
                target_partition,
            } => write!(
                formatter,
                "target {} partition {} has no claimed interrupt",
                target.get(),
                target_partition.index()
            ),
            Self::ClaimMismatch {
                target,
                target_partition,
                expected,
                actual,
            } => write!(
                formatter,
                "target {} partition {} claimed line {}, not line {}",
                target.get(),
                target_partition.index(),
                expected.get(),
                actual.get()
            ),
            Self::NonSignalDelivery { kind } => {
                write!(formatter, "{kind:?} is not a signal delivery event")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for InterruptError {}
