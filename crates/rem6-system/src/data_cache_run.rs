use std::collections::{BTreeMap, BTreeSet};

use rem6_coherence::ParallelCoherenceRunSummary;
use rem6_kernel::{
    DeadlockDiagnostic, ParallelEpochBatchRecord, ParallelPartitionActivity, ParallelRunProfile,
    PartitionId, RecordedRunSummary, SchedulerDispatchRecord, Tick, WaitForEdge, WaitForEdgeKind,
    WaitForNode,
};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn data_cache_runs(&self) -> &[ParallelCoherenceRunSummary] {
        &self.data_cache_runs
    }

    pub fn data_cache_run_count(&self) -> usize {
        self.data_cache_runs.len()
    }

    pub fn data_cache_parallel_scheduler_epochs(&self) -> Vec<RecordedRunSummary> {
        self.data_cache_runs
            .iter()
            .flat_map(|run| run.scheduler_epochs().iter().cloned())
            .collect()
    }

    pub fn data_cache_parallel_scheduler_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        self.data_cache_runs
            .iter()
            .flat_map(ParallelCoherenceRunSummary::dispatches)
            .collect()
    }

    pub fn data_cache_parallel_scheduler_batches(&self) -> Vec<ParallelEpochBatchRecord> {
        self.data_cache_runs
            .iter()
            .flat_map(ParallelCoherenceRunSummary::batches)
            .collect()
    }

    pub fn data_cache_parallel_scheduler_worker_partitions(&self) -> Vec<PartitionId> {
        self.data_cache_runs
            .iter()
            .flat_map(ParallelCoherenceRunSummary::parallel_worker_partitions)
            .collect()
    }

    pub fn data_cache_parallel_scheduler_profile(&self) -> ParallelRunProfile {
        self.data_cache_runs
            .iter()
            .fold(ParallelRunProfile::default(), |profile, run| {
                profile.merge(run.profile())
            })
    }

    pub fn data_cache_parallel_scheduler_epoch_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::epoch_count)
            .sum()
    }

    pub fn data_cache_parallel_scheduler_empty_epoch_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::empty_epoch_count)
            .sum()
    }

    pub fn data_cache_parallel_scheduler_dispatch_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::dispatch_count)
            .sum()
    }

    pub fn data_cache_parallel_scheduler_batch_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::batch_count)
            .sum()
    }

    pub fn data_cache_parallel_scheduler_max_workers(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::max_parallel_workers)
            .max()
            .unwrap_or(0)
    }

    pub fn data_cache_parallel_scheduler_total_workers(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::total_parallel_workers)
            .sum()
    }

    pub fn has_data_cache_parallel_scheduler_work(&self) -> bool {
        self.data_cache_parallel_scheduler_profile()
            .has_parallel_work()
    }

    pub fn data_cache_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.data_cache_parallel_scheduler_partition_activities()
            .remove(&partition)
    }

    pub fn has_data_cache_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> bool {
        self.data_cache_parallel_scheduler_partition_activity(partition)
            .is_some_and(ParallelPartitionActivity::has_activity)
    }

    pub fn active_data_cache_parallel_scheduler_partition_count(&self) -> usize {
        self.data_cache_parallel_scheduler_partition_activities()
            .len()
    }

    pub fn data_cache_parallel_scheduler_partition_activities(
        &self,
    ) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        let mut activities = BTreeMap::new();
        for epoch in self.data_cache_parallel_scheduler_epochs() {
            for (partition, activity) in epoch.partition_activities() {
                merge_partition_activity(&mut activities, partition, activity);
            }
        }
        activities
    }

    pub fn data_cache_parallel_scheduler_dispatches_for_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<SchedulerDispatchRecord> {
        self.data_cache_parallel_scheduler_dispatches()
            .into_iter()
            .filter(|dispatch| dispatch.partition() == partition)
            .collect()
    }

    pub fn full_system_parallel_scheduler_profile(&self) -> ParallelRunProfile {
        self.parallel_scheduler_profile()
            .merge(self.data_cache_parallel_scheduler_profile())
    }

    pub fn full_system_parallel_scheduler_dispatches(&self) -> Vec<SchedulerDispatchRecord> {
        let mut dispatches = self.parallel_scheduler_dispatches();
        dispatches.extend(self.data_cache_parallel_scheduler_dispatches());
        dispatches
    }

    pub fn full_system_parallel_scheduler_batches(&self) -> Vec<ParallelEpochBatchRecord> {
        let mut batches = self.parallel_scheduler_batches();
        batches.extend(self.data_cache_parallel_scheduler_batches());
        batches
    }

    pub fn full_system_parallel_scheduler_worker_partitions(&self) -> Vec<PartitionId> {
        let mut partitions = self.parallel_scheduler_worker_partitions();
        partitions.extend(self.data_cache_parallel_scheduler_worker_partitions());
        partitions
    }

    pub fn full_system_parallel_scheduler_dispatch_count(&self) -> usize {
        self.parallel_scheduler_profile().dispatch_count()
            + self.data_cache_parallel_scheduler_dispatch_count()
    }

    pub fn full_system_parallel_scheduler_batch_count(&self) -> usize {
        self.parallel_scheduler_profile().batch_count()
            + self.data_cache_parallel_scheduler_batch_count()
    }

    pub fn full_system_parallel_scheduler_max_workers(&self) -> usize {
        self.parallel_scheduler_profile()
            .max_parallel_workers()
            .max(self.data_cache_parallel_scheduler_max_workers())
    }

    pub fn has_full_system_parallel_scheduler_work(&self) -> bool {
        self.full_system_parallel_scheduler_profile()
            .has_parallel_work()
    }

    pub fn full_system_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        self.full_system_parallel_scheduler_partition_activities()
            .remove(&partition)
    }

    pub fn has_full_system_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> bool {
        self.full_system_parallel_scheduler_partition_activity(partition)
            .is_some_and(ParallelPartitionActivity::has_activity)
    }

    pub fn active_full_system_parallel_scheduler_partition_count(&self) -> usize {
        self.full_system_parallel_scheduler_partition_activities()
            .len()
    }

    pub fn full_system_parallel_scheduler_partition_activities(
        &self,
    ) -> BTreeMap<PartitionId, ParallelPartitionActivity> {
        let mut activities = self.parallel_scheduler_partition_activities();
        for (partition, activity) in self.data_cache_parallel_scheduler_partition_activities() {
            merge_partition_activity(&mut activities, partition, activity);
        }
        activities
    }

    pub fn full_system_parallel_scheduler_dispatches_for_partition(
        &self,
        partition: PartitionId,
    ) -> Vec<SchedulerDispatchRecord> {
        let mut dispatches = self.parallel_scheduler_dispatches_for_partition(partition);
        dispatches.extend(self.data_cache_parallel_scheduler_dispatches_for_partition(partition));
        dispatches
    }

    pub fn initial_data_cache_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.data_cache_runs
            .iter()
            .flat_map(|run| run.initial_wait_for_edges().iter().cloned())
            .collect()
    }

    pub fn remaining_data_cache_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.data_cache_runs
            .iter()
            .flat_map(|run| run.remaining_wait_for_edges().iter().cloned())
            .collect()
    }

    pub fn data_cache_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.remaining_data_cache_wait_for_edges()
    }

    pub fn initial_data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::initial_wait_for_edge_count)
            .sum()
    }

    pub fn remaining_data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::remaining_wait_for_edge_count)
            .sum()
    }

    pub fn data_cache_wait_for_edge_count(&self) -> usize {
        self.remaining_data_cache_wait_for_edge_count()
    }

    pub fn has_data_cache_wait_for_edges(&self) -> bool {
        self.data_cache_wait_for_edge_count() != 0
    }

    pub fn initial_data_cache_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.data_cache_runs
            .iter()
            .flat_map(ParallelCoherenceRunSummary::initial_wait_for_blocked_nodes)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn remaining_data_cache_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.data_cache_runs
            .iter()
            .flat_map(ParallelCoherenceRunSummary::remaining_wait_for_blocked_nodes)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn data_cache_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.remaining_data_cache_wait_for_blocked_nodes()
    }

    pub fn initial_data_cache_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        let mut counts = BTreeMap::new();
        for run in &self.data_cache_runs {
            merge_counts(&mut counts, run.initial_wait_for_edge_kind_counts());
        }
        counts
    }

    pub fn remaining_data_cache_wait_for_edge_kind_counts(
        &self,
    ) -> BTreeMap<WaitForEdgeKind, usize> {
        let mut counts = BTreeMap::new();
        for run in &self.data_cache_runs {
            merge_counts(&mut counts, run.remaining_wait_for_edge_kind_counts());
        }
        counts
    }

    pub fn data_cache_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        self.remaining_data_cache_wait_for_edge_kind_counts()
    }

    pub fn initial_data_cache_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.data_cache_runs
            .iter()
            .map(|run| run.initial_wait_for_edge_count_by_kind(kind))
            .sum()
    }

    pub fn remaining_data_cache_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.data_cache_runs
            .iter()
            .map(|run| run.remaining_wait_for_edge_count_by_kind(kind))
            .sum()
    }

    pub fn initial_data_cache_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        oldest_edge(self.initial_data_cache_wait_for_edges())
    }

    pub fn remaining_data_cache_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        oldest_edge(self.remaining_data_cache_wait_for_edges())
    }

    pub fn data_cache_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        self.remaining_data_cache_oldest_wait_edge()
    }

    pub fn initial_data_cache_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        newest_edge(self.initial_data_cache_wait_for_edges())
    }

    pub fn remaining_data_cache_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        newest_edge(self.remaining_data_cache_wait_for_edges())
    }

    pub fn data_cache_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        self.remaining_data_cache_newest_observed_wait_edge()
    }

    pub fn initial_data_cache_total_wait_observation_count(&self) -> u64 {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::initial_total_wait_observation_count)
            .sum()
    }

    pub fn remaining_data_cache_total_wait_observation_count(&self) -> u64 {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::remaining_total_wait_observation_count)
            .sum()
    }

    pub fn data_cache_total_wait_observation_count(&self) -> u64 {
        self.remaining_data_cache_total_wait_observation_count()
    }

    pub fn initial_data_cache_first_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::initial_first_wait_tick)
            .min()
    }

    pub fn remaining_data_cache_first_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::remaining_first_wait_tick)
            .min()
    }

    pub fn data_cache_first_wait_tick(&self) -> Option<Tick> {
        self.remaining_data_cache_first_wait_tick()
    }

    pub fn initial_data_cache_last_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::initial_last_wait_tick)
            .max()
    }

    pub fn remaining_data_cache_last_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::remaining_last_wait_tick)
            .max()
    }

    pub fn data_cache_last_wait_tick(&self) -> Option<Tick> {
        self.remaining_data_cache_last_wait_tick()
    }

    pub fn initial_data_cache_longest_observed_wait_span(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::initial_longest_observed_wait_span)
            .max()
    }

    pub fn remaining_data_cache_longest_observed_wait_span(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::remaining_longest_observed_wait_span)
            .max()
    }

    pub fn data_cache_longest_observed_wait_span(&self) -> Option<Tick> {
        self.remaining_data_cache_longest_observed_wait_span()
    }

    pub fn initial_data_cache_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.data_cache_runs
            .iter()
            .filter_map(|run| run.initial_deadlock_diagnostic().cloned())
            .collect()
    }

    pub fn remaining_data_cache_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.data_cache_runs
            .iter()
            .filter_map(|run| run.remaining_deadlock_diagnostic().cloned())
            .collect()
    }

    pub fn data_cache_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.remaining_data_cache_deadlock_diagnostics()
    }

    pub fn initial_data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.initial_data_cache_deadlock_diagnostics().len()
    }

    pub fn remaining_data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.remaining_data_cache_deadlock_diagnostics().len()
    }

    pub fn data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.remaining_data_cache_deadlock_diagnostic_count()
    }
}

fn merge_counts(
    counts: &mut BTreeMap<WaitForEdgeKind, usize>,
    run_counts: BTreeMap<WaitForEdgeKind, usize>,
) {
    for (kind, count) in run_counts {
        *counts.entry(kind).or_insert(0) += count;
    }
}

fn merge_partition_activity(
    activities: &mut BTreeMap<PartitionId, ParallelPartitionActivity>,
    partition: PartitionId,
    activity: ParallelPartitionActivity,
) {
    let previous = activities.remove(&partition).unwrap_or_default();
    activities.insert(
        partition,
        ParallelPartitionActivity::new(
            previous.worker_count() + activity.worker_count(),
            previous.dispatch_count() + activity.dispatch_count(),
            previous
                .max_pending_events()
                .max(activity.max_pending_events()),
        ),
    );
}

fn oldest_edge(edges: Vec<WaitForEdge>) -> Option<WaitForEdge> {
    edges
        .into_iter()
        .min_by_key(|edge| (edge.first_observed_tick(), edge.last_observed_tick()))
}

fn newest_edge(edges: Vec<WaitForEdge>) -> Option<WaitForEdge> {
    edges
        .into_iter()
        .max_by_key(|edge| (edge.last_observed_tick(), edge.first_observed_tick()))
}
