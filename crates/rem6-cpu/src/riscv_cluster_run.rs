use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{
    LivelockTransitionKind, ParallelEpochBatchRecord, ParallelEpochPlan, ParallelPartitionActivity,
    ParallelProgressTransitionRecord, ParallelRemoteFlowRecord, ParallelRemoteSendRecord,
    ParallelRunProfile, ParallelWorkerRecord, PartitionFrontier, PartitionId, ProgressMonitor,
    ProgressMonitorError, ProgressMonitorSnapshot, ReadyPartition, RecordedRunSummary, RunSummary,
    ScheduledEventKind, SchedulerDispatchRecord, Tick,
};

use crate::parallel_flow::merge_parallel_remote_flow_records;
use crate::riscv_activity::{drive_action_partition, RiscvCoreDriveActivity};
use crate::{CpuId, RiscvCoreDriveAction};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterDriveEvent {
    cpu: CpuId,
    action: RiscvCoreDriveAction,
}

impl RiscvClusterDriveEvent {
    pub const fn new(cpu: CpuId, action: RiscvCoreDriveAction) -> Self {
        Self { cpu, action }
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn action(&self) -> &RiscvCoreDriveAction {
        &self.action
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterTurn {
    core_events: Vec<RiscvClusterDriveEvent>,
    scheduler: Option<RunSummary>,
    parallel_scheduler: Option<RiscvClusterSchedulerEpoch>,
    idle_tick: Option<Tick>,
}

impl RiscvClusterTurn {
    pub fn core(core_events: Vec<RiscvClusterDriveEvent>) -> Self {
        Self {
            core_events,
            scheduler: None,
            parallel_scheduler: None,
            idle_tick: None,
        }
    }

    pub const fn scheduler(summary: RunSummary) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: Some(summary),
            parallel_scheduler: None,
            idle_tick: None,
        }
    }

    pub fn parallel_scheduler(plan: ParallelEpochPlan, recorded: RecordedRunSummary) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: None,
            parallel_scheduler: Some(RiscvClusterSchedulerEpoch::new(plan, recorded)),
            idle_tick: None,
        }
    }

    pub const fn idle(tick: Tick) -> Self {
        Self {
            core_events: Vec::new(),
            scheduler: None,
            parallel_scheduler: None,
            idle_tick: Some(tick),
        }
    }

    pub fn core_events(&self) -> &[RiscvClusterDriveEvent] {
        &self.core_events
    }

    pub fn cpu_activity(&self, cpu: CpuId) -> Option<RiscvCoreDriveActivity> {
        self.cpu_activities().remove(&cpu)
    }

    pub fn has_cpu_activity(&self, cpu: CpuId) -> bool {
        self.cpu_activity(cpu)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_cpu_count(&self) -> usize {
        self.cpu_activities().len()
    }

    pub fn cpu_activities(&self) -> BTreeMap<CpuId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for event in &self.core_events {
            activities
                .entry(event.cpu())
                .or_insert_with(RiscvCoreDriveActivity::default)
                .record_action(event.action());
        }
        activities
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<RiscvCoreDriveActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for event in &self.core_events {
            activities
                .entry(drive_action_partition(event.action()))
                .or_insert_with(RiscvCoreDriveActivity::default)
                .record_action(event.action());
        }
        activities
    }

    pub const fn scheduler_summary(&self) -> Option<RunSummary> {
        match (self.scheduler, self.parallel_scheduler.as_ref()) {
            (Some(summary), _) => Some(summary),
            (None, Some(epoch)) => Some(epoch.summary()),
            (None, None) => None,
        }
    }

    pub const fn serial_scheduler_summary(&self) -> Option<RunSummary> {
        self.scheduler
    }

    pub const fn parallel_scheduler_epoch(&self) -> Option<&RiscvClusterSchedulerEpoch> {
        self.parallel_scheduler.as_ref()
    }

    pub const fn idle_tick(&self) -> Option<Tick> {
        self.idle_tick
    }

    pub const fn is_idle(&self) -> bool {
        self.idle_tick.is_some()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterSchedulerEpoch {
    plan: ParallelEpochPlan,
    summary: RunSummary,
    final_frontiers: Vec<PartitionFrontier>,
    dispatches: Vec<SchedulerDispatchRecord>,
    batches: Vec<ParallelEpochBatchRecord>,
    progress_transitions: Vec<ParallelProgressTransitionRecord>,
    batch_worker_counts: Vec<(usize, usize)>,
    batch_worker_count_ticks: Vec<(usize, Tick)>,
    batch_partition_sets: Vec<(Vec<PartitionId>, usize)>,
    batch_partition_streaks: Vec<(Vec<PartitionId>, usize)>,
    remote_flows: Vec<ParallelRemoteFlowRecord>,
    profile: ParallelRunProfile,
    partition_activities: BTreeMap<PartitionId, ParallelPartitionActivity>,
}

impl RiscvClusterSchedulerEpoch {
    pub fn new(plan: ParallelEpochPlan, recorded: RecordedRunSummary) -> Self {
        let profile = recorded.profile();
        let partition_activities = recorded.partition_activities();
        let progress_transitions = recorded.progress_transitions();
        let batch_worker_counts = recorded.batch_worker_count_summaries();
        let batch_worker_count_ticks = recorded.batch_worker_count_tick_summaries();
        let batch_partition_sets = recorded.batch_partition_set_summaries();
        let batch_partition_streaks = recorded.batch_partition_streak_summaries();
        let remote_flows = recorded.remote_flows();
        Self {
            plan,
            summary: recorded.summary(),
            final_frontiers: recorded.final_frontiers().to_vec(),
            dispatches: recorded.dispatches().to_vec(),
            batches: recorded.batches().to_vec(),
            progress_transitions,
            batch_worker_counts,
            batch_worker_count_ticks,
            batch_partition_sets,
            batch_partition_streaks,
            remote_flows,
            profile,
            partition_activities,
        }
    }

    pub const fn plan(&self) -> &ParallelEpochPlan {
        &self.plan
    }

    pub fn horizon(&self) -> Tick {
        self.plan.horizon()
    }

    pub fn ready_partitions(&self) -> &[ReadyPartition] {
        self.plan.ready_partitions()
    }

    pub fn ready_partition_count(&self) -> usize {
        self.plan.ready_partition_count()
    }

    pub fn frontiers(&self) -> &[PartitionFrontier] {
        self.plan.frontiers()
    }

    pub fn frontier(&self, partition: PartitionId) -> Option<PartitionFrontier> {
        self.plan.frontier(partition)
    }

    pub fn initial_frontiers(&self) -> &[PartitionFrontier] {
        self.frontiers()
    }

    pub fn initial_frontier(&self, partition: PartitionId) -> Option<PartitionFrontier> {
        self.frontier(partition)
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

    pub fn serial_blockers(&self) -> &[SchedulerDispatchRecord] {
        self.plan.serial_blockers()
    }

    pub fn serial_blocker_count(&self) -> usize {
        self.plan.serial_blocker_count()
    }

    pub fn first_serial_blocker(&self) -> Option<SchedulerDispatchRecord> {
        self.plan.first_serial_blocker()
    }

    pub fn is_parallel_safe(&self) -> bool {
        self.plan.is_parallel_safe()
    }

    pub const fn summary(&self) -> RunSummary {
        self.summary
    }

    pub const fn turn_summary(&self) -> Option<RunSummary> {
        Some(self.summary)
    }

    pub fn dispatches(&self) -> &[SchedulerDispatchRecord] {
        &self.dispatches
    }

    pub fn batches(&self) -> &[ParallelEpochBatchRecord] {
        &self.batches
    }

    pub const fn profile(&self) -> ParallelRunProfile {
        self.profile
    }

    pub fn dispatch_count(&self) -> usize {
        self.profile.dispatch_count()
    }

    pub fn batch_count(&self) -> usize {
        self.profile.batch_count()
    }

    pub fn progress_transition_count(&self) -> usize {
        self.progress_transitions.len()
    }

    pub fn progress_transition_count_by_kind(&self, kind: LivelockTransitionKind) -> usize {
        self.progress_transitions
            .iter()
            .filter(|transition| transition.kind() == kind)
            .count()
    }

    pub fn progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        self.progress_transitions.clone()
    }

    pub fn progress_monitor_snapshot(
        &self,
        threshold: u64,
    ) -> Result<ProgressMonitorSnapshot, ProgressMonitorError> {
        progress_monitor_snapshot(threshold, self.progress_transitions())
    }

    pub fn batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        self.batch_worker_counts.clone()
    }

    pub fn batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        self.batch_worker_counts
            .iter()
            .find_map(|(workers, count)| (*workers == worker_count).then_some(*count))
            .unwrap_or(0)
    }

    pub fn batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        self.batch_worker_counts
            .iter()
            .filter(|(workers, _)| *workers >= minimum_worker_count)
            .map(|(_, count)| *count)
            .sum()
    }

    pub fn batch_worker_count_tick_summaries(&self) -> Vec<(usize, Tick)> {
        self.batch_worker_count_ticks.clone()
    }

    pub fn batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        self.batch_worker_count_ticks
            .iter()
            .find_map(|(workers, ticks)| (*workers == worker_count).then_some(*ticks))
            .unwrap_or(0)
    }

    pub fn batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        self.batch_worker_count_ticks
            .iter()
            .filter(|(workers, _)| *workers >= minimum_worker_count)
            .map(|(_, ticks)| *ticks)
            .fold(0, Tick::saturating_add)
    }

    pub fn batch_worker_ticks(&self) -> Tick {
        batch_worker_ticks_from_tick_summaries(&self.batch_worker_count_ticks)
    }

    pub fn batch_worker_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        batch_worker_ticks_at_or_above_from_tick_summaries(
            &self.batch_worker_count_ticks,
            minimum_worker_count,
        )
    }

    pub fn batch_partition_set_summaries(&self) -> Vec<(Vec<PartitionId>, usize)> {
        self.batch_partition_sets.clone()
    }

    pub fn batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let expected = normalize_partition_set(partitions);
        self.batch_partition_sets
            .iter()
            .find_map(|(partitions, count)| (partitions == &expected).then_some(*count))
            .unwrap_or(0)
    }

    pub fn batch_partition_streak_summaries(&self) -> Vec<(Vec<PartitionId>, usize)> {
        self.batch_partition_streaks.clone()
    }

    pub fn max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let expected = normalize_partition_set(partitions);
        self.batch_partition_streaks
            .iter()
            .find_map(|(partitions, count)| (partitions == &expected).then_some(*count))
            .unwrap_or(0)
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

    pub fn remote_flow_count(&self, source: PartitionId, target: PartitionId) -> usize {
        self.remote_flows
            .iter()
            .filter(|flow| flow.source() == source && flow.target() == target)
            .map(|flow| flow.send_count())
            .sum()
    }

    pub fn remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        self.remote_flows.clone()
    }

    pub fn remote_sends(&self) -> Vec<ParallelRemoteSendRecord> {
        self.batches
            .iter()
            .flat_map(|batch| batch.remote_sends().iter().copied())
            .collect()
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<ParallelPartitionActivity> {
        self.partition_activities.get(&partition).copied()
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities.len()
    }

    pub fn partition_activities(&self) -> &BTreeMap<PartitionId, ParallelPartitionActivity> {
        &self.partition_activities
    }

    pub fn workers(&self) -> Vec<ParallelWorkerRecord> {
        self.batches
            .iter()
            .flat_map(|batch| batch.workers().iter().copied())
            .collect()
    }

    pub fn dispatches_for_partition(&self, partition: PartitionId) -> Vec<SchedulerDispatchRecord> {
        self.dispatches
            .iter()
            .copied()
            .filter(|record| record.partition() == partition)
            .collect()
    }

    pub fn parallel_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.dispatches
            .iter()
            .copied()
            .filter(|record| record.kind() == ScheduledEventKind::Parallel)
            .collect()
    }

    pub fn serial_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.dispatches
            .iter()
            .copied()
            .filter(|record| record.kind() == ScheduledEventKind::Serial)
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterRun {
    turns: Vec<RiscvClusterTurn>,
    stop_reason: RiscvClusterStopReason,
}

impl RiscvClusterRun {
    pub const fn new(turns: Vec<RiscvClusterTurn>, stop_reason: RiscvClusterStopReason) -> Self {
        Self { turns, stop_reason }
    }

    pub fn turns(&self) -> &[RiscvClusterTurn] {
        &self.turns
    }

    pub fn cpu_activity(&self, cpu: CpuId) -> Option<RiscvCoreDriveActivity> {
        self.cpu_activities().remove(&cpu)
    }

    pub fn has_cpu_activity(&self, cpu: CpuId) -> bool {
        self.cpu_activity(cpu)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_cpu_count(&self) -> usize {
        self.cpu_activities().len()
    }

    pub fn cpu_activities(&self) -> BTreeMap<CpuId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for turn in &self.turns {
            for (cpu, activity) in turn.cpu_activities() {
                activities
                    .entry(cpu)
                    .and_modify(|stored: &mut RiscvCoreDriveActivity| {
                        *stored = stored.merge(activity);
                    })
                    .or_insert(activity);
            }
        }
        activities
    }

    pub fn partition_activity(&self, partition: PartitionId) -> Option<RiscvCoreDriveActivity> {
        self.partition_activities().remove(&partition)
    }

    pub fn has_partition_activity(&self, partition: PartitionId) -> bool {
        self.partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_partition_count(&self) -> usize {
        self.partition_activities().len()
    }

    pub fn partition_activities(&self) -> BTreeMap<PartitionId, RiscvCoreDriveActivity> {
        let mut activities = BTreeMap::new();
        for turn in &self.turns {
            for (partition, activity) in turn.partition_activities() {
                activities
                    .entry(partition)
                    .and_modify(|stored: &mut RiscvCoreDriveActivity| {
                        *stored = stored.merge(activity);
                    })
                    .or_insert(activity);
            }
        }
        activities
    }

    pub const fn stop_reason(&self) -> RiscvClusterStopReason {
        self.stop_reason
    }

    pub fn scheduler_summaries(&self) -> Vec<RunSummary> {
        self.turns
            .iter()
            .filter_map(RiscvClusterTurn::scheduler_summary)
            .collect()
    }

    pub fn parallel_scheduler_epochs(&self) -> Vec<&RiscvClusterSchedulerEpoch> {
        self.turns
            .iter()
            .filter_map(RiscvClusterTurn::parallel_scheduler_epoch)
            .collect()
    }

    pub fn parallel_scheduler_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.dispatches().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.batches().iter().cloned())
            .collect()
    }

    pub fn parallel_scheduler_progress_transition_count(&self) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(RiscvClusterSchedulerEpoch::progress_transition_count)
            .sum()
    }

    pub fn parallel_scheduler_progress_transition_count_by_kind(
        &self,
        kind: LivelockTransitionKind,
    ) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.progress_transition_count_by_kind(kind))
            .sum()
    }

    pub fn parallel_scheduler_progress_transitions(&self) -> Vec<ParallelProgressTransitionRecord> {
        collect_parallel_progress_transitions(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(RiscvClusterSchedulerEpoch::progress_transitions),
        )
    }

    pub fn parallel_scheduler_progress_monitor_snapshot(
        &self,
        threshold: u64,
    ) -> Result<ProgressMonitorSnapshot, ProgressMonitorError> {
        progress_monitor_snapshot(threshold, self.parallel_scheduler_progress_transitions())
    }

    pub fn parallel_scheduler_batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_worker_count_summaries_from_summaries(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(RiscvClusterSchedulerEpoch::batch_worker_count_summaries),
        )
    }

    pub fn parallel_scheduler_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_count_for_worker_count(worker_count))
            .sum()
    }

    pub fn parallel_scheduler_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_count_at_or_above(minimum_worker_count))
            .sum()
    }

    pub fn parallel_scheduler_batch_worker_count_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_worker_count_tick_summaries_from_summaries(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(RiscvClusterSchedulerEpoch::batch_worker_count_tick_summaries),
        )
    }

    pub fn parallel_scheduler_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_ticks_for_worker_count(worker_count))
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_ticks_at_or_above(minimum_worker_count))
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(RiscvClusterSchedulerEpoch::batch_worker_ticks)
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_worker_ticks_at_or_above(minimum_worker_count))
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_batch_partition_set_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_partition_set_summaries_from_summaries(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(RiscvClusterSchedulerEpoch::batch_partition_set_summaries),
        )
    }

    pub fn parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_count_for_partition_set(partitions.iter().copied()))
            .sum()
    }

    pub fn parallel_scheduler_batch_partition_streak_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_batch_partition_streak_summaries(self.parallel_scheduler_batches())
    }

    pub fn parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        max_consecutive_batch_count_for_partition_set(
            self.parallel_scheduler_batch_partition_streak_summaries(),
            partitions,
        )
    }

    pub fn parallel_scheduler_workers(&self) -> Vec<ParallelWorkerRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(RiscvClusterSchedulerEpoch::workers)
            .collect()
    }

    pub fn parallel_scheduler_worker_partitions(&self) -> Vec<PartitionId> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(RiscvClusterSchedulerEpoch::parallel_worker_partitions)
            .collect()
    }

    pub fn parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.remote_flow_count(source, target))
            .sum()
    }

    pub fn parallel_scheduler_remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        merge_parallel_remote_flow_records(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(RiscvClusterSchedulerEpoch::remote_flows),
        )
    }

    pub fn max_parallel_scheduler_workers(&self) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(RiscvClusterSchedulerEpoch::max_parallel_workers)
            .max()
            .unwrap_or(0)
    }

    pub fn parallel_scheduler_profile(&self) -> ParallelRunProfile {
        self.parallel_scheduler_epochs()
            .into_iter()
            .fold(ParallelRunProfile::default(), |profile, epoch| {
                profile.merge(epoch.profile())
            })
    }

    pub fn parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.parallel_scheduler_partition_activities()
            .remove(&partition)
    }

    pub fn has_parallel_scheduler_partition_activity(&self, partition: PartitionId) -> bool {
        self.parallel_scheduler_partition_activity(partition)
            .is_some_and(|activity| activity.has_activity())
    }

    pub fn active_parallel_scheduler_partition_count(&self) -> usize {
        self.parallel_scheduler_partition_activities().len()
    }

    pub fn parallel_scheduler_partition_activities(
        &self,
    ) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        let mut activities = BTreeMap::new();
        for epoch in self.parallel_scheduler_epochs() {
            merge_parallel_partition_activity_maps(&mut activities, epoch.partition_activities());
        }
        activities
    }

    pub fn parallel_scheduler_dispatches_for_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<SchedulerDispatchRecord> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.dispatches_for_partition(partition))
            .collect()
    }

    pub fn parallel_scheduler_frontiers(&self) -> Vec<PartitionFrontier> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.frontiers().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_final_frontiers(&self) -> Vec<PartitionFrontier> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.final_frontiers().iter().copied())
            .collect()
    }

    pub fn parallel_scheduler_ready_partitions(&self) -> Vec<ReadyPartition> {
        self.parallel_scheduler_epochs()
            .into_iter()
            .flat_map(|epoch| epoch.ready_partitions().iter().copied())
            .collect()
    }

    pub const fn idle_tick(&self) -> Option<Tick> {
        match self.stop_reason {
            RiscvClusterStopReason::Idle { tick } => Some(tick),
            RiscvClusterStopReason::StopCondition => None,
        }
    }
}

fn merge_parallel_partition_activity_maps(
    target: &mut BTreeMap<PartitionId, ParallelPartitionActivity>,
    source: &BTreeMap<PartitionId, ParallelPartitionActivity>,
) {
    for (partition, activity) in source {
        target
            .entry(*partition)
            .and_modify(|stored| {
                *stored = ParallelPartitionActivity::with_remote_counts(
                    stored.worker_count() + activity.worker_count(),
                    stored.dispatch_count() + activity.dispatch_count(),
                    stored.remote_send_count() + activity.remote_send_count(),
                    stored.remote_receive_count() + activity.remote_receive_count(),
                    stored
                        .max_pending_events()
                        .max(activity.max_pending_events()),
                );
            })
            .or_insert(*activity);
    }
}

fn collect_parallel_progress_transitions<I>(transitions: I) -> Vec<ParallelProgressTransitionRecord>
where
    I: IntoIterator<Item = ParallelProgressTransitionRecord>,
{
    let mut transitions = transitions.into_iter().collect::<Vec<_>>();
    transitions.sort_by_key(|transition| {
        (
            transition.tick(),
            transition.partition(),
            transition.order(),
            transition.kind(),
            transition.subject().clone(),
        )
    });
    transitions
}

fn progress_monitor_snapshot(
    threshold: u64,
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
) -> Result<ProgressMonitorSnapshot, ProgressMonitorError> {
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

fn collect_worker_count_summaries_from_summaries(
    summaries: impl IntoIterator<Item = (usize, usize)>,
) -> Vec<(usize, usize)> {
    let mut collected = BTreeMap::<usize, usize>::new();
    for (worker_count, count) in summaries {
        if worker_count != 0 && count != 0 {
            *collected.entry(worker_count).or_default() += count;
        }
    }
    collected.into_iter().collect()
}

fn collect_worker_count_tick_summaries_from_summaries(
    summaries: impl IntoIterator<Item = (usize, Tick)>,
) -> Vec<(usize, Tick)> {
    let mut collected = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in summaries {
        if worker_count != 0 && ticks != 0 {
            let stored = collected.entry(worker_count).or_default();
            *stored = stored.saturating_add(ticks);
        }
    }
    collected.into_iter().collect()
}

fn batch_worker_ticks_from_tick_summaries(summaries: &[(usize, Tick)]) -> Tick {
    batch_worker_ticks_at_or_above_from_tick_summaries(summaries, 1)
}

fn batch_worker_ticks_at_or_above_from_tick_summaries(
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    summaries
        .iter()
        .filter(|(worker_count, _)| *worker_count >= minimum_worker_count)
        .map(|(worker_count, ticks)| ticks.saturating_mul(*worker_count as Tick))
        .fold(0, Tick::saturating_add)
}

fn collect_partition_set_summaries_from_summaries(
    summaries: impl IntoIterator<Item = (Vec<PartitionId>, usize)>,
) -> Vec<(Vec<PartitionId>, usize)> {
    let mut collected = BTreeMap::<Vec<PartitionId>, usize>::new();
    for (partitions, count) in summaries {
        if !partitions.is_empty() && count != 0 {
            *collected.entry(partitions).or_default() += count;
        }
    }
    collected.into_iter().collect()
}

fn collect_batch_partition_streak_summaries(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
) -> Vec<(Vec<PartitionId>, usize)> {
    let mut summaries = BTreeMap::<Vec<PartitionId>, usize>::new();
    let mut current: Option<(Vec<PartitionId>, usize)> = None;
    for batch in batches {
        let partitions = batch.partition_set();
        if partitions.is_empty() {
            continue;
        }
        match current.as_mut() {
            Some((current_partitions, count)) if current_partitions == &partitions => {
                *count += 1;
            }
            Some(_) => {
                flush_partition_streak(&mut summaries, current.take());
                current = Some((partitions, 1));
            }
            None => {
                current = Some((partitions, 1));
            }
        }
    }
    flush_partition_streak(&mut summaries, current);
    summaries.into_iter().collect()
}

fn max_consecutive_batch_count_for_partition_set(
    streaks: impl IntoIterator<Item = (Vec<PartitionId>, usize)>,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let expected = normalize_partition_set(partitions);
    streaks
        .into_iter()
        .find_map(|(partitions, count)| (partitions == expected).then_some(count))
        .unwrap_or(0)
}

fn flush_partition_streak(
    summaries: &mut BTreeMap<Vec<PartitionId>, usize>,
    streak: Option<(Vec<PartitionId>, usize)>,
) {
    if let Some((partitions, count)) = streak {
        summaries
            .entry(partitions)
            .and_modify(|stored| *stored = (*stored).max(count))
            .or_insert(count);
    }
}

fn normalize_partition_set(partitions: impl IntoIterator<Item = PartitionId>) -> Vec<PartitionId> {
    partitions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvClusterStopReason {
    StopCondition,
    Idle { tick: Tick },
}
