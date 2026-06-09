use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

use crate::kind::StatKind;
use crate::mem_footprint::MemFootprintGranularity;
use crate::pc_count::PcCountPair;
use crate::probes::{ProbeListenerId, ProbePointId};
use crate::reset::StatResetPolicy;
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
    SchemaLocked {
        history_records: usize,
    },
    UnknownStat {
        stat: StatId,
    },
    StatIsNotCounter {
        stat: StatId,
    },
    StatIsNotAverage {
        stat: StatId,
    },
    UnknownStatGroup {
        group: StatGroupId,
    },
    CounterOverflow {
        stat: StatId,
    },
    AverageUpdateBeforeReset {
        stat: StatId,
        tick: Tick,
        reset_tick: Tick,
    },
    AverageUpdateBeforeLastSample {
        stat: StatId,
        tick: Tick,
        last_tick: Tick,
    },
    AverageReadBeforeLastSample {
        stat: StatId,
        tick: Tick,
        last_tick: Tick,
    },
    AverageTotalOverflow {
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
    HistoryTickBeforeLastRecord {
        tick: Tick,
        last_history_tick: Tick,
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
    SnapshotDeltaResetPolicyMismatch {
        stat: StatId,
        previous_policy: StatResetPolicy,
        current_policy: StatResetPolicy,
    },
    SnapshotDeltaStatKindMismatch {
        stat: StatId,
        previous_kind: StatKind,
        current_kind: StatKind,
    },
    SnapshotDeltaUnsupportedStatKind {
        stat: StatId,
        kind: StatKind,
    },
    SnapshotDeltaValueWentBack {
        stat: StatId,
        previous: u64,
        current: u64,
    },
    EmptyProbeComponent,
    InvalidProbeComponent {
        component: String,
        reason: StatPathError,
    },
    EmptyProbeName,
    InvalidProbeName {
        name: String,
        reason: StatPathError,
    },
    DuplicateProbePoint {
        component: String,
        name: String,
    },
    DuplicateProbePointId {
        point: ProbePointId,
    },
    UnknownProbePoint {
        point: ProbePointId,
    },
    EmptyProbeListenerName,
    InvalidProbeListenerName {
        name: String,
        reason: StatPathError,
    },
    DuplicateProbeListener {
        point: ProbePointId,
        name: String,
    },
    DuplicateProbeListenerId {
        listener: ProbeListenerId,
    },
    UnknownProbeListener {
        listener: ProbeListenerId,
    },
    ProbeListenerPointMismatch {
        point: ProbePointId,
        listener: ProbeListenerId,
    },
    ProbePointCursorBehind {
        next_point: u64,
        highest_point: ProbePointId,
    },
    ProbeListenerCursorBehind {
        next_listener: u64,
        highest_listener: ProbeListenerId,
    },
    ProbeEventCursorBehind {
        next_sequence: u64,
        highest_sequence: u64,
    },
    ProbeEventSequenceNotIncreasing {
        previous_sequence: u64,
        current_sequence: u64,
    },
    ProbeEventTimeWentBack {
        previous_tick: Tick,
        current_tick: Tick,
    },
    ProbePointSequenceOverflow,
    ProbeListenerSequenceOverflow,
    ProbeSequenceOverflow,
    DuplicatePcCountCounter {
        pc: u64,
    },
    DuplicatePcCountTarget {
        pair: PcCountPair,
    },
    MissingPcCountCounter {
        pc: u64,
    },
    UnreachablePcCountTarget {
        pair: PcCountPair,
        current_count: u64,
    },
    PcCountSnapshotTargetStateMismatch {
        armed: bool,
        pending_targets: usize,
    },
    DuplicateInstThreshold {
        threshold: u64,
    },
    UnreachableInstThreshold {
        threshold: u64,
        counter: u64,
    },
    InstTrackerCounterOverflow,
    InvalidMemFootprintGranularity {
        name: String,
        bytes: u64,
    },
    MemFootprintGranularityOrder {
        cache_line_size: u64,
        page_size: u64,
    },
    EmptyMemFootprintAddressMap,
    EmptyMemFootprintAddressRange,
    MemFootprintAddressRangeOverflow {
        start: u64,
        bytes: u64,
    },
    OverlappingMemFootprintAddressRange {
        existing_start: u64,
        existing_end: u64,
        requested_start: u64,
        requested_end: u64,
    },
    UnalignedMemFootprintSnapshotAddress {
        granularity: MemFootprintGranularity,
        address: u64,
    },
    MemFootprintSnapshotAddressOutsideMemory {
        granularity: MemFootprintGranularity,
        address: u64,
    },
    DuplicateMemFootprintSnapshotAddress {
        granularity: MemFootprintGranularity,
        address: u64,
    },
    MemFootprintSnapshotCurrentNotInTotal {
        granularity: MemFootprintGranularity,
        address: u64,
    },
    MemFootprintSnapshotGranularityMismatch {
        finer: MemFootprintGranularity,
        coarser: MemFootprintGranularity,
        address: u64,
        parent: u64,
    },
    MemFootprintSnapshotExceedsMemory {
        granularity: MemFootprintGranularity,
        observed: u64,
        capacity: u64,
    },
    MemFootprintValueOverflow {
        granularity: MemFootprintGranularity,
    },
    GroupSequenceOverflow,
    DumpSequenceOverflow,
    ResetSequenceOverflow,
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
            Self::SchemaLocked { history_records } => write!(
                formatter,
                "cannot register stats after {history_records} stats history records"
            ),
            Self::UnknownStat { stat } => write!(formatter, "unknown stat id {}", stat.get()),
            Self::StatIsNotCounter { stat } => {
                write!(formatter, "stat {} is not a counter", stat.get())
            }
            Self::StatIsNotAverage { stat } => {
                write!(formatter, "stat {} is not an average", stat.get())
            }
            Self::UnknownStatGroup { group } => {
                write!(formatter, "unknown stat group id {}", group.get())
            }
            Self::CounterOverflow { stat } => {
                write!(formatter, "counter {} overflowed", stat.get())
            }
            Self::AverageUpdateBeforeReset {
                stat,
                tick,
                reset_tick,
            } => write!(
                formatter,
                "cannot update average {} at tick {tick}; last reset was at tick {reset_tick}",
                stat.get()
            ),
            Self::AverageUpdateBeforeLastSample {
                stat,
                tick,
                last_tick,
            } => write!(
                formatter,
                "cannot update average {} at tick {tick}; previous sample was at tick {last_tick}",
                stat.get()
            ),
            Self::AverageReadBeforeLastSample {
                stat,
                tick,
                last_tick,
            } => write!(
                formatter,
                "cannot read average {} at tick {tick}; previous sample was at tick {last_tick}",
                stat.get()
            ),
            Self::AverageTotalOverflow { stat } => {
                write!(formatter, "average {} accumulated value overflowed", stat.get())
            }
            Self::SnapshotBeforeReset { tick, reset_tick } => write!(
                formatter,
                "cannot snapshot at tick {tick}; last reset was at tick {reset_tick}"
            ),
            Self::ResetBeforeLastReset { tick, reset_tick } => write!(
                formatter,
                "cannot reset stats at tick {tick}; last reset was at tick {reset_tick}"
            ),
            Self::HistoryTickBeforeLastRecord {
                tick,
                last_history_tick,
            } => write!(
                formatter,
                "cannot record stats history at tick {tick}; previous stats history record was at tick {last_history_tick}"
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
            Self::SnapshotDeltaResetPolicyMismatch {
                stat,
                previous_policy,
                current_policy,
            } => write!(
                formatter,
                "stat snapshot delta reset policy for stat {} changed from {previous_policy} to {current_policy}",
                stat.get()
            ),
            Self::SnapshotDeltaStatKindMismatch {
                stat,
                previous_kind,
                current_kind,
            } => write!(
                formatter,
                "stat snapshot delta kind for stat {} changed from {previous_kind} to {current_kind}",
                stat.get()
            ),
            Self::SnapshotDeltaUnsupportedStatKind { stat, kind } => write!(
                formatter,
                "stat snapshot delta does not support stat {} kind {kind}",
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
            Self::InvalidProbeComponent { component, reason } => {
                write!(formatter, "probe component {component} is invalid: {reason}")
            }
            Self::EmptyProbeName => write!(formatter, "probe point name must not be empty"),
            Self::InvalidProbeName { name, reason } => {
                write!(formatter, "probe point name {name} is invalid: {reason}")
            }
            Self::DuplicateProbePoint { component, name } => {
                write!(formatter, "probe point already exists: {component}.{name}")
            }
            Self::DuplicateProbePointId { point } => {
                write!(formatter, "duplicate probe point id {}", point.get())
            }
            Self::UnknownProbePoint { point } => {
                write!(formatter, "unknown probe point id {}", point.get())
            }
            Self::EmptyProbeListenerName => {
                write!(formatter, "probe listener name must not be empty")
            }
            Self::InvalidProbeListenerName { name, reason } => {
                write!(formatter, "probe listener name {name} is invalid: {reason}")
            }
            Self::DuplicateProbeListener { point, name } => write!(
                formatter,
                "probe listener {name} already exists for point {}",
                point.get()
            ),
            Self::DuplicateProbeListenerId { listener } => {
                write!(formatter, "duplicate probe listener id {}", listener.get())
            }
            Self::UnknownProbeListener { listener } => {
                write!(formatter, "unknown probe listener id {}", listener.get())
            }
            Self::ProbeListenerPointMismatch { point, listener } => write!(
                formatter,
                "probe listener {} is not attached to point {}",
                listener.get(),
                point.get()
            ),
            Self::ProbePointCursorBehind {
                next_point,
                highest_point,
            } => write!(
                formatter,
                "probe point cursor {next_point} does not exceed highest point id {}",
                highest_point.get()
            ),
            Self::ProbeListenerCursorBehind {
                next_listener,
                highest_listener,
            } => write!(
                formatter,
                "probe listener cursor {next_listener} does not exceed highest listener id {}",
                highest_listener.get()
            ),
            Self::ProbeEventCursorBehind {
                next_sequence,
                highest_sequence,
            } => write!(
                formatter,
                "probe event cursor {next_sequence} does not exceed highest event sequence {highest_sequence}"
            ),
            Self::ProbeEventSequenceNotIncreasing {
                previous_sequence,
                current_sequence,
            } => write!(
                formatter,
                "probe event sequence {current_sequence} does not exceed previous sequence {previous_sequence}"
            ),
            Self::ProbeEventTimeWentBack {
                previous_tick,
                current_tick,
            } => write!(
                formatter,
                "probe event tick {current_tick} is before previous tick {previous_tick}"
            ),
            Self::ProbePointSequenceOverflow => {
                write!(formatter, "probe point sequence overflowed")
            }
            Self::ProbeListenerSequenceOverflow => {
                write!(formatter, "probe listener sequence overflowed")
            }
            Self::ProbeSequenceOverflow => write!(formatter, "probe event sequence overflowed"),
            Self::DuplicatePcCountCounter { pc } => {
                write!(formatter, "PC-count snapshot has duplicate PC {pc:#x}")
            }
            Self::DuplicatePcCountTarget { pair } => write!(
                formatter,
                "PC-count snapshot has duplicate target pair ({:#x},{})",
                pair.pc(),
                pair.count()
            ),
            Self::MissingPcCountCounter { pc } => write!(
                formatter,
                "PC-count snapshot target references missing PC counter {pc:#x}"
            ),
            Self::UnreachablePcCountTarget {
                pair,
                current_count,
            } => write!(
                formatter,
                "PC-count snapshot target pair ({:#x},{}) is not above restored count {current_count}",
                pair.pc(),
                pair.count()
            ),
            Self::PcCountSnapshotTargetStateMismatch {
                armed,
                pending_targets,
            } => write!(
                formatter,
                "PC-count snapshot armed state {armed} conflicts with {pending_targets} pending targets"
            ),
            Self::DuplicateInstThreshold { threshold } => {
                write!(formatter, "instruction tracker has duplicate threshold {threshold}")
            }
            Self::UnreachableInstThreshold { threshold, counter } => write!(
                formatter,
                "instruction tracker threshold {threshold} is not above restored count {counter}"
            ),
            Self::InstTrackerCounterOverflow => {
                write!(formatter, "instruction tracker counter overflowed")
            }
            Self::InvalidMemFootprintGranularity { name, bytes } => write!(
                formatter,
                "memory footprint {name} granularity {bytes} is not a nonzero power of two"
            ),
            Self::MemFootprintGranularityOrder {
                cache_line_size,
                page_size,
            } => write!(
                formatter,
                "memory footprint page size {page_size} is smaller than cache line size {cache_line_size}"
            ),
            Self::EmptyMemFootprintAddressMap => {
                write!(formatter, "memory footprint address map must not be empty")
            }
            Self::EmptyMemFootprintAddressRange => {
                write!(formatter, "memory footprint address range must not be empty")
            }
            Self::MemFootprintAddressRangeOverflow { start, bytes } => write!(
                formatter,
                "memory footprint address range starting at {start:#x} with {bytes} bytes overflows"
            ),
            Self::OverlappingMemFootprintAddressRange {
                existing_start,
                existing_end,
                requested_start,
                requested_end,
            } => write!(
                formatter,
                "memory footprint address range {requested_start:#x}..{requested_end:#x} overlaps existing range {existing_start:#x}..{existing_end:#x}"
            ),
            Self::UnalignedMemFootprintSnapshotAddress {
                granularity,
                address,
            } => write!(
                formatter,
                "memory footprint {granularity} snapshot address {address:#x} is not aligned"
            ),
            Self::MemFootprintSnapshotAddressOutsideMemory {
                granularity,
                address,
            } => write!(
                formatter,
                "memory footprint {granularity} snapshot address {address:#x} is outside memory"
            ),
            Self::DuplicateMemFootprintSnapshotAddress {
                granularity,
                address,
            } => write!(
                formatter,
                "memory footprint {granularity} snapshot has duplicate address {address:#x}"
            ),
            Self::MemFootprintSnapshotCurrentNotInTotal {
                granularity,
                address,
            } => write!(
                formatter,
                "memory footprint {granularity} current snapshot address {address:#x} is missing from total footprint"
            ),
            Self::MemFootprintSnapshotGranularityMismatch {
                finer,
                coarser,
                address,
                parent,
            } => write!(
                formatter,
                "memory footprint {finer} snapshot address {address:#x} is missing parent {coarser} address {parent:#x}"
            ),
            Self::MemFootprintSnapshotExceedsMemory {
                granularity,
                observed,
                capacity,
            } => write!(
                formatter,
                "memory footprint {granularity} snapshot has {observed} entries but capacity is {capacity}"
            ),
            Self::MemFootprintValueOverflow { granularity } => write!(
                formatter,
                "memory footprint {granularity} value overflowed"
            ),
            Self::GroupSequenceOverflow => write!(formatter, "stat group sequence overflowed"),
            Self::DumpSequenceOverflow => write!(formatter, "stat dump sequence overflowed"),
            Self::ResetSequenceOverflow => write!(formatter, "stat reset sequence overflowed"),
        }
    }
}

impl Error for StatsError {}
