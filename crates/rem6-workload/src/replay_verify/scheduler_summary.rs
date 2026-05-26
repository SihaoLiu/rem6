use crate::{WorkloadError, WorkloadParallelExecutionSummary, WorkloadParallelSchedulerScope};

pub(crate) fn validate_scheduler_scope_summary(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
) -> Result<(), WorkloadError> {
    match scope {
        WorkloadParallelSchedulerScope::Scheduler
        | WorkloadParallelSchedulerScope::DataCacheScheduler
        | WorkloadParallelSchedulerScope::GpuDmaScheduler
        | WorkloadParallelSchedulerScope::AcceleratorDmaScheduler => {
            validate_scheduler_counts(summary, scope)
        }
        WorkloadParallelSchedulerScope::FullSystem => {
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::Scheduler)?;
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::DataCacheScheduler)?;
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::GpuDmaScheduler)?;
            validate_scheduler_counts(
                summary,
                WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
            )
        }
    }
}

fn validate_scheduler_counts(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
) -> Result<(), WorkloadError> {
    let (epoch_count, empty_epoch_count) = scheduler_counts(summary, scope);
    if empty_epoch_count > epoch_count {
        return Err(WorkloadError::InvalidParallelSchedulerSummary {
            scope,
            epoch_count,
            empty_epoch_count,
        });
    }
    Ok(())
}

fn scheduler_counts(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
) -> (usize, usize) {
    match scope {
        WorkloadParallelSchedulerScope::Scheduler => (
            summary.scheduler_epoch_count(),
            summary.scheduler_empty_epoch_count(),
        ),
        WorkloadParallelSchedulerScope::DataCacheScheduler => (
            summary.data_cache_parallel_scheduler_epoch_count(),
            summary.data_cache_parallel_scheduler_empty_epoch_count(),
        ),
        WorkloadParallelSchedulerScope::GpuDmaScheduler => (
            summary.gpu_dma_scheduler_epoch_count(),
            summary.gpu_dma_scheduler_empty_epoch_count(),
        ),
        WorkloadParallelSchedulerScope::AcceleratorDmaScheduler => (
            summary.accelerator_dma_scheduler_epoch_count(),
            summary.accelerator_dma_scheduler_empty_epoch_count(),
        ),
        WorkloadParallelSchedulerScope::FullSystem => (
            summary.full_system_parallel_scheduler_epoch_count(),
            summary.full_system_parallel_scheduler_empty_epoch_count(),
        ),
    }
}
