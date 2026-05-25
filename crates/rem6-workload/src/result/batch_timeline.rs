use std::collections::BTreeMap;

use crate::parallel_batch::{
    collect_parallel_batch_partition_sets_from_timeline,
    collect_parallel_batch_partition_streaks_from_timeline, collect_parallel_batch_timeline,
    collect_parallel_batch_worker_count_tick_summaries,
    collect_parallel_batch_worker_counts_from_timeline,
    parallel_batch_longest_tick_streak_at_or_above, parallel_batch_ticks_at_or_above,
    parallel_batch_ticks_for_worker_count, parallel_batch_worker_ticks,
    parallel_batch_worker_ticks_at_or_above, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord,
};
use rem6_kernel::Tick;

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub fn with_parallel_scheduler_batch_timeline(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
    ) -> Self {
        let timeline =
            collect_scoped_parallel_batch_timeline(WorkloadParallelBatchScope::Scheduler, records);
        self.scheduler_batch_count = timeline.len();
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
        self.data_cache_parallel_scheduler_batch_count = timeline.len();
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
        self.gpu_dma_scheduler_batch_count = timeline.len();
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
        self.accelerator_dma_scheduler_batch_count = timeline.len();
        self.accelerator_dma_scheduler_batch_worker_counts =
            collect_parallel_batch_worker_counts_from_timeline(&timeline);
        self.accelerator_dma_scheduler_batch_worker_count_ticks =
            collect_parallel_batch_worker_count_tick_summaries(&timeline);
        self.accelerator_dma_scheduler_batch_timeline = timeline;
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

    pub fn full_system_parallel_scheduler_batch_timeline(
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
        if self.gpu_dma_scheduler_batch_timeline.is_empty() {
            summaries.extend(
                self.gpu_dma_scheduler_batch_worker_count_tick_summaries()
                    .iter()
                    .copied(),
            );
        }
        if self.accelerator_dma_scheduler_batch_timeline.is_empty() {
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

    pub fn full_system_parallel_scheduler_batch_ticks_for_worker_count(
        &self,
        worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_ticks_for_worker_count(&timeline, worker_count)
            .saturating_add(self.dma_scheduler_fallback_batch_ticks_for_worker_count(worker_count))
    }

    pub fn full_system_parallel_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_ticks_at_or_above(&timeline, minimum_worker_count).saturating_add(
            self.dma_scheduler_fallback_batch_ticks_at_or_above(minimum_worker_count),
        )
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_worker_ticks(&timeline)
            .saturating_add(self.dma_scheduler_fallback_batch_worker_ticks())
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
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
    if !timeline.is_empty() {
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
    if !timeline.is_empty() {
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
    if !timeline.is_empty() {
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
    if !timeline.is_empty() {
        return 0;
    }
    summaries
        .iter()
        .filter(|(count, _)| *count >= minimum_worker_count)
        .map(|(count, ticks)| ticks.saturating_mul(*count as Tick))
        .fold(0, Tick::saturating_add)
}
