use std::collections::BTreeMap;

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets_from_timeline,
    collect_parallel_batch_partition_streaks_from_timeline, collect_parallel_batch_timeline,
    collect_parallel_batch_worker_count_tick_summaries,
    collect_parallel_batch_worker_counts_from_timeline, parallel_batch_active_partition_count,
    parallel_batch_count_for_partition_set, parallel_batch_longest_tick_streak_at_or_above,
    parallel_batch_partition_activity_for_partition, parallel_batch_streak_activity_for_partition,
    parallel_batch_streak_count_for_partition_set, parallel_batch_ticks_at_or_above,
    parallel_batch_ticks_for_worker_count, parallel_batch_worker_ticks,
    parallel_batch_worker_ticks_at_or_above, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchPartitionStreak, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord,
};
use rem6_kernel::{ParallelPartitionActivity, PartitionId, Tick};

use super::WorkloadParallelExecutionSummary;

use crate::result_partition_activity::merge_parallel_partition_activity_evidence_options;

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

    pub fn with_full_system_parallel_scheduler_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        self.full_system_parallel_scheduler_batch_timeline =
            collect_parallel_batch_timeline(records);
        self
    }

    pub fn parallel_scheduler_batch_timeline(&self) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.parallel_scheduler_batch_timeline
    }

    pub fn data_cache_parallel_scheduler_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.data_cache_parallel_scheduler_batch_timeline
    }

    pub fn gpu_dma_scheduler_batch_timeline(&self) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.gpu_dma_scheduler_batch_timeline
    }

    pub fn accelerator_dma_scheduler_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.accelerator_dma_scheduler_batch_timeline
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
        let gpu_streaks = self.gpu_dma_scheduler_batch_partition_streaks();
        let accelerator_streaks = self.accelerator_dma_scheduler_batch_partition_streaks();
        crate::parallel_batch::collect_parallel_batch_partition_streaks(
            gpu_streaks.into_iter().chain(accelerator_streaks),
        )
    }

    pub fn active_gpu_dma_scheduler_partition_count(&self) -> usize {
        let sets = self.gpu_dma_scheduler_batch_partition_sets();
        let streaks = self.gpu_dma_scheduler_batch_partition_streaks();
        parallel_batch_active_partition_count(&sets, &streaks)
    }

    pub fn active_accelerator_dma_scheduler_partition_count(&self) -> usize {
        let sets = self.accelerator_dma_scheduler_batch_partition_sets();
        let streaks = self.accelerator_dma_scheduler_batch_partition_streaks();
        parallel_batch_active_partition_count(&sets, &streaks)
    }

    pub fn gpu_dma_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        let sets = self.gpu_dma_scheduler_batch_partition_sets();
        let streaks = self.gpu_dma_scheduler_batch_partition_streaks();
        merge_parallel_partition_activity_evidence_options(
            parallel_batch_partition_activity_for_partition(&sets, partition),
            parallel_batch_streak_activity_for_partition(&streaks, partition),
        )
    }

    pub fn accelerator_dma_scheduler_partition_activity(
        &self,
        partition: PartitionId,
    ) -> Option<ParallelPartitionActivity> {
        let sets = self.accelerator_dma_scheduler_batch_partition_sets();
        let streaks = self.accelerator_dma_scheduler_batch_partition_streaks();
        merge_parallel_partition_activity_evidence_options(
            parallel_batch_partition_activity_for_partition(&sets, partition),
            parallel_batch_streak_activity_for_partition(&streaks, partition),
        )
    }

    pub fn full_system_parallel_scheduler_batch_timeline(
        &self,
    ) -> Vec<WorkloadParallelBatchTimelineRecord> {
        if self.has_explicit_full_system_parallel_scheduler_batch_timeline() {
            return self.full_system_parallel_scheduler_batch_timeline.clone();
        }
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
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        let mut summaries = collect_parallel_batch_worker_count_tick_summaries(&timeline);
        if self.has_explicit_full_system_parallel_scheduler_batch_timeline() {
            return collect_batch_worker_count_tick_summaries(summaries);
        }
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

    pub fn full_system_parallel_scheduler_batch_ticks_for_worker_count(
        &self,
        worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        if self.has_explicit_full_system_parallel_scheduler_batch_timeline() {
            return parallel_batch_ticks_for_worker_count(&timeline, worker_count);
        }
        parallel_batch_ticks_for_worker_count(&timeline, worker_count)
            .saturating_add(self.dma_scheduler_fallback_batch_ticks_for_worker_count(worker_count))
    }

    pub fn full_system_parallel_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        if self.has_explicit_full_system_parallel_scheduler_batch_timeline() {
            return parallel_batch_ticks_at_or_above(&timeline, minimum_worker_count);
        }
        parallel_batch_ticks_at_or_above(&timeline, minimum_worker_count).saturating_add(
            self.dma_scheduler_fallback_batch_ticks_at_or_above(minimum_worker_count),
        )
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        if self.has_explicit_full_system_parallel_scheduler_batch_timeline() {
            return parallel_batch_worker_ticks(&timeline);
        }
        parallel_batch_worker_ticks(&timeline)
            .saturating_add(self.dma_scheduler_fallback_batch_worker_ticks())
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        if self.has_explicit_full_system_parallel_scheduler_batch_timeline() {
            return parallel_batch_worker_ticks_at_or_above(&timeline, minimum_worker_count);
        }
        parallel_batch_worker_ticks_at_or_above(&timeline, minimum_worker_count).saturating_add(
            self.dma_scheduler_fallback_batch_worker_ticks_at_or_above(minimum_worker_count),
        )
    }

    pub fn full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_longest_tick_streak_at_or_above(&timeline, minimum_worker_count)
    }

    fn dma_scheduler_fallback_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        fallback_batch_ticks_for_worker_count(
            &self.gpu_dma_scheduler_batch_timeline,
            self.gpu_dma_scheduler_batch_worker_count_tick_summaries(),
            worker_count,
        )
        .saturating_add(fallback_batch_ticks_for_worker_count(
            &self.accelerator_dma_scheduler_batch_timeline,
            self.accelerator_dma_scheduler_batch_worker_count_tick_summaries(),
            worker_count,
        ))
    }

    fn dma_scheduler_fallback_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        fallback_batch_ticks_at_or_above(
            &self.gpu_dma_scheduler_batch_timeline,
            self.gpu_dma_scheduler_batch_worker_count_tick_summaries(),
            minimum_worker_count,
        )
        .saturating_add(fallback_batch_ticks_at_or_above(
            &self.accelerator_dma_scheduler_batch_timeline,
            self.accelerator_dma_scheduler_batch_worker_count_tick_summaries(),
            minimum_worker_count,
        ))
    }

    fn dma_scheduler_fallback_batch_worker_ticks(&self) -> Tick {
        fallback_batch_worker_ticks(
            &self.gpu_dma_scheduler_batch_timeline,
            self.gpu_dma_scheduler_batch_worker_count_tick_summaries(),
        )
        .saturating_add(fallback_batch_worker_ticks(
            &self.accelerator_dma_scheduler_batch_timeline,
            self.accelerator_dma_scheduler_batch_worker_count_tick_summaries(),
        ))
    }

    fn dma_scheduler_fallback_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        fallback_batch_worker_ticks_at_or_above(
            &self.gpu_dma_scheduler_batch_timeline,
            self.gpu_dma_scheduler_batch_worker_count_tick_summaries(),
            minimum_worker_count,
        )
        .saturating_add(fallback_batch_worker_ticks_at_or_above(
            &self.accelerator_dma_scheduler_batch_timeline,
            self.accelerator_dma_scheduler_batch_worker_count_tick_summaries(),
            minimum_worker_count,
        ))
    }

    fn has_explicit_full_system_parallel_scheduler_batch_timeline(&self) -> bool {
        !self
            .full_system_parallel_scheduler_batch_timeline
            .is_empty()
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

fn collect_batch_worker_count_tick_summaries(
    summaries: impl IntoIterator<Item = (usize, Tick)>,
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in summaries {
        if worker_count == 0 || ticks == 0 {
            continue;
        }
        let stored = by_worker_count.entry(worker_count).or_default();
        *stored = stored.saturating_add(ticks);
    }
    by_worker_count.into_iter().collect()
}

fn fallback_batch_ticks_for_worker_count(
    timeline: &[WorkloadParallelBatchTimelineRecord],
    summaries: &[(usize, Tick)],
    worker_count: usize,
) -> Tick {
    if has_parallel_batch_timeline_evidence(timeline) {
        return 0;
    }
    summaries
        .iter()
        .filter(|(count, _)| *count == worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

fn fallback_batch_ticks_at_or_above(
    timeline: &[WorkloadParallelBatchTimelineRecord],
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    if has_parallel_batch_timeline_evidence(timeline) {
        return 0;
    }
    summaries
        .iter()
        .filter(|(count, _)| *count >= minimum_worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

fn fallback_batch_worker_ticks(
    timeline: &[WorkloadParallelBatchTimelineRecord],
    summaries: &[(usize, Tick)],
) -> Tick {
    if has_parallel_batch_timeline_evidence(timeline) {
        return 0;
    }
    summaries
        .iter()
        .map(|(count, ticks)| ticks.saturating_mul(*count as Tick))
        .fold(0, Tick::saturating_add)
}

fn fallback_batch_worker_ticks_at_or_above(
    timeline: &[WorkloadParallelBatchTimelineRecord],
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    if has_parallel_batch_timeline_evidence(timeline) {
        return 0;
    }
    summaries
        .iter()
        .filter(|(count, _)| *count >= minimum_worker_count)
        .map(|(count, ticks)| ticks.saturating_mul(*count as Tick))
        .fold(0, Tick::saturating_add)
}

fn valid_batch_timeline_record_count(timeline: &[WorkloadParallelBatchTimelineRecord]) -> usize {
    timeline.iter().filter(|record| !record.is_empty()).count()
}

fn has_parallel_batch_timeline_evidence(timeline: &[WorkloadParallelBatchTimelineRecord]) -> bool {
    timeline
        .iter()
        .any(WorkloadParallelBatchTimelineRecord::is_parallel_evidence)
}
