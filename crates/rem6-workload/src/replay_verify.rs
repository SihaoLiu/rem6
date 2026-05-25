use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{
    ParallelProgressTransitionRecord, ParallelRemoteFlowRecord, ParallelRemoteSendRecord,
    PartitionId, Tick,
};

use crate::{
    parallel_batch_timeline_expectation::actual_parallel_batch_timeline_records, WorkloadError,
    WorkloadExpectedParallelBatchTimelineRecord, WorkloadExpectedParallelProgressTransition,
    WorkloadExpectedParallelRemoteFlow, WorkloadExpectedParallelRemoteFlowTiming,
    WorkloadExpectedParallelRemoteSend, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchTimelineScope, WorkloadParallelDiagnosticScope,
    WorkloadParallelExecutionSummary, WorkloadParallelProgressTransitionExpectationFailure,
    WorkloadParallelRemoteFlowScope, WorkloadParallelRemoteTrafficConsistencyMismatch,
    WorkloadReplayPlan, WorkloadResult,
};

const PARALLEL_REMOTE_FLOW_SCOPES: [WorkloadParallelRemoteFlowScope; 3] = [
    WorkloadParallelRemoteFlowScope::Scheduler,
    WorkloadParallelRemoteFlowScope::DataCacheScheduler,
    WorkloadParallelRemoteFlowScope::FullSystem,
];

const PARALLEL_BATCH_TIMELINE_SCOPES: [WorkloadParallelBatchTimelineScope; 5] = [
    WorkloadParallelBatchTimelineScope::Scheduler,
    WorkloadParallelBatchTimelineScope::DataCacheScheduler,
    WorkloadParallelBatchTimelineScope::GpuDmaScheduler,
    WorkloadParallelBatchTimelineScope::AcceleratorDmaScheduler,
    WorkloadParallelBatchTimelineScope::FullSystem,
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
    first_unmatched_actual(
        expected_sends,
        actual_sends.iter().copied(),
        |expected, actual| expected.matches_record(*actual),
    )
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

pub(crate) fn verify_expected_parallel_remote_delay_ceilings(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_ceilings = plan.expected_parallel_remote_delay_ceilings();
    if expected_ceilings.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_ceilings[0];
        return Err(WorkloadError::MissingParallelRemoteDelayCeilingSummary {
            scope: expected.scope(),
            maximum_delay: expected.maximum_delay(),
        });
    };

    for expected in expected_ceilings {
        let flows = actual_parallel_remote_flows_for_scope(summary, expected.scope());
        if flows.is_empty() {
            return Err(WorkloadError::MissingParallelRemoteDelayCeilingEvidence {
                scope: expected.scope(),
                maximum_delay: expected.maximum_delay(),
            });
        }
        for flow in flows {
            let Some(actual_maximum_delay) = flow.maximum_delay() else {
                return Err(
                    WorkloadError::MissingParallelRemoteFlowMaximumDelayEvidence {
                        scope: expected.scope(),
                        source: flow.source().index(),
                        target: flow.target().index(),
                        maximum_delay: expected.maximum_delay(),
                    },
                );
            };
            if actual_maximum_delay > expected.maximum_delay() {
                return Err(WorkloadError::ExpectedParallelRemoteDelayAboveCeiling {
                    scope: expected.scope(),
                    source: flow.source().index(),
                    target: flow.target().index(),
                    maximum_delay: expected.maximum_delay(),
                    actual_maximum_delay,
                });
            }
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_remote_traffic_consistency(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_consistency = plan.expected_parallel_remote_traffic_consistency();
    if expected_consistency.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        return Err(
            WorkloadError::MissingParallelRemoteTrafficConsistencySummary {
                scope: expected_consistency[0].scope(),
            },
        );
    };

    for expected in expected_consistency {
        let flows = explicit_parallel_remote_flows_for_scope(summary, expected.scope());
        let sends = actual_parallel_remote_sends_for_scope(summary, expected.scope());
        let send_observations = remote_send_observations(&sends);
        for flow in flows {
            let Some(send_observation) = send_observations.get(&(flow.source(), flow.target()))
            else {
                continue;
            };
            if !remote_traffic_observation_matches(flow, *send_observation) {
                return Err(remote_traffic_consistency_mismatch(
                    expected.scope(),
                    flow,
                    *send_observation,
                ));
            }
        }
    }
    Ok(())
}

fn explicit_parallel_remote_flows_for_scope(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelRemoteFlowScope,
) -> Vec<ParallelRemoteFlowRecord> {
    match scope {
        WorkloadParallelRemoteFlowScope::Scheduler => {
            summary.parallel_scheduler_remote_flows().to_vec()
        }
        WorkloadParallelRemoteFlowScope::DataCacheScheduler => summary
            .data_cache_parallel_scheduler_remote_flows()
            .to_vec(),
        WorkloadParallelRemoteFlowScope::FullSystem => merge_explicit_parallel_remote_flows(
            summary
                .parallel_scheduler_remote_flows()
                .iter()
                .copied()
                .chain(
                    summary
                        .data_cache_parallel_scheduler_remote_flows()
                        .iter()
                        .copied(),
                ),
        ),
    }
}

fn merge_explicit_parallel_remote_flows(
    flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
) -> Vec<ParallelRemoteFlowRecord> {
    let mut by_route = BTreeMap::<(PartitionId, PartitionId), ParallelRemoteFlowRecord>::new();
    for flow in flows {
        if flow.send_count() == 0 {
            continue;
        }
        by_route
            .entry((flow.source(), flow.target()))
            .and_modify(|stored| *stored = stored.merged_with(flow))
            .or_insert(flow);
    }
    by_route.into_values().collect()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct RemoteSendObservation {
    send_count: usize,
    first_tick: Tick,
    last_tick: Tick,
    minimum_delay: Tick,
    maximum_delay: Tick,
}

impl RemoteSendObservation {
    fn from_send(send: ParallelRemoteSendRecord) -> Self {
        let delay = send.delay();
        Self {
            send_count: 1,
            first_tick: send.delivery_tick(),
            last_tick: send.delivery_tick(),
            minimum_delay: delay,
            maximum_delay: delay,
        }
    }

    fn record_send(&mut self, send: ParallelRemoteSendRecord) {
        self.send_count += 1;
        self.first_tick = self.first_tick.min(send.delivery_tick());
        self.last_tick = self.last_tick.max(send.delivery_tick());
        let delay = send.delay();
        self.minimum_delay = self.minimum_delay.min(delay);
        self.maximum_delay = self.maximum_delay.max(delay);
    }
}

fn remote_send_observations(
    sends: &[ParallelRemoteSendRecord],
) -> BTreeMap<(PartitionId, PartitionId), RemoteSendObservation> {
    let mut observations = BTreeMap::new();
    for send in sends {
        observations
            .entry((send.source(), send.target()))
            .and_modify(|observation: &mut RemoteSendObservation| observation.record_send(*send))
            .or_insert_with(|| RemoteSendObservation::from_send(*send));
    }
    observations
}

fn remote_traffic_observation_matches(
    flow: ParallelRemoteFlowRecord,
    sends: RemoteSendObservation,
) -> bool {
    let timing_matches = flow.send_count() == sends.send_count
        && flow.first_tick() == sends.first_tick
        && flow.last_tick() == sends.last_tick;
    let delay_matches = match flow.delay_bounds() {
        Some((minimum_delay, maximum_delay)) => {
            minimum_delay == sends.minimum_delay && maximum_delay == sends.maximum_delay
        }
        None => true,
    };
    timing_matches && delay_matches
}

fn remote_traffic_consistency_mismatch(
    scope: WorkloadParallelRemoteFlowScope,
    flow: ParallelRemoteFlowRecord,
    sends: RemoteSendObservation,
) -> WorkloadError {
    WorkloadError::ParallelRemoteTrafficConsistencyMismatch(Box::new(
        WorkloadParallelRemoteTrafficConsistencyMismatch {
            scope,
            source: flow.source().index(),
            target: flow.target().index(),
            flow_send_count: flow.send_count(),
            send_record_count: sends.send_count,
            flow_first_tick: flow.first_tick(),
            send_first_tick: Some(sends.first_tick),
            flow_last_tick: flow.last_tick(),
            send_last_tick: Some(sends.last_tick),
            flow_minimum_delay: flow.minimum_delay(),
            send_minimum_delay: Some(sends.minimum_delay),
            flow_maximum_delay: flow.maximum_delay(),
            send_maximum_delay: Some(sends.maximum_delay),
        },
    ))
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
                livelock_subjects: actual_livelock_subjects(expected.scope(), summary),
            });
        }
    }
    Ok(())
}

fn actual_livelock_subjects(
    scope: WorkloadParallelDiagnosticScope,
    summary: &WorkloadParallelExecutionSummary,
) -> Vec<String> {
    match scope {
        WorkloadParallelDiagnosticScope::DataCache => summary
            .data_cache_parallel_scheduler_livelock_diagnostics()
            .iter()
            .map(|diagnostic| diagnostic.subject().to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        WorkloadParallelDiagnosticScope::FullSystem => summary
            .full_system_livelock_diagnostics()
            .into_iter()
            .map(|diagnostic| diagnostic.subject().to_string())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        WorkloadParallelDiagnosticScope::Resource
        | WorkloadParallelDiagnosticScope::Compute
        | WorkloadParallelDiagnosticScope::Dma => Vec::new(),
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

pub(crate) fn verify_expected_parallel_batch_worker_tick_buckets(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_buckets = plan.expected_parallel_batch_worker_tick_buckets();
    if expected_buckets.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_buckets[0];
        return Err(WorkloadError::MissingParallelBatchWorkerTickBucketSummary {
            scope: expected.scope(),
            worker_count: expected.worker_count(),
            minimum_ticks: expected.minimum_ticks(),
        });
    };

    for expected in expected_buckets {
        let actual_ticks = expected.actual_ticks(summary);
        if actual_ticks < expected.minimum_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTickBucketBelowMinimum {
                    scope: expected.scope(),
                    worker_count: expected.worker_count(),
                    minimum_ticks: expected.minimum_ticks(),
                    actual_ticks,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_worker_tick_activity(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_activity = plan.expected_parallel_batch_worker_tick_activity();
    if expected_activity.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_activity[0];
        return Err(
            WorkloadError::MissingParallelBatchWorkerTickActivitySummary {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
                minimum_ticks: expected.minimum_ticks(),
            },
        );
    };

    for expected in expected_activity {
        let actual_ticks = expected.actual_ticks(summary);
        if actual_ticks < expected.minimum_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTickActivityBelowMinimum {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                    minimum_ticks: expected.minimum_ticks(),
                    actual_ticks,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_worker_tick_streaks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_streaks = plan.expected_parallel_batch_worker_tick_streaks();
    if expected_streaks.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_streaks[0];
        return Err(WorkloadError::MissingParallelBatchWorkerTickStreakSummary {
            scope: expected.scope(),
            minimum_worker_count: expected.minimum_worker_count(),
            minimum_consecutive_ticks: expected.minimum_consecutive_ticks(),
        });
    };

    for expected in expected_streaks {
        let actual_consecutive_ticks = expected.actual_consecutive_ticks(summary);
        if actual_consecutive_ticks < expected.minimum_consecutive_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTickStreakBelowMinimum {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                    minimum_consecutive_ticks: expected.minimum_consecutive_ticks(),
                    actual_consecutive_ticks,
                },
            );
        }
    }
    Ok(())
}

pub(crate) fn verify_expected_parallel_batch_worker_ticks(
    plan: &WorkloadReplayPlan,
    result: &WorkloadResult,
) -> Result<(), WorkloadError> {
    let expected_worker_ticks = plan.expected_parallel_batch_worker_ticks();
    if expected_worker_ticks.is_empty() {
        return Ok(());
    }
    let Some(summary) = result.parallel_execution_summary() else {
        let expected = expected_worker_ticks[0];
        return Err(WorkloadError::MissingParallelBatchWorkerTicksSummary {
            scope: expected.scope(),
            minimum_worker_count: expected.minimum_worker_count(),
            minimum_worker_ticks: expected.minimum_worker_ticks(),
        });
    };

    for expected in expected_worker_ticks {
        let actual_worker_ticks = expected.actual_worker_ticks(summary);
        if actual_worker_ticks < expected.minimum_worker_ticks() {
            return Err(
                WorkloadError::ExpectedParallelBatchWorkerTicksBelowMinimum {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                    minimum_worker_ticks: expected.minimum_worker_ticks(),
                    actual_worker_ticks,
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
