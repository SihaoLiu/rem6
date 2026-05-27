use rem6_kernel::PartitionId;

use crate::{
    parallel_batch::{
        collect_parallel_batch_partition_sets_from_timeline,
        collect_parallel_batch_partition_streaks_from_timeline,
        parallel_batch_count_for_partition_set, parallel_batch_streak_count_for_partition_set,
    },
    WorkloadError, WorkloadParallelBatchTimelineRecord, WorkloadParallelExecutionSummary,
    WorkloadParallelRemoteFlowScope,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelBatchPartitionScope {
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

impl WorkloadParallelBatchPartitionScope {
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

impl From<WorkloadParallelRemoteFlowScope> for WorkloadParallelBatchPartitionScope {
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

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchPartitionSet {
    scope: WorkloadParallelBatchPartitionScope,
    partitions: Vec<PartitionId>,
    minimum_batch_count: usize,
}

impl WorkloadExpectedParallelBatchPartitionSet {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchPartitionScope>,
        partitions: impl IntoIterator<Item = PartitionId>,
        minimum_batch_count: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        let partitions = collect_partition_set(partitions);
        if partitions.len() < 2 {
            return Err(WorkloadError::InvalidExpectedParallelBatchPartitionSet {
                scope,
                partitions: partition_indexes(&partitions),
            });
        }
        if minimum_batch_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelBatchPartitionSetCount {
                scope,
                partitions: partition_indexes(&partitions),
            });
        }
        Ok(Self {
            scope,
            partitions,
            minimum_batch_count,
        })
    }

    pub const fn scope(&self) -> WorkloadParallelBatchPartitionScope {
        self.scope
    }

    pub fn partitions(&self) -> &[PartitionId] {
        &self.partitions
    }

    pub const fn minimum_batch_count(&self) -> usize {
        self.minimum_batch_count
    }

    pub(crate) fn sort_key(&self) -> (u8, Vec<PartitionId>) {
        (self.scope.sort_rank(), self.partitions.clone())
    }

    pub(crate) fn partition_indexes(&self) -> Vec<u32> {
        partition_indexes(&self.partitions)
    }

    pub(crate) fn actual_batch_count(&self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelBatchPartitionScope::Scheduler => summary
                .parallel_scheduler_batch_count_for_partition_set(self.partitions.iter().copied()),
            WorkloadParallelBatchPartitionScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::GpuDmaScheduler => summary
                .gpu_dma_scheduler_batch_count_for_partition_set(self.partitions.iter().copied()),
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler => summary
                .accelerator_dma_scheduler_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::DmaScheduler => {
                summary.dma_scheduler_batch_count_for_partition_set(self.partitions.iter().copied())
            }
            WorkloadParallelBatchPartitionScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::PlannedScheduler
            | WorkloadParallelBatchPartitionScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchPartitionScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchPartitionScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchPartitionScope::PlannedDmaScheduler
            | WorkloadParallelBatchPartitionScope::PlannedFullSystem => {
                planned_batch_count_for_partition_set(
                    self.scope,
                    summary,
                    self.partitions.iter().copied(),
                )
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelBatchPartitionStreak {
    scope: WorkloadParallelBatchPartitionScope,
    partitions: Vec<PartitionId>,
    minimum_consecutive_batch_count: usize,
}

impl WorkloadExpectedParallelBatchPartitionStreak {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchPartitionScope>,
        partitions: impl IntoIterator<Item = PartitionId>,
        minimum_consecutive_batch_count: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        let partitions = collect_partition_set(partitions);
        if partitions.len() < 2 {
            return Err(WorkloadError::InvalidExpectedParallelBatchPartitionStreak {
                scope,
                partitions: partition_indexes(&partitions),
            });
        }
        if minimum_consecutive_batch_count == 0 {
            return Err(
                WorkloadError::ZeroExpectedParallelBatchPartitionStreakCount {
                    scope,
                    partitions: partition_indexes(&partitions),
                },
            );
        }
        Ok(Self {
            scope,
            partitions,
            minimum_consecutive_batch_count,
        })
    }

    pub const fn scope(&self) -> WorkloadParallelBatchPartitionScope {
        self.scope
    }

    pub fn partitions(&self) -> &[PartitionId] {
        &self.partitions
    }

    pub const fn minimum_consecutive_batch_count(&self) -> usize {
        self.minimum_consecutive_batch_count
    }

    pub(crate) fn sort_key(&self) -> (u8, Vec<PartitionId>) {
        (self.scope.sort_rank(), self.partitions.clone())
    }

    pub(crate) fn partition_indexes(&self) -> Vec<u32> {
        partition_indexes(&self.partitions)
    }

    pub(crate) fn actual_consecutive_batch_count(
        &self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> usize {
        match self.scope {
            WorkloadParallelBatchPartitionScope::Scheduler => summary
                .parallel_scheduler_max_consecutive_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::DataCacheScheduler => summary
                .data_cache_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::GpuDmaScheduler => summary
                .gpu_dma_scheduler_max_consecutive_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler => summary
                .accelerator_dma_scheduler_max_consecutive_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::DmaScheduler => summary
                .dma_scheduler_max_consecutive_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::FullSystem => summary
                .full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
            WorkloadParallelBatchPartitionScope::PlannedScheduler
            | WorkloadParallelBatchPartitionScope::PlannedDataCacheScheduler
            | WorkloadParallelBatchPartitionScope::PlannedGpuDmaScheduler
            | WorkloadParallelBatchPartitionScope::PlannedAcceleratorDmaScheduler
            | WorkloadParallelBatchPartitionScope::PlannedDmaScheduler
            | WorkloadParallelBatchPartitionScope::PlannedFullSystem => {
                planned_batch_streak_count_for_partition_set(
                    self.scope,
                    summary,
                    self.partitions.iter().copied(),
                )
            }
        }
    }
}

fn planned_batch_count_for_partition_set(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let timeline = planned_batch_timeline(scope, summary);
    let sets = collect_parallel_batch_partition_sets_from_timeline(&timeline);
    let streaks = collect_parallel_batch_partition_streaks_from_timeline(&timeline);
    parallel_batch_count_for_partition_set(&sets, &streaks, partitions)
}

fn planned_batch_streak_count_for_partition_set(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let timeline = planned_batch_timeline(scope, summary);
    let streaks = collect_parallel_batch_partition_streaks_from_timeline(&timeline);
    parallel_batch_streak_count_for_partition_set(&streaks, partitions)
}

fn planned_batch_timeline(
    scope: WorkloadParallelBatchPartitionScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    match scope {
        WorkloadParallelBatchPartitionScope::PlannedScheduler => {
            summary.parallel_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchPartitionScope::PlannedDataCacheScheduler => summary
            .data_cache_parallel_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchPartitionScope::PlannedGpuDmaScheduler => {
            summary.gpu_dma_scheduler_planned_batch_timeline().to_vec()
        }
        WorkloadParallelBatchPartitionScope::PlannedAcceleratorDmaScheduler => summary
            .accelerator_dma_scheduler_planned_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchPartitionScope::PlannedDmaScheduler => {
            summary.dma_scheduler_planned_batch_timeline()
        }
        WorkloadParallelBatchPartitionScope::PlannedFullSystem => {
            summary.full_system_parallel_scheduler_planned_batch_timeline()
        }
        _ => Vec::new(),
    }
}

fn collect_partition_set(partitions: impl IntoIterator<Item = PartitionId>) -> Vec<PartitionId> {
    let mut partitions = partitions.into_iter().collect::<Vec<_>>();
    partitions.sort_unstable();
    partitions.dedup();
    partitions
}

fn partition_indexes(partitions: &[PartitionId]) -> Vec<u32> {
    partitions
        .iter()
        .map(|partition| partition.index())
        .collect()
}
