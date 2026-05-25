use std::collections::{BTreeMap, BTreeSet};

use crate::scheduler::{ConservativeRunSummary, PartitionEventId, PartitionId, RunSummary};
use crate::{
    LivelockTransitionKind, ProgressMonitor, ProgressMonitorError, ProgressMonitorSnapshot, Tick,
    WaitForNode,
};

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
    remote_sends: Vec<ParallelRemoteSendRecord>,
    progress_transitions: Vec<ParallelProgressTransitionRecord>,
}

impl ParallelEpochBatchRecord {
    pub fn new(
        horizon: Tick,
        workers: Vec<ParallelWorkerRecord>,
        dispatches: Vec<SchedulerDispatchRecord>,
        remote_sends: Vec<ParallelRemoteSendRecord>,
        progress_transitions: Vec<ParallelProgressTransitionRecord>,
    ) -> Self {
        Self {
            horizon,
            workers,
            dispatches,
            remote_sends,
            progress_transitions,
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
        self.remote_sends.len()
    }

    pub fn remote_sends(&self) -> &[ParallelRemoteSendRecord] {
        &self.remote_sends
    }

    pub fn remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_remote_source_partitions(self.remote_sends.iter())
    }

    pub fn remote_source_partition_count(&self) -> usize {
        self.remote_source_partitions().len()
    }

    pub fn remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_remote_target_partitions(self.remote_sends.iter())
    }

    pub fn remote_target_partition_count(&self) -> usize {
        self.remote_target_partitions().len()
    }

    pub fn progress_transitions(&self) -> &[ParallelProgressTransitionRecord] {
        &self.progress_transitions
    }

    pub fn progress_transition_count(&self) -> usize {
        self.progress_transitions.len()
    }

    pub fn progress_transition_count_for_partition(&self, partition: PartitionId) -> usize {
        self.progress_transitions
            .iter()
            .filter(|record| record.partition() == partition)
            .count()
    }

    pub fn progress_transition_count_by_kind(&self, kind: LivelockTransitionKind) -> usize {
        self.progress_transitions
            .iter()
            .filter(|record| record.kind() == kind)
            .count()
    }

    pub fn remote_send_count_for_partition(&self, partition: PartitionId) -> usize {
        self.remote_sends
            .iter()
            .filter(|record| record.source() == partition)
            .count()
    }

    pub fn remote_flow_count(&self, source: PartitionId, target: PartitionId) -> usize {
        self.remote_sends
            .iter()
            .filter(|record| record.source() == source && record.target() == target)
            .count()
    }

    pub fn remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flows(&self.remote_sends)
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelProgressTransitionRecord {
    partition: PartitionId,
    subject: WaitForNode,
    kind: LivelockTransitionKind,
    tick: Tick,
    order: u64,
}

impl ParallelProgressTransitionRecord {
    pub const fn new(
        partition: PartitionId,
        subject: WaitForNode,
        kind: LivelockTransitionKind,
        tick: Tick,
        order: u64,
    ) -> Self {
        Self {
            partition,
            subject,
            kind,
            tick,
            order,
        }
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn subject(&self) -> &WaitForNode {
        &self.subject
    }

    pub const fn kind(&self) -> LivelockTransitionKind {
        self.kind
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn order(&self) -> u64 {
        self.order
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParallelRemoteSendRecord {
    source: PartitionId,
    target: PartitionId,
    source_tick: Tick,
    delivery_tick: Tick,
    order: u64,
}

impl ParallelRemoteSendRecord {
    pub const fn new(source: PartitionId, target: PartitionId, tick: Tick, order: u64) -> Self {
        Self::with_timing(source, target, tick, tick, order)
    }

    pub const fn with_timing(
        source: PartitionId,
        target: PartitionId,
        source_tick: Tick,
        delivery_tick: Tick,
        order: u64,
    ) -> Self {
        Self {
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        }
    }

    pub const fn source(self) -> PartitionId {
        self.source
    }

    pub const fn target(self) -> PartitionId {
        self.target
    }

    pub const fn tick(self) -> Tick {
        self.delivery_tick
    }

    pub const fn source_tick(self) -> Tick {
        self.source_tick
    }

    pub const fn delivery_tick(self) -> Tick {
        self.delivery_tick
    }

    pub fn delay(self) -> Tick {
        self.delivery_tick.saturating_sub(self.source_tick)
    }

    pub const fn order(self) -> u64 {
        self.order
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ParallelRemoteFlowRecord {
    source: PartitionId,
    target: PartitionId,
    send_count: usize,
    first_tick: Tick,
    last_tick: Tick,
    minimum_delay: Option<Tick>,
    maximum_delay: Option<Tick>,
}

impl ParallelRemoteFlowRecord {
    pub const fn new(
        source: PartitionId,
        target: PartitionId,
        send_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Self {
        Self {
            source,
            target,
            send_count,
            first_tick,
            last_tick,
            minimum_delay: None,
            maximum_delay: None,
        }
    }

    pub const fn with_delay_bounds(
        source: PartitionId,
        target: PartitionId,
        send_count: usize,
        first_tick: Tick,
        last_tick: Tick,
        minimum_delay: Tick,
        maximum_delay: Tick,
    ) -> Self {
        Self {
            source,
            target,
            send_count,
            first_tick,
            last_tick,
            minimum_delay: Some(minimum_delay),
            maximum_delay: Some(maximum_delay),
        }
    }

    pub const fn source(self) -> PartitionId {
        self.source
    }

    pub const fn target(self) -> PartitionId {
        self.target
    }

    pub const fn send_count(self) -> usize {
        self.send_count
    }

    pub const fn first_tick(self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(self) -> Tick {
        self.last_tick
    }

    pub const fn minimum_delay(self) -> Option<Tick> {
        self.minimum_delay
    }

    pub const fn maximum_delay(self) -> Option<Tick> {
        self.maximum_delay
    }

    pub const fn delay_bounds(self) -> Option<(Tick, Tick)> {
        match (self.minimum_delay, self.maximum_delay) {
            (Some(minimum_delay), Some(maximum_delay)) => Some((minimum_delay, maximum_delay)),
            _ => None,
        }
    }

    pub fn merged_with(self, other: Self) -> Self {
        let delay_bounds = merge_remote_flow_delay_bounds(self, other);
        Self {
            source: self.source,
            target: self.target,
            send_count: self.send_count + other.send_count,
            first_tick: self.first_tick.min(other.first_tick),
            last_tick: self.last_tick.max(other.last_tick),
            minimum_delay: delay_bounds.map(|(minimum_delay, _)| minimum_delay),
            maximum_delay: delay_bounds.map(|(_, maximum_delay)| maximum_delay),
        }
    }

    fn from_send(send: ParallelRemoteSendRecord) -> Self {
        Self::with_delay_bounds(
            send.source(),
            send.target(),
            1,
            send.delivery_tick(),
            send.delivery_tick(),
            send.delay(),
            send.delay(),
        )
    }

    fn record_send(&mut self, send: ParallelRemoteSendRecord) {
        self.send_count += 1;
        self.first_tick = self.first_tick.min(send.delivery_tick());
        self.last_tick = self.last_tick.max(send.delivery_tick());
        let delay = send.delay();
        self.minimum_delay = Some(
            self.minimum_delay
                .map_or(delay, |minimum_delay| minimum_delay.min(delay)),
        );
        self.maximum_delay = Some(
            self.maximum_delay
                .map_or(delay, |maximum_delay| maximum_delay.max(delay)),
        );
    }
}

fn merge_remote_flow_delay_bounds(
    left: ParallelRemoteFlowRecord,
    right: ParallelRemoteFlowRecord,
) -> Option<(Tick, Tick)> {
    match (left.delay_bounds(), right.delay_bounds()) {
        (Some((left_minimum, left_maximum)), Some((right_minimum, right_maximum))) => Some((
            left_minimum.min(right_minimum),
            left_maximum.max(right_maximum),
        )),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub struct ParallelPartitionActivity {
    worker_count: usize,
    dispatch_count: usize,
    remote_send_count: usize,
    remote_receive_count: usize,
    max_pending_events: usize,
}

impl ParallelPartitionActivity {
    pub const fn new(
        worker_count: usize,
        dispatch_count: usize,
        max_pending_events: usize,
    ) -> Self {
        Self::with_remote_counts(worker_count, dispatch_count, 0, 0, max_pending_events)
    }

    pub const fn with_remote_send_count(
        worker_count: usize,
        dispatch_count: usize,
        remote_send_count: usize,
        max_pending_events: usize,
    ) -> Self {
        Self::with_remote_counts(
            worker_count,
            dispatch_count,
            remote_send_count,
            0,
            max_pending_events,
        )
    }

    pub const fn with_remote_counts(
        worker_count: usize,
        dispatch_count: usize,
        remote_send_count: usize,
        remote_receive_count: usize,
        max_pending_events: usize,
    ) -> Self {
        Self {
            worker_count,
            dispatch_count,
            remote_send_count,
            remote_receive_count,
            max_pending_events,
        }
    }

    pub const fn worker_count(self) -> usize {
        self.worker_count
    }

    pub const fn dispatch_count(self) -> usize {
        self.dispatch_count
    }

    pub const fn remote_send_count(self) -> usize {
        self.remote_send_count
    }

    pub const fn remote_receive_count(self) -> usize {
        self.remote_receive_count
    }

    pub const fn max_pending_events(self) -> usize {
        self.max_pending_events
    }

    pub const fn has_activity(self) -> bool {
        self.worker_count != 0
            || self.dispatch_count != 0
            || self.remote_send_count != 0
            || self.remote_receive_count != 0
    }

    fn record_worker(&mut self, worker: ParallelWorkerRecord) {
        self.worker_count += 1;
        self.max_pending_events = self.max_pending_events.max(worker.pending_events());
    }

    fn record_dispatch(&mut self) {
        self.dispatch_count += 1;
    }

    fn record_remote_send(&mut self) {
        self.remote_send_count += 1;
    }

    fn record_remote_receive(&mut self) {
        self.remote_receive_count += 1;
    }

    fn merge(self, other: Self) -> Self {
        Self {
            worker_count: self.worker_count + other.worker_count,
            dispatch_count: self.dispatch_count + other.dispatch_count,
            remote_send_count: self.remote_send_count + other.remote_send_count,
            remote_receive_count: self.remote_receive_count + other.remote_receive_count,
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
    pub(super) next_remote_order: u64,
    pub(super) next_progress_order: u64,
    pub(super) pending_events: Vec<PendingEventSnapshot>,
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
    pub(super) initial_frontiers: Vec<PartitionFrontier>,
    pub(super) final_frontiers: Vec<PartitionFrontier>,
    pub(super) dispatches: Vec<SchedulerDispatchRecord>,
    pub(super) batches: Vec<ParallelEpochBatchRecord>,
    pub(super) profile: ParallelRunProfile,
}

impl RecordedRunSummary {
    pub fn summary(&self) -> RunSummary {
        self.summary
    }

    pub fn initial_frontiers(&self) -> &[PartitionFrontier] {
        &self.initial_frontiers
    }

    pub fn initial_frontier(&self, partition: PartitionId) -> Option<PartitionFrontier> {
        self.initial_frontiers
            .iter()
            .copied()
            .find(|frontier| frontier.partition() == partition)
    }

    pub fn initial_frontier_count(&self) -> usize {
        self.initial_frontiers.len()
    }

    pub fn final_frontiers(&self) -> &[PartitionFrontier] {
        &self.final_frontiers
    }

    pub fn final_frontier(&self, partition: PartitionId) -> Option<PartitionFrontier> {
        self.final_frontiers
            .iter()
            .copied()
            .find(|frontier| frontier.partition() == partition)
    }

    pub fn final_frontier_count(&self) -> usize {
        self.final_frontiers.len()
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

    pub fn remote_sends(&self) -> Vec<ParallelRemoteSendRecord> {
        ordered_remote_sends(
            self.batches
                .iter()
                .flat_map(ParallelEpochBatchRecord::remote_sends)
                .copied()
                .collect(),
        )
    }

    pub fn remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_remote_source_partitions(
            self.batches
                .iter()
                .flat_map(ParallelEpochBatchRecord::remote_sends),
        )
    }

    pub fn remote_source_partition_count(&self) -> usize {
        self.remote_source_partitions().len()
    }

    pub fn remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_remote_target_partitions(
            self.batches
                .iter()
                .flat_map(ParallelEpochBatchRecord::remote_sends),
        )
    }

    pub fn remote_target_partition_count(&self) -> usize {
        self.remote_target_partitions().len()
    }

    pub fn remote_flow_count(&self, source: PartitionId, target: PartitionId) -> usize {
        self.batches
            .iter()
            .map(|batch| batch.remote_flow_count(source, target))
            .sum()
    }

    pub fn remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flows(
            self.batches
                .iter()
                .flat_map(ParallelEpochBatchRecord::remote_sends),
        )
    }

    pub fn progress_transition_count(&self) -> usize {
        self.batches
            .iter()
            .map(ParallelEpochBatchRecord::progress_transition_count)
            .sum()
    }

    pub fn progress_transition_count_by_kind(&self, kind: LivelockTransitionKind) -> usize {
        self.batches
            .iter()
            .map(|batch| batch.progress_transition_count_by_kind(kind))
            .sum()
    }

    pub fn progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        self.batches
            .iter()
            .flat_map(|batch| batch.progress_transitions().iter().cloned())
            .collect()
    }

    pub fn progress_monitor_snapshot(
        &self,
        threshold: u64,
    ) -> Result<ProgressMonitorSnapshot, ProgressMonitorError> {
        progress_monitor_snapshot(threshold, self.progress_transitions())
    }

    pub fn batch_count(&self) -> usize {
        self.profile.batch_count()
    }

    pub fn batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_batch_worker_count_summaries(&self.batches)
    }

    pub fn batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        batch_count_for_worker_count(&self.batches, worker_count)
    }

    pub fn batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        batch_count_at_or_above(&self.batches, minimum_worker_count)
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

    pub fn remote_sends(&self) -> Vec<ParallelRemoteSendRecord> {
        ordered_remote_sends(
            self.epochs
                .iter()
                .flat_map(RecordedRunSummary::remote_sends)
                .collect(),
        )
    }

    pub fn remote_source_partitions(&self) -> Vec<PartitionId> {
        collect_remote_source_partitions(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.batches().iter())
                .flat_map(ParallelEpochBatchRecord::remote_sends),
        )
    }

    pub fn remote_source_partition_count(&self) -> usize {
        self.remote_source_partitions().len()
    }

    pub fn remote_target_partitions(&self) -> Vec<PartitionId> {
        collect_remote_target_partitions(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.batches().iter())
                .flat_map(ParallelEpochBatchRecord::remote_sends),
        )
    }

    pub fn remote_target_partition_count(&self) -> usize {
        self.remote_target_partitions().len()
    }

    pub fn remote_flow_count(&self, source: PartitionId, target: PartitionId) -> usize {
        self.epochs
            .iter()
            .map(|epoch| epoch.remote_flow_count(source, target))
            .sum()
    }

    pub fn remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flows(self.epochs.iter().flat_map(|epoch| {
            epoch
                .batches()
                .iter()
                .flat_map(ParallelEpochBatchRecord::remote_sends)
        }))
    }

    pub fn progress_transition_count(&self) -> usize {
        self.epochs
            .iter()
            .map(RecordedRunSummary::progress_transition_count)
            .sum()
    }

    pub fn progress_transition_count_by_kind(&self, kind: LivelockTransitionKind) -> usize {
        self.epochs
            .iter()
            .map(|epoch| epoch.progress_transition_count_by_kind(kind))
            .sum()
    }

    pub fn progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        self.epochs
            .iter()
            .flat_map(RecordedRunSummary::progress_transitions)
            .collect()
    }

    pub fn progress_monitor_snapshot(
        &self,
        threshold: u64,
    ) -> Result<ProgressMonitorSnapshot, ProgressMonitorError> {
        progress_monitor_snapshot(threshold, self.progress_transitions())
    }

    pub fn batch_count(&self) -> usize {
        self.profile.batch_count()
    }

    pub fn batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_batch_worker_count_summaries(
            self.epochs.iter().flat_map(|epoch| epoch.batches().iter()),
        )
    }

    pub fn batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        batch_count_for_worker_count(
            self.epochs.iter().flat_map(|epoch| epoch.batches().iter()),
            worker_count,
        )
    }

    pub fn batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        batch_count_at_or_above(
            self.epochs.iter().flat_map(|epoch| epoch.batches().iter()),
            minimum_worker_count,
        )
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
        for remote_send in batch.remote_sends() {
            activities
                .entry(remote_send.source())
                .or_default()
                .record_remote_send();
            activities
                .entry(remote_send.target())
                .or_default()
                .record_remote_receive();
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

fn collect_batch_worker_count_summaries<'a, I>(batches: I) -> Vec<(usize, usize)>
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    let mut summaries = BTreeMap::<usize, usize>::new();
    for batch in batches {
        let worker_count = batch.worker_count();
        if worker_count != 0 {
            *summaries.entry(worker_count).or_default() += 1;
        }
    }
    summaries.into_iter().collect()
}

fn batch_count_for_worker_count<'a, I>(batches: I, worker_count: usize) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() == worker_count)
        .count()
}

fn batch_count_at_or_above<'a, I>(batches: I, minimum_worker_count: usize) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .count()
}

fn progress_monitor_snapshot<I>(
    threshold: u64,
    transitions: I,
) -> Result<ProgressMonitorSnapshot, ProgressMonitorError>
where
    I: IntoIterator<Item = ParallelProgressTransitionRecord>,
{
    let mut monitor = ProgressMonitor::with_transition_threshold(threshold)?;
    for transition in transitions {
        monitor.record_transition(
            transition.subject().clone(),
            transition.kind(),
            transition.tick(),
        )?;
    }
    Ok(monitor.snapshot())
}

fn collect_parallel_remote_flows<'a, I>(remote_sends: I) -> Vec<ParallelRemoteFlowRecord>
where
    I: IntoIterator<Item = &'a ParallelRemoteSendRecord>,
{
    let mut flows: BTreeMap<(PartitionId, PartitionId), ParallelRemoteFlowRecord> = BTreeMap::new();
    for send in remote_sends {
        flows
            .entry((send.source(), send.target()))
            .and_modify(|flow| flow.record_send(*send))
            .or_insert_with(|| ParallelRemoteFlowRecord::from_send(*send));
    }
    flows.into_values().collect()
}

fn ordered_remote_sends(mut sends: Vec<ParallelRemoteSendRecord>) -> Vec<ParallelRemoteSendRecord> {
    sends.sort_by_key(remote_send_delivery_key);
    sends
}

fn remote_send_delivery_key(
    send: &ParallelRemoteSendRecord,
) -> (PartitionId, Tick, PartitionId, u64) {
    (
        send.target(),
        send.delivery_tick(),
        send.source(),
        send.order(),
    )
}

fn collect_remote_source_partitions<'a, I>(remote_sends: I) -> Vec<PartitionId>
where
    I: IntoIterator<Item = &'a ParallelRemoteSendRecord>,
{
    remote_sends
        .into_iter()
        .map(|send| send.source())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn collect_remote_target_partitions<'a, I>(remote_sends: I) -> Vec<PartitionId>
where
    I: IntoIterator<Item = &'a ParallelRemoteSendRecord>,
{
    remote_sends
        .into_iter()
        .map(|send| send.target())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
