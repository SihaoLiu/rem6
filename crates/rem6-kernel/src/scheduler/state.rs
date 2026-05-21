use crate::scheduler::{PartitionEventId, PartitionId, RunSummary};
use crate::Tick;

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
