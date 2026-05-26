use std::collections::BTreeMap;

use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, ParallelRemoteSendRecord,
    PartitionFrontier, PartitionId,
};

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets, collect_parallel_batch_partition_streaks,
    collect_parallel_batch_partition_streaks_from_sequence, collect_parallel_batch_worker_counts,
    max_parallel_batch_activity_worker_count, parallel_batch_active_partition_count,
    parallel_batch_activity_count_at_or_above, parallel_batch_count_for_partition_set,
    parallel_batch_partition_activity_for_partition, parallel_batch_streak_activity_for_partition,
    parallel_batch_streak_count_for_partition_set, strongest_parallel_batch_count,
    total_parallel_batch_activity_worker_count, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchWorkerCount,
};
use crate::result_collect::{
    collect_parallel_partition_activities, collect_parallel_remote_flow_evidence,
    collect_parallel_remote_flows, collect_parallel_remote_sends, collect_partition_frontiers,
    parallel_remote_flow_evidence_count, parallel_remote_send_count,
};
use crate::result_partition_activity::{
    merge_parallel_partition_activity_evidence_options, parallel_active_partition_count,
    parallel_partition_activity_for_partition, parallel_partition_dispatch_count,
    parallel_partition_worker_count,
};

use super::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadParallelExecutionSummary,
};

impl WorkloadParallelExecutionSummary {
    pub const fn with_data_cache_parallel_counts(
        mut self,
        run_count: usize,
        scheduler_epoch_count: usize,
        scheduler_dispatch_count: usize,
        scheduler_batch_count: usize,
        scheduler_max_workers: usize,
    ) -> Self {
        self.data_cache_parallel_run_count = run_count;
        self.data_cache_parallel_scheduler_epoch_count = scheduler_epoch_count;
        self.data_cache_parallel_scheduler_dispatch_count = scheduler_dispatch_count;
        self.data_cache_parallel_scheduler_batch_count = scheduler_batch_count;
        self.data_cache_parallel_scheduler_max_workers = scheduler_max_workers;
        self
    }

    pub const fn with_data_cache_parallel_empty_epoch_count(
        mut self,
        empty_epoch_count: usize,
    ) -> Self {
        self.data_cache_parallel_scheduler_empty_epoch_count = empty_epoch_count;
        self
    }

    pub const fn with_data_cache_parallel_partitions(
        mut self,
        active_partition_count: usize,
    ) -> Self {
        self.active_data_cache_parallel_scheduler_partition_count = active_partition_count;
        self
    }

    pub const fn with_data_cache_parallel_worker_count(
        mut self,
        total_worker_count: usize,
    ) -> Self {
        self.data_cache_parallel_scheduler_total_workers = total_worker_count;
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_worker_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadParallelBatchWorkerCount>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts(counts);
        self.data_cache_parallel_scheduler_batch_count = self
            .data_cache_parallel_scheduler_batch_worker_counts
            .iter()
            .map(WorkloadParallelBatchWorkerCount::batch_count)
            .sum();
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_partition_sets(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_partition_sets =
            collect_parallel_batch_partition_sets(sets);
        self.data_cache_parallel_scheduler_batch_count = self
            .data_cache_parallel_scheduler_batch_partition_sets
            .iter()
            .map(WorkloadParallelBatchPartitionSet::batch_count)
            .sum();
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_partition_streaks(
        mut self,
        streaks: impl IntoIterator<Item = WorkloadParallelBatchPartitionStreak>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks(streaks);
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_partition_streak_sequence(
        mut self,
        sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
    ) -> Self {
        self.data_cache_parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks_from_sequence(sets);
        self
    }

    pub fn with_data_cache_parallel_scheduler_remote_flows(
        mut self,
        flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    ) -> Self {
        self.data_cache_parallel_scheduler_remote_flows = collect_parallel_remote_flows(flows);
        self
    }

    pub fn with_data_cache_parallel_scheduler_remote_sends(
        mut self,
        sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
    ) -> Self {
        self.data_cache_parallel_scheduler_remote_sends = collect_parallel_remote_sends(sends);
        self
    }

    pub fn with_data_cache_parallel_scheduler_frontiers(
        mut self,
        initial_frontiers: impl IntoIterator<Item = PartitionFrontier>,
        final_frontiers: impl IntoIterator<Item = PartitionFrontier>,
    ) -> Self {
        self.data_cache_parallel_scheduler_initial_frontiers =
            collect_partition_frontiers(initial_frontiers);
        self.data_cache_parallel_scheduler_final_frontiers =
            collect_partition_frontiers(final_frontiers);
        self
    }

    pub fn with_data_cache_parallel_scheduler_partition_activities(
        mut self,
        activities: impl IntoIterator<Item = (PartitionId, ParallelPartitionActivity)>,
    ) -> Self {
        self.data_cache_parallel_scheduler_partition_activities =
            collect_parallel_partition_activities(activities);
        self.active_data_cache_parallel_scheduler_partition_count = self
            .data_cache_parallel_scheduler_partition_activities
            .len();
        self
    }

    pub const fn with_data_cache_run_attribution(
        mut self,
        attributed_run_count: usize,
        unattributed_run_count: usize,
    ) -> Self {
        self.attributed_data_cache_parallel_run_count = attributed_run_count;
        self.unattributed_data_cache_parallel_run_count = unattributed_run_count;
        self
    }

    pub fn with_data_cache_protocol_counts(
        mut self,
        counts: impl IntoIterator<Item = WorkloadDataCacheProtocolCount>,
    ) -> Self {
        let mut by_protocol = BTreeMap::new();
        for count in counts {
            if count.run_count() != 0 {
                by_protocol.insert(count.protocol(), count.run_count());
            }
        }
        self.data_cache_protocol_counts = by_protocol
            .into_iter()
            .map(|(protocol, run_count)| WorkloadDataCacheProtocolCount::new(protocol, run_count))
            .collect();
        self
    }

    pub const fn with_data_cache_diagnostics(
        mut self,
        wait_for_edge_count: usize,
        deadlock_diagnostic_count: usize,
    ) -> Self {
        self.data_cache_wait_for_edge_count = wait_for_edge_count;
        self.data_cache_deadlock_diagnostic_count = deadlock_diagnostic_count;
        self
    }

    pub fn with_data_cache_parallel_scheduler_livelock_diagnostics(
        mut self,
        progress_transition_count: usize,
        livelock_diagnostic_count: usize,
    ) -> Self {
        self.data_cache_parallel_scheduler_progress_transition_count = progress_transition_count
            .max(
                self.data_cache_parallel_scheduler_progress_transitions
                    .len(),
            );
        self.data_cache_parallel_scheduler_livelock_diagnostic_count = livelock_diagnostic_count;
        self
    }

    pub const fn data_cache_parallel_run_count(&self) -> usize {
        self.data_cache_parallel_run_count
    }

    pub const fn data_cache_parallel_scheduler_epoch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_epoch_count
    }

    pub const fn data_cache_parallel_scheduler_empty_epoch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_empty_epoch_count
    }

    pub fn data_cache_parallel_scheduler_dispatch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_dispatch_count.max(
            total_parallel_batch_activity_worker_count(
                &self.data_cache_parallel_scheduler_batch_worker_counts,
                &self.data_cache_parallel_scheduler_batch_partition_sets,
                &self.data_cache_parallel_scheduler_batch_partition_streaks,
            )
            .max(parallel_partition_dispatch_count(
                &self.data_cache_parallel_scheduler_partition_activities,
            )),
        )
    }

    pub fn data_cache_parallel_scheduler_batch_count(&self) -> usize {
        self.data_cache_parallel_scheduler_batch_count
            .max(strongest_parallel_batch_count(
                &self.data_cache_parallel_scheduler_batch_worker_counts,
                &self.data_cache_parallel_scheduler_batch_partition_sets,
                &self.data_cache_parallel_scheduler_batch_partition_streaks,
            ))
    }

    pub fn active_data_cache_parallel_scheduler_partition_count(&self) -> usize {
        self.active_data_cache_parallel_scheduler_partition_count
            .max(parallel_batch_active_partition_count(
                &self.data_cache_parallel_scheduler_batch_partition_sets,
                &self.data_cache_parallel_scheduler_batch_partition_streaks,
            ))
            .max(parallel_active_partition_count(
                &self.data_cache_parallel_scheduler_partition_activities,
                &self.data_cache_parallel_scheduler_remote_flows,
                &self.data_cache_parallel_scheduler_remote_sends,
            ))
    }

    pub fn data_cache_parallel_scheduler_max_workers(&self) -> usize {
        self.data_cache_parallel_scheduler_max_workers.max(
            max_parallel_batch_activity_worker_count(
                &self.data_cache_parallel_scheduler_batch_worker_counts,
                &self.data_cache_parallel_scheduler_batch_partition_sets,
                &self.data_cache_parallel_scheduler_batch_partition_streaks,
            ),
        )
    }

    pub fn data_cache_parallel_scheduler_total_workers(&self) -> usize {
        self.data_cache_parallel_scheduler_total_workers.max(
            total_parallel_batch_activity_worker_count(
                &self.data_cache_parallel_scheduler_batch_worker_counts,
                &self.data_cache_parallel_scheduler_batch_partition_sets,
                &self.data_cache_parallel_scheduler_batch_partition_streaks,
            )
            .max(parallel_partition_worker_count(
                &self.data_cache_parallel_scheduler_partition_activities,
            )),
        )
    }

    pub fn data_cache_parallel_scheduler_remote_flows(&self) -> &[ParallelRemoteFlowRecord] {
        &self.data_cache_parallel_scheduler_remote_flows
    }

    pub fn data_cache_parallel_scheduler_remote_flow_evidence(
        &self,
    ) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_evidence(
            self.data_cache_parallel_scheduler_remote_flows
                .iter()
                .copied(),
            self.data_cache_parallel_scheduler_remote_sends
                .iter()
                .copied(),
        )
    }

    pub fn data_cache_parallel_scheduler_remote_sends(&self) -> &[ParallelRemoteSendRecord] {
        &self.data_cache_parallel_scheduler_remote_sends
    }

    pub fn data_cache_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> &[WorkloadParallelBatchWorkerCount] {
        &self.data_cache_parallel_scheduler_batch_worker_counts
    }

    pub fn data_cache_parallel_scheduler_batch_partition_sets(
        &self,
    ) -> &[WorkloadParallelBatchPartitionSet] {
        &self.data_cache_parallel_scheduler_batch_partition_sets
    }

    pub fn data_cache_parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> &[WorkloadParallelBatchPartitionStreak] {
        &self.data_cache_parallel_scheduler_batch_partition_streaks
    }

    pub fn data_cache_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        parallel_batch_activity_count_at_or_above(
            &self.data_cache_parallel_scheduler_batch_worker_counts,
            &self.data_cache_parallel_scheduler_batch_partition_sets,
            &self.data_cache_parallel_scheduler_batch_partition_streaks,
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_count_for_partition_set(
            &self.data_cache_parallel_scheduler_batch_partition_sets,
            &self.data_cache_parallel_scheduler_batch_partition_streaks,
            partitions,
        )
    }

    pub fn data_cache_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_streak_count_for_partition_set(
            &self.data_cache_parallel_scheduler_batch_partition_streaks,
            partitions,
        )
    }

    pub fn data_cache_parallel_scheduler_partition_activities(
        &self,
    ) -> &[(PartitionId, ParallelPartitionActivity)] {
        &self.data_cache_parallel_scheduler_partition_activities
    }

    pub fn data_cache_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        merge_parallel_partition_activity_evidence_options(
            merge_parallel_partition_activity_evidence_options(
                parallel_partition_activity_for_partition(
                    &self.data_cache_parallel_scheduler_partition_activities,
                    &self.data_cache_parallel_scheduler_remote_flows,
                    &self.data_cache_parallel_scheduler_remote_sends,
                    partition,
                ),
                parallel_batch_partition_activity_for_partition(
                    &self.data_cache_parallel_scheduler_batch_partition_sets,
                    partition,
                ),
            ),
            parallel_batch_streak_activity_for_partition(
                &self.data_cache_parallel_scheduler_batch_partition_streaks,
                partition,
            ),
        )
    }

    pub fn data_cache_parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_evidence_count(
            &self.data_cache_parallel_scheduler_remote_flows,
            &self.data_cache_parallel_scheduler_remote_sends,
            source,
            target,
        )
    }

    pub fn has_data_cache_parallel_scheduler_remote_flows(&self) -> bool {
        !self
            .data_cache_parallel_scheduler_remote_flow_evidence()
            .is_empty()
    }

    pub fn data_cache_parallel_scheduler_remote_send_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_send_count(
            &self.data_cache_parallel_scheduler_remote_sends,
            source,
            target,
        )
    }

    pub fn has_data_cache_parallel_scheduler_remote_sends(&self) -> bool {
        !self.data_cache_parallel_scheduler_remote_sends.is_empty()
    }

    pub fn data_cache_parallel_scheduler_initial_frontiers(&self) -> &[PartitionFrontier] {
        &self.data_cache_parallel_scheduler_initial_frontiers
    }

    pub fn data_cache_parallel_scheduler_final_frontiers(&self) -> &[PartitionFrontier] {
        &self.data_cache_parallel_scheduler_final_frontiers
    }

    pub fn data_cache_parallel_scheduler_initial_frontier_count(&self) -> usize {
        self.data_cache_parallel_scheduler_initial_frontiers.len()
    }

    pub fn data_cache_parallel_scheduler_final_frontier_count(&self) -> usize {
        self.data_cache_parallel_scheduler_final_frontiers.len()
    }

    pub fn has_data_cache_parallel_scheduler_frontiers(&self) -> bool {
        !self
            .data_cache_parallel_scheduler_initial_frontiers
            .is_empty()
            || !self
                .data_cache_parallel_scheduler_final_frontiers
                .is_empty()
    }

    pub const fn attributed_data_cache_parallel_run_count(&self) -> usize {
        self.attributed_data_cache_parallel_run_count
    }

    pub const fn unattributed_data_cache_parallel_run_count(&self) -> usize {
        self.unattributed_data_cache_parallel_run_count
    }

    pub fn data_cache_protocol_counts(&self) -> &[WorkloadDataCacheProtocolCount] {
        &self.data_cache_protocol_counts
    }

    pub fn data_cache_protocols(&self) -> Vec<WorkloadDataCacheProtocol> {
        self.data_cache_protocol_counts
            .iter()
            .map(WorkloadDataCacheProtocolCount::protocol)
            .collect()
    }

    pub fn data_cache_protocol_count_map(&self) -> BTreeMap<WorkloadDataCacheProtocol, usize> {
        self.data_cache_protocol_counts
            .iter()
            .map(|count| (count.protocol(), count.run_count()))
            .collect()
    }

    pub fn attributed_data_cache_protocol_run_count(&self) -> usize {
        self.data_cache_protocol_counts
            .iter()
            .map(WorkloadDataCacheProtocolCount::run_count)
            .sum()
    }

    pub fn data_cache_parallel_run_count_for_protocol(
        &self,
        protocol: WorkloadDataCacheProtocol,
    ) -> usize {
        self.data_cache_protocol_counts
            .iter()
            .find(|count| count.protocol() == protocol)
            .map(WorkloadDataCacheProtocolCount::run_count)
            .unwrap_or(0)
    }

    pub fn has_data_cache_protocol(&self, protocol: WorkloadDataCacheProtocol) -> bool {
        self.data_cache_parallel_run_count_for_protocol(protocol) != 0
    }

    pub const fn has_unattributed_data_cache_parallel_runs(&self) -> bool {
        self.unattributed_data_cache_parallel_run_count != 0
    }

    pub const fn data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.data_cache_deadlock_diagnostic_count
    }

    pub const fn data_cache_parallel_scheduler_progress_transition_count(&self) -> usize {
        self.data_cache_parallel_scheduler_progress_transition_count
    }

    pub const fn data_cache_parallel_scheduler_livelock_diagnostic_count(&self) -> usize {
        self.data_cache_parallel_scheduler_livelock_diagnostic_count
    }

    pub const fn has_data_cache_parallel_scheduler_livelock_diagnostics(&self) -> bool {
        self.data_cache_parallel_scheduler_livelock_diagnostic_count != 0
    }

    pub const fn has_data_cache_diagnostics(&self) -> bool {
        self.data_cache_wait_for_edge_count != 0
            || self.data_cache_deadlock_diagnostic_count != 0
            || self.has_data_cache_parallel_scheduler_livelock_diagnostics()
    }
}
