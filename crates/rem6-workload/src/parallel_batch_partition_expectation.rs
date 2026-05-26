use rem6_kernel::PartitionId;

use crate::{WorkloadError, WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelBatchPartitionScope {
    Scheduler,
    DataCacheScheduler,
    GpuDmaScheduler,
    AcceleratorDmaScheduler,
    FullSystem,
}

impl WorkloadParallelBatchPartitionScope {
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

impl From<WorkloadParallelRemoteFlowScope> for WorkloadParallelBatchPartitionScope {
    fn from(scope: WorkloadParallelRemoteFlowScope) -> Self {
        match scope {
            WorkloadParallelRemoteFlowScope::Scheduler => Self::Scheduler,
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => Self::DataCacheScheduler,
            WorkloadParallelRemoteFlowScope::GpuDmaScheduler => Self::GpuDmaScheduler,
            WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
                Self::AcceleratorDmaScheduler
            }
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
            WorkloadParallelBatchPartitionScope::FullSystem => summary
                .full_system_parallel_scheduler_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
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
            WorkloadParallelBatchPartitionScope::FullSystem => summary
                .full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
                    self.partitions.iter().copied(),
                ),
        }
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
