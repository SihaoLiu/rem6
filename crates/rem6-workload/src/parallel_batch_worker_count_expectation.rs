use rem6_kernel::Tick;

use crate::{
    WorkloadError, WorkloadManifest, WorkloadManifestBuilder, WorkloadParallelExecutionSummary,
    WorkloadParallelRemoteFlowScope, WorkloadReplayPlan,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelBatchWorkerScope {
    Scheduler,
    DataCacheScheduler,
    GpuDmaScheduler,
    AcceleratorDmaScheduler,
    FullSystem,
}

impl WorkloadParallelBatchWorkerScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::DataCacheScheduler => "data-cache-scheduler",
            Self::GpuDmaScheduler => "gpu-dma-scheduler",
            Self::AcceleratorDmaScheduler => "accelerator-dma-scheduler",
            Self::FullSystem => "full-system",
        }
    }

    pub(crate) const fn sort_rank(self) -> u8 {
        match self {
            Self::Scheduler => 0,
            Self::DataCacheScheduler => 1,
            Self::GpuDmaScheduler => 2,
            Self::AcceleratorDmaScheduler => 3,
            Self::FullSystem => 4,
        }
    }
}

impl From<WorkloadParallelRemoteFlowScope> for WorkloadParallelBatchWorkerScope {
    fn from(scope: WorkloadParallelRemoteFlowScope) -> Self {
        match scope {
            WorkloadParallelRemoteFlowScope::Scheduler => Self::Scheduler,
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => Self::DataCacheScheduler,
            WorkloadParallelRemoteFlowScope::FullSystem => Self::FullSystem,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchWorkerBucket {
    scope: WorkloadParallelBatchWorkerScope,
    worker_count: usize,
    minimum_batch_count: usize,
}

impl WorkloadExpectedParallelBatchWorkerBucket {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        worker_count: usize,
        minimum_batch_count: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if worker_count < 2 {
            return Err(WorkloadError::InvalidExpectedParallelBatchWorkerBucket {
                scope,
                worker_count,
            });
        }
        if minimum_batch_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchWorkerBucket {
                scope,
                worker_count,
            });
        }
        Ok(Self {
            scope,
            worker_count,
            minimum_batch_count,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn worker_count(self) -> usize {
        self.worker_count
    }

    pub const fn minimum_batch_count(self) -> usize {
        self.minimum_batch_count
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        (self.scope.sort_rank(), self.worker_count)
    }

    pub(crate) fn actual_batch_count(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelBatchWorkerScope::Scheduler => {
                summary.parallel_scheduler_batch_count_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_batch_count_for_worker_count(self.worker_count),
            WorkloadParallelBatchWorkerScope::GpuDmaScheduler => {
                summary.gpu_dma_scheduler_batch_count_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => {
                summary.accelerator_dma_scheduler_batch_count_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_count_for_worker_count(self.worker_count),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchWorkerTickBucket {
    scope: WorkloadParallelBatchWorkerScope,
    worker_count: usize,
    minimum_ticks: Tick,
}

impl WorkloadExpectedParallelBatchWorkerTickBucket {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        worker_count: usize,
        minimum_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if worker_count < 2 {
            return Err(
                WorkloadError::InvalidExpectedParallelBatchWorkerTickBucket {
                    scope,
                    worker_count,
                },
            );
        }
        if minimum_ticks == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchWorkerTickBucket {
                scope,
                worker_count,
            });
        }
        Ok(Self {
            scope,
            worker_count,
            minimum_ticks,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn worker_count(self) -> usize {
        self.worker_count
    }

    pub const fn minimum_ticks(self) -> Tick {
        self.minimum_ticks
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        (self.scope.sort_rank(), self.worker_count)
    }

    pub(crate) fn actual_ticks(self, summary: &WorkloadParallelExecutionSummary) -> Tick {
        match self.scope {
            WorkloadParallelBatchWorkerScope::Scheduler => {
                summary.parallel_scheduler_batch_ticks_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_batch_ticks_for_worker_count(self.worker_count),
            WorkloadParallelBatchWorkerScope::GpuDmaScheduler => {
                summary.gpu_dma_scheduler_batch_ticks_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => {
                summary.accelerator_dma_scheduler_batch_ticks_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_ticks_for_worker_count(self.worker_count),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchWorkerTickActivity {
    scope: WorkloadParallelBatchWorkerScope,
    minimum_worker_count: usize,
    minimum_ticks: Tick,
}

impl WorkloadExpectedParallelBatchWorkerTickActivity {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        minimum_worker_count: usize,
        minimum_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_worker_count < 2 {
            return Err(
                WorkloadError::InvalidExpectedParallelBatchWorkerTickActivity {
                    scope,
                    minimum_worker_count,
                },
            );
        }
        if minimum_ticks == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchWorkerTickActivity {
                scope,
                minimum_worker_count,
            });
        }
        Ok(Self {
            scope,
            minimum_worker_count,
            minimum_ticks,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn minimum_worker_count(self) -> usize {
        self.minimum_worker_count
    }

    pub const fn minimum_ticks(self) -> Tick {
        self.minimum_ticks
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        (self.scope.sort_rank(), self.minimum_worker_count)
    }

    pub(crate) fn actual_ticks(self, summary: &WorkloadParallelExecutionSummary) -> Tick {
        match self.scope {
            WorkloadParallelBatchWorkerScope::Scheduler => {
                summary.parallel_scheduler_batch_ticks_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelBatchWorkerScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_batch_ticks_at_or_above(self.minimum_worker_count),
            WorkloadParallelBatchWorkerScope::GpuDmaScheduler => {
                summary.gpu_dma_scheduler_batch_ticks_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => {
                summary.accelerator_dma_scheduler_batch_ticks_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_ticks_at_or_above(self.minimum_worker_count),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchWorkerTickStreak {
    scope: WorkloadParallelBatchWorkerScope,
    minimum_worker_count: usize,
    minimum_consecutive_ticks: Tick,
}

impl WorkloadExpectedParallelBatchWorkerTickStreak {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        minimum_worker_count: usize,
        minimum_consecutive_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_worker_count < 2 {
            return Err(
                WorkloadError::InvalidExpectedParallelBatchWorkerTickStreak {
                    scope,
                    minimum_worker_count,
                },
            );
        }
        if minimum_consecutive_ticks == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchWorkerTickStreak {
                scope,
                minimum_worker_count,
            });
        }
        Ok(Self {
            scope,
            minimum_worker_count,
            minimum_consecutive_ticks,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn minimum_worker_count(self) -> usize {
        self.minimum_worker_count
    }

    pub const fn minimum_consecutive_ticks(self) -> Tick {
        self.minimum_consecutive_ticks
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        (self.scope.sort_rank(), self.minimum_worker_count)
    }

    pub(crate) fn actual_consecutive_ticks(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Tick {
        match self.scope {
            WorkloadParallelBatchWorkerScope::Scheduler => summary
                .parallel_scheduler_longest_batch_tick_streak_at_or_above(
                    self.minimum_worker_count,
                ),
            WorkloadParallelBatchWorkerScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_longest_batch_tick_streak_at_or_above(
                    self.minimum_worker_count,
                ),
            WorkloadParallelBatchWorkerScope::GpuDmaScheduler => summary
                .gpu_dma_scheduler_longest_batch_tick_streak_at_or_above(self.minimum_worker_count),
            WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => summary
                .accelerator_dma_scheduler_longest_batch_tick_streak_at_or_above(
                    self.minimum_worker_count,
                ),
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(
                    self.minimum_worker_count,
                ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchWorkerTicks {
    scope: WorkloadParallelBatchWorkerScope,
    minimum_worker_count: usize,
    minimum_worker_ticks: Tick,
}

impl WorkloadExpectedParallelBatchWorkerTicks {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        minimum_worker_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_worker_ticks == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchWorkerTicks {
                scope,
                minimum_worker_count: 1,
            });
        }
        Ok(Self {
            scope,
            minimum_worker_count: 1,
            minimum_worker_ticks,
        })
    }

    pub fn new_at_or_above(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        minimum_worker_count: usize,
        minimum_worker_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if minimum_worker_count < 2 {
            return Err(WorkloadError::InvalidExpectedParallelBatchWorkerTicks {
                scope,
                minimum_worker_count,
            });
        }
        if minimum_worker_ticks == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchWorkerTicks {
                scope,
                minimum_worker_count,
            });
        }
        Ok(Self {
            scope,
            minimum_worker_count,
            minimum_worker_ticks,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn minimum_worker_count(self) -> usize {
        self.minimum_worker_count
    }

    pub const fn minimum_worker_ticks(self) -> Tick {
        self.minimum_worker_ticks
    }

    pub(crate) const fn sort_key(self) -> (u8, usize) {
        (self.scope.sort_rank(), self.minimum_worker_count)
    }

    pub(crate) fn actual_worker_ticks(self, summary: &WorkloadParallelExecutionSummary) -> Tick {
        match self.scope {
            WorkloadParallelBatchWorkerScope::Scheduler => {
                summary.parallel_scheduler_batch_worker_ticks_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelBatchWorkerScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_batch_worker_ticks_at_or_above(
                    self.minimum_worker_count,
                ),
            WorkloadParallelBatchWorkerScope::GpuDmaScheduler => {
                summary.gpu_dma_scheduler_batch_worker_ticks_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelBatchWorkerScope::AcceleratorDmaScheduler => summary
                .accelerator_dma_scheduler_batch_worker_ticks_at_or_above(
                    self.minimum_worker_count,
                ),
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_worker_ticks_at_or_above(
                    self.minimum_worker_count,
                ),
        }
    }
}

impl WorkloadManifest {
    pub fn expected_parallel_batch_worker_buckets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerBucket] {
        &self.expected_parallel_batch_worker_buckets
    }

    pub fn expected_parallel_batch_worker_tick_buckets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTickBucket] {
        &self.expected_parallel_batch_worker_tick_buckets
    }

    pub fn expected_parallel_batch_worker_tick_activity(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTickActivity] {
        &self.expected_parallel_batch_worker_tick_activity
    }

    pub fn expected_parallel_batch_worker_tick_streaks(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTickStreak] {
        &self.expected_parallel_batch_worker_tick_streaks
    }

    pub fn expected_parallel_batch_worker_ticks(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTicks] {
        &self.expected_parallel_batch_worker_ticks
    }
}

impl WorkloadManifestBuilder {
    pub fn add_expected_parallel_batch_worker_bucket(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerBucket,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_buckets
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchWorkerBucket {
                scope: expected.scope(),
                worker_count: expected.worker_count(),
            });
        }
        self.expected_parallel_batch_worker_buckets.push(expected);
        self.expected_parallel_batch_worker_buckets
            .sort_by_key(|bucket| bucket.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_batch_worker_tick_bucket(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTickBucket,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_tick_buckets
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchWorkerTickBucket {
                    scope: expected.scope(),
                    worker_count: expected.worker_count(),
                },
            );
        }
        self.expected_parallel_batch_worker_tick_buckets
            .push(expected);
        self.expected_parallel_batch_worker_tick_buckets
            .sort_by_key(|bucket| bucket.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_batch_worker_tick_activity(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTickActivity,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_tick_activity
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchWorkerTickActivity {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                },
            );
        }
        self.expected_parallel_batch_worker_tick_activity
            .push(expected);
        self.expected_parallel_batch_worker_tick_activity
            .sort_by_key(|activity| activity.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_batch_worker_tick_streak(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTickStreak,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_tick_streaks
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchWorkerTickStreak {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                },
            );
        }
        self.expected_parallel_batch_worker_tick_streaks
            .push(expected);
        self.expected_parallel_batch_worker_tick_streaks
            .sort_by_key(|streak| streak.sort_key());
        Ok(self)
    }

    pub fn add_expected_parallel_batch_worker_ticks(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTicks,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_ticks
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchWorkerTicks {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
            });
        }
        self.expected_parallel_batch_worker_ticks.push(expected);
        self.expected_parallel_batch_worker_ticks
            .sort_by_key(|expected| expected.sort_key());
        Ok(self)
    }
}

impl WorkloadReplayPlan {
    pub fn add_expected_parallel_batch_worker_bucket(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerBucket,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_buckets
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchWorkerBucket {
                scope: expected.scope(),
                worker_count: expected.worker_count(),
            });
        }
        self.expected_parallel_batch_worker_buckets.push(expected);
        self.expected_parallel_batch_worker_buckets
            .sort_by_key(|bucket| bucket.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_worker_buckets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerBucket] {
        &self.expected_parallel_batch_worker_buckets
    }

    pub fn add_expected_parallel_batch_worker_tick_bucket(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTickBucket,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_tick_buckets
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchWorkerTickBucket {
                    scope: expected.scope(),
                    worker_count: expected.worker_count(),
                },
            );
        }
        self.expected_parallel_batch_worker_tick_buckets
            .push(expected);
        self.expected_parallel_batch_worker_tick_buckets
            .sort_by_key(|bucket| bucket.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_worker_tick_buckets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTickBucket] {
        &self.expected_parallel_batch_worker_tick_buckets
    }

    pub fn add_expected_parallel_batch_worker_tick_activity(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTickActivity,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_tick_activity
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchWorkerTickActivity {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                },
            );
        }
        self.expected_parallel_batch_worker_tick_activity
            .push(expected);
        self.expected_parallel_batch_worker_tick_activity
            .sort_by_key(|activity| activity.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_worker_tick_activity(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTickActivity] {
        &self.expected_parallel_batch_worker_tick_activity
    }

    pub fn add_expected_parallel_batch_worker_tick_streak(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTickStreak,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_tick_streaks
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchWorkerTickStreak {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                },
            );
        }
        self.expected_parallel_batch_worker_tick_streaks
            .push(expected);
        self.expected_parallel_batch_worker_tick_streaks
            .sort_by_key(|streak| streak.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_worker_tick_streaks(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTickStreak] {
        &self.expected_parallel_batch_worker_tick_streaks
    }

    pub fn add_expected_parallel_batch_worker_ticks(
        mut self,
        expected: WorkloadExpectedParallelBatchWorkerTicks,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_worker_ticks
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchWorkerTicks {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
            });
        }
        self.expected_parallel_batch_worker_ticks.push(expected);
        self.expected_parallel_batch_worker_ticks
            .sort_by_key(|expected| expected.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_worker_ticks(
        &self,
    ) -> &[WorkloadExpectedParallelBatchWorkerTicks] {
        &self.expected_parallel_batch_worker_ticks
    }
}
