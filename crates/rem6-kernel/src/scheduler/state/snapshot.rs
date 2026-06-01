use crate::scheduler::{PartitionEventId, PartitionId};
use crate::Tick;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum ScheduledEventKind {
    Serial,
    Parallel,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SchedulerSnapshot {
    pub(in crate::scheduler) now: Tick,
    pub(in crate::scheduler) min_remote_delay: Tick,
    pub(in crate::scheduler) max_parallel_workers: usize,
    pub(in crate::scheduler) partitions: Vec<PartitionSnapshot>,
}

impl SchedulerSnapshot {
    pub fn new(now: Tick, min_remote_delay: Tick, partitions: Vec<PartitionSnapshot>) -> Self {
        Self::with_parallel_worker_limit(now, min_remote_delay, usize::MAX, partitions)
    }

    pub fn with_parallel_worker_limit(
        now: Tick,
        min_remote_delay: Tick,
        max_parallel_workers: usize,
        partitions: Vec<PartitionSnapshot>,
    ) -> Self {
        Self {
            now,
            min_remote_delay,
            max_parallel_workers,
            partitions,
        }
    }

    pub fn now(&self) -> Tick {
        self.now
    }

    pub fn min_remote_delay(&self) -> Tick {
        self.min_remote_delay
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.max_parallel_workers
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
    pub(in crate::scheduler) partition: PartitionId,
    pub(in crate::scheduler) now: Tick,
    pub(in crate::scheduler) next_event_local: u64,
    pub(in crate::scheduler) next_event_order: u64,
    pub(in crate::scheduler) next_remote_order: u64,
    pub(in crate::scheduler) next_progress_order: u64,
    pub(in crate::scheduler) pending_events: Vec<PendingEventSnapshot>,
}

impl PartitionSnapshot {
    pub fn quiescent(
        partition: PartitionId,
        now: Tick,
        next_event_local: u64,
        next_event_order: u64,
    ) -> Self {
        Self::quiescent_with_remote_order(partition, now, next_event_local, next_event_order, 0)
    }

    pub fn quiescent_with_remote_order(
        partition: PartitionId,
        now: Tick,
        next_event_local: u64,
        next_event_order: u64,
        next_remote_order: u64,
    ) -> Self {
        Self::quiescent_with_orders(
            partition,
            now,
            next_event_local,
            next_event_order,
            next_remote_order,
            0,
        )
    }

    pub fn quiescent_with_orders(
        partition: PartitionId,
        now: Tick,
        next_event_local: u64,
        next_event_order: u64,
        next_remote_order: u64,
        next_progress_order: u64,
    ) -> Self {
        Self {
            partition,
            now,
            next_event_local,
            next_event_order,
            next_remote_order,
            next_progress_order,
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

    pub fn next_remote_order(&self) -> u64 {
        self.next_remote_order
    }

    pub fn next_progress_order(&self) -> u64 {
        self.next_progress_order
    }

    pub fn pending_events(&self) -> &[PendingEventSnapshot] {
        &self.pending_events
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct PendingEventSnapshot {
    pub(in crate::scheduler) id: PartitionEventId,
    pub(in crate::scheduler) tick: Tick,
    pub(in crate::scheduler) order: u64,
    pub(in crate::scheduler) kind: ScheduledEventKind,
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
    pub(in crate::scheduler) id: PartitionEventId,
    pub(in crate::scheduler) tick: Tick,
    pub(in crate::scheduler) kind: ScheduledEventKind,
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
