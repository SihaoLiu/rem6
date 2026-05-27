use rem6_kernel::PartitionFrontier;

use super::scheduler_summary::validate_scheduler_scope_summary;
use crate::{
    WorkloadError, WorkloadParallelExecutionSummary, WorkloadParallelFrontierStage,
    WorkloadParallelSchedulerScope, WorkloadReplayPlan, WorkloadResult,
};

pub(crate) fn verify_expected_parallel_frontiers(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_frontiers = plan.expected_parallel_frontiers();
    if expected_frontiers.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_frontiers[0];
        return Err(WorkloadError::MissingParallelFrontierSummary {
            scope: expected.scope(),
            stage: expected.stage(),
            partition: expected.partition().index(),
            minimum_now: expected.minimum_now(),
            minimum_safe_until: expected.minimum_safe_until(),
        });
    };

    for expected in expected_frontiers {
        validate_scheduler_scope_summary(summary, expected.scope())?;
        validate_frontier_scope_summary(summary, expected.scope(), expected.stage())?;
        let actual = expected.actual_frontier(summary);
        let actual_now = actual.map(|frontier| frontier.now());
        let actual_safe_until = actual.map(|frontier| frontier.safe_until());
        if actual_now.unwrap_or(0) < expected.minimum_now()
            || actual_safe_until.unwrap_or(0) < expected.minimum_safe_until()
        {
            return Err(WorkloadError::ExpectedParallelFrontierBelowMinimum {
                scope: expected.scope(),
                stage: expected.stage(),
                partition: expected.partition().index(),
                minimum_now: expected.minimum_now(),
                actual_now,
                minimum_safe_until: expected.minimum_safe_until(),
                actual_safe_until,
            });
        }
    }
    Ok(())
}

fn validate_frontier_scope_summary(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
    stage: WorkloadParallelFrontierStage,
) -> Result<(), WorkloadError> {
    match scope {
        WorkloadParallelSchedulerScope::Scheduler
        | WorkloadParallelSchedulerScope::DataCacheScheduler
        | WorkloadParallelSchedulerScope::GpuDmaScheduler
        | WorkloadParallelSchedulerScope::AcceleratorDmaScheduler => {
            validate_frontiers(scope, stage, frontier_records(summary, scope, stage))
        }
        WorkloadParallelSchedulerScope::FullSystem => {
            for scoped in [
                WorkloadParallelSchedulerScope::Scheduler,
                WorkloadParallelSchedulerScope::DataCacheScheduler,
                WorkloadParallelSchedulerScope::GpuDmaScheduler,
                WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
                WorkloadParallelSchedulerScope::FullSystem,
            ] {
                validate_frontiers(scope, stage, frontier_records(summary, scoped, stage))?;
            }
            Ok(())
        }
    }
}

fn frontier_records(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
    stage: WorkloadParallelFrontierStage,
) -> &[PartitionFrontier] {
    match (scope, stage) {
        (WorkloadParallelSchedulerScope::Scheduler, WorkloadParallelFrontierStage::Initial) => {
            summary.parallel_scheduler_initial_frontiers()
        }
        (WorkloadParallelSchedulerScope::Scheduler, WorkloadParallelFrontierStage::Final) => {
            summary.parallel_scheduler_final_frontiers()
        }
        (
            WorkloadParallelSchedulerScope::DataCacheScheduler,
            WorkloadParallelFrontierStage::Initial,
        ) => summary.data_cache_parallel_scheduler_initial_frontiers(),
        (
            WorkloadParallelSchedulerScope::DataCacheScheduler,
            WorkloadParallelFrontierStage::Final,
        ) => summary.data_cache_parallel_scheduler_final_frontiers(),
        (
            WorkloadParallelSchedulerScope::GpuDmaScheduler,
            WorkloadParallelFrontierStage::Initial,
        ) => summary.gpu_dma_scheduler_initial_frontiers(),
        (WorkloadParallelSchedulerScope::GpuDmaScheduler, WorkloadParallelFrontierStage::Final) => {
            summary.gpu_dma_scheduler_final_frontiers()
        }
        (
            WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
            WorkloadParallelFrontierStage::Initial,
        ) => summary.accelerator_dma_scheduler_initial_frontiers(),
        (
            WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
            WorkloadParallelFrontierStage::Final,
        ) => summary.accelerator_dma_scheduler_final_frontiers(),
        (WorkloadParallelSchedulerScope::FullSystem, WorkloadParallelFrontierStage::Initial) => {
            summary.raw_full_system_parallel_scheduler_initial_frontiers()
        }
        (WorkloadParallelSchedulerScope::FullSystem, WorkloadParallelFrontierStage::Final) => {
            summary.raw_full_system_parallel_scheduler_final_frontiers()
        }
    }
}

fn validate_frontiers(
    scope: WorkloadParallelSchedulerScope,
    stage: WorkloadParallelFrontierStage,
    frontiers: &[PartitionFrontier],
) -> Result<(), WorkloadError> {
    for frontier in frontiers {
        if frontier.safe_until() < frontier.now()
            || frontier
                .next_tick()
                .is_some_and(|next_tick| next_tick < frontier.now())
        {
            return Err(WorkloadError::InvalidParallelFrontierSummary {
                scope,
                stage,
                partition: frontier.partition().index(),
                now: frontier.now(),
                safe_until: frontier.safe_until(),
                next_tick: frontier.next_tick(),
                pending_events: frontier.pending_events(),
            });
        }
    }
    Ok(())
}
