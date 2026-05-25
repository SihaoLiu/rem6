use rem6_kernel::{ParallelRemoteFlowRecord, ParallelRemoteSendRecord};

use crate::{
    WorkloadError, WorkloadExpectedParallelRemoteFlow, WorkloadExpectedParallelRemoteFlowTiming,
    WorkloadExpectedParallelRemoteSend, WorkloadParallelExecutionSummary,
    WorkloadParallelRemoteFlowScope, WorkloadReplayPlan, WorkloadResult,
};

const PARALLEL_REMOTE_FLOW_SCOPES: [WorkloadParallelRemoteFlowScope; 3] = [
    WorkloadParallelRemoteFlowScope::Scheduler,
    WorkloadParallelRemoteFlowScope::DataCacheScheduler,
    WorkloadParallelRemoteFlowScope::FullSystem,
];

pub(crate) fn verify_expected_parallel_remote_sends(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_sends = plan.expected_parallel_remote_sends();
    if expected_sends.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_sends[0];
        return Err(WorkloadError::MissingParallelRemoteSendSummary {
            scope: expected.scope(),
            source: expected.source().index(),
            target: expected.target().index(),
            source_tick: expected.source_tick(),
            delivery_tick: expected.delivery_tick(),
            order: expected.order(),
        });
    };

    for expected in expected_sends {
        if expected.actual_record(summary).is_none() {
            return Err(WorkloadError::ExpectedParallelRemoteSendMissing {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
                source_tick: expected.source_tick(),
                delivery_tick: expected.delivery_tick(),
                order: expected.order(),
            });
        }
    }
    for scope in PARALLEL_REMOTE_FLOW_SCOPES {
        let expected_for_scope = expected_parallel_remote_sends_for_scope(expected_sends, scope);
        if expected_for_scope.is_empty() {
            continue;
        }
        let actual_for_scope = actual_parallel_remote_sends_for_scope(summary, scope);
        if let Some(actual) =
            unexpected_parallel_remote_send(&expected_for_scope, &actual_for_scope)
        {
            return Err(WorkloadError::UnexpectedParallelRemoteSend {
                scope,
                source: actual.source().index(),
                target: actual.target().index(),
                source_tick: actual.source_tick(),
                delivery_tick: actual.delivery_tick(),
                order: actual.order(),
            });
        }
    }
    Ok(())
}

fn expected_parallel_remote_sends_for_scope(
    expected_sends: &[WorkloadExpectedParallelRemoteSend],
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<WorkloadExpectedParallelRemoteSend> {
    expected_sends
        .iter()
        .copied()
        .filter(|expected| expected.scope() == scope)
        .collect()
}

fn actual_parallel_remote_sends_for_scope(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<ParallelRemoteSendRecord> {
    match scope {
        WorkloadParallelRemoteFlowScope::Scheduler => {
            summary.parallel_scheduler_remote_sends().to_vec()
        }
        WorkloadParallelRemoteFlowScope::DataCacheScheduler => summary
            .data_cache_parallel_scheduler_remote_sends()
            .to_vec(),
        WorkloadParallelRemoteFlowScope::FullSystem => {
            summary.full_system_parallel_scheduler_remote_sends()
        }
    }
}

fn unexpected_parallel_remote_send(
    expected_sends: &[WorkloadExpectedParallelRemoteSend],
    actual_sends: &[ParallelRemoteSendRecord],
) -> Option<ParallelRemoteSendRecord> {
    let mut matched_expectations = vec![false; expected_sends.len()];
    for actual in actual_sends {
        let matching_index = expected_sends
            .iter()
            .enumerate()
            .find(|(index, expected)| {
                !matched_expectations[*index] && expected.matches_record(*actual)
            })
            .map(|(index, _)| index);
        match matching_index {
            Some(index) => matched_expectations[index] = true,
            None => return Some(*actual),
        }
    }
    None
}

pub(crate) fn verify_expected_parallel_remote_flows(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_flows = plan.expected_parallel_remote_flows();
    if expected_flows.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_flows[0];
        return Err(WorkloadError::MissingParallelExecutionSummary {
            scope: expected.scope(),
            source: expected.source().index(),
            target: expected.target().index(),
            expected_send_count: expected.send_count(),
        });
    };

    for expected in expected_flows {
        let actual_send_count = expected.actual_send_count(summary);
        if actual_send_count != expected.send_count() {
            return Err(WorkloadError::ExpectedParallelRemoteFlowCountMismatch {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
                expected_send_count: expected.send_count(),
                actual_send_count,
            });
        }
    }
    for scope in PARALLEL_REMOTE_FLOW_SCOPES {
        let expected_for_scope = expected_parallel_remote_flows_for_scope(expected_flows, scope);
        if expected_for_scope.is_empty() {
            continue;
        }
        let actual_for_scope = actual_parallel_remote_flows_for_scope(summary, scope);
        if let Some(actual) =
            unexpected_parallel_remote_flow(&expected_for_scope, &actual_for_scope)
        {
            return Err(WorkloadError::UnexpectedParallelRemoteFlow {
                scope,
                source: actual.source().index(),
                target: actual.target().index(),
                actual_send_count: actual.send_count(),
            });
        }
    }
    Ok(())
}

fn expected_parallel_remote_flows_for_scope(
    expected_flows: &[WorkloadExpectedParallelRemoteFlow],
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<WorkloadExpectedParallelRemoteFlow> {
    expected_flows
        .iter()
        .copied()
        .filter(|expected| expected.scope() == scope)
        .collect()
}

fn actual_parallel_remote_flows_for_scope(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<ParallelRemoteFlowRecord> {
    match scope {
        WorkloadParallelRemoteFlowScope::Scheduler => {
            summary.parallel_scheduler_remote_flow_evidence()
        }
        WorkloadParallelRemoteFlowScope::DataCacheScheduler => {
            summary.data_cache_parallel_scheduler_remote_flow_evidence()
        }
        WorkloadParallelRemoteFlowScope::FullSystem => {
            summary.full_system_parallel_scheduler_remote_flows()
        }
    }
}

fn unexpected_parallel_remote_flow(
    expected_flows: &[WorkloadExpectedParallelRemoteFlow],
    actual_flows: &[ParallelRemoteFlowRecord],
) -> Option<ParallelRemoteFlowRecord> {
    actual_flows.iter().copied().find(|actual| {
        !expected_flows
            .iter()
            .any(|expected| expected.matches_record(*actual))
    })
}

pub(crate) fn verify_expected_parallel_remote_delay_floors(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_floors = plan.expected_parallel_remote_delay_floors();
    if expected_floors.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_floors[0];
        return Err(WorkloadError::MissingParallelRemoteDelayFloorSummary {
            scope: expected.scope(),
            minimum_delay: expected.minimum_delay(),
        });
    };

    for expected in expected_floors {
        let flows = actual_parallel_remote_flows_for_scope(summary, expected.scope());
        if flows.is_empty() {
            return Err(WorkloadError::MissingParallelRemoteDelayEvidence {
                scope: expected.scope(),
                minimum_delay: expected.minimum_delay(),
            });
        }
        for flow in flows {
            let Some(actual_minimum_delay) = flow.minimum_delay() else {
                return Err(WorkloadError::MissingParallelRemoteFlowDelayEvidence {
                    scope: expected.scope(),
                    source: flow.source().index(),
                    target: flow.target().index(),
                    minimum_delay: expected.minimum_delay(),
                });
            };
            if actual_minimum_delay < expected.minimum_delay() {
                return Err(WorkloadError::ExpectedParallelRemoteDelayBelowFloor {
                    scope: expected.scope(),
                    source: flow.source().index(),
                    target: flow.target().index(),
                    minimum_delay: expected.minimum_delay(),
                    actual_minimum_delay,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_remote_endpoints(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_endpoints = plan.expected_parallel_remote_endpoints();
    if expected_endpoints.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_endpoints[0];
        return Err(WorkloadError::MissingParallelRemoteEndpointSummary {
            scope: expected.scope(),
            expected_sources: expected.source_partition_indexes(),
            expected_targets: expected.target_partition_indexes(),
        });
    };

    for expected in expected_endpoints {
        let actual_sources = expected.actual_source_partitions(summary);
        let actual_targets = expected.actual_target_partitions(summary);
        if actual_sources != expected.source_partitions()
            || actual_targets != expected.target_partitions()
        {
            return Err(WorkloadError::ExpectedParallelRemoteEndpointsMismatch {
                scope: expected.scope(),
                expected_sources: expected.source_partition_indexes(),
                actual_sources: partition_indexes(&actual_sources),
                expected_targets: expected.target_partition_indexes(),
                actual_targets: partition_indexes(&actual_targets),
            });
        }
    }
    Ok(())
}

fn partition_indexes(partitions: &[rem6_kernel::PartitionId]) -> Vec<u32> {
    partitions
        .iter()
        .map(|partition| partition.index())
        .collect()
}

pub(crate) fn verify_expected_parallel_remote_flow_timings(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_timings = plan.expected_parallel_remote_flow_timings();
    if expected_timings.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_timings[0];
        return Err(WorkloadError::MissingParallelRemoteFlowTimingSummary {
            scope: expected.scope(),
            source: expected.source().index(),
            target: expected.target().index(),
            expected_send_count: expected.send_count(),
            expected_first_tick: expected.first_tick(),
            expected_last_tick: expected.last_tick(),
        });
    };

    for expected in expected_timings {
        let actual = expected.actual_record(summary);
        let actual_send_count = actual.map(|record| record.send_count()).unwrap_or(0);
        let actual_first_tick = actual.map(|record| record.first_tick());
        let actual_last_tick = actual.map(|record| record.last_tick());
        if actual_send_count != expected.send_count()
            || actual_first_tick != Some(expected.first_tick())
            || actual_last_tick != Some(expected.last_tick())
        {
            return Err(WorkloadError::ExpectedParallelRemoteFlowTimingMismatch {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
                expected_send_count: expected.send_count(),
                actual_send_count,
                expected_first_tick: expected.first_tick(),
                actual_first_tick,
                expected_last_tick: expected.last_tick(),
                actual_last_tick,
            });
        }
        if let Some(error) = expected.delay_bounds_mismatch(actual) {
            return Err(error);
        }
    }
    for scope in PARALLEL_REMOTE_FLOW_SCOPES {
        let expected_for_scope =
            expected_parallel_remote_flow_timings_for_scope(expected_timings, scope);
        if expected_for_scope.is_empty() {
            continue;
        }
        let actual_for_scope = actual_parallel_remote_flows_for_scope(summary, scope);
        if let Some(actual) =
            unexpected_parallel_remote_flow_timing(&expected_for_scope, &actual_for_scope)
        {
            return Err(WorkloadError::UnexpectedParallelRemoteFlowTiming {
                scope,
                source: actual.source().index(),
                target: actual.target().index(),
                actual_send_count: actual.send_count(),
                actual_first_tick: actual.first_tick(),
                actual_last_tick: actual.last_tick(),
            });
        }
    }
    Ok(())
}

fn expected_parallel_remote_flow_timings_for_scope(
    expected_timings: &[WorkloadExpectedParallelRemoteFlowTiming],
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<WorkloadExpectedParallelRemoteFlowTiming> {
    expected_timings
        .iter()
        .copied()
        .filter(|expected| expected.scope() == scope)
        .collect()
}

fn unexpected_parallel_remote_flow_timing(
    expected_timings: &[WorkloadExpectedParallelRemoteFlowTiming],
    actual_flows: &[ParallelRemoteFlowRecord],
) -> Option<ParallelRemoteFlowRecord> {
    actual_flows.iter().copied().find(|actual| {
        !expected_timings
            .iter()
            .any(|expected| expected.matches_timing_record(*actual))
    })
}

pub(crate) fn verify_expected_clean_parallel_diagnostics(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_diagnostics = plan.expected_clean_parallel_diagnostics();
    if expected_diagnostics.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_diagnostics[0];
        return Err(WorkloadError::MissingParallelDiagnosticSummary {
            scope: expected.scope(),
        });
    };

    for expected in expected_diagnostics {
        let (wait_for_edge_count, deadlock_diagnostic_count, livelock_diagnostic_count) =
            expected.actual_counts(summary);
        if wait_for_edge_count != 0
            || deadlock_diagnostic_count != 0
            || livelock_diagnostic_count != 0
        {
            return Err(WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
                scope: expected.scope(),
                wait_for_edge_count,
                deadlock_diagnostic_count,
                livelock_diagnostic_count,
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_resource_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_resource_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_activity[0];
        return Err(WorkloadError::MissingResourceActivitySummary {
            scope: expected.scope(),
            minimum_operation_count: expected.minimum_operation_count(),
            minimum_active_resource_count: expected.minimum_active_resource_count(),
        });
    };

    for expected in expected_activity {
        let (actual_operation_count, actual_active_resource_count) =
            expected.actual_counts(summary);
        if actual_operation_count < expected.minimum_operation_count()
            || actual_active_resource_count < expected.minimum_active_resource_count()
        {
            return Err(WorkloadError::ExpectedResourceActivityBelowMinimum {
                scope: expected.scope(),
                minimum_operation_count: expected.minimum_operation_count(),
                actual_operation_count,
                minimum_active_resource_count: expected.minimum_active_resource_count(),
                actual_active_resource_count,
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_data_cache_protocol_run_counts(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_counts = plan.expected_data_cache_protocol_run_counts();
    if expected_counts.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_counts[0];
        return Err(WorkloadError::MissingDataCacheProtocolSummary {
            protocol: expected.protocol(),
            minimum_run_count: expected.minimum_run_count(),
        });
    };

    for expected in expected_counts {
        let actual_run_count = expected.actual_run_count(summary);
        if actual_run_count < expected.minimum_run_count() {
            return Err(
                WorkloadError::ExpectedDataCacheProtocolRunCountBelowMinimum {
                    protocol: expected.protocol(),
                    minimum_run_count: expected.minimum_run_count(),
                    actual_run_count,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_data_cache_run_attribution(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let Some(expected) = plan.expected_data_cache_run_attribution() else {
        return Ok(());
    };
    let Some(summary) = result.parallel_execution_summary() else {
        return Err(WorkloadError::MissingDataCacheRunAttributionSummary {
            minimum_attributed_run_count: expected.minimum_attributed_run_count(),
            maximum_unattributed_run_count: expected.maximum_unattributed_run_count(),
        });
    };

    let (actual_attributed_run_count, actual_unattributed_run_count) =
        expected.actual_counts(summary);
    if actual_attributed_run_count < expected.minimum_attributed_run_count() {
        return Err(WorkloadError::ExpectedDataCacheRunAttributionBelowMinimum {
            minimum_attributed_run_count: expected.minimum_attributed_run_count(),
            actual_attributed_run_count,
        });
    }
    if actual_unattributed_run_count > expected.maximum_unattributed_run_count() {
        return Err(WorkloadError::ExpectedDataCacheRunAttributionAboveMaximum {
            maximum_unattributed_run_count: expected.maximum_unattributed_run_count(),
            actual_unattributed_run_count,
        });
    }
    Ok(())
}

pub(crate) fn verify_expected_data_cache_run_accounting(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    if plan.expected_data_cache_protocol_run_counts().is_empty()
        && plan.expected_data_cache_run_attribution().is_none()
    {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        return Ok(());
    };

    let attributed_run_count = summary.attributed_data_cache_parallel_run_count();
    let unattributed_run_count = summary.unattributed_data_cache_parallel_run_count();
    let accounted_run_count = attributed_run_count.saturating_add(unattributed_run_count);
    if accounted_run_count != summary.data_cache_parallel_run_count() {
        return Err(WorkloadError::DataCacheRunAccountingMismatch {
            data_cache_parallel_run_count: summary.data_cache_parallel_run_count(),
            attributed_run_count,
            unattributed_run_count,
        });
    }

    let protocol_run_count = summary.attributed_data_cache_protocol_run_count();
    if protocol_run_count != attributed_run_count {
        return Err(WorkloadError::DataCacheProtocolAccountingMismatch {
            attributed_run_count,
            protocol_run_count,
        });
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_scheduler_progress(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_progress = plan.expected_parallel_scheduler_progress();
    if expected_progress.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_progress[0];
        return Err(WorkloadError::MissingParallelSchedulerProgressSummary {
            scope: expected.scope(),
            minimum_epoch_count: expected.minimum_epoch_count(),
            minimum_dispatch_count: expected.minimum_dispatch_count(),
        });
    };

    for expected in expected_progress {
        let (actual_epoch_count, actual_dispatch_count) = expected.actual_counts(summary);
        if actual_epoch_count < expected.minimum_epoch_count()
            || actual_dispatch_count < expected.minimum_dispatch_count()
        {
            return Err(
                WorkloadError::ExpectedParallelSchedulerProgressBelowMinimum {
                    scope: expected.scope(),
                    minimum_epoch_count: expected.minimum_epoch_count(),
                    actual_epoch_count,
                    minimum_dispatch_count: expected.minimum_dispatch_count(),
                    actual_dispatch_count,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_parallel_batch_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_activity[0];
        return Err(WorkloadError::MissingParallelBatchActivitySummary {
            scope: expected.scope(),
            minimum_worker_count: expected.minimum_worker_count(),
            minimum_batch_count: expected.minimum_batch_count(),
        });
    };

    for expected in expected_activity {
        let actual_batch_count = expected.actual_batch_count(summary);
        if actual_batch_count < expected.minimum_batch_count() {
            return Err(WorkloadError::ExpectedParallelBatchActivityBelowMinimum {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
                minimum_batch_count: expected.minimum_batch_count(),
                actual_batch_count,
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_partition_sets(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_sets = plan.expected_parallel_batch_partition_sets();
    if expected_sets.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_sets[0];
        return Err(WorkloadError::MissingParallelBatchPartitionSetSummary {
            scope: expected.scope(),
            partitions: expected.partition_indexes(),
            minimum_batch_count: expected.minimum_batch_count(),
        });
    };

    for expected in expected_sets {
        let actual_batch_count = expected.actual_batch_count(summary);
        if actual_batch_count < expected.minimum_batch_count() {
            return Err(
                WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
                    scope: expected.scope(),
                    partitions: expected.partition_indexes(),
                    minimum_batch_count: expected.minimum_batch_count(),
                    actual_batch_count,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_partition_streaks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_streaks = plan.expected_parallel_batch_partition_streaks();
    if expected_streaks.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_streaks[0];
        return Err(WorkloadError::MissingParallelBatchPartitionStreakSummary {
            scope: expected.scope(),
            partitions: expected.partition_indexes(),
            minimum_consecutive_batch_count: expected.minimum_consecutive_batch_count(),
        });
    };

    for expected in expected_streaks {
        let actual_consecutive_batch_count = expected.actual_consecutive_batch_count(summary);
        if actual_consecutive_batch_count < expected.minimum_consecutive_batch_count() {
            return Err(
                WorkloadError::ExpectedParallelBatchPartitionStreakBelowMinimum {
                    scope: expected.scope(),
                    partitions: expected.partition_indexes(),
                    minimum_consecutive_batch_count: expected.minimum_consecutive_batch_count(),
                    actual_consecutive_batch_count,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_scheduler_idle_bounds(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_bounds = plan.expected_parallel_scheduler_idle_bounds();
    if expected_bounds.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_bounds[0];
        return Err(WorkloadError::MissingParallelSchedulerIdleSummary {
            scope: expected.scope(),
            maximum_empty_epoch_count: expected.maximum_empty_epoch_count(),
        });
    };

    for expected in expected_bounds {
        let actual_empty_epoch_count = expected.actual_empty_epoch_count(summary);
        if actual_empty_epoch_count > expected.maximum_empty_epoch_count() {
            return Err(WorkloadError::ExpectedParallelSchedulerIdleAboveMaximum {
                scope: expected.scope(),
                maximum_empty_epoch_count: expected.maximum_empty_epoch_count(),
                actual_empty_epoch_count,
            });
        }
    }
    Ok(())
}

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
