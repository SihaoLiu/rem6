use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, PartitionFrontier, PartitionId,
};

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets, collect_parallel_batch_partition_streaks,
    collect_parallel_batch_worker_counts, combined_parallel_batch_active_partition_count,
    normalize_partition_set, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchWorkerCount,
};
use crate::result_collect::{
    collect_conservative_partition_frontiers, collect_parallel_partition_activities,
    collect_parallel_remote_flows,
};
use crate::result_partition_activity::{
    combined_parallel_active_partition_count, merge_parallel_partition_activity_options,
};

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub const fn full_system_parallel_scheduler_epoch_count(&self) -> usize {
        self.scheduler_epoch_count + self.data_cache_parallel_scheduler_epoch_count
    }

    pub const fn full_system_parallel_scheduler_empty_epoch_count(&self) -> usize {
        self.scheduler_empty_epoch_count + self.data_cache_parallel_scheduler_empty_epoch_count
    }

    pub fn full_system_parallel_scheduler_dispatch_count(&self) -> usize {
        self.scheduler_dispatch_count() + self.data_cache_parallel_scheduler_dispatch_count()
    }

    pub fn full_system_parallel_scheduler_batch_count(&self) -> usize {
        self.scheduler_batch_count() + self.data_cache_parallel_scheduler_batch_count()
    }

    pub fn active_full_system_parallel_scheduler_partition_count(&self) -> usize {
        self.active_full_system_parallel_scheduler_partition_count
            .max(combined_parallel_batch_active_partition_count(
                &self.parallel_scheduler_batch_partition_sets,
                &self.parallel_scheduler_batch_partition_streaks,
                &self.data_cache_parallel_scheduler_batch_partition_sets,
                &self.data_cache_parallel_scheduler_batch_partition_streaks,
            ))
            .max(combined_parallel_active_partition_count(
                &self.parallel_scheduler_partition_activities,
                &self.parallel_scheduler_remote_flows,
                &self.data_cache_parallel_scheduler_partition_activities,
                &self.data_cache_parallel_scheduler_remote_flows,
            ))
    }

    pub fn full_system_parallel_scheduler_max_workers(&self) -> usize {
        self.max_parallel_scheduler_workers()
            .max(self.data_cache_parallel_scheduler_max_workers())
    }

    pub fn full_system_parallel_scheduler_total_workers(&self) -> usize {
        self.total_parallel_scheduler_workers() + self.data_cache_parallel_scheduler_total_workers()
    }

    pub fn full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerCount> {
        collect_parallel_batch_worker_counts(
            self.parallel_scheduler_batch_worker_counts
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_batch_worker_counts
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        self.parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
            + self.data_cache_parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_partition_sets(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionSet> {
        collect_parallel_batch_partition_sets(
            self.parallel_scheduler_batch_partition_sets
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_batch_partition_sets
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        collect_parallel_batch_partition_streaks(
            self.parallel_scheduler_batch_partition_streaks
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_batch_partition_streaks
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        self.parallel_scheduler_batch_count_for_partition_set(partitions.iter().copied())
            + self.data_cache_parallel_scheduler_batch_count_for_partition_set(
                partitions.iter().copied(),
            )
    }

    pub fn full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        self.parallel_scheduler_max_consecutive_batch_count_for_partition_set(
            partitions.iter().copied(),
        )
        .max(
            self.data_cache_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
                partitions.iter().copied(),
            ),
        )
    }

    pub fn full_system_parallel_scheduler_remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flows(
            self.parallel_scheduler_remote_flows.iter().copied().chain(
                self.data_cache_parallel_scheduler_remote_flows
                    .iter()
                    .copied(),
            ),
        )
    }

    pub fn full_system_parallel_scheduler_initial_frontiers(&self) -> Vec<PartitionFrontier> {
        collect_conservative_partition_frontiers(
            self.parallel_scheduler_initial_frontiers
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_initial_frontiers
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_final_frontiers(&self) -> Vec<PartitionFrontier> {
        collect_conservative_partition_frontiers(
            self.parallel_scheduler_final_frontiers
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_final_frontiers
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_initial_frontier_count(&self) -> usize {
        self.full_system_parallel_scheduler_initial_frontiers()
            .len()
    }

    pub fn full_system_parallel_scheduler_final_frontier_count(&self) -> usize {
        self.full_system_parallel_scheduler_final_frontiers().len()
    }

    pub fn has_full_system_parallel_scheduler_frontiers(&self) -> bool {
        self.has_parallel_scheduler_frontiers()
            || self.has_data_cache_parallel_scheduler_frontiers()
    }

    pub fn full_system_parallel_scheduler_partition_activities(
        &self,
    ) -> Vec<(PartitionId, ParallelPartitionActivity)> {
        collect_parallel_partition_activities(
            self.parallel_scheduler_partition_activities
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_partition_activities
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        merge_parallel_partition_activity_options(
            self.parallel_scheduler_partition_activity(partition),
            self.data_cache_parallel_scheduler_partition_activity(partition),
        )
    }

    pub fn full_system_parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        self.parallel_scheduler_remote_flow_count(source, target)
            + self.data_cache_parallel_scheduler_remote_flow_count(source, target)
    }

    pub fn has_full_system_parallel_scheduler_remote_flows(&self) -> bool {
        self.has_parallel_scheduler_remote_flows()
            || self.has_data_cache_parallel_scheduler_remote_flows()
    }

    pub fn has_full_system_parallel_scheduler_work(&self) -> bool {
        self.active_full_system_parallel_scheduler_partition_count() != 0
            || self.has_parallel_scheduler_work()
            || self.has_data_cache_parallel_work()
    }

    pub fn has_parallel_scheduler_work(&self) -> bool {
        self.active_scheduler_partition_count() != 0
            || self.scheduler_dispatch_count != 0
            || self.scheduler_batch_count != 0
            || self.total_parallel_scheduler_workers != 0
            || self.max_parallel_scheduler_workers != 0
            || !self.parallel_scheduler_batch_worker_counts.is_empty()
            || !self.parallel_scheduler_batch_partition_sets.is_empty()
            || !self.parallel_scheduler_batch_partition_streaks.is_empty()
            || self.has_parallel_scheduler_frontiers()
    }

    pub fn has_data_cache_parallel_work(&self) -> bool {
        self.data_cache_parallel_run_count != 0
            || self.active_data_cache_parallel_scheduler_partition_count() != 0
            || self.data_cache_parallel_scheduler_dispatch_count != 0
            || self.data_cache_parallel_scheduler_total_workers != 0
            || self.data_cache_parallel_scheduler_max_workers != 0
            || !self
                .data_cache_parallel_scheduler_batch_worker_counts
                .is_empty()
            || !self
                .data_cache_parallel_scheduler_batch_partition_sets
                .is_empty()
            || !self
                .data_cache_parallel_scheduler_batch_partition_streaks
                .is_empty()
            || self.has_data_cache_parallel_scheduler_frontiers()
    }
}
