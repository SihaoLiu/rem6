use std::fmt;

use rem6_kernel::{ParallelBatchUtilizationRatio, Tick};

use crate::{
    parallel_batch::{
        parallel_batch_longest_tick_streak_at_or_above, parallel_batch_ticks_at_or_above,
        parallel_batch_ticks_for_worker_count, parallel_batch_worker_ticks_at_or_above,
    },
    WorkloadError, WorkloadManifest, WorkloadManifestBuilder, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope, WorkloadReplayPlan,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelBatchWorkerScope {
    Scheduler,
    DataCacheScheduler,
    GpuDmaScheduler,
    AcceleratorDmaScheduler,
    DmaScheduler,
    FullSystem,
    PlannedScheduler,
    PlannedDataCacheScheduler,
    PlannedGpuDmaScheduler,
    PlannedAcceleratorDmaScheduler,
    PlannedDmaScheduler,
    PlannedFullSystem,
}

impl WorkloadParallelBatchWorkerScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::DataCacheScheduler => "data-cache-scheduler",
            Self::GpuDmaScheduler => "gpu-dma-scheduler",
            Self::AcceleratorDmaScheduler => "accelerator-dma-scheduler",
            Self::DmaScheduler => "dma-scheduler",
            Self::FullSystem => "full-system",
            Self::PlannedScheduler => "planned-scheduler",
            Self::PlannedDataCacheScheduler => "planned-data-cache-scheduler",
            Self::PlannedGpuDmaScheduler => "planned-gpu-dma-scheduler",
            Self::PlannedAcceleratorDmaScheduler => "planned-accelerator-dma-scheduler",
            Self::PlannedDmaScheduler => "planned-dma-scheduler",
            Self::PlannedFullSystem => "planned-full-system",
        }
    }

    pub(crate) const fn sort_rank(self) -> u8 {
        match self {
            Self::Scheduler => 0,
            Self::DataCacheScheduler => 1,
            Self::GpuDmaScheduler => 2,
            Self::AcceleratorDmaScheduler => 3,
            Self::DmaScheduler => 4,
            Self::FullSystem => 5,
            Self::PlannedScheduler => 6,
            Self::PlannedDataCacheScheduler => 7,
            Self::PlannedGpuDmaScheduler => 8,
            Self::PlannedAcceleratorDmaScheduler => 9,
            Self::PlannedDmaScheduler => 10,
            Self::PlannedFullSystem => 11,
        }
    }
}

impl From<WorkloadParallelRemoteFlowScope> for WorkloadParallelBatchWorkerScope {
    fn from(scope: WorkloadParallelRemoteFlowScope) -> Self {
        match scope {
            WorkloadParallelRemoteFlowScope::Scheduler => Self::Scheduler,
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => Self::DataCacheScheduler,
            WorkloadParallelRemoteFlowScope::GpuDmaScheduler => Self::GpuDmaScheduler,
            WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
                Self::AcceleratorDmaScheduler
            }
            WorkloadParallelRemoteFlowScope::DmaScheduler => Self::DmaScheduler,
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
            WorkloadParallelBatchWorkerScope::DmaScheduler => {
                summary.dma_scheduler_batch_count_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_count_for_worker_count(self.worker_count),
            WorkloadParallelBatchWorkerScope::PlannedScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
                planned_batch_count_for_worker_count(self.scope, summary, self.worker_count)
            }
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
            WorkloadParallelBatchWorkerScope::DmaScheduler => {
                summary.dma_scheduler_batch_ticks_for_worker_count(self.worker_count)
            }
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_ticks_for_worker_count(self.worker_count),
            WorkloadParallelBatchWorkerScope::PlannedScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
                let timeline = planned_batch_timeline(self.scope, summary);
                parallel_batch_ticks_for_worker_count(&timeline, self.worker_count)
            }
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
            WorkloadParallelBatchWorkerScope::DmaScheduler => {
                summary.dma_scheduler_batch_ticks_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_ticks_at_or_above(self.minimum_worker_count),
            WorkloadParallelBatchWorkerScope::PlannedScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
                let timeline = planned_batch_timeline(self.scope, summary);
                parallel_batch_ticks_at_or_above(&timeline, self.minimum_worker_count)
            }
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
            WorkloadParallelBatchWorkerScope::DmaScheduler => summary
                .dma_scheduler_longest_batch_tick_streak_at_or_above(self.minimum_worker_count),
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(
                    self.minimum_worker_count,
                ),
            WorkloadParallelBatchWorkerScope::PlannedScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
                let timeline = planned_batch_timeline(self.scope, summary);
                parallel_batch_longest_tick_streak_at_or_above(&timeline, self.minimum_worker_count)
            }
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
            WorkloadParallelBatchWorkerScope::DmaScheduler => {
                summary.dma_scheduler_batch_worker_ticks_at_or_above(self.minimum_worker_count)
            }
            WorkloadParallelBatchWorkerScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_worker_ticks_at_or_above(
                    self.minimum_worker_count,
                ),
            WorkloadParallelBatchWorkerScope::PlannedScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
                let timeline = planned_batch_timeline(self.scope, summary);
                parallel_batch_worker_ticks_at_or_above(&timeline, self.minimum_worker_count)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedPlannedParallelBatchUtilization {
    scope: WorkloadParallelBatchWorkerScope,
    minimum_numerator: Tick,
    minimum_denominator: Tick,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedPlannedParallelBatchIdleWorkerTicks {
    scope: WorkloadParallelBatchWorkerScope,
    maximum_idle_worker_ticks: Tick,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadPlannedParallelBatchUtilizationExpectationError {
    InvalidScope {
        scope: WorkloadParallelBatchWorkerScope,
    },
    ZeroDenominator {
        scope: WorkloadParallelBatchWorkerScope,
    },
    Duplicate {
        scope: WorkloadParallelBatchWorkerScope,
    },
    MissingSummary {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_numerator: Tick,
        minimum_denominator: Tick,
    },
    BelowMinimum {
        scope: WorkloadParallelBatchWorkerScope,
        minimum_numerator: Tick,
        minimum_denominator: Tick,
        actual_numerator: Tick,
        actual_denominator: Tick,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadPlannedParallelBatchIdleExpectationError {
    InvalidScope {
        scope: WorkloadParallelBatchWorkerScope,
    },
    Duplicate {
        scope: WorkloadParallelBatchWorkerScope,
    },
    MissingSummary {
        scope: WorkloadParallelBatchWorkerScope,
        maximum_idle_worker_ticks: Tick,
    },
    AboveMaximum {
        scope: WorkloadParallelBatchWorkerScope,
        maximum_idle_worker_ticks: Tick,
        actual_idle_worker_ticks: Tick,
    },
}

impl fmt::Display for WorkloadPlannedParallelBatchUtilizationExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScope { scope } => write!(
                formatter,
                "expected planned parallel batch utilization scope {} must expose planned worker capacity",
                scope.as_str()
            ),
            Self::ZeroDenominator { scope } => write!(
                formatter,
                "expected {} planned parallel batch utilization must use a nonzero denominator",
                scope.as_str()
            ),
            Self::Duplicate { scope } => write!(
                formatter,
                "expected {} planned parallel batch utilization is already declared",
                scope.as_str()
            ),
            Self::MissingSummary {
                scope,
                minimum_numerator,
                minimum_denominator,
            } => write!(
                formatter,
                "missing planned parallel batch utilization summary for {} with minimum {minimum_numerator}/{minimum_denominator}",
                scope.as_str()
            ),
            Self::BelowMinimum {
                scope,
                minimum_numerator,
                minimum_denominator,
                actual_numerator,
                actual_denominator,
            } => write!(
                formatter,
                "expected {} planned parallel batch utilization to reach at least {minimum_numerator}/{minimum_denominator}, got {actual_numerator}/{actual_denominator}",
                scope.as_str()
            ),
        }
    }
}

impl fmt::Display for WorkloadPlannedParallelBatchIdleExpectationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidScope { scope } => write!(
                formatter,
                "expected planned parallel batch idle scope {} must expose planned worker capacity",
                scope.as_str()
            ),
            Self::Duplicate { scope } => write!(
                formatter,
                "expected {} planned parallel batch idle budget is already declared",
                scope.as_str()
            ),
            Self::MissingSummary {
                scope,
                maximum_idle_worker_ticks,
            } => write!(
                formatter,
                "missing planned parallel batch idle summary for {} with maximum {maximum_idle_worker_ticks} idle worker-ticks",
                scope.as_str()
            ),
            Self::AboveMaximum {
                scope,
                maximum_idle_worker_ticks,
                actual_idle_worker_ticks,
            } => write!(
                formatter,
                "expected {} planned parallel batch idle worker-ticks to stay at or below {maximum_idle_worker_ticks}, got {actual_idle_worker_ticks}",
                scope.as_str()
            ),
        }
    }
}

impl WorkloadExpectedPlannedParallelBatchUtilization {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        minimum_numerator: Tick,
        minimum_denominator: Tick,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if !is_planned_capacity_scope(scope) {
            return Err(WorkloadError::PlannedParallelBatchUtilizationExpectation(
                WorkloadPlannedParallelBatchUtilizationExpectationError::InvalidScope { scope },
            ));
        }
        if minimum_denominator == 0 {
            return Err(WorkloadError::PlannedParallelBatchUtilizationExpectation(
                WorkloadPlannedParallelBatchUtilizationExpectationError::ZeroDenominator { scope },
            ));
        }
        Ok(Self {
            scope,
            minimum_numerator,
            minimum_denominator,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn minimum_numerator(self) -> Tick {
        self.minimum_numerator
    }

    pub const fn minimum_denominator(self) -> Tick {
        self.minimum_denominator
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_utilization(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<ParallelBatchUtilizationRatio> {
        match self.scope {
            WorkloadParallelBatchWorkerScope::PlannedScheduler => {
                summary.parallel_scheduler_planned_batch_utilization_ratio()
            }
            WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler => {
                summary.data_cache_parallel_scheduler_planned_batch_utilization_ratio()
            }
            WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler => {
                summary.gpu_dma_scheduler_planned_batch_utilization_ratio()
            }
            WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler => {
                summary.accelerator_dma_scheduler_planned_batch_utilization_ratio()
            }
            WorkloadParallelBatchWorkerScope::PlannedDmaScheduler => {
                summary.dma_scheduler_planned_batch_utilization_ratio()
            }
            WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
                summary.full_system_parallel_scheduler_planned_batch_utilization_ratio()
            }
            _ => None,
        }
    }
}

impl WorkloadExpectedPlannedParallelBatchIdleWorkerTicks {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchWorkerScope>,
        maximum_idle_worker_ticks: Tick,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        if !is_planned_capacity_scope(scope) {
            return Err(WorkloadError::PlannedParallelBatchIdleExpectation(
                WorkloadPlannedParallelBatchIdleExpectationError::InvalidScope { scope },
            ));
        }
        Ok(Self {
            scope,
            maximum_idle_worker_ticks,
        })
    }

    pub const fn scope(self) -> WorkloadParallelBatchWorkerScope {
        self.scope
    }

    pub const fn maximum_idle_worker_ticks(self) -> Tick {
        self.maximum_idle_worker_ticks
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_idle_worker_ticks(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<Tick> {
        let (capacity, idle) = match self.scope {
            WorkloadParallelBatchWorkerScope::PlannedScheduler => (
                summary.parallel_scheduler_planned_batch_worker_capacity_ticks(),
                summary.parallel_scheduler_planned_batch_idle_worker_ticks(),
            ),
            WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler => (
                summary.data_cache_parallel_scheduler_planned_batch_worker_capacity_ticks(),
                summary.data_cache_parallel_scheduler_planned_batch_idle_worker_ticks(),
            ),
            WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler => (
                summary.gpu_dma_scheduler_planned_batch_worker_capacity_ticks(),
                summary.gpu_dma_scheduler_planned_batch_idle_worker_ticks(),
            ),
            WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler => (
                summary.accelerator_dma_scheduler_planned_batch_worker_capacity_ticks(),
                summary.accelerator_dma_scheduler_planned_batch_idle_worker_ticks(),
            ),
            WorkloadParallelBatchWorkerScope::PlannedDmaScheduler => (
                summary.dma_scheduler_planned_batch_worker_capacity_ticks(),
                summary.dma_scheduler_planned_batch_idle_worker_ticks(),
            ),
            WorkloadParallelBatchWorkerScope::PlannedFullSystem => (
                summary.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks(),
                summary.full_system_parallel_scheduler_planned_batch_idle_worker_ticks(),
            ),
            _ => return None,
        };
        (capacity != 0).then_some(idle)
    }
}

const fn is_planned_capacity_scope(scope: WorkloadParallelBatchWorkerScope) -> bool {
    matches!(
        scope,
        WorkloadParallelBatchWorkerScope::PlannedScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedDmaScheduler
            | WorkloadParallelBatchWorkerScope::PlannedFullSystem
    )
}

fn planned_batch_count_for_worker_count(
    scope: WorkloadParallelBatchWorkerScope,
    summary: &WorkloadParallelExecutionSummary,
    worker_count: usize,
) -> usize {
    planned_batch_timeline(scope, summary)
        .into_iter()
        .filter(WorkloadParallelBatchTimelineRecord::is_parallel_evidence)
        .filter(|record| record.worker_count() == worker_count)
        .count()
}

fn planned_batch_timeline(
    scope: WorkloadParallelBatchWorkerScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    match scope {
        WorkloadParallelBatchWorkerScope::PlannedScheduler => {
            summary.parallel_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchWorkerScope::PlannedDataCacheScheduler => summary
            .data_cache_parallel_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchWorkerScope::PlannedGpuDmaScheduler => {
            summary.gpu_dma_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchWorkerScope::PlannedAcceleratorDmaScheduler => summary
            .accelerator_dma_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchWorkerScope::PlannedDmaScheduler => {
            summary.dma_scheduler_planned_batch_timeline()
        }
        WorkloadParallelBatchWorkerScope::PlannedFullSystem => {
            summary.full_system_parallel_scheduler_planned_batch_timeline()
        }
        _ => Vec::new(),
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

    pub fn expected_planned_parallel_batch_utilization(
        &self,
    ) -> &[WorkloadExpectedPlannedParallelBatchUtilization] {
        &self.expected_planned_parallel_batch_utilization
    }

    pub fn expected_planned_parallel_batch_idle_worker_ticks(
        &self,
    ) -> &[WorkloadExpectedPlannedParallelBatchIdleWorkerTicks] {
        &self.expected_planned_parallel_batch_idle_worker_ticks
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

    pub fn add_expected_planned_parallel_batch_utilization(
        mut self,
        expected: WorkloadExpectedPlannedParallelBatchUtilization,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_planned_parallel_batch_utilization
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::PlannedParallelBatchUtilizationExpectation(
                WorkloadPlannedParallelBatchUtilizationExpectationError::Duplicate {
                    scope: expected.scope(),
                },
            ));
        }
        self.expected_planned_parallel_batch_utilization
            .push(expected);
        self.expected_planned_parallel_batch_utilization
            .sort_by_key(|expected| expected.sort_key());
        Ok(self)
    }

    pub fn add_expected_planned_parallel_batch_idle_worker_ticks(
        mut self,
        expected: WorkloadExpectedPlannedParallelBatchIdleWorkerTicks,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_planned_parallel_batch_idle_worker_ticks
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::PlannedParallelBatchIdleExpectation(
                WorkloadPlannedParallelBatchIdleExpectationError::Duplicate {
                    scope: expected.scope(),
                },
            ));
        }
        self.expected_planned_parallel_batch_idle_worker_ticks
            .push(expected);
        self.expected_planned_parallel_batch_idle_worker_ticks
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

    pub fn add_expected_planned_parallel_batch_utilization(
        mut self,
        expected: WorkloadExpectedPlannedParallelBatchUtilization,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_planned_parallel_batch_utilization
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::PlannedParallelBatchUtilizationExpectation(
                WorkloadPlannedParallelBatchUtilizationExpectationError::Duplicate {
                    scope: expected.scope(),
                },
            ));
        }
        self.expected_planned_parallel_batch_utilization
            .push(expected);
        self.expected_planned_parallel_batch_utilization
            .sort_by_key(|expected| expected.sort_key());
        Ok(self)
    }

    pub fn expected_planned_parallel_batch_utilization(
        &self,
    ) -> &[WorkloadExpectedPlannedParallelBatchUtilization] {
        &self.expected_planned_parallel_batch_utilization
    }

    pub fn add_expected_planned_parallel_batch_idle_worker_ticks(
        mut self,
        expected: WorkloadExpectedPlannedParallelBatchIdleWorkerTicks,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_planned_parallel_batch_idle_worker_ticks
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::PlannedParallelBatchIdleExpectation(
                WorkloadPlannedParallelBatchIdleExpectationError::Duplicate {
                    scope: expected.scope(),
                },
            ));
        }
        self.expected_planned_parallel_batch_idle_worker_ticks
            .push(expected);
        self.expected_planned_parallel_batch_idle_worker_ticks
            .sort_by_key(|expected| expected.sort_key());
        Ok(self)
    }

    pub fn expected_planned_parallel_batch_idle_worker_ticks(
        &self,
    ) -> &[WorkloadExpectedPlannedParallelBatchIdleWorkerTicks] {
        &self.expected_planned_parallel_batch_idle_worker_ticks
    }
}
