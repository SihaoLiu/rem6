use crate::{WorkloadError, WorkloadParallelExecutionSummary, WorkloadParallelSchedulerScope};

pub(crate) fn validate_scheduler_scope_summary(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
) -> Result<(), WorkloadError> {
    match scope {
        WorkloadParallelSchedulerScope::Scheduler
        | WorkloadParallelSchedulerScope::DataCacheScheduler
        | WorkloadParallelSchedulerScope::GpuDmaScheduler
        | WorkloadParallelSchedulerScope::AcceleratorDmaScheduler
        | WorkloadParallelSchedulerScope::DmaScheduler => validate_scheduler_counts(summary, scope),
        WorkloadParallelSchedulerScope::FullSystem => {
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::FullSystem)?;
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::Scheduler)?;
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::DataCacheScheduler)?;
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::GpuDmaScheduler)?;
            validate_scheduler_counts(
                summary,
                WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
            )?;
            validate_scheduler_counts(summary, WorkloadParallelSchedulerScope::DmaScheduler)
        }
    }
}

pub(crate) fn validate_full_system_scheduler_progress_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
    minimum_epoch_count: usize,
) -> Result<(), WorkloadError> {
    if scope != WorkloadParallelSchedulerScope::FullSystem
        || !summary.has_explicit_full_system_parallel_scheduler_counts()
    {
        return Ok(());
    }
    let minimum_dispatch_count =
        summary.full_system_parallel_scheduler_dispatch_count_lower_bound();
    let actual_dispatch_count = summary.raw_full_system_parallel_scheduler_dispatch_count();
    if actual_dispatch_count < minimum_dispatch_count {
        return Err(
            WorkloadError::ExpectedParallelSchedulerProgressBelowMinimum {
                scope: WorkloadParallelSchedulerScope::FullSystem,
                minimum_epoch_count,
                actual_epoch_count: summary.raw_full_system_parallel_scheduler_epoch_count(),
                minimum_dispatch_count,
                actual_dispatch_count,
            },
        );
    }
    Ok(())
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
        WorkloadParallelSchedulerScope::DmaScheduler => (
            summary.dma_scheduler_epoch_count(),
            summary.dma_scheduler_empty_epoch_count(),
        ),
        WorkloadParallelSchedulerScope::FullSystem => (
            summary.full_system_parallel_scheduler_epoch_count(),
            summary.full_system_parallel_scheduler_empty_epoch_count(),
        ),
    }
}
