use std::collections::BTreeSet;

use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, ParallelRemoteSendRecord,
    PartitionFrontier, PartitionId,
};

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets, collect_parallel_batch_partition_sets_from_streaks,
    collect_parallel_batch_partition_sets_from_timeline, collect_parallel_batch_partition_streaks,
    collect_parallel_batch_partition_streaks_from_timeline, collect_parallel_batch_worker_counts,
    collect_parallel_batch_worker_counts_from_partition_sets,
    collect_parallel_batch_worker_counts_from_streaks,
    collect_parallel_batch_worker_counts_from_timeline,
    collect_strongest_parallel_batch_partition_sets,
    collect_strongest_parallel_batch_worker_counts, max_parallel_batch_activity_worker_count,
    normalize_partition_set, parallel_batch_active_partition_count,
    parallel_batch_count_at_or_above, parallel_batch_count_for_partition_set,
    parallel_batch_partition_activity_for_partition, parallel_batch_streak_activity_for_partition,
    parallel_batch_streak_count_for_partition_set, total_parallel_batch_count,
    total_parallel_batch_worker_count, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchWorkerCount,
};
use crate::result_collect::{
    collect_conservative_partition_frontiers, collect_parallel_partition_activities,
    collect_parallel_remote_flow_aggregates, collect_parallel_remote_flow_evidence,
    collect_parallel_remote_flows, collect_parallel_remote_sends,
    collect_strongest_parallel_remote_flow_aggregates, is_parallel_remote_flow_evidence,
    is_parallel_remote_send_evidence, parallel_remote_flow_evidence_count,
    parallel_remote_send_count,
};
use crate::result_partition_activity::{
    combined_parallel_active_partition_count, merge_parallel_partition_activity_evidence_options,
    merge_parallel_partition_activity_options, parallel_partition_activity_for_partition,
};

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub const fn with_full_system_parallel_scheduler_counts(
        mut self,
        epoch_count: usize,
        empty_epoch_count: usize,
        dispatch_count: usize,
    ) -> Self {
        self.has_full_system_parallel_scheduler_counts = true;
        self.full_system_parallel_scheduler_epoch_count = epoch_count;
        self.full_system_parallel_scheduler_empty_epoch_count = empty_epoch_count;
        self.full_system_parallel_scheduler_dispatch_count = dispatch_count;
        self
    }

    pub fn with_full_system_parallel_scheduler_remote_flows(
        mut self,
        flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    ) -> Self {
        self.full_system_parallel_scheduler_remote_flows = collect_parallel_remote_flows(flows);
        self
    }

    pub fn with_full_system_parallel_scheduler_remote_sends(
        mut self,
        sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
    ) -> Self {
        self.full_system_parallel_scheduler_remote_sends = collect_parallel_remote_sends(sends);
        self
    }

    pub fn full_system_parallel_scheduler_epoch_count(&self) -> usize {
        if self.has_full_system_parallel_scheduler_counts {
            self.full_system_parallel_scheduler_epoch_count
                .max(self.full_system_parallel_scheduler_epoch_count_lower_bound())
        } else {
            self.scheduler_epoch_count
                + self.data_cache_parallel_scheduler_epoch_count
                + self.dma_scheduler_epoch_count()
        }
    }

    pub(crate) fn full_system_parallel_scheduler_epoch_count_lower_bound(&self) -> usize {
        self.scheduler_epoch_count()
            .max(self.data_cache_parallel_scheduler_epoch_count())
            .max(self.dma_scheduler_epoch_count())
            .max(self.gpu_dma_scheduler_epoch_count())
            .max(self.accelerator_dma_scheduler_epoch_count())
    }

    pub const fn full_system_parallel_scheduler_empty_epoch_count(&self) -> usize {
        if self.has_full_system_parallel_scheduler_counts {
            self.full_system_parallel_scheduler_empty_epoch_count
        } else {
            self.scheduler_empty_epoch_count
                + self.data_cache_parallel_scheduler_empty_epoch_count
                + self.dma_scheduler_empty_epoch_count()
        }
    }

    pub fn full_system_parallel_scheduler_dispatch_count(&self) -> usize {
        let dispatch_count_lower_bound =
            self.full_system_parallel_scheduler_dispatch_count_lower_bound();
        if self.has_explicit_full_system_parallel_scheduler_counts() {
            self.full_system_parallel_scheduler_dispatch_count
                .max(dispatch_count_lower_bound)
        } else {
            dispatch_count_lower_bound
        }
    }

    pub(crate) fn full_system_parallel_scheduler_dispatch_count_lower_bound(&self) -> usize {
        let scoped_dispatch_count = self.scheduler_dispatch_count()
            + self.data_cache_parallel_scheduler_dispatch_count()
            + self.dma_scheduler_dispatch_count();
        let counts = self.full_system_parallel_scheduler_batch_worker_counts();
        scoped_dispatch_count.max(total_parallel_batch_worker_count(&counts))
    }

    pub fn full_system_parallel_scheduler_batch_count(&self) -> usize {
        let scoped_batch_count = self.scheduler_batch_count()
            + self.data_cache_parallel_scheduler_batch_count()
            + self.dma_scheduler_batch_count();
        let counts = self.preferred_full_system_parallel_scheduler_batch_worker_counts();
        scoped_batch_count.max(total_parallel_batch_count(&counts))
    }

    pub fn active_full_system_parallel_scheduler_partition_count(&self) -> usize {
        self.active_full_system_parallel_scheduler_partition_count
            .max(self.full_system_parallel_scheduler_active_partition_count_lower_bound())
    }

    pub(crate) const fn raw_full_system_parallel_scheduler_partition_count(&self) -> usize {
        self.active_full_system_parallel_scheduler_partition_count
    }

    pub(crate) const fn has_explicit_full_system_parallel_scheduler_counts(&self) -> bool {
        self.has_full_system_parallel_scheduler_counts
    }

    pub(crate) const fn raw_full_system_parallel_scheduler_epoch_count(&self) -> usize {
        self.full_system_parallel_scheduler_epoch_count
    }

    pub(crate) const fn raw_full_system_parallel_scheduler_empty_epoch_count(&self) -> usize {
        self.full_system_parallel_scheduler_empty_epoch_count
    }

    pub(crate) const fn raw_full_system_parallel_scheduler_dispatch_count(&self) -> usize {
        self.full_system_parallel_scheduler_dispatch_count
    }

    pub(crate) fn full_system_parallel_scheduler_active_partition_count_lower_bound(
        &self,
    ) -> usize {
        let batch_partition_sets = self.full_system_parallel_scheduler_batch_partition_sets();
        let batch_partition_streaks = self.full_system_parallel_scheduler_batch_partition_streaks();
        parallel_batch_active_partition_count(&batch_partition_sets, &batch_partition_streaks)
            .max(
                self.full_system_parallel_scheduler_partition_activities()
                    .len(),
            )
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
        let timeline_counts = collect_parallel_batch_worker_counts_from_timeline(
            &self.full_system_parallel_scheduler_batch_timeline,
        );
        let timeline_sets = collect_parallel_batch_partition_sets_from_timeline(
            &self.full_system_parallel_scheduler_batch_timeline,
        );
        let timeline_streaks = collect_parallel_batch_partition_streaks_from_timeline(
            &self.full_system_parallel_scheduler_batch_timeline,
        );
        self.max_parallel_scheduler_workers()
            .max(self.data_cache_parallel_scheduler_max_workers())
            .max(self.dma_scheduler_max_workers())
            .max(max_parallel_batch_activity_worker_count(
                &self.full_system_parallel_scheduler_batch_worker_counts,
                &self.full_system_parallel_scheduler_batch_partition_sets,
                &self.full_system_parallel_scheduler_batch_partition_streaks,
            ))
            .max(max_parallel_batch_activity_worker_count(
                &timeline_counts,
                &timeline_sets,
                &timeline_streaks,
            ))
    }

    pub fn full_system_parallel_scheduler_total_workers(&self) -> usize {
        let scoped_total_workers = self.total_parallel_scheduler_workers()
            + self.data_cache_parallel_scheduler_total_workers()
            + self.dma_scheduler_total_workers();
        let counts = self.preferred_full_system_parallel_scheduler_batch_worker_counts();
        scoped_total_workers.max(total_parallel_batch_worker_count(&counts))
    }

    fn preferred_full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerCount> {
        let scheduler_counts = preferred_batch_count_worker_counts(
            &self.parallel_scheduler_batch_worker_counts,
            &self.parallel_scheduler_batch_partition_sets,
            &self.parallel_scheduler_batch_partition_streaks,
        );
        let data_cache_counts = preferred_batch_count_worker_counts(
            &self.data_cache_parallel_scheduler_batch_worker_counts,
            &self.data_cache_parallel_scheduler_batch_partition_sets,
            &self.data_cache_parallel_scheduler_batch_partition_streaks,
        );
        let scoped_counts = collect_parallel_batch_worker_counts(
            scheduler_counts
                .into_iter()
                .chain(data_cache_counts)
                .chain(self.dma_scheduler_batch_worker_counts()),
        );
        let full_system_counts = self.explicit_full_system_parallel_scheduler_batch_worker_counts();
        collect_strongest_parallel_batch_worker_counts(scoped_counts, full_system_counts)
    }

    pub fn full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerCount> {
        let scoped_counts = self.scoped_full_system_parallel_scheduler_batch_worker_counts();
        let full_system_counts = self.explicit_full_system_parallel_scheduler_batch_worker_counts();
        collect_strongest_parallel_batch_worker_counts(scoped_counts, full_system_counts)
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerCount> {
        let scheduler_counts = collect_strongest_parallel_batch_worker_counts(
            self.parallel_scheduler_batch_worker_counts.iter().copied(),
            collect_strongest_parallel_batch_worker_counts(
                collect_parallel_batch_worker_counts_from_partition_sets(
                    &self.parallel_scheduler_batch_partition_sets,
                ),
                collect_parallel_batch_worker_counts_from_streaks(
                    &self.parallel_scheduler_batch_partition_streaks,
                ),
            ),
        );
        let data_cache_counts = collect_strongest_parallel_batch_worker_counts(
            self.data_cache_parallel_scheduler_batch_worker_counts
                .iter()
                .copied(),
            collect_strongest_parallel_batch_worker_counts(
                collect_parallel_batch_worker_counts_from_partition_sets(
                    &self.data_cache_parallel_scheduler_batch_partition_sets,
                ),
                collect_parallel_batch_worker_counts_from_streaks(
                    &self.data_cache_parallel_scheduler_batch_partition_streaks,
                ),
            ),
        );
        let dma_partition_sets = self.dma_scheduler_batch_partition_sets();
        let dma_partition_streaks = self.dma_scheduler_batch_partition_streaks();
        let dma_counts = collect_strongest_parallel_batch_worker_counts(
            self.dma_scheduler_batch_worker_counts(),
            collect_strongest_parallel_batch_worker_counts(
                collect_parallel_batch_worker_counts_from_partition_sets(&dma_partition_sets),
                collect_parallel_batch_worker_counts_from_streaks(&dma_partition_streaks),
            ),
        );
        collect_parallel_batch_worker_counts(
            scheduler_counts
                .into_iter()
                .chain(data_cache_counts)
                .chain(dma_counts),
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        let counts = self.full_system_parallel_scheduler_batch_worker_counts();
        parallel_batch_count_at_or_above(&counts, minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_partition_sets(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionSet> {
        let scoped_sets = self.scoped_full_system_parallel_scheduler_batch_partition_sets();
        let full_system_sets = self.explicit_full_system_parallel_scheduler_batch_partition_sets();
        collect_strongest_parallel_batch_partition_sets(scoped_sets, full_system_sets)
    }

    pub fn full_system_parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        collect_parallel_batch_partition_streaks(
            self.explicit_full_system_parallel_scheduler_batch_partition_streaks()
                .into_iter()
                .chain(self.scoped_full_system_parallel_scheduler_batch_partition_streaks()),
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
            &self.explicit_full_system_parallel_scheduler_batch_partition_sets(),
            &self.explicit_full_system_parallel_scheduler_batch_partition_streaks(),
            partitions.iter().copied(),
        ))
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_batch_count_for_partition_set(
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
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        parallel_batch_count_for_partition_set(
            &self.explicit_full_system_parallel_scheduler_batch_partition_sets(),
            &self.explicit_full_system_parallel_scheduler_batch_partition_streaks(),
            partitions,
        )
    }

    pub fn full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let streaks = self.full_system_parallel_scheduler_batch_partition_streaks();
        parallel_batch_streak_count_for_partition_set(&streaks, partitions)
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let streaks = self.scoped_full_system_parallel_scheduler_batch_partition_streaks();
        parallel_batch_streak_count_for_partition_set(&streaks, partitions)
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let streaks = self.explicit_full_system_parallel_scheduler_batch_partition_streaks();
        parallel_batch_streak_count_for_partition_set(&streaks, partitions)
    }

    pub fn full_system_parallel_scheduler_remote_flows(&self) -> Vec<ParallelRemoteFlowRecord> {
        let scoped_flows = self.scoped_full_system_parallel_scheduler_remote_flow_evidence();
        let explicit_flows = self.explicit_full_system_parallel_scheduler_remote_flow_evidence();
        collect_strongest_parallel_remote_flow_aggregates(scoped_flows, explicit_flows)
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_remote_flow_evidence(
        &self,
    ) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_aggregates(
            self.parallel_scheduler_remote_flow_evidence()
                .into_iter()
                .chain(self.data_cache_parallel_scheduler_remote_flow_evidence())
                .chain(self.dma_scheduler_remote_flows()),
        )
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_remote_flow_evidence(
        &self,
    ) -> Vec<ParallelRemoteFlowRecord> {
        collect_parallel_remote_flow_evidence(
            self.full_system_parallel_scheduler_remote_flows
                .iter()
                .copied(),
            self.full_system_parallel_scheduler_remote_sends
                .iter()
                .copied(),
        )
    }

    pub(crate) fn raw_full_system_parallel_scheduler_remote_flows(
        &self,
    ) -> &[ParallelRemoteFlowRecord] {
        &self.full_system_parallel_scheduler_remote_flows
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_remote_sends(
        &self,
    ) -> Vec<ParallelRemoteSendRecord> {
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

    pub(crate) fn raw_full_system_parallel_scheduler_remote_sends(
        &self,
    ) -> &[ParallelRemoteSendRecord] {
        &self.full_system_parallel_scheduler_remote_sends
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
                .chain(self.dma_scheduler_remote_sends())
                .chain(
                    self.full_system_parallel_scheduler_remote_sends
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
                )
                .chain(self.dma_scheduler_initial_frontiers())
                .chain(
                    self.full_system_parallel_scheduler_initial_frontiers
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
                )
                .chain(self.dma_scheduler_final_frontiers())
                .chain(
                    self.full_system_parallel_scheduler_final_frontiers
                        .iter()
                        .copied(),
                ),
        )
    }

    pub(crate) fn raw_full_system_parallel_scheduler_initial_frontiers(
        &self,
    ) -> &[PartitionFrontier] {
        &self.full_system_parallel_scheduler_initial_frontiers
    }

    pub(crate) fn raw_full_system_parallel_scheduler_final_frontiers(
        &self,
    ) -> &[PartitionFrontier] {
        &self.full_system_parallel_scheduler_final_frontiers
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
            || !self
                .full_system_parallel_scheduler_initial_frontiers
                .is_empty()
            || !self
                .full_system_parallel_scheduler_final_frontiers
                .is_empty()
    }

    pub fn full_system_parallel_scheduler_partition_activities(
        &self,
    ) -> Vec<(PartitionId, ParallelPartitionActivity)> {
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
        partitions.extend(
            self.full_system_parallel_scheduler_partition_activities
                .iter()
                .map(|(partition, _)| *partition),
        );
        for set in self.full_system_parallel_scheduler_batch_partition_sets() {
            partitions.extend(set.partitions().iter().copied());
        }
        for streak in self.full_system_parallel_scheduler_batch_partition_streaks() {
            partitions.extend(streak.partitions().iter().copied());
        }
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
        collect_parallel_partition_activities(partitions.into_iter().filter_map(|partition| {
            self.full_system_parallel_scheduler_partition_activity(partition)
                .map(|activity| (partition, activity))
        }))
    }

    pub fn full_system_parallel_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        let lower_bound =
            self.full_system_parallel_scheduler_partition_activity_lower_bound(partition);
        let explicit_full_system_activity = parallel_partition_activity_for_partition(
            &self.full_system_parallel_scheduler_partition_activities,
            &[],
            &[],
            partition,
        );
        merge_parallel_partition_activity_evidence_options(
            lower_bound,
            explicit_full_system_activity,
        )
    }

    pub(crate) fn raw_full_system_parallel_scheduler_partition_activities(
        &self,
    ) -> &[(PartitionId, ParallelPartitionActivity)] {
        &self.full_system_parallel_scheduler_partition_activities
    }

    pub(crate) fn full_system_parallel_scheduler_partition_activity_lower_bound(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        let scoped_activity =
            self.scoped_full_system_parallel_scheduler_partition_activity(partition);
        let batch_partition_sets = self.full_system_parallel_scheduler_batch_partition_sets();
        let batch_partition_streaks = self.full_system_parallel_scheduler_batch_partition_streaks();
        let batch_activity = merge_parallel_partition_activity_evidence_options(
            parallel_batch_partition_activity_for_partition(&batch_partition_sets, partition),
            parallel_batch_streak_activity_for_partition(&batch_partition_streaks, partition),
        );
        merge_parallel_partition_activity_evidence_options(scoped_activity, batch_activity)
    }

    fn scoped_full_system_parallel_scheduler_partition_activity(
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
            || !collect_parallel_remote_flow_evidence(
                self.full_system_parallel_scheduler_remote_flows
                    .iter()
                    .copied(),
                self.full_system_parallel_scheduler_remote_sends
                    .iter()
                    .copied(),
            )
            .is_empty()
    }

    pub fn has_full_system_parallel_scheduler_remote_sends(&self) -> bool {
        self.has_parallel_scheduler_remote_sends()
            || self.has_data_cache_parallel_scheduler_remote_sends()
            || self.has_dma_scheduler_remote_sends()
            || self
                .full_system_parallel_scheduler_remote_sends
                .iter()
                .copied()
                .any(is_parallel_remote_send_evidence)
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerCount> {
        collect_strongest_parallel_batch_worker_counts(
            self.full_system_parallel_scheduler_batch_worker_counts
                .iter()
                .copied(),
            collect_strongest_parallel_batch_worker_counts(
                collect_parallel_batch_worker_counts_from_timeline(
                    &self.full_system_parallel_scheduler_batch_timeline,
                ),
                collect_strongest_parallel_batch_worker_counts(
                    collect_parallel_batch_worker_counts_from_partition_sets(
                        &self.full_system_parallel_scheduler_batch_partition_sets,
                    ),
                    collect_parallel_batch_worker_counts_from_streaks(
                        &self.full_system_parallel_scheduler_batch_partition_streaks,
                    ),
                ),
            ),
        )
    }

    pub(crate) fn raw_full_system_parallel_scheduler_batch_worker_counts(
        &self,
    ) -> &[WorkloadParallelBatchWorkerCount] {
        &self.raw_full_system_parallel_scheduler_batch_worker_counts
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_batch_partition_sets(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionSet> {
        let explicit_scoped_sets = collect_parallel_batch_partition_sets(
            self.parallel_scheduler_batch_partition_sets
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_batch_partition_sets
                        .iter()
                        .cloned(),
                )
                .chain(self.dma_scheduler_batch_partition_sets()),
        );
        let scoped_streaks = self.scoped_full_system_parallel_scheduler_batch_partition_streaks();
        let scoped_streak_sets =
            collect_parallel_batch_partition_sets_from_streaks(&scoped_streaks);
        collect_strongest_parallel_batch_partition_sets(explicit_scoped_sets, scoped_streak_sets)
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_batch_partition_streaks(
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
                )
                .chain(self.dma_scheduler_batch_partition_streaks()),
        )
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_batch_partition_sets(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionSet> {
        let timeline_sets = collect_parallel_batch_partition_sets_from_timeline(
            &self.full_system_parallel_scheduler_batch_timeline,
        );
        let full_system_sets = collect_strongest_parallel_batch_partition_sets(
            self.full_system_parallel_scheduler_batch_partition_sets
                .iter()
                .cloned(),
            timeline_sets,
        );
        let streak_sets = collect_parallel_batch_partition_sets_from_streaks(
            &self.full_system_parallel_scheduler_batch_partition_streaks,
        );
        collect_strongest_parallel_batch_partition_sets(full_system_sets, streak_sets)
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        collect_parallel_batch_partition_streaks(
            self.full_system_parallel_scheduler_batch_partition_streaks
                .iter()
                .cloned()
                .chain(collect_parallel_batch_partition_streaks_from_timeline(
                    &self.full_system_parallel_scheduler_batch_timeline,
                )),
        )
    }

    pub fn has_full_system_parallel_scheduler_work(&self) -> bool {
        self.active_full_system_parallel_scheduler_partition_count() != 0
            || (self.has_full_system_parallel_scheduler_counts
                && (self.full_system_parallel_scheduler_epoch_count != 0
                    || self.full_system_parallel_scheduler_empty_epoch_count != 0
                    || self.full_system_parallel_scheduler_dispatch_count != 0))
            || !self
                .full_system_parallel_scheduler_batch_worker_counts
                .is_empty()
            || !self
                .full_system_parallel_scheduler_batch_worker_count_ticks
                .is_empty()
            || !self
                .full_system_parallel_scheduler_batch_worker_tick_streaks
                .is_empty()
            || !self
                .full_system_parallel_scheduler_partition_activities
                .is_empty()
            || self.has_full_system_progress_transitions()
            || self.full_system_livelock_diagnostic_count() != 0
            || self.has_full_system_parallel_scheduler_remote_flows()
            || self.has_full_system_parallel_scheduler_remote_sends()
            || self.has_full_system_parallel_scheduler_frontiers()
            || self.has_parallel_scheduler_work()
            || self.has_data_cache_parallel_work()
            || self.has_dma_parallel_work()
    }

    pub fn has_parallel_scheduler_work(&self) -> bool {
        self.active_scheduler_partition_count() != 0
            || self.scheduler_epoch_count != 0
            || self.scheduler_empty_epoch_count != 0
            || self.scheduler_dispatch_count != 0
            || self.scheduler_batch_count != 0
            || self.total_parallel_scheduler_workers != 0
            || self.max_parallel_scheduler_workers != 0
            || !self.parallel_scheduler_batch_worker_counts.is_empty()
            || !self.parallel_scheduler_batch_partition_sets.is_empty()
            || !self.parallel_scheduler_batch_partition_streaks.is_empty()
            || self.has_parallel_scheduler_progress_transitions()
            || self.has_parallel_scheduler_livelock_diagnostics()
            || self.has_parallel_scheduler_remote_sends()
            || self.has_parallel_scheduler_frontiers()
    }

    pub fn has_data_cache_parallel_work(&self) -> bool {
        self.data_cache_parallel_run_count != 0
            || self.active_data_cache_parallel_scheduler_partition_count() != 0
            || self.data_cache_parallel_scheduler_epoch_count != 0
            || self.data_cache_parallel_scheduler_empty_epoch_count != 0
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
            || self.has_data_cache_parallel_scheduler_progress_transitions()
            || self.has_data_cache_parallel_scheduler_livelock_diagnostics()
            || self.has_data_cache_parallel_scheduler_remote_sends()
            || self.has_data_cache_parallel_scheduler_frontiers()
    }

    pub fn has_dma_parallel_work(&self) -> bool {
        self.dma_scheduler_epoch_count() != 0
            || self.dma_scheduler_empty_epoch_count() != 0
            || self.dma_scheduler_dispatch_count() != 0
            || self.dma_scheduler_batch_count() != 0
            || self.dma_scheduler_batch_worker_ticks() != 0
            || self.has_dma_scheduler_frontiers()
            || self.has_dma_scheduler_remote_flows()
            || self.has_dma_scheduler_remote_sends()
            || !self.gpu_dma_scheduler_progress_transitions.is_empty()
            || !self
                .accelerator_dma_scheduler_progress_transitions
                .is_empty()
    }
}

fn preferred_batch_count_worker_counts(
    explicit_counts: &[WorkloadParallelBatchWorkerCount],
    partition_sets: &[WorkloadParallelBatchPartitionSet],
    partition_streaks: &[WorkloadParallelBatchPartitionStreak],
) -> Vec<WorkloadParallelBatchWorkerCount> {
    if !explicit_counts.is_empty() {
        return explicit_counts.to_vec();
    }
    collect_strongest_parallel_batch_worker_counts(
        collect_parallel_batch_worker_counts_from_partition_sets(partition_sets),
        collect_parallel_batch_worker_counts_from_streaks(partition_streaks),
    )
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
