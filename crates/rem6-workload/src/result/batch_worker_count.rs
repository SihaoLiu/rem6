use crate::parallel_batch::{
    collect_parallel_batch_worker_counts_from_timeline,
    parallel_batch_activity_count_for_worker_count,
};
use crate::{WorkloadError, WorkloadParallelRemoteFlowScope};

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub fn parallel_scheduler_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        parallel_batch_activity_count_for_worker_count(
            &self.parallel_scheduler_batch_worker_counts,
            &self.parallel_scheduler_batch_partition_sets,
            &self.parallel_scheduler_batch_partition_streaks,
            worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_count_for_worker_count(
        &self,
        worker_count: usize,
    ) -> usize {
        parallel_batch_activity_count_for_worker_count(
            &self.data_cache_parallel_scheduler_batch_worker_counts,
            &self.data_cache_parallel_scheduler_batch_partition_sets,
            &self.data_cache_parallel_scheduler_batch_partition_streaks,
            worker_count,
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_for_worker_count(
        &self,
        worker_count: usize,
    ) -> usize {
        let timeline_counts = collect_parallel_batch_worker_counts_from_timeline(
            &self.full_system_parallel_scheduler_batch_timeline,
        );
        (self.parallel_scheduler_batch_count_for_worker_count(worker_count)
            + self.data_cache_parallel_scheduler_batch_count_for_worker_count(worker_count)
            + self.dma_scheduler_batch_count_for_worker_count(worker_count))
        .max(parallel_batch_activity_count_for_worker_count(
            &timeline_counts,
            &self.full_system_parallel_scheduler_batch_partition_sets,
            &self.full_system_parallel_scheduler_batch_partition_streaks,
            worker_count,
        ))
    }

    pub fn verify_minimum_parallel_batch_count_for_worker_count(
        &self,
        scope: WorkloadParallelRemoteFlowScope,
        worker_count: usize,
        minimum_batch_count: usize,
    ) -> Result<(), WorkloadError> {
        let actual_batch_count = match scope {
            WorkloadParallelRemoteFlowScope::Scheduler => {
                self.parallel_scheduler_batch_count_for_worker_count(worker_count)
            }
            WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
                self.data_cache_parallel_scheduler_batch_count_for_worker_count(worker_count)
            }
            WorkloadParallelRemoteFlowScope::GpuDmaScheduler => {
                self.gpu_dma_scheduler_batch_count_for_worker_count(worker_count)
            }
            WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
                self.accelerator_dma_scheduler_batch_count_for_worker_count(worker_count)
            }
            WorkloadParallelRemoteFlowScope::FullSystem => {
                self.full_system_parallel_scheduler_batch_count_for_worker_count(worker_count)
            }
        };
        if actual_batch_count < minimum_batch_count {
            return Err(WorkloadError::ParallelBatchWorkerCountBelowMinimum {
                scope,
                worker_count,
                minimum_batch_count,
                actual_batch_count,
            });
        }
        Ok(())
    }
}
