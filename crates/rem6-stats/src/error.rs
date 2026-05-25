use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

use crate::probes::{ProbeListenerId, ProbePointId};
use crate::stats::{
    StatDescription, StatDescriptionError, StatGroupDescriptor, StatGroupId, StatId, StatPathError,
    StatUnitError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum StatsError {
    EmptyPath,
    InvalidPath {
        path: String,
        reason: StatPathError,
    },
    InvalidUnit {
        unit: String,
        reason: StatUnitError,
    },
    InvalidDescription {
        description: String,
        reason: StatDescriptionError,
    },
    DuplicatePath {
        path: String,
    },
    DuplicateGroup {
        scope: String,
    },
    UnknownStat {
        stat: StatId,
    },
    UnknownStatGroup {
        group: StatGroupId,
    },
    CounterOverflow {
        stat: StatId,
    },
    SnapshotBeforeReset {
        tick: Tick,
        reset_tick: Tick,
    },
    ResetBeforeLastReset {
        tick: Tick,
        reset_tick: Tick,
    },
    SnapshotDeltaTimeWentBack {
        previous_tick: Tick,
        current_tick: Tick,
    },
    SnapshotDeltaScopeMismatch {
        previous_epoch: u64,
        current_epoch: u64,
        previous_reset_tick: Tick,
        current_reset_tick: Tick,
    },
    SnapshotDeltaGroupCatalogMismatch {
        previous_groups: Vec<StatGroupDescriptor>,
        current_groups: Vec<StatGroupDescriptor>,
    },
    SnapshotDeltaMissingStat {
        stat: StatId,
    },
    SnapshotDeltaUnexpectedStat {
        stat: StatId,
    },
    SnapshotDeltaDescriptorMismatch {
        stat: StatId,
        previous_path: String,
        current_path: String,
        previous_unit: String,
        current_unit: String,
    },
    SnapshotDeltaDescriptionMismatch {
        stat: StatId,
        previous_description: Option<StatDescription>,
        current_description: Option<StatDescription>,
    },
    SnapshotDeltaValueWentBack {
        stat: StatId,
        previous: u64,
        current: u64,
    },
    EmptyProbeComponent,
    EmptyProbeName,
    DuplicateProbePoint {
        component: String,
        name: String,
    },
    UnknownProbePoint {
        point: ProbePointId,
    },
    EmptyProbeListenerName,
    DuplicateProbeListener {
        point: ProbePointId,
        name: String,
    },
    UnknownProbeListener {
        listener: ProbeListenerId,
    },
    ProbeListenerPointMismatch {
        point: ProbePointId,
        listener: ProbeListenerId,
    },
    ProbeSequenceOverflow,
    GroupSequenceOverflow,
    DumpSequenceOverflow,
}

impl fmt::Display for StatsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyPath => write!(formatter, "stat path must not be empty"),
            Self::InvalidPath { path, reason } => {
                write!(formatter, "stat path {path} is invalid: {reason}")
            }
            Self::InvalidUnit { unit, reason } => {
                write!(formatter, "stat unit {unit} is invalid: {reason}")
            }
            Self::InvalidDescription {
                description,
                reason,
            } => {
                write!(
                    formatter,
                    "stat description {description:?} is invalid: {reason}"
                )
            }
            Self::DuplicatePath { path } => write!(formatter, "stat path already exists: {path}"),
            Self::DuplicateGroup { scope } => {
                write!(formatter, "stat group already exists: {scope}")
            }
            Self::UnknownStat { stat } => write!(formatter, "unknown stat id {}", stat.get()),
            Self::UnknownStatGroup { group } => {
                write!(formatter, "unknown stat group id {}", group.get())
            }
            Self::CounterOverflow { stat } => {
                write!(formatter, "counter {} overflowed", stat.get())
            }
            Self::SnapshotBeforeReset { tick, reset_tick } => write!(
                formatter,
                "cannot snapshot at tick {tick}; last reset was at tick {reset_tick}"
            ),
            Self::ResetBeforeLastReset { tick, reset_tick } => write!(
                formatter,
                "cannot reset stats at tick {tick}; last reset was at tick {reset_tick}"
            ),
            Self::SnapshotDeltaTimeWentBack {
                previous_tick,
                current_tick,
            } => write!(
                formatter,
                "stat snapshot delta tick {current_tick} is before previous tick {previous_tick}"
            ),
            Self::SnapshotDeltaScopeMismatch {
                previous_epoch,
                current_epoch,
                previous_reset_tick,
                current_reset_tick,
            } => write!(
                formatter,
                "stat snapshot delta scopes differ: previous epoch {previous_epoch} reset {previous_reset_tick}, current epoch {current_epoch} reset {current_reset_tick}"
            ),
            Self::SnapshotDeltaGroupCatalogMismatch { .. } => {
                write!(
                    formatter,
                    "stat snapshot group catalog changed between delta endpoints"
                )
            }
            Self::SnapshotDeltaMissingStat { stat } => {
                write!(formatter, "stat snapshot delta is missing stat {}", stat.get())
            }
            Self::SnapshotDeltaUnexpectedStat { stat } => {
                write!(
                    formatter,
                    "stat snapshot delta has unexpected stat {}",
                    stat.get()
                )
            }
            Self::SnapshotDeltaDescriptorMismatch {
                stat,
                previous_path,
                current_path,
                previous_unit,
                current_unit,
            } => write!(
                formatter,
                "stat snapshot delta descriptor for stat {} changed from {previous_path} {previous_unit} to {current_path} {current_unit}",
                stat.get()
            ),
            Self::SnapshotDeltaDescriptionMismatch { stat, .. } => write!(
                formatter,
                "stat snapshot delta description for stat {} changed",
                stat.get()
            ),
            Self::SnapshotDeltaValueWentBack {
                stat,
                previous,
                current,
            } => write!(
                formatter,
                "stat snapshot delta value for stat {} went from {previous} down to {current}",
                stat.get()
            ),
            Self::EmptyProbeComponent => write!(formatter, "probe component must not be empty"),
            Self::EmptyProbeName => write!(formatter, "probe point name must not be empty"),
            Self::DuplicateProbePoint { component, name } => {
                write!(formatter, "probe point already exists: {component}.{name}")
            }
            Self::UnknownProbePoint { point } => {
                write!(formatter, "unknown probe point id {}", point.get())
            }
            Self::EmptyProbeListenerName => {
                write!(formatter, "probe listener name must not be empty")
            }
            Self::DuplicateProbeListener { point, name } => write!(
                formatter,
                "probe listener {name} already exists for point {}",
                point.get()
            ),
            Self::UnknownProbeListener { listener } => {
                write!(formatter, "unknown probe listener id {}", listener.get())
            }
            Self::ProbeListenerPointMismatch { point, listener } => write!(
                formatter,
                "probe listener {} is not attached to point {}",
                listener.get(),
                point.get()
            ),
            Self::ProbeSequenceOverflow => write!(formatter, "probe event sequence overflowed"),
            Self::GroupSequenceOverflow => write!(formatter, "stat group sequence overflowed"),
            Self::DumpSequenceOverflow => write!(formatter, "stat dump sequence overflowed"),
        }
    }
}

impl Error for StatsError {}
