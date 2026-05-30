use std::error::Error;
use std::fmt;

use crate::Tick;

use super::{PartitionEventId, PartitionId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SchedulerError {
    NoPartitions,
    ZeroLookahead,
    ZeroParallelWorkers,
    UnknownPartition {
        partition: PartitionId,
        partitions: u32,
    },
    InThePast {
        partition: PartitionId,
        now: Tick,
        requested: Tick,
    },
    TickOverflow {
        now: Tick,
        delay: Tick,
    },
    ZeroDelayRemoteMessage {
        source: PartitionId,
        target: PartitionId,
    },
    RemoteDelayBelowLookahead {
        source: PartitionId,
        target: PartitionId,
        delay: Tick,
        minimum: Tick,
    },
    RemoteDeliveryBeforeLookaheadBoundary {
        source: PartitionId,
        target: PartitionId,
        source_tick: Tick,
        delivery_tick: Tick,
        minimum_delivery_tick: Tick,
    },
    SerialEventInParallelEpoch {
        partition: PartitionId,
        tick: Tick,
    },
    ParallelWorkerPanicked {
        partition: PartitionId,
    },
    EpochHorizonOverflow {
        partition: PartitionId,
        now: Tick,
        delay: Tick,
    },
    EventNotPending {
        id: PartitionEventId,
    },
    SnapshotContainsPendingEvents {
        pending_events: usize,
    },
    RestoreWouldDiscardPendingEvents {
        pending_events: usize,
    },
    SnapshotPartitionCountMismatch {
        snapshot_partitions: u32,
        scheduler_partitions: u32,
    },
    SnapshotPartitionIdMismatch {
        expected_partition: PartitionId,
        snapshot_partition: PartitionId,
    },
    SnapshotGlobalTickBeforePartitionClock {
        snapshot_now: Tick,
        partition: PartitionId,
        partition_now: Tick,
    },
    SnapshotLookaheadMismatch {
        snapshot_min_remote_delay: Tick,
        scheduler_min_remote_delay: Tick,
    },
    SnapshotParallelWorkerLimitMismatch {
        snapshot_max_parallel_workers: usize,
        scheduler_max_parallel_workers: usize,
    },
}

impl fmt::Display for SchedulerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPartitions => write!(formatter, "scheduler requires at least one partition"),
            Self::ZeroLookahead => write!(formatter, "scheduler lookahead must be positive"),
            Self::ZeroParallelWorkers => {
                write!(formatter, "scheduler parallel worker limit must be positive")
            }
            Self::UnknownPartition {
                partition,
                partitions,
            } => write!(
                formatter,
                "partition {} is outside scheduler partition count {partitions}",
                partition.index()
            ),
            Self::InThePast {
                partition,
                now,
                requested,
            } => write!(
                formatter,
                "cannot schedule partition {} at tick {requested}; current tick is {now}",
                partition.index()
            ),
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::ZeroDelayRemoteMessage { source, target } => write!(
                formatter,
                "remote message from partition {} to {} requires positive delay",
                source.index(),
                target.index()
            ),
            Self::RemoteDelayBelowLookahead {
                source,
                target,
                delay,
                minimum,
            } => write!(
                formatter,
                "remote message from partition {} to {} has delay {delay}; configured lookahead is {minimum}",
                source.index(),
                target.index()
            ),
            Self::RemoteDeliveryBeforeLookaheadBoundary {
                source,
                target,
                source_tick,
                delivery_tick,
                minimum_delivery_tick,
            } => write!(
                formatter,
                "remote message from partition {} to {} delivers at tick {delivery_tick}; source tick {source_tick} requires delivery no earlier than {minimum_delivery_tick}",
                source.index(),
                target.index()
            ),
            Self::SerialEventInParallelEpoch { partition, tick } => write!(
                formatter,
                "parallel epoch cannot dispatch serial event in partition {} at tick {tick}",
                partition.index()
            ),
            Self::ParallelWorkerPanicked { partition } => write!(
                formatter,
                "parallel worker for partition {} panicked",
                partition.index()
            ),
            Self::EpochHorizonOverflow {
                partition,
                now,
                delay,
            } => write!(
                formatter,
                "partition {} cannot compute parallel epoch horizon from tick {now} with delay {delay}",
                partition.index()
            ),
            Self::EventNotPending { id } => write!(
                formatter,
                "event {} in partition {} is not pending",
                id.local(),
                id.partition().index()
            ),
            Self::SnapshotContainsPendingEvents { pending_events } => write!(
                formatter,
                "scheduler snapshot contains {pending_events} pending events"
            ),
            Self::RestoreWouldDiscardPendingEvents { pending_events } => write!(
                formatter,
                "scheduler restore would discard {pending_events} pending events"
            ),
            Self::SnapshotPartitionCountMismatch {
                snapshot_partitions,
                scheduler_partitions,
            } => write!(
                formatter,
                "scheduler snapshot has {snapshot_partitions} partitions; scheduler has {scheduler_partitions}"
            ),
            Self::SnapshotPartitionIdMismatch {
                expected_partition,
                snapshot_partition,
            } => write!(
                formatter,
                "scheduler snapshot partition {} appears in slot {}",
                snapshot_partition.index(),
                expected_partition.index()
            ),
            Self::SnapshotGlobalTickBeforePartitionClock {
                snapshot_now,
                partition,
                partition_now,
            } => write!(
                formatter,
                "scheduler snapshot tick {snapshot_now} is before partition {} clock {partition_now}",
                partition.index()
            ),
            Self::SnapshotLookaheadMismatch {
                snapshot_min_remote_delay,
                scheduler_min_remote_delay,
            } => write!(
                formatter,
                "scheduler snapshot lookahead is {snapshot_min_remote_delay}; scheduler lookahead is {scheduler_min_remote_delay}"
            ),
            Self::SnapshotParallelWorkerLimitMismatch {
                snapshot_max_parallel_workers,
                scheduler_max_parallel_workers,
            } => write!(
                formatter,
                "scheduler snapshot parallel worker limit is {snapshot_max_parallel_workers}; scheduler parallel worker limit is {scheduler_max_parallel_workers}"
            ),
        }
    }
}

impl Error for SchedulerError {}
