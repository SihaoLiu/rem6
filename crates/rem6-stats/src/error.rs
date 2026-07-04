use std::error::Error;
use std::fmt;

use rem6_kernel::Tick;

use crate::kind::StatKind;
use crate::mem_footprint::MemFootprintGranularity;
use crate::pc_count::PcCountPair;
use crate::probes::{MemProbePacketAccess, ProbeListenerId, ProbePointId};
use crate::reset::StatResetPolicy;
use crate::stat_metadata::{StatDescription, StatDescriptionError, StatPathError, StatUnitError};
use crate::stats::{StatGroupDescriptor, StatGroupId, StatId};

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
    StatIsNotResettable {
        stat: StatId,
        reset_policy: StatResetPolicy,
    },
    StatIsNotAverage {
        stat: StatId,
    },
    StatIsNotHistogram {
        stat: StatId,
    },
    UnknownStatGroup {
        group: StatGroupId,
    },
    CounterOverflow {
        stat: StatId,
    },
    HistogramBucketOverflow {
        stat: StatId,
        bucket: u64,
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
    SnapshotDeltaHistogramBucketWentBack {
        stat: StatId,
        bucket: u64,
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
    EmptyMemTraceObjectId,
    InvalidMemTraceTickFrequency {
        frequency: u64,
    },
    DuplicateMemTraceRequestor {
        requestor: u32,
    },
    EmptyMemTraceRequestorName {
        requestor: u32,
    },
    MemTraceRecordTimeWentBack {
        previous_tick: Tick,
        current_tick: Tick,
    },
    MemTraceSnapshotUnexpectedProgramCounter {
        tick: Tick,
        program_counter: u64,
    },
    MemTraceSnapshotZeroProgramCounter {
        tick: Tick,
    },
    InvalidMemCheckerAccessSize {
        size: usize,
    },
    MemCheckerAddressRangeOverflow {
        address: u64,
        size: usize,
    },
    MemCheckerTransactionTimeWentBack {
        serial: u64,
        start_tick: Tick,
        complete_tick: Tick,
    },
    MemCheckerWriteAlreadyCompleted {
        serial: u64,
    },
    DuplicateMemCheckerSnapshotAddress {
        address: u64,
    },
    EmptyMemCheckerReadObservations {
        address: u64,
    },
    DuplicateMemCheckerSnapshotSerial {
        serial: u64,
    },
    MemCheckerSnapshotSerialCursorBehind {
        next_serial: u64,
        highest_serial: u64,
    },
    MemCheckerSnapshotClusterIncompleteMismatch {
        expected: usize,
        observed: usize,
    },
    MemCheckerSnapshotClusterCompletionMismatch {
        expected: Tick,
        observed: Tick,
    },
    MemCheckerSerialOverflow,
    MemCheckerCounterOverflow {
        name: &'static str,
    },
    MemCheckerMonitorRequestDataMissing {
        packet_id: u64,
    },
    MemCheckerMonitorRequestDataSizeMismatch {
        packet_id: u64,
        packet_size: u64,
        data_size: usize,
    },
    DuplicateMemCheckerMonitorPendingPacket {
        packet_id: u64,
    },
    DuplicateMemCheckerMonitorPendingSerial {
        serial: u64,
    },
    UnknownMemCheckerMonitorPendingPacket {
        packet_id: u64,
    },
    InvalidMemCheckerMonitorPendingAccess {
        packet_id: u64,
        access: MemProbePacketAccess,
    },
    MemCheckerMonitorPendingSerialNotAllocated {
        packet_id: u64,
        serial: u64,
        next_serial: u64,
    },
    MemCheckerMonitorResponseAccessMismatch {
        packet_id: u64,
        request_access: MemProbePacketAccess,
        response_access: MemProbePacketAccess,
    },
    MemCheckerMonitorResponseAddressMismatch {
        packet_id: u64,
        request_address: u64,
        response_address: u64,
    },
    MemCheckerMonitorResponseSizeMismatch {
        packet_id: u64,
        request_size: u64,
        response_size: u64,
    },
    MemCheckerMonitorResponseDataMissing {
        packet_id: u64,
    },
    MemCheckerMonitorResponseDataSizeMismatch {
        packet_id: u64,
        packet_size: u64,
        data_size: usize,
    },
    InvalidStackDistLineSize {
        line_size: u64,
    },
    InvalidStackDistSystemLineSize {
        system_line_size: u64,
    },
    StackDistLineSizeSmallerThanSystem {
        line_size: u64,
        system_line_size: u64,
    },
    InvalidStackDistHistogramBins {
        name: &'static str,
        bins: usize,
    },
    UnalignedStackDistSnapshotAddress {
        line_size: u64,
        address: u64,
    },
    DuplicateStackDistSnapshotAddress {
        address: u64,
    },
    DuplicateStackDistHistogramBucket {
        name: &'static str,
        bucket: u64,
    },
    StackDistSampleCountOverflow,
    StackDistSnapshotSampleCountMismatch {
        expected: u64,
        observed: u64,
    },
    StackDistSnapshotStackDepthMismatch {
        stack_depth: u64,
        infinite_samples: u64,
    },
    InvalidCommMonitorSamplePeriod {
        sample_period: u64,
    },
    InvalidCommMonitorTickFrequency {
        tick_frequency: u64,
    },
    InvalidCommMonitorHistogramBins {
        name: &'static str,
        bins: usize,
    },
    DuplicateCommMonitorPendingRequest {
        packet_id: u64,
    },
    InvalidCommMonitorPendingAccess {
        packet_id: u64,
        access: MemProbePacketAccess,
    },
    UnknownCommMonitorPendingRequest {
        packet_id: u64,
    },
    CommMonitorResponseTimeWentBack {
        packet_id: u64,
        request_tick: Tick,
        response_tick: Tick,
    },
    CommMonitorRequestTimeWentBack {
        previous_tick: Tick,
        current_tick: Tick,
    },
    CommMonitorResponseAccessMismatch {
        packet_id: u64,
        request_access: MemProbePacketAccess,
        response_access: MemProbePacketAccess,
    },
    DuplicateCommMonitorHistogramBucket {
        name: &'static str,
        bucket: u64,
    },
    CommMonitorSnapshotOutstandingMismatch {
        access: MemProbePacketAccess,
        expected: u64,
        observed: u64,
    },
    CommMonitorCounterOverflow {
        name: &'static str,
    },
    CommMonitorSamplePeriodOverflow {
        last_sample_tick: Tick,
        sample_period: Tick,
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
            Self::StatIsNotResettable { stat, reset_policy } => write!(
                formatter,
                "stat {} has {reset_policy} reset policy and cannot be overwritten",
                stat.get()
            ),
            Self::StatIsNotAverage { stat } => {
                write!(formatter, "stat {} is not an average", stat.get())
            }
            Self::StatIsNotHistogram { stat } => {
                write!(formatter, "stat {} is not a histogram", stat.get())
            }
            Self::UnknownStatGroup { group } => {
                write!(formatter, "unknown stat group id {}", group.get())
            }
            Self::CounterOverflow { stat } => {
                write!(formatter, "counter {} overflowed", stat.get())
            }
            Self::HistogramBucketOverflow { stat, bucket } => write!(
                formatter,
                "histogram {} bucket {bucket} overflowed",
                stat.get()
            ),
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
            Self::SnapshotDeltaHistogramBucketWentBack {
                stat,
                bucket,
                previous,
                current,
            } => write!(
                formatter,
                "stat snapshot delta histogram bucket {bucket} for stat {} went from {previous} down to {current}",
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
            Self::EmptyMemTraceObjectId => write!(
                formatter,
                "memory trace object id must not be empty"
            ),
            Self::InvalidMemTraceTickFrequency { frequency } => write!(
                formatter,
                "memory trace tick frequency {frequency} must not be zero"
            ),
            Self::DuplicateMemTraceRequestor { requestor } => write!(
                formatter,
                "memory trace requestor id {requestor} appears more than once"
            ),
            Self::EmptyMemTraceRequestorName { requestor } => write!(
                formatter,
                "memory trace requestor id {requestor} has an empty name"
            ),
            Self::MemTraceRecordTimeWentBack {
                previous_tick,
                current_tick,
            } => write!(
                formatter,
                "memory trace record tick {current_tick} is before previous tick {previous_tick}"
            ),
            Self::MemTraceSnapshotUnexpectedProgramCounter {
                tick,
                program_counter,
            } => write!(
                formatter,
                "memory trace snapshot record at tick {tick} has unexpected PC {program_counter:#x}"
            ),
            Self::MemTraceSnapshotZeroProgramCounter { tick } => write!(
                formatter,
                "memory trace snapshot record at tick {tick} carries a zero PC"
            ),
            Self::InvalidMemCheckerAccessSize { size } => {
                write!(formatter, "memory checker access size {size} must not be zero")
            }
            Self::MemCheckerAddressRangeOverflow { address, size } => write!(
                formatter,
                "memory checker address range starting at {address:#x} with size {size} overflows"
            ),
            Self::MemCheckerTransactionTimeWentBack {
                serial,
                start_tick,
                complete_tick,
            } => write!(
                formatter,
                "memory checker transaction {serial} completed at tick {complete_tick} before start tick {start_tick}"
            ),
            Self::MemCheckerWriteAlreadyCompleted { serial } => write!(
                formatter,
                "memory checker write transaction {serial} was already completed"
            ),
            Self::DuplicateMemCheckerSnapshotAddress { address } => write!(
                formatter,
                "memory checker snapshot has duplicate byte address {address:#x}"
            ),
            Self::EmptyMemCheckerReadObservations { address } => write!(
                formatter,
                "memory checker snapshot byte {address:#x} has no read observations"
            ),
            Self::DuplicateMemCheckerSnapshotSerial { serial } => write!(
                formatter,
                "memory checker snapshot has duplicate transaction serial {serial}"
            ),
            Self::MemCheckerSnapshotSerialCursorBehind {
                next_serial,
                highest_serial,
            } => write!(
                formatter,
                "memory checker snapshot next serial {next_serial} does not exceed highest serial {highest_serial}"
            ),
            Self::MemCheckerSnapshotClusterIncompleteMismatch { expected, observed } => write!(
                formatter,
                "memory checker write cluster reports {observed} incomplete writes but contains {expected}"
            ),
            Self::MemCheckerSnapshotClusterCompletionMismatch { expected, observed } => write!(
                formatter,
                "memory checker write cluster completion maximum is {observed} but completed writes require {expected}"
            ),
            Self::MemCheckerSerialOverflow => write!(formatter, "memory checker serial overflowed"),
            Self::MemCheckerCounterOverflow { name } => {
                write!(formatter, "memory checker counter {name} overflowed")
            }
            Self::MemCheckerMonitorRequestDataMissing { packet_id } => write!(
                formatter,
                "memory checker monitor request packet {packet_id} is a write without request data"
            ),
            Self::MemCheckerMonitorRequestDataSizeMismatch {
                packet_id,
                packet_size,
                data_size,
            } => write!(
                formatter,
                "memory checker monitor request packet {packet_id} has size {packet_size} but carries {data_size} data bytes"
            ),
            Self::DuplicateMemCheckerMonitorPendingPacket { packet_id } => write!(
                formatter,
                "memory checker monitor already has pending packet {packet_id}"
            ),
            Self::DuplicateMemCheckerMonitorPendingSerial { serial } => write!(
                formatter,
                "memory checker monitor already has pending transaction serial {serial}"
            ),
            Self::UnknownMemCheckerMonitorPendingPacket { packet_id } => write!(
                formatter,
                "memory checker monitor response packet {packet_id} has no pending request"
            ),
            Self::InvalidMemCheckerMonitorPendingAccess { packet_id, access } => write!(
                formatter,
                "memory checker monitor pending packet {packet_id} has unsupported access {access:?}"
            ),
            Self::MemCheckerMonitorPendingSerialNotAllocated {
                packet_id,
                serial,
                next_serial,
            } => write!(
                formatter,
                "memory checker monitor pending packet {packet_id} references serial {serial} outside allocated cursor {next_serial}"
            ),
            Self::MemCheckerMonitorResponseAccessMismatch {
                packet_id,
                request_access,
                response_access,
            } => write!(
                formatter,
                "memory checker monitor response packet {packet_id} has access {response_access:?} but pending request uses {request_access:?}"
            ),
            Self::MemCheckerMonitorResponseAddressMismatch {
                packet_id,
                request_address,
                response_address,
            } => write!(
                formatter,
                "memory checker monitor response packet {packet_id} has address {response_address:#x} but pending request uses {request_address:#x}"
            ),
            Self::MemCheckerMonitorResponseSizeMismatch {
                packet_id,
                request_size,
                response_size,
            } => write!(
                formatter,
                "memory checker monitor response packet {packet_id} has size {response_size} but pending request uses {request_size}"
            ),
            Self::MemCheckerMonitorResponseDataMissing { packet_id } => write!(
                formatter,
                "memory checker monitor response packet {packet_id} is a read without response data"
            ),
            Self::MemCheckerMonitorResponseDataSizeMismatch {
                packet_id,
                packet_size,
                data_size,
            } => write!(
                formatter,
                "memory checker monitor response packet {packet_id} has size {packet_size} but carries {data_size} data bytes"
            ),
            Self::InvalidStackDistLineSize { line_size } => write!(
                formatter,
                "stack distance line size {line_size} is not a nonzero power of two"
            ),
            Self::InvalidStackDistSystemLineSize { system_line_size } => write!(
                formatter,
                "stack distance system line size {system_line_size} is not a nonzero power of two"
            ),
            Self::StackDistLineSizeSmallerThanSystem {
                line_size,
                system_line_size,
            } => write!(
                formatter,
                "stack distance line size {line_size} is smaller than system line size {system_line_size}"
            ),
            Self::InvalidStackDistHistogramBins { name, bins } => write!(
                formatter,
                "stack distance {name} histogram bin count {bins} must not be zero"
            ),
            Self::UnalignedStackDistSnapshotAddress { line_size, address } => write!(
                formatter,
                "stack distance snapshot address {address:#x} is not aligned to line size {line_size}"
            ),
            Self::DuplicateStackDistSnapshotAddress { address } => write!(
                formatter,
                "stack distance snapshot has duplicate address {address:#x}"
            ),
            Self::DuplicateStackDistHistogramBucket { name, bucket } => write!(
                formatter,
                "stack distance {name} histogram has duplicate bucket {bucket}"
            ),
            Self::StackDistSampleCountOverflow => {
                write!(formatter, "stack distance sample count overflowed")
            }
            Self::StackDistSnapshotSampleCountMismatch { expected, observed } => write!(
                formatter,
                "stack distance snapshot reports {expected} finite samples but histograms contain {observed}"
            ),
            Self::StackDistSnapshotStackDepthMismatch {
                stack_depth,
                infinite_samples,
            } => write!(
                formatter,
                "stack distance snapshot stack depth {stack_depth} does not match {infinite_samples} infinite samples"
            ),
            Self::InvalidCommMonitorSamplePeriod { sample_period } => write!(
                formatter,
                "communication monitor sample period {sample_period} must not be zero"
            ),
            Self::InvalidCommMonitorTickFrequency { tick_frequency } => write!(
                formatter,
                "communication monitor tick frequency {tick_frequency} must not be zero"
            ),
            Self::InvalidCommMonitorHistogramBins { name, bins } => write!(
                formatter,
                "communication monitor {name} histogram bin count {bins} must not be zero"
            ),
            Self::DuplicateCommMonitorPendingRequest { packet_id } => write!(
                formatter,
                "communication monitor has duplicate pending packet id {packet_id}"
            ),
            Self::InvalidCommMonitorPendingAccess { packet_id, access } => write!(
                formatter,
                "communication monitor pending packet id {packet_id} has unsupported access {access:?}"
            ),
            Self::UnknownCommMonitorPendingRequest { packet_id } => write!(
                formatter,
                "communication monitor has no pending packet id {packet_id}"
            ),
            Self::CommMonitorResponseTimeWentBack {
                packet_id,
                request_tick,
                response_tick,
            } => write!(
                formatter,
                "communication monitor response for packet id {packet_id} at tick {response_tick} is before request tick {request_tick}"
            ),
            Self::CommMonitorRequestTimeWentBack {
                previous_tick,
                current_tick,
            } => write!(
                formatter,
                "communication monitor request tick {current_tick} is before previous request tick {previous_tick}"
            ),
            Self::CommMonitorResponseAccessMismatch {
                packet_id,
                request_access,
                response_access,
            } => write!(
                formatter,
                "communication monitor response for packet id {packet_id} has access {response_access:?}, expected {request_access:?}"
            ),
            Self::DuplicateCommMonitorHistogramBucket { name, bucket } => write!(
                formatter,
                "communication monitor {name} histogram has duplicate bucket {bucket}"
            ),
            Self::CommMonitorSnapshotOutstandingMismatch {
                access,
                expected,
                observed,
            } => write!(
                formatter,
                "communication monitor snapshot has {observed} outstanding {access:?} packets but pending state contains {expected}"
            ),
            Self::CommMonitorCounterOverflow { name } => {
                write!(formatter, "communication monitor counter {name} overflowed")
            }
            Self::CommMonitorSamplePeriodOverflow {
                last_sample_tick,
                sample_period,
            } => write!(
                formatter,
                "communication monitor sample tick {last_sample_tick} plus period {sample_period} overflowed"
            ),
            Self::GroupSequenceOverflow => write!(formatter, "stat group sequence overflowed"),
            Self::DumpSequenceOverflow => write!(formatter, "stat dump sequence overflowed"),
            Self::ResetSequenceOverflow => write!(formatter, "stat reset sequence overflowed"),
        }
    }
}

impl Error for StatsError {}
