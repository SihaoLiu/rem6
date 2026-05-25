use crate::parallel_batch::{
    collect_parallel_batch_partition_sets_from_timeline,
    collect_parallel_batch_partition_streaks_from_timeline, collect_parallel_batch_timeline,
    collect_parallel_batch_worker_counts_from_timeline, WorkloadParallelBatchScope,
    WorkloadParallelBatchTimelineRecord,
};

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
