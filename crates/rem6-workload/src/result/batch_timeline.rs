mod worker_ticks;

use std::collections::BTreeSet;

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets_from_timeline,
    collect_parallel_batch_partition_streaks_from_timeline, collect_parallel_batch_timeline,
    collect_parallel_batch_worker_count_tick_summaries,
    collect_parallel_batch_worker_counts_from_timeline, parallel_batch_count_for_partition_set,
    parallel_batch_longest_tick_streak_at_or_above,
    parallel_batch_partition_activity_for_partition, parallel_batch_streak_activity_for_partition,
    parallel_batch_streak_count_for_partition_set, parallel_batch_ticks_at_or_above,
    parallel_batch_ticks_for_worker_count, parallel_batch_worker_ticks,
    parallel_batch_worker_ticks_at_or_above, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord,
};
use rem6_kernel::{
    ParallelBatchUtilizationRatio, ParallelPartitionActivity, ParallelRemoteFlowRecord,
    ParallelRemoteSendRecord, PartitionId, Tick,
};

use super::WorkloadParallelExecutionSummary;

use self::worker_ticks::{
    batch_ticks_at_or_above, batch_ticks_for_worker_count, batch_worker_tick_streak_at_or_above,
    batch_worker_ticks, batch_worker_ticks_at_or_above, collect_batch_worker_count_tick_summaries,
    collect_batch_worker_slot_tick_summaries, collect_batch_worker_tick_streak_summaries,
    collect_parallel_batch_worker_tick_streak_summaries,
    collect_strongest_batch_worker_count_tick_summaries,
    collect_strongest_batch_worker_slot_tick_summaries,
    collect_strongest_batch_worker_tick_streak_summaries, planned_batch_worker_slot_tick_summaries,
    planned_batch_worker_ticks, recorded_batch_worker_ticks,
};

use crate::result_collect::{is_parallel_remote_flow_evidence, is_parallel_remote_send_evidence};
use crate::result_partition_activity::{
    merge_parallel_partition_activity_evidence_options,
    parallel_partition_activity_for_partition as remote_partition_activity_for_partition,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct WorkloadPlannedBatchWorkerCapacityTicks {
    parallel_scheduler: Tick,
    data_cache_parallel_scheduler: Tick,
    gpu_dma_scheduler: Tick,
    accelerator_dma_scheduler: Tick,
    full_system_parallel_scheduler: Tick,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct WorkloadRecordedBatchWorkerCapacityTicks {
    parallel_scheduler: Tick,
    data_cache_parallel_scheduler: Tick,
    gpu_dma_scheduler: Tick,
    accelerator_dma_scheduler: Tick,
    full_system_parallel_scheduler: Tick,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct WorkloadRecordedBatchWorkerSlotTickSummaries {
    parallel_scheduler: Vec<(usize, Tick, Tick)>,
    data_cache_parallel_scheduler: Vec<(usize, Tick, Tick)>,
    gpu_dma_scheduler: Vec<(usize, Tick, Tick)>,
    accelerator_dma_scheduler: Vec<(usize, Tick, Tick)>,
    full_system_parallel_scheduler: Vec<(usize, Tick, Tick)>,
}

impl WorkloadParallelExecutionSummary {
    pub fn with_parallel_scheduler_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        let timeline =
            collect_scoped_parallel_batch_timeline(WorkloadParallelBatchScope::Scheduler, records);
        self.scheduler_batch_count = valid_batch_timeline_record_count(&timeline);
        self.parallel_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts_from_timeline(&timeline);
        self.parallel_scheduler_batch_partition_sets =
            collect_parallel_batch_partition_sets_from_timeline(&timeline);
        self.parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks_from_timeline(&timeline);
        self.parallel_scheduler_batch_timeline = timeline;
        self
    }

    pub fn with_parallel_scheduler_planned_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        self.parallel_scheduler_planned_batch_timeline =
            collect_scoped_parallel_batch_timeline(WorkloadParallelBatchScope::Scheduler, records);
        self
    }

    pub fn with_parallel_scheduler_planned_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.planned_batch_worker_capacity_ticks.parallel_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_parallel_scheduler_recorded_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.recorded_batch_worker_capacity_ticks.parallel_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
        mut self,
        summaries: impl IntoIterator<Item = (usize, Tick, Tick)>,
    ) -> Self {
        self.recorded_batch_worker_slot_tick_summaries
            .parallel_scheduler = collect_batch_worker_slot_tick_summaries(summaries);
        self
    }

    pub fn with_data_cache_parallel_scheduler_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        let timeline = collect_scoped_parallel_batch_timeline(
            WorkloadParallelBatchScope::DataCacheScheduler,
            records,
        );
        self.data_cache_parallel_scheduler_batch_count =
            valid_batch_timeline_record_count(&timeline);
        self.data_cache_parallel_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts_from_timeline(&timeline);
        self.data_cache_parallel_scheduler_batch_partition_sets =
            collect_parallel_batch_partition_sets_from_timeline(&timeline);
        self.data_cache_parallel_scheduler_batch_partition_streaks =
            collect_parallel_batch_partition_streaks_from_timeline(&timeline);
        self.data_cache_parallel_scheduler_batch_timeline = timeline;
        self
    }

    pub fn with_data_cache_parallel_scheduler_planned_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        self.data_cache_parallel_scheduler_planned_batch_timeline =
            collect_scoped_parallel_batch_timeline(
                WorkloadParallelBatchScope::DataCacheScheduler,
                records,
            );
        self
    }

    pub fn with_data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.planned_batch_worker_capacity_ticks
            .data_cache_parallel_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.recorded_batch_worker_capacity_ticks
            .data_cache_parallel_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_data_cache_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
        mut self,
        summaries: impl IntoIterator<Item = (usize, Tick, Tick)>,
    ) -> Self {
        self.recorded_batch_worker_slot_tick_summaries
            .data_cache_parallel_scheduler = collect_batch_worker_slot_tick_summaries(summaries);
        self
    }

    pub fn with_gpu_dma_scheduler_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        let timeline = collect_scoped_parallel_batch_timeline(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            records,
        );
        self.gpu_dma_scheduler_batch_count = valid_batch_timeline_record_count(&timeline);
        self.gpu_dma_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts_from_timeline(&timeline);
        self.gpu_dma_scheduler_batch_worker_count_ticks =
            collect_parallel_batch_worker_count_tick_summaries(&timeline);
        self.gpu_dma_scheduler_batch_timeline = timeline;
        self
    }

    pub fn with_gpu_dma_scheduler_planned_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        self.gpu_dma_scheduler_planned_batch_timeline = collect_scoped_parallel_batch_timeline(
            WorkloadParallelBatchScope::GpuDmaScheduler,
            records,
        );
        self
    }

    pub fn with_gpu_dma_scheduler_planned_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.planned_batch_worker_capacity_ticks.gpu_dma_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.recorded_batch_worker_capacity_ticks.gpu_dma_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries(
        mut self,
        summaries: impl IntoIterator<Item = (usize, Tick, Tick)>,
    ) -> Self {
        self.recorded_batch_worker_slot_tick_summaries
            .gpu_dma_scheduler = collect_batch_worker_slot_tick_summaries(summaries);
        self
    }

    pub fn with_accelerator_dma_scheduler_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        let timeline = collect_scoped_parallel_batch_timeline(
            WorkloadParallelBatchScope::AcceleratorDmaScheduler,
            records,
        );
        self.accelerator_dma_scheduler_batch_count = valid_batch_timeline_record_count(&timeline);
        self.accelerator_dma_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts_from_timeline(&timeline);
        self.accelerator_dma_scheduler_batch_worker_count_ticks =
            collect_parallel_batch_worker_count_tick_summaries(&timeline);
        self.accelerator_dma_scheduler_batch_timeline = timeline;
        self
    }

    pub fn with_accelerator_dma_scheduler_planned_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        self.accelerator_dma_scheduler_planned_batch_timeline =
            collect_scoped_parallel_batch_timeline(
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                records,
            );
        self
    }

    pub fn with_accelerator_dma_scheduler_planned_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.planned_batch_worker_capacity_ticks
            .accelerator_dma_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.recorded_batch_worker_capacity_ticks
            .accelerator_dma_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_accelerator_dma_scheduler_recorded_batch_worker_slot_tick_summaries(
        mut self,
        summaries: impl IntoIterator<Item = (usize, Tick, Tick)>,
    ) -> Self {
        self.recorded_batch_worker_slot_tick_summaries
            .accelerator_dma_scheduler = collect_batch_worker_slot_tick_summaries(summaries);
        self
    }

    pub fn with_full_system_parallel_scheduler_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        self.full_system_parallel_scheduler_batch_timeline =
            collect_parallel_batch_timeline(records);
        self
    }

    pub fn with_full_system_parallel_scheduler_planned_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        self.full_system_parallel_scheduler_planned_batch_timeline =
            collect_parallel_batch_timeline(records);
        self
    }

    pub fn with_full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.planned_batch_worker_capacity_ticks
            .full_system_parallel_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(
        mut self,
        worker_capacity_ticks: Tick,
    ) -> Self {
        self.recorded_batch_worker_capacity_ticks
            .full_system_parallel_scheduler = worker_capacity_ticks;
        self
    }

    pub fn with_full_system_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
        mut self,
        summaries: impl IntoIterator<Item = (usize, Tick, Tick)>,
    ) -> Self {
        self.recorded_batch_worker_slot_tick_summaries
            .full_system_parallel_scheduler = collect_batch_worker_slot_tick_summaries(summaries);
        self
    }

    pub fn with_full_system_parallel_scheduler_batch_worker_count_tick_summaries(
        mut self,
        summaries: impl IntoIterator<Item = (usize, Tick)>,
    ) -> Self {
        self.raw_full_system_parallel_scheduler_batch_worker_count_ticks =
            summaries.into_iter().collect();
        self.full_system_parallel_scheduler_batch_worker_count_ticks =
            collect_batch_worker_count_tick_summaries(
                self.raw_full_system_parallel_scheduler_batch_worker_count_ticks
                    .iter()
                    .copied(),
            );
        self
    }

    pub fn with_full_system_parallel_scheduler_batch_worker_tick_streak_summaries(
        mut self,
        summaries: impl IntoIterator<Item = (usize, Tick)>,
    ) -> Self {
        self.raw_full_system_parallel_scheduler_batch_worker_tick_streaks =
            summaries.into_iter().collect();
        self.full_system_parallel_scheduler_batch_worker_tick_streaks =
            collect_batch_worker_tick_streak_summaries(
                self.raw_full_system_parallel_scheduler_batch_worker_tick_streaks
                    .iter()
                    .copied(),
            );
        self
    }

    pub fn parallel_scheduler_batch_timeline(&self) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.parallel_scheduler_batch_timeline
    }

    pub fn parallel_scheduler_planned_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.parallel_scheduler_planned_batch_timeline
    }

    pub fn data_cache_parallel_scheduler_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.data_cache_parallel_scheduler_batch_timeline
    }

    pub fn data_cache_parallel_scheduler_planned_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.data_cache_parallel_scheduler_planned_batch_timeline
    }

    pub fn gpu_dma_scheduler_batch_timeline(&self) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.gpu_dma_scheduler_batch_timeline
    }

    pub fn gpu_dma_scheduler_planned_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.gpu_dma_scheduler_planned_batch_timeline
    }

    pub fn accelerator_dma_scheduler_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.accelerator_dma_scheduler_batch_timeline
    }

    pub fn accelerator_dma_scheduler_planned_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.accelerator_dma_scheduler_planned_batch_timeline
    }

    pub fn dma_scheduler_batch_timeline(&self) -> Vec<WorkloadParallelBatchTimelineRecord> {
        collect_parallel_batch_timeline(
            self.gpu_dma_scheduler_batch_timeline.iter().cloned().chain(
                self.accelerator_dma_scheduler_batch_timeline
                    .iter()
                    .cloned(),
            ),
        )
    }

    pub fn dma_scheduler_planned_batch_timeline(&self) -> Vec<WorkloadParallelBatchTimelineRecord> {
        collect_parallel_batch_timeline(
            self.gpu_dma_scheduler_planned_batch_timeline
                .iter()
                .cloned()
                .chain(
                    self.accelerator_dma_scheduler_planned_batch_timeline
                        .iter()
                        .cloned(),
                ),
        )
    }

    pub fn gpu_dma_scheduler_batch_partition_sets(&self) -> Vec<WorkloadParallelBatchPartitionSet> {
        collect_parallel_batch_partition_sets_from_timeline(&self.gpu_dma_scheduler_batch_timeline)
    }

    pub fn accelerator_dma_scheduler_batch_partition_sets(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionSet> {
        collect_parallel_batch_partition_sets_from_timeline(
            &self.accelerator_dma_scheduler_batch_timeline,
        )
    }

    pub fn dma_scheduler_batch_partition_sets(&self) -> Vec<WorkloadParallelBatchPartitionSet> {
        let gpu_sets = self.gpu_dma_scheduler_batch_partition_sets();
        let accelerator_sets = self.accelerator_dma_scheduler_batch_partition_sets();
        crate::parallel_batch::collect_parallel_batch_partition_sets(
            gpu_sets.into_iter().chain(accelerator_sets),
        )
    }

    pub fn gpu_dma_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        collect_parallel_batch_partition_streaks_from_timeline(
            &self.gpu_dma_scheduler_batch_timeline,
        )
    }

    pub fn accelerator_dma_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        collect_parallel_batch_partition_streaks_from_timeline(
            &self.accelerator_dma_scheduler_batch_timeline,
        )
    }

    pub fn dma_scheduler_batch_partition_streaks(
        &self,
    ) -> Vec<WorkloadParallelBatchPartitionStreak> {
        let timeline = self.dma_scheduler_batch_timeline();
        collect_parallel_batch_partition_streaks_from_timeline(&timeline)
    }

    pub fn active_gpu_dma_scheduler_partition_count(&self) -> usize {
        let sets = self.gpu_dma_scheduler_batch_partition_sets();
        let streaks = self.gpu_dma_scheduler_batch_partition_streaks();
        batch_and_remote_active_partition_count(
            &sets,
            &streaks,
            &self.gpu_dma_scheduler_remote_flows,
            &self.gpu_dma_scheduler_remote_sends,
        )
    }

    pub fn active_accelerator_dma_scheduler_partition_count(&self) -> usize {
        let sets = self.accelerator_dma_scheduler_batch_partition_sets();
        let streaks = self.accelerator_dma_scheduler_batch_partition_streaks();
        batch_and_remote_active_partition_count(
            &sets,
            &streaks,
            &self.accelerator_dma_scheduler_remote_flows,
            &self.accelerator_dma_scheduler_remote_sends,
        )
    }

    pub fn active_dma_scheduler_partition_count(&self) -> usize {
        let sets = self.dma_scheduler_batch_partition_sets();
        let streaks = self.dma_scheduler_batch_partition_streaks();
        let flows = self.dma_scheduler_remote_flows();
        let sends = self.dma_scheduler_remote_sends();
        batch_and_remote_active_partition_count(&sets, &streaks, &flows, &sends)
    }

    pub fn gpu_dma_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        let sets = self.gpu_dma_scheduler_batch_partition_sets();
        let streaks = self.gpu_dma_scheduler_batch_partition_streaks();
        let batch_activity = merge_parallel_partition_activity_evidence_options(
            parallel_batch_partition_activity_for_partition(&sets, partition),
            parallel_batch_streak_activity_for_partition(&streaks, partition),
        );
        merge_parallel_partition_activity_evidence_options(
            batch_activity,
            remote_partition_activity_for_partition(
                &[],
                &self.gpu_dma_scheduler_remote_flows,
                &self.gpu_dma_scheduler_remote_sends,
                partition,
            ),
        )
    }

    pub fn accelerator_dma_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        let sets = self.accelerator_dma_scheduler_batch_partition_sets();
        let streaks = self.accelerator_dma_scheduler_batch_partition_streaks();
        let batch_activity = merge_parallel_partition_activity_evidence_options(
            parallel_batch_partition_activity_for_partition(&sets, partition),
            parallel_batch_streak_activity_for_partition(&streaks, partition),
        );
        merge_parallel_partition_activity_evidence_options(
            batch_activity,
            remote_partition_activity_for_partition(
                &[],
                &self.accelerator_dma_scheduler_remote_flows,
                &self.accelerator_dma_scheduler_remote_sends,
                partition,
            ),
        )
    }

    pub fn dma_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        let sets = self.dma_scheduler_batch_partition_sets();
        let streaks = self.dma_scheduler_batch_partition_streaks();
        let batch_activity = merge_parallel_partition_activity_evidence_options(
            parallel_batch_partition_activity_for_partition(&sets, partition),
            parallel_batch_streak_activity_for_partition(&streaks, partition),
        );
        let flows = self.dma_scheduler_remote_flows();
        let sends = self.dma_scheduler_remote_sends();
        merge_parallel_partition_activity_evidence_options(
            batch_activity,
            remote_partition_activity_for_partition(&[], &flows, &sends, partition),
        )
    }

    pub fn full_system_parallel_scheduler_batch_timeline(
        &self,
    ) -> Vec<WorkloadParallelBatchTimelineRecord> {
        if self.has_explicit_full_system_parallel_scheduler_batch_timeline() {
            if !self.explicit_full_system_parallel_scheduler_batch_timeline_covers_scoped() {
                return self.scoped_full_system_parallel_scheduler_batch_timeline();
            }
            return self.full_system_parallel_scheduler_batch_timeline.clone();
        }
        self.scoped_full_system_parallel_scheduler_batch_timeline()
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.full_system_parallel_scheduler_batch_timeline
    }

    pub fn full_system_parallel_scheduler_planned_batch_timeline(
        &self,
    ) -> Vec<WorkloadParallelBatchTimelineRecord> {
        if self.has_explicit_full_system_parallel_scheduler_planned_batch_timeline() {
            return self
                .full_system_parallel_scheduler_planned_batch_timeline
                .clone();
        }
        self.scoped_full_system_parallel_scheduler_planned_batch_timeline()
    }

    pub fn parallel_scheduler_planned_batch_worker_ticks(&self) -> Tick {
        planned_batch_worker_ticks(&self.parallel_scheduler_planned_batch_timeline)
    }

    pub fn parallel_scheduler_planned_batch_worker_capacity_ticks(&self) -> Tick {
        self.planned_batch_worker_capacity_ticks.parallel_scheduler
    }

    pub fn parallel_scheduler_planned_batch_idle_worker_ticks(&self) -> Tick {
        self.parallel_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_sub(self.parallel_scheduler_planned_batch_worker_ticks())
    }

    pub fn parallel_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        planned_batch_worker_slot_tick_summaries(
            &self.parallel_scheduler_planned_batch_timeline,
            self.parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn parallel_scheduler_planned_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.parallel_scheduler_planned_batch_worker_ticks(),
            self.parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_ticks(&self) -> Tick {
        planned_batch_worker_ticks(&self.data_cache_parallel_scheduler_planned_batch_timeline)
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(&self) -> Tick {
        self.planned_batch_worker_capacity_ticks
            .data_cache_parallel_scheduler
    }

    pub fn data_cache_parallel_scheduler_planned_batch_idle_worker_ticks(&self) -> Tick {
        self.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_sub(self.data_cache_parallel_scheduler_planned_batch_worker_ticks())
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        planned_batch_worker_slot_tick_summaries(
            &self.data_cache_parallel_scheduler_planned_batch_timeline,
            self.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn data_cache_parallel_scheduler_planned_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.data_cache_parallel_scheduler_planned_batch_worker_ticks(),
            self.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn gpu_dma_scheduler_planned_batch_worker_ticks(&self) -> Tick {
        planned_batch_worker_ticks(&self.gpu_dma_scheduler_planned_batch_timeline)
    }

    pub fn gpu_dma_scheduler_planned_batch_worker_capacity_ticks(&self) -> Tick {
        self.planned_batch_worker_capacity_ticks.gpu_dma_scheduler
    }

    pub fn gpu_dma_scheduler_planned_batch_idle_worker_ticks(&self) -> Tick {
        self.gpu_dma_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_sub(self.gpu_dma_scheduler_planned_batch_worker_ticks())
    }

    pub fn gpu_dma_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        planned_batch_worker_slot_tick_summaries(
            &self.gpu_dma_scheduler_planned_batch_timeline,
            self.gpu_dma_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn gpu_dma_scheduler_planned_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.gpu_dma_scheduler_planned_batch_worker_ticks(),
            self.gpu_dma_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn accelerator_dma_scheduler_planned_batch_worker_ticks(&self) -> Tick {
        planned_batch_worker_ticks(&self.accelerator_dma_scheduler_planned_batch_timeline)
    }

    pub fn accelerator_dma_scheduler_planned_batch_worker_capacity_ticks(&self) -> Tick {
        self.planned_batch_worker_capacity_ticks
            .accelerator_dma_scheduler
    }

    pub fn accelerator_dma_scheduler_planned_batch_idle_worker_ticks(&self) -> Tick {
        self.accelerator_dma_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_sub(self.accelerator_dma_scheduler_planned_batch_worker_ticks())
    }

    pub fn accelerator_dma_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        planned_batch_worker_slot_tick_summaries(
            &self.accelerator_dma_scheduler_planned_batch_timeline,
            self.accelerator_dma_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn accelerator_dma_scheduler_planned_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.accelerator_dma_scheduler_planned_batch_worker_ticks(),
            self.accelerator_dma_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn dma_scheduler_planned_batch_worker_ticks(&self) -> Tick {
        let timeline = self.dma_scheduler_planned_batch_timeline();
        planned_batch_worker_ticks(&timeline)
    }

    pub fn dma_scheduler_planned_batch_worker_capacity_ticks(&self) -> Tick {
        self.gpu_dma_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_add(self.accelerator_dma_scheduler_planned_batch_worker_capacity_ticks())
    }

    pub fn dma_scheduler_planned_batch_idle_worker_ticks(&self) -> Tick {
        self.dma_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_sub(self.dma_scheduler_planned_batch_worker_ticks())
    }

    pub fn dma_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        let timeline = self.dma_scheduler_planned_batch_timeline();
        planned_batch_worker_slot_tick_summaries(
            &timeline,
            self.dma_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn dma_scheduler_planned_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.dma_scheduler_planned_batch_worker_ticks(),
            self.dma_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_ticks(&self) -> Tick {
        let timeline = self.full_system_parallel_scheduler_planned_batch_timeline();
        planned_batch_worker_ticks(&timeline)
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(&self) -> Tick {
        let scoped_capacity_ticks = self
            .parallel_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_add(
                self.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
            )
            .saturating_add(self.dma_scheduler_planned_batch_worker_capacity_ticks());
        self.planned_batch_worker_capacity_ticks
            .full_system_parallel_scheduler
            .max(scoped_capacity_ticks)
    }

    pub fn full_system_parallel_scheduler_planned_batch_idle_worker_ticks(&self) -> Tick {
        self.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks()
            .saturating_sub(self.full_system_parallel_scheduler_planned_batch_worker_ticks())
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        let scoped_summaries =
            self.scoped_full_system_parallel_scheduler_planned_batch_worker_slot_tick_summaries();
        if !self.has_explicit_full_system_parallel_scheduler_planned_batch_timeline() {
            return scoped_summaries;
        }
        if !self.explicit_full_system_parallel_scheduler_planned_batch_timeline_covers_scoped() {
            return scoped_summaries;
        }
        let explicit_summaries =
            self.explicit_full_system_parallel_scheduler_planned_batch_worker_slot_tick_summaries();
        collect_strongest_batch_worker_slot_tick_summaries(&explicit_summaries, &scoped_summaries)
    }

    pub fn full_system_parallel_scheduler_planned_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.full_system_parallel_scheduler_planned_batch_worker_ticks(),
            self.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(),
        )
    }

    pub fn parallel_scheduler_recorded_batch_worker_ticks(&self) -> Tick {
        recorded_batch_worker_ticks(
            &self.parallel_scheduler_batch_timeline,
            &self
                .recorded_batch_worker_slot_tick_summaries
                .parallel_scheduler,
        )
    }

    pub fn parallel_scheduler_recorded_batch_worker_capacity_ticks(&self) -> Tick {
        self.recorded_batch_worker_capacity_ticks.parallel_scheduler
    }

    pub fn parallel_scheduler_recorded_batch_idle_worker_ticks(&self) -> Tick {
        self.parallel_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_sub(self.parallel_scheduler_recorded_batch_worker_ticks())
    }

    pub fn parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        self.recorded_batch_worker_slot_tick_summaries
            .parallel_scheduler
            .clone()
    }

    pub fn parallel_scheduler_recorded_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.parallel_scheduler_recorded_batch_worker_ticks(),
            self.parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        )
    }

    pub fn data_cache_parallel_scheduler_recorded_batch_worker_ticks(&self) -> Tick {
        recorded_batch_worker_ticks(
            &self.data_cache_parallel_scheduler_batch_timeline,
            &self
                .recorded_batch_worker_slot_tick_summaries
                .data_cache_parallel_scheduler,
        )
    }

    pub fn data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(&self) -> Tick {
        self.recorded_batch_worker_capacity_ticks
            .data_cache_parallel_scheduler
    }

    pub fn data_cache_parallel_scheduler_recorded_batch_idle_worker_ticks(&self) -> Tick {
        self.data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_sub(self.data_cache_parallel_scheduler_recorded_batch_worker_ticks())
    }

    pub fn data_cache_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        self.recorded_batch_worker_slot_tick_summaries
            .data_cache_parallel_scheduler
            .clone()
    }

    pub fn data_cache_parallel_scheduler_recorded_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.data_cache_parallel_scheduler_recorded_batch_worker_ticks(),
            self.data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        )
    }

    pub fn gpu_dma_scheduler_recorded_batch_worker_ticks(&self) -> Tick {
        recorded_batch_worker_ticks(
            &self.gpu_dma_scheduler_batch_timeline,
            &self
                .recorded_batch_worker_slot_tick_summaries
                .gpu_dma_scheduler,
        )
    }

    pub fn gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(&self) -> Tick {
        self.recorded_batch_worker_capacity_ticks.gpu_dma_scheduler
    }

    pub fn gpu_dma_scheduler_recorded_batch_idle_worker_ticks(&self) -> Tick {
        self.gpu_dma_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_sub(self.gpu_dma_scheduler_recorded_batch_worker_ticks())
    }

    pub fn gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        self.recorded_batch_worker_slot_tick_summaries
            .gpu_dma_scheduler
            .clone()
    }

    pub fn gpu_dma_scheduler_recorded_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.gpu_dma_scheduler_recorded_batch_worker_ticks(),
            self.gpu_dma_scheduler_recorded_batch_worker_capacity_ticks(),
        )
    }

    pub fn accelerator_dma_scheduler_recorded_batch_worker_ticks(&self) -> Tick {
        recorded_batch_worker_ticks(
            &self.accelerator_dma_scheduler_batch_timeline,
            &self
                .recorded_batch_worker_slot_tick_summaries
                .accelerator_dma_scheduler,
        )
    }

    pub fn accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks(&self) -> Tick {
        self.recorded_batch_worker_capacity_ticks
            .accelerator_dma_scheduler
    }

    pub fn accelerator_dma_scheduler_recorded_batch_idle_worker_ticks(&self) -> Tick {
        self.accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_sub(self.accelerator_dma_scheduler_recorded_batch_worker_ticks())
    }

    pub fn accelerator_dma_scheduler_recorded_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        self.recorded_batch_worker_slot_tick_summaries
            .accelerator_dma_scheduler
            .clone()
    }

    pub fn accelerator_dma_scheduler_recorded_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.accelerator_dma_scheduler_recorded_batch_worker_ticks(),
            self.accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks(),
        )
    }

    pub fn dma_scheduler_recorded_batch_worker_ticks(&self) -> Tick {
        self.gpu_dma_scheduler_recorded_batch_worker_ticks()
            .saturating_add(self.accelerator_dma_scheduler_recorded_batch_worker_ticks())
    }

    pub fn dma_scheduler_recorded_batch_worker_capacity_ticks(&self) -> Tick {
        self.gpu_dma_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_add(self.accelerator_dma_scheduler_recorded_batch_worker_capacity_ticks())
    }

    pub fn dma_scheduler_recorded_batch_idle_worker_ticks(&self) -> Tick {
        self.dma_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_sub(self.dma_scheduler_recorded_batch_worker_ticks())
    }

    pub fn dma_scheduler_recorded_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        collect_batch_worker_slot_tick_summaries(
            self.gpu_dma_scheduler_recorded_batch_worker_slot_tick_summaries()
                .into_iter()
                .chain(self.accelerator_dma_scheduler_recorded_batch_worker_slot_tick_summaries()),
        )
    }

    pub fn dma_scheduler_recorded_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.dma_scheduler_recorded_batch_worker_ticks(),
            self.dma_scheduler_recorded_batch_worker_capacity_ticks(),
        )
    }

    pub fn full_system_parallel_scheduler_recorded_batch_worker_ticks(&self) -> Tick {
        self.parallel_scheduler_recorded_batch_worker_ticks()
            .saturating_add(self.data_cache_parallel_scheduler_recorded_batch_worker_ticks())
            .saturating_add(self.dma_scheduler_recorded_batch_worker_ticks())
    }

    pub fn full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(&self) -> Tick {
        let scoped_capacity_ticks = self
            .parallel_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_add(
                self.data_cache_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
            )
            .saturating_add(self.dma_scheduler_recorded_batch_worker_capacity_ticks());
        self.recorded_batch_worker_capacity_ticks
            .full_system_parallel_scheduler
            .max(scoped_capacity_ticks)
    }

    pub fn full_system_parallel_scheduler_recorded_batch_idle_worker_ticks(&self) -> Tick {
        self.full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks()
            .saturating_sub(self.full_system_parallel_scheduler_recorded_batch_worker_ticks())
    }

    pub fn full_system_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        let scoped_summaries = collect_batch_worker_slot_tick_summaries(
            self.parallel_scheduler_recorded_batch_worker_slot_tick_summaries()
                .into_iter()
                .chain(
                    self.data_cache_parallel_scheduler_recorded_batch_worker_slot_tick_summaries(),
                )
                .chain(self.dma_scheduler_recorded_batch_worker_slot_tick_summaries()),
        );
        collect_strongest_batch_worker_slot_tick_summaries(
            &self
                .recorded_batch_worker_slot_tick_summaries
                .full_system_parallel_scheduler,
            &scoped_summaries,
        )
    }

    pub fn full_system_parallel_scheduler_recorded_batch_utilization_ratio(
        &self,
    ) -> Option<ParallelBatchUtilizationRatio> {
        ParallelBatchUtilizationRatio::new(
            self.full_system_parallel_scheduler_recorded_batch_worker_ticks(),
            self.full_system_parallel_scheduler_recorded_batch_worker_capacity_ticks(),
        )
    }

    pub fn parallel_scheduler_batch_worker_count_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_count_tick_summaries(&self.parallel_scheduler_batch_timeline)
    }

    pub fn data_cache_parallel_scheduler_batch_worker_count_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_count_tick_summaries(
            &self.data_cache_parallel_scheduler_batch_timeline,
        )
    }

    pub fn full_system_parallel_scheduler_batch_worker_count_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_strongest_batch_worker_count_tick_summaries(
            &self.explicit_full_system_parallel_scheduler_batch_worker_count_tick_summaries(),
            &self.scoped_full_system_parallel_scheduler_batch_worker_count_tick_summaries(),
        )
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_batch_worker_count_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_strongest_batch_worker_count_tick_summaries(
            &self.full_system_parallel_scheduler_batch_worker_count_ticks,
            &collect_parallel_batch_worker_count_tick_summaries(
                &self.full_system_parallel_scheduler_batch_timeline,
            ),
        )
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_batch_worker_count_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        let timeline = self.scoped_full_system_parallel_scheduler_batch_timeline();
        let mut summaries = collect_parallel_batch_worker_count_tick_summaries(&timeline);
        if !has_parallel_batch_timeline_evidence(&self.gpu_dma_scheduler_batch_timeline) {
            summaries.extend(
                self.gpu_dma_scheduler_batch_worker_count_tick_summaries()
                    .iter()
                    .copied(),
            );
        }
        if !has_parallel_batch_timeline_evidence(&self.accelerator_dma_scheduler_batch_timeline) {
            summaries.extend(
                self.accelerator_dma_scheduler_batch_worker_count_tick_summaries()
                    .iter()
                    .copied(),
            );
        }
        collect_batch_worker_count_tick_summaries(summaries)
    }

    pub fn full_system_parallel_scheduler_batch_worker_tick_streak_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_strongest_batch_worker_tick_streak_summaries(
            &self.explicit_full_system_parallel_scheduler_batch_worker_tick_streak_summaries(),
            &self.scoped_full_system_parallel_scheduler_batch_worker_tick_streak_summaries(),
        )
    }

    pub(crate) fn explicit_full_system_parallel_scheduler_batch_worker_tick_streak_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_strongest_batch_worker_tick_streak_summaries(
            &self.full_system_parallel_scheduler_batch_worker_tick_streaks,
            &collect_parallel_batch_worker_tick_streak_summaries(
                &self.full_system_parallel_scheduler_batch_timeline,
            ),
        )
    }

    pub(crate) fn scoped_full_system_parallel_scheduler_batch_worker_tick_streak_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_tick_streak_summaries(
            &self.scoped_full_system_parallel_scheduler_batch_timeline(),
        )
    }

    pub fn parallel_scheduler_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        parallel_batch_ticks_for_worker_count(&self.parallel_scheduler_batch_timeline, worker_count)
    }

    pub fn parallel_scheduler_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        parallel_batch_ticks_at_or_above(
            &self.parallel_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        parallel_batch_worker_ticks(&self.parallel_scheduler_batch_timeline)
    }

    pub fn parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_worker_ticks_at_or_above(
            &self.parallel_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_longest_tick_streak_at_or_above(
            &self.parallel_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_ticks_for_worker_count(
        &self,
        worker_count: usize,
    ) -> Tick {
        parallel_batch_ticks_for_worker_count(
            &self.data_cache_parallel_scheduler_batch_timeline,
            worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_ticks_at_or_above(
            &self.data_cache_parallel_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        parallel_batch_worker_ticks(&self.data_cache_parallel_scheduler_batch_timeline)
    }

    pub fn data_cache_parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_worker_ticks_at_or_above(
            &self.data_cache_parallel_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_longest_tick_streak_at_or_above(
            &self.data_cache_parallel_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn gpu_dma_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_longest_tick_streak_at_or_above(
            &self.gpu_dma_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn accelerator_dma_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_longest_tick_streak_at_or_above(
            &self.accelerator_dma_scheduler_batch_timeline,
            minimum_worker_count,
        )
    }

    pub fn dma_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        parallel_batch_longest_tick_streak_at_or_above(
            &self.dma_scheduler_batch_timeline(),
            minimum_worker_count,
        )
    }

    pub fn gpu_dma_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let sets = self.gpu_dma_scheduler_batch_partition_sets();
        let streaks = self.gpu_dma_scheduler_batch_partition_streaks();
        parallel_batch_count_for_partition_set(&sets, &streaks, partitions)
    }

    pub fn accelerator_dma_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let sets = self.accelerator_dma_scheduler_batch_partition_sets();
        let streaks = self.accelerator_dma_scheduler_batch_partition_streaks();
        parallel_batch_count_for_partition_set(&sets, &streaks, partitions)
    }

    pub fn dma_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let sets = self.dma_scheduler_batch_partition_sets();
        let streaks = self.dma_scheduler_batch_partition_streaks();
        parallel_batch_count_for_partition_set(&sets, &streaks, partitions)
    }

    pub fn gpu_dma_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let streaks = self.gpu_dma_scheduler_batch_partition_streaks();
        parallel_batch_streak_count_for_partition_set(&streaks, partitions)
    }

    pub fn accelerator_dma_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let streaks = self.accelerator_dma_scheduler_batch_partition_streaks();
        parallel_batch_streak_count_for_partition_set(&streaks, partitions)
    }

    pub fn dma_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let streaks = self.dma_scheduler_batch_partition_streaks();
        parallel_batch_streak_count_for_partition_set(&streaks, partitions)
    }

    pub fn full_system_parallel_scheduler_batch_ticks_for_worker_count(
        &self,
        worker_count: usize,
    ) -> Tick {
        let summaries = self.full_system_parallel_scheduler_batch_worker_count_tick_summaries();
        batch_ticks_for_worker_count(&summaries, worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let summaries = self.full_system_parallel_scheduler_batch_worker_count_tick_summaries();
        batch_ticks_at_or_above(&summaries, minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        let summaries = self.full_system_parallel_scheduler_batch_worker_count_tick_summaries();
        batch_worker_ticks(&summaries)
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let summaries = self.full_system_parallel_scheduler_batch_worker_count_tick_summaries();
        batch_worker_ticks_at_or_above(&summaries, minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let summaries = self.full_system_parallel_scheduler_batch_worker_tick_streak_summaries();
        batch_worker_tick_streak_at_or_above(&summaries, minimum_worker_count)
    }

    fn has_explicit_full_system_parallel_scheduler_batch_timeline(&self) -> bool {
        !self
            .full_system_parallel_scheduler_batch_timeline
            .is_empty()
    }

    fn scoped_full_system_parallel_scheduler_batch_timeline(
        &self,
    ) -> Vec<WorkloadParallelBatchTimelineRecord> {
        collect_parallel_batch_timeline(
            self.parallel_scheduler_batch_timeline
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_batch_timeline
                        .iter()
                        .cloned(),
                )
                .chain(self.dma_scheduler_batch_timeline()),
        )
    }

    fn explicit_full_system_parallel_scheduler_batch_timeline_covers_scoped(&self) -> bool {
        let scoped_timeline = self.scoped_full_system_parallel_scheduler_batch_timeline();
        scoped_timeline.iter().all(|scoped| {
            self.full_system_parallel_scheduler_batch_timeline
                .iter()
                .any(|record| record == scoped)
        })
    }

    fn has_explicit_full_system_parallel_scheduler_planned_batch_timeline(&self) -> bool {
        !self
            .full_system_parallel_scheduler_planned_batch_timeline
            .is_empty()
    }

    fn scoped_full_system_parallel_scheduler_planned_batch_timeline(
        &self,
    ) -> Vec<WorkloadParallelBatchTimelineRecord> {
        collect_parallel_batch_timeline(
            self.parallel_scheduler_planned_batch_timeline
                .iter()
                .cloned()
                .chain(
                    self.data_cache_parallel_scheduler_planned_batch_timeline
                        .iter()
                        .cloned(),
                )
                .chain(self.dma_scheduler_planned_batch_timeline()),
        )
    }

    fn explicit_full_system_parallel_scheduler_planned_batch_timeline_covers_scoped(&self) -> bool {
        let scoped_timeline = self.scoped_full_system_parallel_scheduler_planned_batch_timeline();
        scoped_timeline.iter().all(|scoped| {
            self.full_system_parallel_scheduler_planned_batch_timeline
                .iter()
                .any(|record| record == scoped)
        })
    }

    fn explicit_full_system_parallel_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        planned_batch_worker_slot_tick_summaries(
            &self.full_system_parallel_scheduler_planned_batch_timeline,
            self.planned_batch_worker_capacity_ticks
                .full_system_parallel_scheduler,
        )
    }

    fn scoped_full_system_parallel_scheduler_planned_batch_worker_slot_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick, Tick)> {
        collect_batch_worker_slot_tick_summaries(
            self.parallel_scheduler_planned_batch_worker_slot_tick_summaries()
                .into_iter()
                .chain(
                    self.data_cache_parallel_scheduler_planned_batch_worker_slot_tick_summaries(),
                )
                .chain(self.dma_scheduler_planned_batch_worker_slot_tick_summaries()),
        )
    }
}

fn collect_scoped_parallel_batch_timeline(
    scope: WorkloadParallelBatchScope,
    records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    collect_parallel_batch_timeline(records.into_iter().map(|record| {
        WorkloadParallelBatchTimelineRecord::new(
            scope,
            record.start_tick(),
            record.horizon(),
            record.partitions().iter().copied(),
            record.worker_count(),
        )
    }))
}

fn batch_and_remote_active_partition_count(
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
    flows: &[ParallelRemoteFlowRecord],
    sends: &[ParallelRemoteSendRecord],
) -> usize {
    let mut partitions = BTreeSet::new();
    for set in sets {
        if set.is_parallel_evidence() {
            partitions.extend(set.partitions().iter().copied());
        }
    }
    for streak in streaks {
        if streak.is_parallel_evidence() {
            partitions.extend(streak.partitions().iter().copied());
        }
    }
    for flow in flows {
        if is_parallel_remote_flow_evidence(*flow) {
            partitions.insert(flow.source());
            partitions.insert(flow.target());
        }
    }
    for send in sends {
        if is_parallel_remote_send_evidence(*send) {
            partitions.insert(send.source());
            partitions.insert(send.target());
        }
    }
    partitions.len()
}

fn valid_batch_timeline_record_count(timeline: &[WorkloadParallelBatchTimelineRecord]) -> usize {
    timeline.iter().filter(|record| !record.is_empty()).count()
}

fn has_parallel_batch_timeline_evidence(timeline: &[WorkloadParallelBatchTimelineRecord]) -> bool {
    timeline
        .iter()
        .any(WorkloadParallelBatchTimelineRecord::is_parallel_evidence)
}
