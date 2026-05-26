use std::collections::BTreeSet;

use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, ParallelRemoteSendRecord,
    PartitionFrontier, PartitionId,
};

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets, collect_parallel_batch_partition_streaks,
    collect_parallel_batch_worker_counts, collect_parallel_batch_worker_counts_from_streaks,
    collect_strongest_parallel_batch_worker_counts, max_parallel_batch_activity_worker_count,
    normalize_partition_set, parallel_batch_active_partition_count,
    parallel_batch_activity_count_at_or_above, parallel_batch_count_for_partition_set,
    parallel_batch_partition_activity_for_partition, parallel_batch_streak_activity_for_partition,
    parallel_batch_streak_count_for_partition_set, strongest_parallel_batch_count,
    total_parallel_batch_activity_worker_count, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchWorkerCount,
};
use crate::result_collect::{
    collect_conservative_partition_frontiers, collect_parallel_partition_activities,
    collect_parallel_remote_flow_aggregates, collect_parallel_remote_sends,
    is_parallel_remote_flow_evidence, is_parallel_remote_send_evidence,
    parallel_remote_flow_evidence_count, parallel_remote_send_count,
};
use crate::result_partition_activity::{
    combined_parallel_active_partition_count, merge_parallel_partition_activity_evidence_options,
    merge_parallel_partition_activity_options, parallel_partition_activity_for_partition,
};

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub const fn full_system_parallel_scheduler_epoch_count(&self) -> usize {
        self.scheduler_epoch_count
            + self.data_cache_parallel_scheduler_epoch_count
            + self.dma_scheduler_epoch_count()
    }

    pub const fn full_system_parallel_scheduler_empty_epoch_count(&self) -> usize {
        self.scheduler_empty_epoch_count
            + self.data_cache_parallel_scheduler_empty_epoch_count
            + self.dma_scheduler_empty_epoch_count()
    }

    pub fn full_system_parallel_scheduler_dispatch_count(&self) -> usize {
        self.scheduler_dispatch_count()
            + self.data_cache_parallel_scheduler_dispatch_count()
            + self.dma_scheduler_dispatch_count()
    }

    pub fn full_system_parallel_scheduler_batch_count(&self) -> usize {
        (self.scheduler_batch_count()
            + self.data_cache_parallel_scheduler_batch_count()
            + self.dma_scheduler_batch_count())
        .max(strongest_parallel_batch_count(
            &[],
            &[],
            &self.full_system_parallel_scheduler_batch_partition_streaks,
        ))
    }

    pub fn active_full_system_parallel_scheduler_partition_count(&self) -> usize {
        let batch_partition_sets = self.full_system_parallel_scheduler_batch_partition_sets();
        let batch_partition_streaks = self.full_system_parallel_scheduler_batch_partition_streaks();
        self.active_full_system_parallel_scheduler_partition_count
            .max(parallel_batch_active_partition_count(
                &batch_partition_sets,
                &batch_partition_streaks,
            ))
            .max(combined_parallel_active_partition_count(
                &self.parallel_scheduler_partition_activities,
                &self.parallel_scheduler_remote_flows,
                &self.parallel_scheduler_remote_sends,
                &self.data_cache_parallel_scheduler_partition_activities,
                &self.data_cache_parallel_scheduler_remote_flows,
                &self.data_cache_parallel_scheduler_remote_sends,
            ))
            .max(self.active_full_system_remote_partition_count())
    }

    pub fn full_system_parallel_scheduler_max_workers(&self) -> usize {
        self.max_parallel_scheduler_workers()
            .max(self.data_cache_parallel_scheduler_max_workers())
            .max(self.dma_scheduler_max_workers())
            .max(max_parallel_batch_activity_worker_count(
                &[],
                &[],
                &self.full_system_parallel_scheduler_batch_partition_streaks,
            ))
    }

    pub fn full_system_parallel_scheduler_total_workers(&self) -> usize {
        (self.total_parallel_scheduler_workers()
            + self.data_cache_parallel_scheduler_total_workers()
            + self.dma_scheduler_total_workers())
        .max(total_parallel_batch_activity_worker_count(
            &[],
            &[],
            &self.full_system_parallel_scheduler_batch_partition_streaks,
        ))
    }

    pub fn full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerCount> {
        let scoped_counts = collect_parallel_batch_worker_counts(
            self.parallel_scheduler_batch_worker_counts
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_batch_worker_counts
                        .iter()
                        .copied(),
                )
                .chain(self.dma_scheduler_batch_worker_counts()),
        );
        let full_system_counts = collect_parallel_batch_worker_counts_from_streaks(
            &self.full_system_parallel_scheduler_batch_partition_streaks,
        );
        collect_strongest_parallel_batch_worker_counts(scoped_counts, full_system_counts)
    }

    pub fn full_system_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        (self.parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
            + self.data_cache_parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
            + self.dma_scheduler_batch_count_at_or_above(minimum_worker_count))
        .max(parallel_batch_activity_count_at_or_above(
            &[],
            &[],
            &self.full_system_parallel_scheduler_batch_partition_streaks,
            minimum_worker_count,
        ))
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
                )
                .chain(self.dma_scheduler_batch_partition_sets()),
        )
    }

    pub fn full_system_parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        collect_parallel_batch_partition_streaks(
            self.full_system_parallel_scheduler_batch_partition_streaks
                .iter()
                .cloned()
                .chain(
                    self.parallel_scheduler_batch_partition_streaks
                        .iter()
                        .cloned(),
                )
                .chain(
                    self.data_cache_parallel_scheduler_batch_partition_streaks
                        .iter()
                        .cloned(),
                )
                .chain(self.dma_scheduler_batch_partition_streaks()),
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        (self.parallel_scheduler_batch_count_for_partition_set(partitions.iter().copied())
            + self.data_cache_parallel_scheduler_batch_count_for_partition_set(
                partitions.iter().copied(),
            ))
        .saturating_add(
            self.gpu_dma_scheduler_batch_count_for_partition_set(partitions.iter().copied()),
        )
        .saturating_add(
            self.accelerator_dma_scheduler_batch_count_for_partition_set(
                partitions.iter().copied(),
            ),
        )
        .max(parallel_batch_count_for_partition_set(
            &[],
            &self.full_system_parallel_scheduler_batch_partition_streaks,
            partitions.iter().copied(),
        ))
    }

    pub fn full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let streaks = self.full_system_parallel_scheduler_batch_partition_streaks();
        parallel_batch_streak_count_for_partition_set(&streaks, partitions)
    }

    pub fn full_system_parallel_scheduler_remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_aggregates(
            self.parallel_scheduler_remote_flow_evidence()
                .into_iter()
                .chain(self.data_cache_parallel_scheduler_remote_flow_evidence())
                .chain(self.dma_scheduler_remote_flows()),
        )
    }

    pub fn full_system_parallel_scheduler_remote_sends(&self) -> Vec<ParallelRemoteSendRecord> {
        collect_parallel_remote_sends(
            self.parallel_scheduler_remote_sends
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_remote_sends
                        .iter()
                        .copied(),
                )
                .chain(self.dma_scheduler_remote_sends()),
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
                )
                .chain(self.dma_scheduler_initial_frontiers()),
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
                )
                .chain(self.dma_scheduler_final_frontiers()),
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
            || self.has_dma_scheduler_frontiers()
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
        let dma_partition_sets = self.dma_scheduler_batch_partition_sets();
        let dma_partition_streaks = self.dma_scheduler_batch_partition_streaks();
        let dma_activity = merge_parallel_partition_activity_evidence_options(
            parallel_batch_partition_activity_for_partition(&dma_partition_sets, partition),
            parallel_batch_streak_activity_for_partition(&dma_partition_streaks, partition),
        );
        let dma_remote_activity = parallel_partition_activity_for_partition(
            &[],
            &self.dma_scheduler_remote_flows(),
            &self.dma_scheduler_remote_sends(),
            partition,
        );
        merge_parallel_partition_activity_options(
            merge_parallel_partition_activity_options(
                self.parallel_scheduler_partition_activity(partition),
                self.data_cache_parallel_scheduler_partition_activity(partition),
            ),
            merge_parallel_partition_activity_evidence_options(dma_activity, dma_remote_activity),
        )
    }

    pub fn full_system_parallel_scheduler_remote_flow_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_flow_evidence_count(
            &self.full_system_parallel_scheduler_remote_flows(),
            &[],
            source,
            target,
        )
    }

    pub fn full_system_parallel_scheduler_remote_send_count(
        &self,
        source: PartitionId,
        target: PartitionId,
    ) -> usize {
        parallel_remote_send_count(
            &self.full_system_parallel_scheduler_remote_sends(),
            source,
            target,
        )
    }

    pub fn has_full_system_parallel_scheduler_remote_flows(&self) -> bool {
        self.has_parallel_scheduler_remote_flows()
            || self.has_data_cache_parallel_scheduler_remote_flows()
            || self.has_dma_scheduler_remote_flows()
    }

    pub fn has_full_system_parallel_scheduler_remote_sends(&self) -> bool {
        self.has_parallel_scheduler_remote_sends()
            || self.has_data_cache_parallel_scheduler_remote_sends()
            || self.has_dma_scheduler_remote_sends()
    }

    pub fn has_full_system_parallel_scheduler_work(&self) -> bool {
        self.active_full_system_parallel_scheduler_partition_count() != 0
            || self.has_parallel_scheduler_work()
            || self.has_data_cache_parallel_work()
            || self.has_dma_parallel_work()
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
            || self.has_parallel_scheduler_remote_sends()
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
            || self.has_data_cache_parallel_scheduler_remote_sends()
            || self.has_data_cache_parallel_scheduler_frontiers()
    }

    pub fn has_dma_parallel_work(&self) -> bool {
        self.dma_scheduler_epoch_count() != 0
            || self.dma_scheduler_dispatch_count() != 0
            || self.dma_scheduler_batch_count() != 0
            || self.dma_scheduler_batch_worker_ticks() != 0
            || self.has_dma_scheduler_frontiers()
            || self.has_dma_scheduler_remote_flows()
            || self.has_dma_scheduler_remote_sends()
    }
}

impl WorkloadParallelExecutionSummary {
    fn active_full_system_remote_partition_count(&self) -> usize {
        let mut partitions = BTreeSet::new();
        partitions.extend(
            self.parallel_scheduler_partition_activities
                .iter()
                .map(|(partition, _)| *partition),
        );
        partitions.extend(
            self.data_cache_parallel_scheduler_partition_activities
                .iter()
                .map(|(partition, _)| *partition),
        );
        for flow in self.full_system_parallel_scheduler_remote_flows() {
            if is_parallel_remote_flow_evidence(flow) {
                partitions.insert(flow.source());
                partitions.insert(flow.target());
            }
        }
        for send in self.full_system_parallel_scheduler_remote_sends() {
            if is_parallel_remote_send_evidence(send) {
                partitions.insert(send.source());
                partitions.insert(send.target());
            }
        }
        partitions.len()
    }
}
