use rem6_kernel::PartitionFrontier;

use super::scheduler_summary::validate_scheduler_scope_summary;
use crate::{
    result_collect::collect_conservative_partition_frontiers, WorkloadError,
    WorkloadParallelExecutionSummary, WorkloadParallelFrontierStage,
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
        | WorkloadParallelSchedulerScope::AcceleratorDmaScheduler
        | WorkloadParallelSchedulerScope::DmaScheduler => {
            validate_frontiers(scope, stage, frontier_records(summary, scope, stage))
        }
        WorkloadParallelSchedulerScope::FullSystem => {
            for scoped in [
                WorkloadParallelSchedulerScope::Scheduler,
                WorkloadParallelSchedulerScope::DataCacheScheduler,
                WorkloadParallelSchedulerScope::GpuDmaScheduler,
                WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
                WorkloadParallelSchedulerScope::DmaScheduler,
                WorkloadParallelSchedulerScope::FullSystem,
            ] {
                validate_frontiers(scope, stage, frontier_records(summary, scoped, stage))?;
            }
            validate_full_system_frontier_merge_summary(summary, stage)?;
            Ok(())
        }
    }
}

fn validate_full_system_frontier_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
    stage: WorkloadParallelFrontierStage,
) -> Result<(), WorkloadError> {
    let explicit_frontiers =
        frontier_records(summary, WorkloadParallelSchedulerScope::FullSystem, stage);
    if explicit_frontiers.is_empty() {
        return Ok(());
    }

    let scoped_frontiers = scoped_full_system_frontier_records(summary, stage);
    for scoped_frontier in scoped_frontiers {
        let explicit_frontier = explicit_frontiers
            .iter()
            .copied()
            .find(|frontier| frontier.partition() == scoped_frontier.partition());
        if explicit_frontier
            .is_some_and(|frontier| frontier_covers_scoped_frontier(frontier, scoped_frontier))
        {
            continue;
        }
        return Err(invalid_full_system_frontier_merge_error(
            stage,
            scoped_frontier,
            explicit_frontier,
        ));
    }
    Ok(())
}

fn scoped_full_system_frontier_records(
    summary: &WorkloadParallelExecutionSummary,
    stage: WorkloadParallelFrontierStage,
) -> Vec<PartitionFrontier> {
    collect_conservative_partition_frontiers(
        frontier_records(summary, WorkloadParallelSchedulerScope::Scheduler, stage)
            .into_iter()
            .chain(frontier_records(
                summary,
                WorkloadParallelSchedulerScope::DataCacheScheduler,
                stage,
            ))
            .chain(frontier_records(
                summary,
                WorkloadParallelSchedulerScope::DmaScheduler,
                stage,
            )),
    )
}

fn frontier_covers_scoped_frontier(
    merged_frontier: PartitionFrontier,
    scoped_frontier: PartitionFrontier,
) -> bool {
    merged_frontier.now() <= scoped_frontier.now()
        && merged_frontier.safe_until() <= scoped_frontier.safe_until()
        && next_tick_covers_scoped_frontier(
            merged_frontier.next_tick(),
            scoped_frontier.next_tick(),
        )
        && merged_frontier.pending_events() >= scoped_frontier.pending_events()
}

fn next_tick_covers_scoped_frontier(
    merged_next_tick: Option<u64>,
    scoped_next_tick: Option<u64>,
) -> bool {
    match (merged_next_tick, scoped_next_tick) {
        (_, None) => true,
        (Some(merged_next_tick), Some(scoped_next_tick)) => merged_next_tick <= scoped_next_tick,
        (None, Some(_)) => false,
    }
}

fn invalid_full_system_frontier_merge_error(
    stage: WorkloadParallelFrontierStage,
    scoped_frontier: PartitionFrontier,
    explicit_frontier: Option<PartitionFrontier>,
) -> WorkloadError {
    WorkloadError::InvalidParallelFrontierMergeSummary {
        scope: WorkloadParallelSchedulerScope::FullSystem,
        stage,
        partition: scoped_frontier.partition().index(),
        merged_now: explicit_frontier.map(|frontier| frontier.now()),
        scoped_now: scoped_frontier.now(),
        merged_safe_until: explicit_frontier.map(|frontier| frontier.safe_until()),
        scoped_safe_until: scoped_frontier.safe_until(),
        merged_next_tick: explicit_frontier.and_then(|frontier| frontier.next_tick()),
        scoped_next_tick: scoped_frontier.next_tick(),
        merged_pending_events: explicit_frontier.map(|frontier| frontier.pending_events()),
        scoped_pending_events: scoped_frontier.pending_events(),
    }
}

fn frontier_records(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelSchedulerScope,
    stage: WorkloadParallelFrontierStage,
) -> Vec<PartitionFrontier> {
    match (scope, stage) {
        (WorkloadParallelSchedulerScope::Scheduler, WorkloadParallelFrontierStage::Initial) => {
            summary.parallel_scheduler_initial_frontiers().to_vec()
        }
        (WorkloadParallelSchedulerScope::Scheduler, WorkloadParallelFrontierStage::Final) => {
            summary.parallel_scheduler_final_frontiers().to_vec()
        }
        (
            WorkloadParallelSchedulerScope::DataCacheScheduler,
            WorkloadParallelFrontierStage::Initial,
        ) => summary
            .data_cache_parallel_scheduler_initial_frontiers()
            .to_vec(),
        (
            WorkloadParallelSchedulerScope::DataCacheScheduler,
            WorkloadParallelFrontierStage::Final,
        ) => summary
            .data_cache_parallel_scheduler_final_frontiers()
            .to_vec(),
        (
            WorkloadParallelSchedulerScope::GpuDmaScheduler,
            WorkloadParallelFrontierStage::Initial,
        ) => summary.gpu_dma_scheduler_initial_frontiers().to_vec(),
        (WorkloadParallelSchedulerScope::GpuDmaScheduler, WorkloadParallelFrontierStage::Final) => {
            summary.gpu_dma_scheduler_final_frontiers().to_vec()
        }
        (
            WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
            WorkloadParallelFrontierStage::Initial,
        ) => summary
            .accelerator_dma_scheduler_initial_frontiers()
            .to_vec(),
        (
            WorkloadParallelSchedulerScope::AcceleratorDmaScheduler,
            WorkloadParallelFrontierStage::Final,
        ) => summary.accelerator_dma_scheduler_final_frontiers().to_vec(),
        (WorkloadParallelSchedulerScope::DmaScheduler, WorkloadParallelFrontierStage::Initial) => {
            summary.dma_scheduler_initial_frontiers()
        }
        (WorkloadParallelSchedulerScope::DmaScheduler, WorkloadParallelFrontierStage::Final) => {
            summary.dma_scheduler_final_frontiers()
        }
        (WorkloadParallelSchedulerScope::FullSystem, WorkloadParallelFrontierStage::Initial) => {
            summary
                .raw_full_system_parallel_scheduler_initial_frontiers()
                .to_vec()
        }
        (WorkloadParallelSchedulerScope::FullSystem, WorkloadParallelFrontierStage::Final) => {
            summary
                .raw_full_system_parallel_scheduler_final_frontiers()
                .to_vec()
        }
    }
}

fn validate_frontiers(
    scope: WorkloadParallelSchedulerScope,
    stage: WorkloadParallelFrontierStage,
    frontiers: impl IntoIterator<Item = PartitionFrontier>,
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
