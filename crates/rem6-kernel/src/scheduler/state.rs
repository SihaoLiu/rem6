use crate::scheduler::{PartitionEventId, PartitionId, RunSummary};
use crate::Tick;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EpochPlan {
    pub(super) horizon: Tick,
    pub(super) ready_partitions: Vec<ReadyPartition>,
}

impl EpochPlan {
    pub fn horizon(&self) -> Tick {
        self.horizon
    }

    pub fn ready_partitions(&self) -> &[ReadyPartition] {
        &self.ready_partitions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelEpochPlan {
    horizon: Tick,
    ready_partitions: Vec<ReadyPartition>,
    frontiers: Vec<PartitionFrontier>,
    serial_blockers: Vec<SchedulerDispatchRecord>,
}

impl ParallelEpochPlan {
    pub fn new(
        horizon: Tick,
        ready_partitions: Vec<ReadyPartition>,
        frontiers: Vec<PartitionFrontier>,
        serial_blockers: Vec<SchedulerDispatchRecord>,
    ) -> Self {
        Self {
            horizon,
            ready_partitions,
            frontiers,
            serial_blockers,
        }
    }

    pub fn horizon(&self) -> Tick {
        self.horizon
    }

    pub fn ready_partitions(&self) -> &[ReadyPartition] {
        &self.ready_partitions
    }

    pub fn ready_partition_count(&self) -> usize {
        self.ready_partitions.len()
    }

    pub fn frontiers(&self) -> &[PartitionFrontier] {
        &self.frontiers
    }

    pub fn frontier(&self, partition: PartitionId) -> Option<PartitionFrontier> {
        self.frontiers
            .iter()
            .copied()
            .find(|frontier| frontier.partition == partition)
    }

    pub fn frontier_count(&self) -> usize {
        self.frontiers.len()
    }

    pub fn serial_blockers(&self) -> &[SchedulerDispatchRecord] {
        &self.serial_blockers
    }

    pub fn serial_blocker_count(&self) -> usize {
        self.serial_blockers.len()
    }

    pub fn first_serial_blocker(&self) -> Option<SchedulerDispatchRecord> {
        self.serial_blockers.first().copied()
    }

    pub fn is_parallel_safe(&self) -> bool {
        self.serial_blockers.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PartitionFrontier {
    partition: PartitionId,
    now: Tick,
    safe_until: Tick,
    next_tick: Option<Tick>,
    pending_events: usize,
}

impl PartitionFrontier {
    pub const fn new(
        partition: PartitionId,
        now: Tick,
        safe_until: Tick,
        next_tick: Option<Tick>,
        pending_events: usize,
    ) -> Self {
        Self {
            partition,
            now,
            safe_until,
            next_tick,
            pending_events,
        }
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn now(self) -> Tick {
        self.now
    }

    pub const fn safe_until(self) -> Tick {
        self.safe_until
    }

    pub const fn next_tick(self) -> Option<Tick> {
        self.next_tick
    }

    pub const fn pending_events(self) -> usize {
        self.pending_events
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReadyPartition {
    pub partition: PartitionId,
    pub next_tick: Tick,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScheduledEventKind {
    Serial,
    Parallel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerSnapshot {
    pub(super) now: Tick,
    pub(super) min_remote_delay: Tick,
    pub(super) partitions: Vec<PartitionSnapshot>,
}

impl SchedulerSnapshot {
    pub fn new(now: Tick, min_remote_delay: Tick, partitions: Vec<PartitionSnapshot>) -> Self {
        Self {
            now,
            min_remote_delay,
            partitions,
        }
    }

    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn min_remote_delay(&self) -> Tick {
        self.min_remote_delay
    }

    pub fn partitions(&self) -> &[PartitionSnapshot] {
        &self.partitions
    }

    pub fn total_pending_events(&self) -> usize {
        self.partitions
            .iter()
            .map(|partition| partition.pending_events.len())
            .sum()
    }

    pub fn is_quiescent(&self) -> bool {
        self.total_pending_events() == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionSnapshot {
    pub(super) partition: PartitionId,
    pub(super) now: Tick,
    pub(super) next_event_local: u64,
    pub(super) next_event_order: u64,
    pub(super) pending_events: Vec<PendingEventSnapshot>,
}

impl PartitionSnapshot {
    pub fn quiescent(
        partition: PartitionId,
        now: Tick,
        next_event_local: u64,
        next_event_order: u64,
    ) -> Self {
        Self {
            partition,
            now,
            next_event_local,
            next_event_order,
            pending_events: Vec::new(),
        }
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn next_event_local(&self) -> u64 {
        self.next_event_local
    }

    pub fn next_event_order(&self) -> u64 {
        self.next_event_order
    }

    pub fn pending_events(&self) -> &[PendingEventSnapshot] {
        &self.pending_events
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PendingEventSnapshot {
    pub(super) id: PartitionEventId,
    pub(super) tick: Tick,
    pub(super) order: u64,
    pub(super) kind: ScheduledEventKind,
}

impl PendingEventSnapshot {
    pub fn id(self) -> PartitionEventId {
        self.id
    }

    pub fn partition(self) -> PartitionId {
        self.id.partition()
    }

    pub fn tick(self) -> Tick {
        self.tick
    }

    pub fn order(self) -> u64 {
        self.order
    }

    pub fn kind(self) -> ScheduledEventKind {
        self.kind
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct SchedulerDispatchRecord {
    pub(super) id: PartitionEventId,
    pub(super) tick: Tick,
    pub(super) kind: ScheduledEventKind,
}

impl SchedulerDispatchRecord {
    pub const fn new(id: PartitionEventId, tick: Tick, kind: ScheduledEventKind) -> Self {
        Self { id, tick, kind }
    }

    pub const fn id(self) -> PartitionEventId {
        self.id
    }

    pub const fn partition(self) -> PartitionId {
        self.id.partition()
    }

    pub const fn tick(self) -> Tick {
        self.tick
    }

    pub const fn kind(self) -> ScheduledEventKind {
        self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedRunSummary {
    pub(super) summary: RunSummary,
    pub(super) dispatches: Vec<SchedulerDispatchRecord>,
}

impl RecordedRunSummary {
    pub fn summary(&self) -> RunSummary {
        self.summary
    }

    pub fn dispatches(&self) -> &[SchedulerDispatchRecord] {
        &self.dispatches
    }
}
