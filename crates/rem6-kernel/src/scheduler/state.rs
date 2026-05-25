use std::collections::BTreeMap;

use crate::scheduler::{ConservativeRunSummary, PartitionEventId, PartitionId, RunSummary};
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelEpochBatchRecord {
    horizon: Tick,
    workers: Vec<ParallelWorkerRecord>,
    dispatches: Vec<SchedulerDispatchRecord>,
    remote_send_count: usize,
}

impl ParallelEpochBatchRecord {
    pub fn new(
        horizon: Tick,
        workers: Vec<ParallelWorkerRecord>,
        dispatches: Vec<SchedulerDispatchRecord>,
        remote_send_count: usize,
    ) -> Self {
        Self {
            horizon,
            workers,
            dispatches,
            remote_send_count,
        }
    }

    pub fn horizon(&self) -> Tick {
        self.horizon
    }

    pub fn workers(&self) -> &[ParallelWorkerRecord] {
        &self.workers
    }

    pub fn worker_count(&self) -> usize {
        self.workers.len()
    }

    pub fn worker_partitions(&self) -> Vec<PartitionId> {
        self.workers
            .iter()
            .map(|worker| worker.partition())
            .collect()
    }

    pub fn contains_worker(&self, partition: PartitionId) -> bool {
        self.workers
            .iter()
            .any(|worker| worker.partition() == partition)
    }

    pub fn dispatches(&self) -> &[SchedulerDispatchRecord] {
        &self.dispatches
    }

    pub fn dispatch_count(&self) -> usize {
        self.dispatches.len()
    }

    pub fn remote_send_count(&self) -> usize {
        self.remote_send_count
    }

    pub fn dispatches_for_partition(&self, partition: PartitionId) -> Vec<SchedulerDispatchRecord> {
        self.dispatches
            .iter()
            .copied()
            .filter(|record| record.partition() == partition)
            .collect()
    }

    pub fn dispatch_count_for_partition(&self, partition: PartitionId) -> usize {
        self.dispatches_for_partition(partition).len()
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        collect_parallel_partition_activity([self])
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParallelWorkerRecord {
    partition: PartitionId,
    start_tick: Tick,
    safe_until: Tick,
    next_tick: Option<Tick>,
    pending_events: usize,
}

impl ParallelWorkerRecord {
    pub const fn new(
        partition: PartitionId,
        start_tick: Tick,
        safe_until: Tick,
        next_tick: Option<Tick>,
        pending_events: usize,
    ) -> Self {
        Self {
            partition,
            start_tick,
            safe_until,
            next_tick,
            pending_events,
        }
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn start_tick(self) -> Tick {
        self.start_tick
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

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ParallelPartitionActivity {
    worker_count: usize,
    dispatch_count: usize,
    max_pending_events: usize,
}

impl ParallelPartitionActivity {
    pub const fn new(
        worker_count: usize,
        dispatch_count: usize,
        max_pending_events: usize,
    ) -> Self {
        Self {
            worker_count,
            dispatch_count,
            max_pending_events,
        }
    }

    pub const fn worker_count(self) -> usize {
        self.worker_count
    }

    pub const fn dispatch_count(self) -> usize {
        self.dispatch_count
    }

    pub const fn max_pending_events(self) -> usize {
        self.max_pending_events
    }

    pub const fn has_activity(self) -> bool {
        self.worker_count != 0 || self.dispatch_count != 0
    }

    fn record_worker(&mut self, worker: ParallelWorkerRecord) {
        self.worker_count += 1;
        self.max_pending_events = self.max_pending_events.max(worker.pending_events());
    }

    fn record_dispatch(&mut self) {
        self.dispatch_count += 1;
    }

    fn merge(self, other: Self) -> Self {
        Self {
            worker_count: self.worker_count + other.worker_count,
            dispatch_count: self.dispatch_count + other.dispatch_count,
            max_pending_events: self.max_pending_events.max(other.max_pending_events),
        }
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
    pub(super) max_parallel_workers: usize,
    pub(super) partitions: Vec<PartitionSnapshot>,
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
    pub(super) batches: Vec<ParallelEpochBatchRecord>,
    pub(super) profile: ParallelRunProfile,
}

impl RecordedRunSummary {
    pub fn summary(&self) -> RunSummary {
        self.summary
    }

    pub fn dispatches(&self) -> &[SchedulerDispatchRecord] {
        &self.dispatches
    }

    pub fn batches(&self) -> &[ParallelEpochBatchRecord] {
        &self.batches
    }

    pub fn profile(&self) -> ParallelRunProfile {
        self.profile
    }

    pub fn dispatch_count(&self) -> usize {
        self.profile.dispatch_count()
    }

    pub fn remote_send_count(&self) -> usize {
        self.batches
            .iter()
            .map(ParallelEpochBatchRecord::remote_send_count)
            .sum()
    }

    pub fn batch_count(&self) -> usize {
        self.profile.batch_count()
    }

    pub fn empty_epoch_count(&self) -> usize {
        self.profile.empty_epoch_count()
    }

    pub fn is_empty_epoch(&self) -> bool {
        self.profile.empty_epoch_count() != 0
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.profile.max_parallel_workers()
    }

    pub fn total_parallel_workers(&self) -> usize {
        self.profile.total_parallel_workers()
    }

    pub fn has_parallel_work(&self) -> bool {
        self.profile.has_parallel_work()
    }

    pub fn parallel_worker_partitions(&self) -> Vec<PartitionId> {
        self.batches
            .iter()
            .flat_map(ParallelEpochBatchRecord::worker_partitions)
            .collect()
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        collect_parallel_partition_activity(&self.batches)
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ParallelRunProfile {
    epoch_count: usize,
    empty_epoch_count: usize,
    batch_count: usize,
    dispatch_count: usize,
    total_parallel_workers: usize,
    max_parallel_workers: usize,
}

impl ParallelRunProfile {
    pub const fn new(
        epoch_count: usize,
        empty_epoch_count: usize,
        batch_count: usize,
        dispatch_count: usize,
        total_parallel_workers: usize,
        max_parallel_workers: usize,
    ) -> Self {
        Self {
            epoch_count,
            empty_epoch_count,
            batch_count,
            dispatch_count,
            total_parallel_workers,
            max_parallel_workers,
        }
    }

    pub fn for_epoch(
        batches: &[ParallelEpochBatchRecord],
        dispatch_count: usize,
        empty_epoch: bool,
    ) -> Self {
        Self {
            epoch_count: 1,
            empty_epoch_count: usize::from(empty_epoch),
            batch_count: batches.len(),
            dispatch_count,
            total_parallel_workers: batches
                .iter()
                .map(ParallelEpochBatchRecord::worker_count)
                .sum(),
            max_parallel_workers: batches
                .iter()
                .map(ParallelEpochBatchRecord::worker_count)
                .max()
                .unwrap_or(0),
        }
    }

    pub const fn epoch_count(self) -> usize {
        self.epoch_count
    }

    pub const fn empty_epoch_count(self) -> usize {
        self.empty_epoch_count
    }

    pub const fn batch_count(self) -> usize {
        self.batch_count
    }

    pub const fn dispatch_count(self) -> usize {
        self.dispatch_count
    }

    pub const fn total_parallel_workers(self) -> usize {
        self.total_parallel_workers
    }

    pub const fn max_parallel_workers(self) -> usize {
        self.max_parallel_workers
    }

    pub const fn has_parallel_work(self) -> bool {
        self.total_parallel_workers != 0
    }

    pub fn merge(self, other: Self) -> Self {
        Self {
            epoch_count: self.epoch_count + other.epoch_count,
            empty_epoch_count: self.empty_epoch_count + other.empty_epoch_count,
            batch_count: self.batch_count + other.batch_count,
            dispatch_count: self.dispatch_count + other.dispatch_count,
            total_parallel_workers: self.total_parallel_workers + other.total_parallel_workers,
            max_parallel_workers: self.max_parallel_workers.max(other.max_parallel_workers),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RecordedConservativeRunSummary {
    pub(super) summary: ConservativeRunSummary,
    pub(super) epochs: Vec<RecordedRunSummary>,
    pub(super) profile: ParallelRunProfile,
}

impl RecordedConservativeRunSummary {
    pub fn empty(final_tick: Tick) -> Self {
        Self {
            summary: ConservativeRunSummary {
                epochs: 0,
                executed_events: 0,
                final_tick,
            },
            epochs: Vec::new(),
            profile: ParallelRunProfile::default(),
        }
    }

    pub fn summary(&self) -> ConservativeRunSummary {
        self.summary
    }

    pub fn epochs(&self) -> &[RecordedRunSummary] {
        &self.epochs
    }

    pub fn epoch_count(&self) -> usize {
        self.epochs.len()
    }

    pub fn profile(&self) -> ParallelRunProfile {
        self.profile
    }

    pub fn dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.epochs
            .iter()
            .flat_map(|epoch| epoch.dispatches().iter().copied())
            .collect()
    }

    pub fn batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.epochs
            .iter()
            .flat_map(|epoch| epoch.batches().iter().cloned())
            .collect()
    }

    pub fn dispatch_count(&self) -> usize {
        self.profile.dispatch_count()
    }

    pub fn remote_send_count(&self) -> usize {
        self.epochs
            .iter()
            .map(RecordedRunSummary::remote_send_count)
            .sum()
    }

    pub fn batch_count(&self) -> usize {
        self.profile.batch_count()
    }

    pub fn empty_epoch_count(&self) -> usize {
        self.profile.empty_epoch_count()
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.profile.max_parallel_workers()
    }

    pub fn total_parallel_workers(&self) -> usize {
        self.profile.total_parallel_workers()
    }

    pub fn has_parallel_work(&self) -> bool {
        self.profile.has_parallel_work()
    }

    pub fn parallel_worker_partitions(&self) -> Vec<PartitionId> {
        self.epochs
            .iter()
            .flat_map(RecordedRunSummary::parallel_worker_partitions)
            .collect()
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        let mut activities = BTreeMap::new();
        for epoch in &self.epochs {
            merge_parallel_partition_activities(&mut activities, epoch.partition_activities());
        }
        activities
    }
}

fn collect_parallel_partition_activity<'a, I>(
    batches: I,
) -> BTreeMap<PartitionId, ParallelPartitionActivity>
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    let mut activities: BTreeMap<PartitionId, ParallelPartitionActivity> = BTreeMap::new();
    for batch in batches {
        for worker in batch.workers() {
            activities
                .entry(worker.partition())
                .or_default()
                .record_worker(*worker);
        }
        for dispatch in batch.dispatches() {
            activities
                .entry(dispatch.partition())
                .or_default()
                .record_dispatch();
        }
    }
    activities
}

fn merge_parallel_partition_activities(
    target: &mut BTreeMap<PartitionId, ParallelPartitionActivity>,
    source: BTreeMap<PartitionId, ParallelPartitionActivity>,
) {
    for (partition, activity) in source {
        target
            .entry(partition)
            .and_modify(|stored| *stored = stored.merge(activity))
            .or_insert(activity);
    }
}
