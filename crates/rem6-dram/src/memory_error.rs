use std::error::Error;
use std::fmt;

use rem6_fabric::QosError;
use rem6_memory::{MemoryError, MemoryTargetId};

use crate::{DramError, DramProfileSnapshotMismatch};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DramMemoryError {
    Memory(MemoryError),
    Dram {
        target: MemoryTargetId,
        source: DramError,
    },
    TargetLineSizeMismatch {
        target: MemoryTargetId,
        layout: u64,
        geometry: u64,
    },
    ProfileSnapshotMismatch {
        target: MemoryTargetId,
        mismatch: Box<DramProfileSnapshotMismatch>,
    },
    MissingDramTarget {
        target: MemoryTargetId,
    },
    Qos {
        source: QosError,
    },
}

impl DramMemoryError {
    pub(crate) fn profile_snapshot_mismatch(
        target: MemoryTargetId,
        mismatch: DramProfileSnapshotMismatch,
    ) -> Self {
        Self::ProfileSnapshotMismatch {
            target,
            mismatch: Box::new(mismatch),
        }
    }
}

impl fmt::Display for DramMemoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Dram { target, source } => {
                write!(
                    formatter,
                    "DRAM target {} rejected request: {source}",
                    target.get()
                )
            }
            Self::TargetLineSizeMismatch {
                target,
                layout,
                geometry,
            } => write!(
                formatter,
                "DRAM target {} uses {geometry}-byte geometry lines but memory layout uses {layout}",
                target.get()
            ),
            Self::ProfileSnapshotMismatch { target, mismatch } => {
                write!(formatter, "DRAM target {} {mismatch}", target.get())
            }
            Self::MissingDramTarget { target } => {
                write!(formatter, "DRAM target {} is missing timing state", target.get())
            }
            Self::Qos { source } => write!(formatter, "DRAM QoS scheduling failed: {source}"),
        }
    }
}

impl Error for DramMemoryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Dram { source, .. } => Some(source),
            Self::Qos { source } => Some(source),
            Self::TargetLineSizeMismatch { .. }
            | Self::ProfileSnapshotMismatch { .. }
            | Self::MissingDramTarget { .. } => None,
        }
    }
}

impl From<MemoryError> for DramMemoryError {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}
