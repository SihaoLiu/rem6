use rem6_kernel::{PartitionId, Tick};

use crate::{
    WorkloadError, WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelBatchTimelineScope {
    Scheduler,
    DataCacheScheduler,
    GpuDmaScheduler,
    AcceleratorDmaScheduler,
    FullSystem,
}

impl WorkloadParallelBatchTimelineScope {
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

impl From<WorkloadParallelRemoteFlowScope> for WorkloadParallelBatchTimelineScope {
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
pub struct WorkloadExpectedParallelBatchTimelineRecord {
    scope: WorkloadParallelBatchTimelineScope,
    batch_scope: WorkloadParallelBatchScope,
    start_tick: Tick,
    horizon: Tick,
    partitions: Vec<PartitionId>,
    worker_count: usize,
}

impl WorkloadExpectedParallelBatchTimelineRecord {
    pub fn new(
        scope: impl Into<WorkloadParallelBatchTimelineScope>,
        batch_scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: impl IntoIterator<Item = PartitionId>,
        worker_count: usize,
    ) -> Result<Self, WorkloadError> {
        let scope = scope.into();
        let partitions = collect_partition_set(partitions);
        if partitions.len() < 2 || worker_count < 2 || horizon <= start_tick {
            return Err(WorkloadError::InvalidExpectedParallelBatchTimelineRecord {
                scope,
                batch_scope,
                start_tick,
                horizon,
                partitions: partition_indexes(&partitions),
                worker_count,
            });
        }
        Ok(Self {
            scope,
            batch_scope,
            start_tick,
            horizon,
            partitions,
            worker_count,
        })
    }

    pub const fn scope(&self) -> WorkloadParallelBatchTimelineScope {
        self.scope
    }

    pub const fn batch_scope(&self) -> WorkloadParallelBatchScope {
        self.batch_scope
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn horizon(&self) -> Tick {
        self.horizon
    }

    pub fn partitions(&self) -> &[PartitionId] {
        &self.partitions
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub(crate) fn sort_key(&self) -> (u8, u8, Tick, Tick, Vec<PartitionId>, usize) {
        (
            self.scope.sort_rank(),
            self.batch_scope.sort_rank(),
            self.start_tick,
            self.horizon,
            self.partitions.clone(),
            self.worker_count,
        )
    }

    pub(crate) fn partition_indexes(&self) -> Vec<u32> {
        partition_indexes(&self.partitions)
    }

    pub(crate) fn actual_record(
        &self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<WorkloadParallelBatchTimelineRecord> {
        actual_parallel_batch_timeline_records(self.scope, summary)
            .into_iter()
            .find(|record| self.matches_record(record))
    }

    pub(crate) fn matches_record(&self, record: &WorkloadParallelBatchTimelineRecord) -> bool {
        record.scope() == self.batch_scope
            && record.start_tick() == self.start_tick
            && record.horizon() == self.horizon
            && record.partitions() == self.partitions.as_slice()
            && record.worker_count() == self.worker_count
    }
}

pub(crate) fn actual_parallel_batch_timeline_records(
    scope: WorkloadParallelBatchTimelineScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    match scope {
        WorkloadParallelBatchTimelineScope::Scheduler => {
            summary.parallel_scheduler_batch_timeline().to_vec()
        }
        WorkloadParallelBatchTimelineScope::DataCacheScheduler => summary
            .data_cache_parallel_scheduler_batch_timeline()
            .to_vec(),
        WorkloadParallelBatchTimelineScope::GpuDmaScheduler => {
            summary.gpu_dma_scheduler_batch_timeline().to_vec()
        }
        WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler => {
            summary.accelerator_dma_scheduler_batch_timeline().to_vec()
        }
        WorkloadParallelBatchTimelineScope::FullSystem => {
            summary.full_system_parallel_scheduler_batch_timeline()
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
