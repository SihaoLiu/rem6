use std::collections::BTreeSet;

use rem6_kernel::ParallelProgressTransitionRecord;

use crate::{
    parallel_batch_timeline_expectation::actual_parallel_batch_timeline_records, WorkloadError,
    WorkloadExpectedParallelBatchTimelineRecord, WorkloadExpectedParallelProgressTransition,
    WorkloadParallelBatchTimelineRecord, WorkloadParallelBatchTimelineScope,
    WorkloadParallelDiagnosticScope, WorkloadParallelExecutionSummary,
    WorkloadParallelProgressTransitionExpectationFailure, WorkloadParallelRemoteFlowScope,
    WorkloadReplayPlan, WorkloadResult,
};

mod batch_partition;
mod batch_timeline;
mod batch_worker_count;
mod batch_worker_ticks;
mod frontier;
mod partition_activity;
mod remote_traffic;
mod scheduler_summary;

pub(crate) use batch_partition::{
    validate_partition_scope_batch_partition_set_evidence,
    validate_partition_scope_batch_partition_streak_evidence,
};
use batch_timeline::{
    validate_full_system_batch_timeline_merge_summary,
    validate_planned_full_system_batch_timeline_merge_summary,
    validate_scheduler_scope_batch_timeline_evidence,
};
pub(crate) use batch_timeline::{
    validate_partition_scope_batch_timeline_evidence, validate_worker_scope_batch_timeline_evidence,
};
pub(crate) use batch_worker_count::{
    validate_worker_scope_batch_activity_evidence,
    validate_worker_scope_batch_worker_count_evidence,
    validate_worker_scope_batch_worker_tick_activity_evidence,
    validate_worker_scope_batch_worker_tick_bucket_evidence,
    validate_worker_scope_batch_worker_tick_streak_evidence,
    validate_worker_scope_batch_worker_ticks_evidence,
};
pub(crate) use batch_worker_ticks::{
    verify_expected_parallel_batch_worker_tick_activity,
    verify_expected_parallel_batch_worker_tick_buckets,
    verify_expected_parallel_batch_worker_tick_streaks,
    verify_expected_parallel_batch_worker_ticks,
};
pub(crate) use frontier::verify_expected_parallel_frontiers;
pub(crate) use partition_activity::{
    validate_partition_scope_activity_evidence, validate_partition_scope_count_evidence,
};

pub(crate) use remote_traffic::{
    validate_remote_partition_scope_evidence, verify_expected_parallel_remote_delay_ceilings,
    verify_expected_parallel_remote_delay_floors, verify_expected_parallel_remote_endpoints,
    verify_expected_parallel_remote_flow_timings, verify_expected_parallel_remote_flows,
    verify_expected_parallel_remote_sends, verify_expected_parallel_remote_traffic_consistency,
};
use scheduler_summary::validate_scheduler_scope_summary;

const PARALLEL_REMOTE_FLOW_SCOPES: [WorkloadParallelRemoteFlowScope; 6] = [
    WorkloadParallelRemoteFlowScope::Scheduler,
    WorkloadParallelRemoteFlowScope::DataCacheScheduler,
    WorkloadParallelRemoteFlowScope::GpuDmaScheduler,
    WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler,
    WorkloadParallelRemoteFlowScope::DmaScheduler,
    WorkloadParallelRemoteFlowScope::FullSystem,
];

const PARALLEL_BATCH_TIMELINE_SCOPES: [WorkloadParallelBatchTimelineScope; 12] = [
    WorkloadParallelBatchTimelineScope::Scheduler,
    WorkloadParallelBatchTimelineScope::DataCacheScheduler,
    WorkloadParallelBatchTimelineScope::GpuDmaScheduler,
    WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler,
    WorkloadParallelBatchTimelineScope::DmaScheduler,
    WorkloadParallelBatchTimelineScope::FullSystem,
    WorkloadParallelBatchTimelineScope::PlannedScheduler,
    WorkloadParallelBatchTimelineScope::PlannedDataCacheScheduler,
    WorkloadParallelBatchTimelineScope::PlannedGpuDmaScheduler,
    WorkloadParallelBatchTimelineScope::PlannedAcceleratorDmaScheduler,
    WorkloadParallelBatchTimelineScope::PlannedDmaScheduler,
    WorkloadParallelBatchTimelineScope::PlannedFullSystem,
];

pub(crate) fn verify_expected_parallel_batch_timeline_records(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_records = plan.expected_parallel_batch_timeline_records();
    if expected_records.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_records[0];
        return Err(WorkloadError::MissingParallelBatchTimelineSummary {
            scope: expected.scope(),
            batch_scope: expected.batch_scope(),
            start_tick: expected.start_tick(),
            horizon: expected.horizon(),
            partitions: expected.partition_indexes(),
            worker_count: expected.worker_count(),
        });
    };
    if expected_records
        .iter()
        .any(|expected| expected.scope() == WorkloadParallelBatchTimelineScope::FullSystem)
    {
        validate_full_system_batch_timeline_merge_summary(summary)?;
    }
    if expected_records
        .iter()
        .any(|expected| expected.scope() == WorkloadParallelBatchTimelineScope::PlannedFullSystem)
    {
        validate_planned_full_system_batch_timeline_merge_summary(summary)?;
    }

    for expected in expected_records {
        if expected.actual_record(summary).is_none() {
            return Err(WorkloadError::ExpectedParallelBatchTimelineRecordMissing {
                scope: expected.scope(),
                batch_scope: expected.batch_scope(),
                start_tick: expected.start_tick(),
                horizon: expected.horizon(),
                partitions: expected.partition_indexes(),
                worker_count: expected.worker_count(),
            });
        }
    }
    for scope in PARALLEL_BATCH_TIMELINE_SCOPES {
        let expected_for_scope =
            expected_parallel_batch_timeline_records_for_scope(expected_records, scope);
        if expected_for_scope.is_empty() {
            continue;
        }
        let actual_for_scope = actual_parallel_batch_timeline_records(scope, summary);
        if let Some(actual) =
            unexpected_parallel_batch_timeline_record(&expected_for_scope, &actual_for_scope)
        {
            return Err(WorkloadError::UnexpectedParallelBatchTimelineRecord {
                scope,
                batch_scope: actual.scope(),
                start_tick: actual.start_tick(),
                horizon: actual.horizon(),
                partitions: actual
                    .partitions()
                    .iter()
                    .map(|partition| partition.index())
                    .collect(),
                worker_count: actual.worker_count(),
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_progress_transitions(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_transitions = plan.expected_parallel_progress_transitions();
    if expected_transitions.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        return Err(expected_transitions[0]
            .to_error(WorkloadParallelProgressTransitionExpectationFailure::MissingSummary));
    };

    for expected in expected_transitions {
        if expected.actual_record(summary).is_none() {
            return Err(expected
                .to_error(WorkloadParallelProgressTransitionExpectationFailure::MissingRecord));
        }
    }
    for scope in PARALLEL_REMOTE_FLOW_SCOPES {
        let expected_for_scope =
            expected_parallel_progress_transitions_for_scope(expected_transitions, scope);
        if expected_for_scope.is_empty() {
            continue;
        }
        if scope == WorkloadParallelRemoteFlowScope::FullSystem {
            validate_full_system_progress_transition_record_merge_summary(summary)?;
        }
        let actual_for_scope = actual_parallel_progress_transitions_for_scope(summary, scope);
        if let Some(actual) =
            unexpected_parallel_progress_transition(&expected_for_scope, &actual_for_scope)
        {
            return Err(WorkloadExpectedParallelProgressTransition::new(
                scope,
                actual.partition(),
                actual.subject().clone(),
                actual.kind(),
                actual.tick(),
                actual.order(),
            )
            .to_error(WorkloadParallelProgressTransitionExpectationFailure::UnexpectedRecord));
        }
    }
    Ok(())
}

fn validate_full_system_progress_transition_record_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    let explicit_transitions = summary.raw_full_system_progress_transitions();
    if explicit_transitions.is_empty() {
        return Ok(());
    }

    for scoped_transition in summary.scoped_full_system_progress_transitions() {
        if explicit_transitions.contains(&scoped_transition) {
            continue;
        }
        return Err(
            WorkloadError::InvalidParallelProgressTransitionRecordMergeSummary {
                scope: WorkloadParallelDiagnosticScope::FullSystem,
                partition: scoped_transition.partition(),
                subject: scoped_transition.subject().clone(),
                kind: scoped_transition.kind(),
                tick: scoped_transition.tick(),
                order: scoped_transition.order(),
            },
        );
    }
    Ok(())
}

fn expected_parallel_batch_timeline_records_for_scope(
    expected_records: &[WorkloadExpectedParallelBatchTimelineRecord],
    scope: WorkloadParallelBatchTimelineScope,
) -> Vec<WorkloadExpectedParallelBatchTimelineRecord> {
    expected_records
        .iter()
        .filter(|expected| expected.scope() == scope)
        .cloned()
        .collect()
}

fn unexpected_parallel_batch_timeline_record(
    expected_records: &[WorkloadExpectedParallelBatchTimelineRecord],
    actual_records: &[WorkloadParallelBatchTimelineRecord],
) -> Option<WorkloadParallelBatchTimelineRecord> {
    first_unmatched_actual(
        expected_records,
        actual_records.iter().cloned(),
        |expected, actual| expected.matches_record(actual),
    )
}

fn expected_parallel_progress_transitions_for_scope(
    expected_transitions: &[WorkloadExpectedParallelProgressTransition],
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<WorkloadExpectedParallelProgressTransition> {
    expected_transitions
        .iter()
        .filter(|expected| expected.scope() == scope)
        .cloned()
        .collect()
}

fn actual_parallel_progress_transitions_for_scope(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<ParallelProgressTransitionRecord> {
    match scope {
        WorkloadParallelRemoteFlowScope::Scheduler => {
            summary.parallel_scheduler_progress_transitions().to_vec()
        }
        WorkloadParallelRemoteFlowScope::DataCacheScheduler => summary
            .data_cache_parallel_scheduler_progress_transitions()
            .to_vec(),
        WorkloadParallelRemoteFlowScope::GpuDmaScheduler => {
            summary.gpu_dma_scheduler_progress_transitions().to_vec()
        }
        WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => summary
            .accelerator_dma_scheduler_progress_transitions()
            .to_vec(),
        WorkloadParallelRemoteFlowScope::DmaScheduler => {
            summary.dma_scheduler_progress_transitions()
        }
        WorkloadParallelRemoteFlowScope::FullSystem => summary.full_system_progress_transitions(),
    }
}

fn unexpected_parallel_progress_transition(
    expected_transitions: &[WorkloadExpectedParallelProgressTransition],
    actual_transitions: &[ParallelProgressTransitionRecord],
) -> Option<ParallelProgressTransitionRecord> {
    first_unmatched_actual(
        expected_transitions,
        actual_transitions.iter().cloned(),
        |expected, actual| expected.matches_record(actual),
    )
}

fn first_unmatched_actual<Expected, Actual>(
    expected_records: &[Expected],
    actual_records: impl IntoIterator<Item = Actual>,
    matches_record: impl Fn(&Expected, &Actual) -> bool,
) -> Option<Actual> {
    let mut matched_expectations = vec![false; expected_records.len()];
    for actual in actual_records {
        let matching_index = expected_records
            .iter()
            .enumerate()
            .find(|(index, expected)| {
                !matched_expectations[*index] && matches_record(expected, &actual)
            })
            .map(|(index, _)| index);
        match matching_index {
            Some(index) => matched_expectations[index] = true,
            None => return Some(actual),
        }
    }
    None
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
        summary.validate_parallel_diagnostic_scope_summary(expected.scope())?;
        let (wait_for_edge_count, deadlock_diagnostic_count, diagnostic_livelock_count) =
            expected.actual_counts(summary);
        let diagnostic_subjects = actual_livelock_subject_set(expected.scope(), summary);
        let threshold_subjects = expected
            .livelock_transition_threshold()
            .map(|threshold| {
                transition_threshold_livelock_subjects(expected.scope(), threshold, summary)
            })
            .unwrap_or_default();
        let threshold_only_subject_count =
            threshold_subjects.difference(&diagnostic_subjects).count();
        let livelock_diagnostic_count = diagnostic_livelock_count + threshold_only_subject_count;
        if wait_for_edge_count != 0
            || deadlock_diagnostic_count != 0
            || livelock_diagnostic_count != 0
        {
            let mut livelock_subjects = diagnostic_subjects;
            livelock_subjects.extend(threshold_subjects);
            return Err(WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
                scope: expected.scope(),
                wait_for_edge_count,
                deadlock_diagnostic_count,
                livelock_diagnostic_count,
                livelock_subjects: livelock_subjects.into_iter().collect(),
            });
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_wait_for_edge_kind_counts(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_counts = plan.expected_parallel_wait_for_edge_kind_counts();
    if expected_counts.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_counts[0];
        return Err(WorkloadError::MissingParallelDiagnosticSummary {
            scope: expected.scope(),
        });
    };

    for expected in expected_counts {
        summary.validate_parallel_diagnostic_scope_summary(expected.scope())?;
        let actual_edge_count = expected.actual_count(summary);
        if actual_edge_count < expected.minimum_edge_count() {
            return Err(
                WorkloadError::ExpectedParallelWaitForEdgeKindCountBelowMinimum {
                    scope: expected.scope(),
                    kind: expected.kind(),
                    minimum_edge_count: expected.minimum_edge_count(),
                    actual_edge_count,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_wait_for_edge_kind_windows(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_windows = plan.expected_parallel_wait_for_edge_kind_windows();
    if expected_windows.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_windows[0];
        return Err(WorkloadError::MissingParallelDiagnosticSummary {
            scope: expected.scope(),
        });
    };

    for expected in expected_windows {
        summary.validate_parallel_diagnostic_scope_summary(expected.scope())?;
        let actual_window = expected.actual_window(summary);
        let actual_edge_count = actual_window.map(|window| window.edge_count()).unwrap_or(0);
        let actual_first_tick = actual_window.map(|window| window.first_tick());
        let actual_last_tick = actual_window.map(|window| window.last_tick());
        if actual_edge_count != expected.edge_count()
            || actual_first_tick != Some(expected.first_tick())
            || actual_last_tick != Some(expected.last_tick())
        {
            return Err(
                WorkloadError::ExpectedParallelWaitForEdgeKindWindowMismatch {
                    scope: expected.scope(),
                    kind: expected.kind(),
                    expected_edge_count: expected.edge_count(),
                    actual_edge_count,
                    expected_first_tick: expected.first_tick(),
                    actual_first_tick,
                    expected_last_tick: expected.last_tick(),
                    actual_last_tick,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_wait_for_blocked_node_windows(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_windows = plan.expected_parallel_wait_for_blocked_node_windows();
    if expected_windows.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_windows[0];
        return Err(WorkloadError::MissingParallelDiagnosticSummary {
            scope: expected.scope(),
        });
    };

    for expected in expected_windows {
        summary.validate_parallel_diagnostic_scope_summary(expected.scope())?;
        let actual_window = expected.actual_window(summary);
        let actual_edge_count = actual_window
            .as_ref()
            .map(|window| window.edge_count())
            .unwrap_or(0);
        let actual_first_tick = actual_window.as_ref().map(|window| window.first_tick());
        let actual_last_tick = actual_window.as_ref().map(|window| window.last_tick());
        if actual_edge_count != expected.edge_count()
            || actual_first_tick != Some(expected.first_tick())
            || actual_last_tick != Some(expected.last_tick())
        {
            return Err(
                WorkloadError::ExpectedParallelWaitForBlockedNodeWindowMismatch {
                    scope: expected.scope(),
                    node: expected.node().clone(),
                    expected_edge_count: expected.edge_count(),
                    actual_edge_count,
                    expected_first_tick: expected.first_tick(),
                    actual_first_tick,
                    expected_last_tick: expected.last_tick(),
                    actual_last_tick,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_wait_for_target_node_windows(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_windows = plan.expected_parallel_wait_for_target_node_windows();
    if expected_windows.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_windows[0];
        return Err(WorkloadError::MissingParallelDiagnosticSummary {
            scope: expected.scope(),
        });
    };

    for expected in expected_windows {
        summary.validate_parallel_diagnostic_scope_summary(expected.scope())?;
        let actual_window = expected.actual_window(summary);
        let actual_edge_count = actual_window
            .as_ref()
            .map(|window| window.edge_count())
            .unwrap_or(0);
        let actual_first_tick = actual_window.as_ref().map(|window| window.first_tick());
        let actual_last_tick = actual_window.as_ref().map(|window| window.last_tick());
        if actual_edge_count != expected.edge_count()
            || actual_first_tick != Some(expected.first_tick())
            || actual_last_tick != Some(expected.last_tick())
        {
            return Err(
                WorkloadError::ExpectedParallelWaitForTargetNodeWindowMismatch {
                    scope: expected.scope(),
                    node: expected.node().clone(),
                    expected_edge_count: expected.edge_count(),
                    actual_edge_count,
                    expected_first_tick: expected.first_tick(),
                    actual_first_tick,
                    expected_last_tick: expected.last_tick(),
                    actual_last_tick,
                },
            );
        }
    }
    Ok(())
}

fn actual_livelock_subject_set(
    scope: WorkloadParallelDiagnosticScope,
    summary: &WorkloadParallelExecutionSummary,
) -> BTreeSet<String> {
    match scope {
        WorkloadParallelDiagnosticScope::DataCache => summary
            .data_cache_parallel_scheduler_livelock_diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.subject().to_string())
            .collect(),
        WorkloadParallelDiagnosticScope::FullSystem => summary
            .full_system_livelock_diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.subject().to_string())
            .collect(),
        WorkloadParallelDiagnosticScope::Resource
        | WorkloadParallelDiagnosticScope::Compute
        | WorkloadParallelDiagnosticScope::Dma => BTreeSet::new(),
    }
}

fn transition_threshold_livelock_subjects(
    scope: WorkloadParallelDiagnosticScope,
    threshold: u64,
    summary: &WorkloadParallelExecutionSummary,
) -> BTreeSet<String> {
    match scope {
        WorkloadParallelDiagnosticScope::DataCache => summary
            .data_cache_parallel_scheduler_progress_transition_subject_summaries()
            .into_iter()
            .filter_map(|(subject, count, _, _)| {
                ((count as u64) >= threshold).then(|| subject.to_string())
            })
            .collect(),
        WorkloadParallelDiagnosticScope::FullSystem => summary
            .full_system_progress_transition_subject_summaries()
            .into_iter()
            .filter_map(|(subject, count, _, _)| {
                ((count as u64) >= threshold).then(|| subject.to_string())
            })
            .collect(),
        WorkloadParallelDiagnosticScope::Resource
        | WorkloadParallelDiagnosticScope::Compute
        | WorkloadParallelDiagnosticScope::Dma => BTreeSet::new(),
    }
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

pub(crate) fn verify_expected_fabric_lane_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_fabric_lane_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_activity[0];
        return Err(missing_fabric_lane_activity_summary(expected));
    };

    for expected in expected_activity {
        let Some(actual) =
            summary.fabric_lane_activity(expected.link(), expected.virtual_network())
        else {
            return Err(missing_fabric_lane_activity_summary(expected));
        };
        if expected.below_minimum(&actual) {
            return Err(WorkloadError::ExpectedFabricLaneActivityBelowMinimum {
                link: expected.link().clone(),
                virtual_network: expected.virtual_network(),
                minimum_transfer_count: expected.minimum_transfer_count(),
                actual_transfer_count: actual.transfer_count(),
                minimum_byte_count: expected.minimum_byte_count(),
                actual_byte_count: actual.byte_count(),
                minimum_occupied_ticks: expected.minimum_occupied_ticks(),
                actual_occupied_ticks: actual.occupied_ticks(),
                minimum_queue_delay_ticks: expected.minimum_queue_delay_ticks(),
                actual_queue_delay_ticks: actual.queue_delay_ticks(),
                minimum_max_queue_delay_ticks: expected.minimum_max_queue_delay_ticks(),
                actual_max_queue_delay_ticks: actual.max_queue_delay_ticks(),
                required_first_tick: expected.required_first_tick(),
                actual_first_tick: actual.first_tick(),
                required_last_tick: expected.required_last_tick(),
                actual_last_tick: actual.last_tick(),
            });
        }
        if let Some((maximum_queue_delay_ticks, maximum_max_queue_delay_ticks)) =
            expected.queue_delay_budget()
        {
            if expected.above_maximum(&actual) {
                return Err(WorkloadError::ExpectedFabricLaneActivityAboveMaximum {
                    link: expected.link().clone(),
                    virtual_network: expected.virtual_network(),
                    maximum_queue_delay_ticks,
                    actual_queue_delay_ticks: actual.queue_delay_ticks(),
                    maximum_max_queue_delay_ticks,
                    actual_max_queue_delay_ticks: actual.max_queue_delay_ticks(),
                });
            }
        }
    }
    Ok(())
}

fn missing_fabric_lane_activity_summary(
    expected: &crate::WorkloadExpectedFabricLaneActivity,
) -> WorkloadError {
    WorkloadError::MissingFabricLaneActivitySummary {
        link: expected.link().clone(),
        virtual_network: expected.virtual_network(),
        minimum_transfer_count: expected.minimum_transfer_count(),
        minimum_byte_count: expected.minimum_byte_count(),
        minimum_occupied_ticks: expected.minimum_occupied_ticks(),
        minimum_queue_delay_ticks: expected.minimum_queue_delay_ticks(),
        minimum_max_queue_delay_ticks: expected.minimum_max_queue_delay_ticks(),
        required_first_tick: expected.required_first_tick(),
        required_last_tick: expected.required_last_tick(),
    }
}

pub(crate) fn verify_expected_fabric_link_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_fabric_link_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_activity[0];
        return Err(missing_fabric_link_activity_summary(expected));
    };

    for expected in expected_activity {
        let Some(actual) = summary.fabric_link_activity(expected.link()) else {
            return Err(missing_fabric_link_activity_summary(expected));
        };
        if expected.below_minimum(&actual) {
            return Err(WorkloadError::ExpectedFabricLinkActivityBelowMinimum {
                link: expected.link().clone(),
                minimum_transfer_count: expected.minimum_transfer_count(),
                actual_transfer_count: actual.transfer_count(),
                minimum_active_virtual_network_count: expected
                    .minimum_active_virtual_network_count(),
                actual_active_virtual_network_count: actual.active_virtual_network_count(),
                minimum_queue_delay_ticks: expected.minimum_queue_delay_ticks(),
                actual_queue_delay_ticks: actual.queue_delay_ticks(),
                minimum_contended_virtual_network_count: expected
                    .minimum_contended_virtual_network_count(),
                actual_contended_virtual_network_count: actual.contended_virtual_network_count(),
                required_first_tick: expected.required_first_tick(),
                actual_first_tick: actual.first_tick(),
                required_last_tick: expected.required_last_tick(),
                actual_last_tick: actual.last_tick(),
            });
        }
        if let Some((maximum_queue_delay_ticks, maximum_max_queue_delay_ticks)) =
            expected.queue_delay_budget()
        {
            if expected.above_maximum(&actual) {
                return Err(WorkloadError::ExpectedFabricLinkActivityAboveMaximum {
                    link: expected.link().clone(),
                    maximum_queue_delay_ticks,
                    actual_queue_delay_ticks: actual.queue_delay_ticks(),
                    maximum_max_queue_delay_ticks,
                    actual_max_queue_delay_ticks: actual.max_queue_delay_ticks(),
                });
            }
        }
    }
    Ok(())
}

fn missing_fabric_link_activity_summary(
    expected: &crate::WorkloadExpectedFabricLinkActivity,
) -> WorkloadError {
    WorkloadError::MissingFabricLinkActivitySummary {
        link: expected.link().clone(),
        minimum_transfer_count: expected.minimum_transfer_count(),
        minimum_active_virtual_network_count: expected.minimum_active_virtual_network_count(),
        minimum_queue_delay_ticks: expected.minimum_queue_delay_ticks(),
        minimum_contended_virtual_network_count: expected.minimum_contended_virtual_network_count(),
        required_first_tick: expected.required_first_tick(),
        required_last_tick: expected.required_last_tick(),
    }
}

pub(crate) fn verify_expected_fabric_virtual_network_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_fabric_virtual_network_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = &expected_activity[0];
        return Err(missing_fabric_virtual_network_activity_summary(expected));
    };

    for expected in expected_activity {
        let Some(actual) = summary.fabric_virtual_network_activity(expected.virtual_network())
        else {
            return Err(missing_fabric_virtual_network_activity_summary(expected));
        };
        if expected.below_minimum(&actual) {
            return Err(
                WorkloadError::ExpectedFabricVirtualNetworkActivityBelowMinimum {
                    virtual_network: expected.virtual_network(),
                    minimum_transfer_count: expected.minimum_transfer_count(),
                    actual_transfer_count: actual.transfer_count(),
                    minimum_active_lane_count: expected.minimum_active_lane_count(),
                    actual_active_lane_count: actual.active_lane_count(),
                    minimum_queue_delay_ticks: expected.minimum_queue_delay_ticks(),
                    actual_queue_delay_ticks: actual.queue_delay_ticks(),
                    minimum_contended_lane_count: expected.minimum_contended_lane_count(),
                    actual_contended_lane_count: actual.contended_lane_count(),
                    required_first_tick: expected.required_first_tick(),
                    actual_first_tick: actual.first_tick(),
                    required_last_tick: expected.required_last_tick(),
                    actual_last_tick: actual.last_tick(),
                },
            );
        }
        if let Some((maximum_queue_delay_ticks, maximum_max_queue_delay_ticks)) =
            expected.queue_delay_budget()
        {
            if expected.above_maximum(&actual) {
                return Err(
                    WorkloadError::ExpectedFabricVirtualNetworkActivityAboveMaximum {
                        virtual_network: expected.virtual_network(),
                        maximum_queue_delay_ticks,
                        actual_queue_delay_ticks: actual.queue_delay_ticks(),
                        maximum_max_queue_delay_ticks,
                        actual_max_queue_delay_ticks: actual.max_queue_delay_ticks(),
                    },
                );
            }
        }
        if let Some((maximum_active_lane_count, maximum_contended_lane_count)) =
            expected.lane_budget()
        {
            if expected.above_lane_budget(&actual) {
                return Err(
                    WorkloadError::ExpectedFabricVirtualNetworkActivityAboveLaneBudget {
                        virtual_network: expected.virtual_network(),
                        maximum_active_lane_count,
                        actual_active_lane_count: actual.active_lane_count(),
                        maximum_contended_lane_count,
                        actual_contended_lane_count: actual.contended_lane_count(),
                    },
                );
            }
        }
        if !expected.required_links().is_empty() {
            let actual_links = summary.fabric_virtual_network_links(expected.virtual_network());
            if actual_links.is_empty() {
                return Err(WorkloadError::MissingFabricVirtualNetworkLinkCoverage {
                    virtual_network: expected.virtual_network(),
                    required_links: expected.required_links().to_vec(),
                });
            }
            let actual_link_set = actual_links.iter().collect::<BTreeSet<_>>();
            let missing_links = expected
                .required_links()
                .iter()
                .filter(|link| !actual_link_set.contains(link))
                .cloned()
                .collect::<Vec<_>>();
            if !missing_links.is_empty() {
                return Err(
                    WorkloadError::ExpectedFabricVirtualNetworkLinkCoverageMissing {
                        virtual_network: expected.virtual_network(),
                        required_links: expected.required_links().to_vec(),
                        actual_links,
                        missing_links,
                    },
                );
            }
        }
    }
    Ok(())
}

fn missing_fabric_virtual_network_activity_summary(
    expected: &crate::WorkloadExpectedFabricVirtualNetworkActivity,
) -> WorkloadError {
    WorkloadError::MissingFabricVirtualNetworkActivitySummary {
        virtual_network: expected.virtual_network(),
        minimum_transfer_count: expected.minimum_transfer_count(),
        minimum_active_lane_count: expected.minimum_active_lane_count(),
        minimum_queue_delay_ticks: expected.minimum_queue_delay_ticks(),
        minimum_contended_lane_count: expected.minimum_contended_lane_count(),
        required_first_tick: expected.required_first_tick(),
        required_last_tick: expected.required_last_tick(),
    }
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
        validate_scheduler_scope_summary(summary, expected.scope())?;
        validate_scheduler_scope_batch_timeline_evidence(summary, expected.scope())?;
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
        validate_worker_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_worker_scope_batch_activity_evidence(
            summary,
            expected.scope(),
            expected.minimum_worker_count(),
        )?;
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

pub(crate) fn verify_expected_parallel_batch_worker_buckets(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_buckets = plan.expected_parallel_batch_worker_buckets();
    if expected_buckets.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_buckets[0];
        return Err(WorkloadError::MissingParallelBatchWorkerBucketSummary {
            scope: expected.scope(),
            worker_count: expected.worker_count(),
            minimum_batch_count: expected.minimum_batch_count(),
        });
    };

    for expected in expected_buckets {
        validate_worker_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_worker_scope_batch_worker_count_evidence(
            summary,
            expected.scope(),
            expected.worker_count(),
        )?;
        let actual_batch_count = expected.actual_batch_count(summary);
        if actual_batch_count < expected.minimum_batch_count() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerBucketBelowMinimum {
                    scope: expected.scope(),
                    worker_count: expected.worker_count(),
                    minimum_batch_count: expected.minimum_batch_count(),
                    actual_batch_count,
                },
            );
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
        validate_partition_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_partition_scope_batch_partition_set_evidence(
            summary,
            expected.scope(),
            expected.partitions(),
        )?;
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
        validate_partition_scope_batch_timeline_evidence(summary, expected.scope())?;
        validate_partition_scope_batch_partition_streak_evidence(
            summary,
            expected.scope(),
            expected.partitions(),
        )?;
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
        validate_scheduler_scope_summary(summary, expected.scope())?;
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
