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

    pub fn parallel_scheduler_batch_timeline(&self) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.parallel_scheduler_batch_timeline
    }

    pub fn data_cache_parallel_scheduler_batch_timeline(
        &self,
    ) -> &[WorkloadParallelBatchTimelineRecord] {
        &self.data_cache_parallel_scheduler_batch_timeline
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
                ),
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
        collect_parallel_batch_worker_count_tick_summaries(&timeline)
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
    }

    pub fn full_system_parallel_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_ticks_at_or_above(&timeline, minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_worker_ticks(&timeline)
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_worker_ticks_at_or_above(&timeline, minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        let timeline = self.full_system_parallel_scheduler_batch_timeline();
        parallel_batch_longest_tick_streak_at_or_above(&timeline, minimum_worker_count)
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
