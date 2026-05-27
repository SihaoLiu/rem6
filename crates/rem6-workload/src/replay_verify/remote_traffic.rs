use std::collections::BTreeMap;

use rem6_kernel::{ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId, Tick};

use super::{first_unmatched_actual, PARALLEL_REMOTE_FLOW_SCOPES};
use crate::{
    WorkloadError, WorkloadExpectedParallelRemoteFlow, WorkloadExpectedParallelRemoteFlowTiming,
    WorkloadExpectedParallelRemoteSend, WorkloadParallelBatchPartitionScope,
    WorkloadParallelExecutionSummary, WorkloadParallelRemoteFlowScope,
    WorkloadParallelRemoteTrafficConsistencyMismatch, WorkloadReplayPlan, WorkloadResult,
};

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

    for scope in PARALLEL_REMOTE_FLOW_SCOPES {
        let expected_for_scope = expected_parallel_remote_sends_for_scope(expected_sends, scope);
        if !expected_for_scope.is_empty() {
            validate_remote_send_scope_evidence(summary, scope)?;
        }
    }
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
        WorkloadParallelRemoteFlowScope::GpuDmaScheduler => {
            summary.gpu_dma_scheduler_remote_sends().to_vec()
        }
        WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
            summary.accelerator_dma_scheduler_remote_sends().to_vec()
        }
        WorkloadParallelRemoteFlowScope::DmaScheduler => summary.dma_scheduler_remote_sends(),
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

    for scope in PARALLEL_REMOTE_FLOW_SCOPES {
        let expected_for_scope = expected_parallel_remote_flows_for_scope(expected_flows, scope);
        if !expected_for_scope.is_empty() {
            validate_remote_flow_scope_evidence(summary, scope)?;
        }
    }
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
        WorkloadParallelRemoteFlowScope::GpuDmaScheduler => {
            summary.gpu_dma_scheduler_remote_flow_evidence()
        }
        WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
            summary.accelerator_dma_scheduler_remote_flow_evidence()
        }
        WorkloadParallelRemoteFlowScope::DmaScheduler => summary.dma_scheduler_remote_flows(),
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
        validate_remote_flow_scope_evidence(summary, expected.scope())?;
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
        validate_remote_flow_scope_evidence(summary, expected.scope())?;
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
        validate_raw_explicit_remote_flow_scope_evidence(summary, expected.scope())?;
        let flows = explicit_parallel_remote_flows_for_scope(summary, expected.scope());
        let sends = actual_parallel_remote_sends_for_scope(summary, expected.scope());
        for flow in &flows {
            validate_remote_traffic_flow_evidence(expected.scope(), *flow)?;
        }
        for send in &sends {
            validate_remote_traffic_send_evidence(expected.scope(), *send)?;
        }
        let send_observations = remote_send_observations(&sends);
        for flow in &flows {
            let flow_record = *flow;
            let Some(send_observation) =
                send_observations.get(&(flow_record.source(), flow_record.target()))
            else {
                return Err(remote_traffic_missing_sends_mismatch(
                    expected.scope(),
                    flow_record,
                ));
            };
            if !remote_traffic_observation_matches(flow_record, *send_observation) {
                return Err(remote_traffic_consistency_mismatch(
                    expected.scope(),
                    flow_record,
                    *send_observation,
                ));
            }
        }
        if !flows.is_empty() {
            for ((source, target), send_observation) in &send_observations {
                if !flows
                    .iter()
                    .any(|flow| flow.source() == *source && flow.target() == *target)
                {
                    return Err(remote_traffic_missing_flow_mismatch(
                        expected.scope(),
                        *source,
                        *target,
                        *send_observation,
                    ));
                }
            }
        }
    }
    Ok(())
}

fn validate_remote_send_scope_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelRemoteFlowScope,
) -> Result<(), WorkloadError> {
    let sends = actual_parallel_remote_sends_for_scope(summary, scope);
    for send in sends {
        validate_remote_traffic_send_evidence(scope, send)?;
    }
    Ok(())
}

pub(crate) fn validate_remote_partition_scope_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelBatchPartitionScope,
) -> Result<(), WorkloadError> {
    let Some(scope) = remote_scope_for_partition_scope(scope) else {
        return Ok(());
    };
    validate_remote_flow_scope_evidence(summary, scope)
}

fn remote_scope_for_partition_scope(
    scope: WorkloadParallelBatchPartitionScope,
) -> Option<WorkloadParallelRemoteFlowScope> {
    match scope {
        WorkloadParallelBatchPartitionScope::Scheduler => {
            Some(WorkloadParallelRemoteFlowScope::Scheduler)
        }
        WorkloadParallelBatchPartitionScope::DataCacheScheduler => {
            Some(WorkloadParallelRemoteFlowScope::DataCacheScheduler)
        }
        WorkloadParallelBatchPartitionScope::GpuDmaScheduler => {
            Some(WorkloadParallelRemoteFlowScope::GpuDmaScheduler)
        }
        WorkloadParallelBatchPartitionScope::AcceleratorDmaScheduler => {
            Some(WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler)
        }
        WorkloadParallelBatchPartitionScope::DmaScheduler => {
            Some(WorkloadParallelRemoteFlowScope::DmaScheduler)
        }
        WorkloadParallelBatchPartitionScope::FullSystem => {
            Some(WorkloadParallelRemoteFlowScope::FullSystem)
        }
        WorkloadParallelBatchPartitionScope::PlannedScheduler
        | WorkloadParallelBatchPartitionScope::PlannedDataCacheScheduler
        | WorkloadParallelBatchPartitionScope::PlannedGpuDmaScheduler
        | WorkloadParallelBatchPartitionScope::PlannedAcceleratorDmaScheduler
        | WorkloadParallelBatchPartitionScope::PlannedDmaScheduler
        | WorkloadParallelBatchPartitionScope::PlannedFullSystem => None,
    }
}

fn validate_remote_flow_scope_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelRemoteFlowScope,
) -> Result<(), WorkloadError> {
    validate_remote_send_scope_evidence(summary, scope)?;
    validate_raw_explicit_remote_flow_scope_evidence(summary, scope)?;
    if scope == WorkloadParallelRemoteFlowScope::FullSystem {
        validate_raw_full_system_remote_flow_evidence(summary)?;
        validate_full_system_remote_flow_merge_summary(summary)?;
    }
    let flows = actual_parallel_remote_flows_for_scope(summary, scope);
    for flow in flows {
        validate_remote_traffic_flow_evidence(scope, flow)?;
    }
    Ok(())
}

fn validate_raw_explicit_remote_flow_scope_evidence(
    summary: &WorkloadParallelExecutionSummary,
    scope: WorkloadParallelRemoteFlowScope,
) -> Result<(), WorkloadError> {
    let explicit_flows = raw_explicit_parallel_remote_flows_for_scope(summary, scope);
    for flow in explicit_flows {
        validate_remote_traffic_flow_evidence(scope, flow)?;
    }
    Ok(())
}

fn validate_raw_full_system_remote_flow_evidence(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    for flow in summary.raw_full_system_parallel_scheduler_remote_flows() {
        validate_remote_traffic_flow_evidence(WorkloadParallelRemoteFlowScope::FullSystem, *flow)?;
    }
    Ok(())
}

fn validate_full_system_remote_flow_merge_summary(
    summary: &WorkloadParallelExecutionSummary,
) -> Result<(), WorkloadError> {
    let merged_flows = summary.explicit_full_system_parallel_scheduler_remote_flow_evidence();
    if merged_flows.is_empty() {
        return Ok(());
    }

    let merged_by_route = merged_flows
        .into_iter()
        .map(|flow| ((flow.source(), flow.target()), flow))
        .collect::<BTreeMap<_, _>>();
    for scoped_flow in summary.scoped_full_system_parallel_scheduler_remote_flow_evidence() {
        let route = (scoped_flow.source(), scoped_flow.target());
        let Some(merged_flow) = merged_by_route.get(&route).copied() else {
            continue;
        };
        if !remote_flow_covers_scoped_flow(merged_flow, scoped_flow) {
            return Err(WorkloadError::InvalidParallelRemoteFlowMergeSummary {
                scope: WorkloadParallelRemoteFlowScope::FullSystem,
                source: scoped_flow.source().index(),
                target: scoped_flow.target().index(),
                merged_send_count: merged_flow.send_count(),
                scoped_send_count: scoped_flow.send_count(),
                merged_first_tick: Some(merged_flow.first_tick()),
                scoped_first_tick: scoped_flow.first_tick(),
                merged_last_tick: Some(merged_flow.last_tick()),
                scoped_last_tick: scoped_flow.last_tick(),
                merged_minimum_delay: merged_flow.minimum_delay(),
                scoped_minimum_delay: scoped_flow.minimum_delay(),
                merged_maximum_delay: merged_flow.maximum_delay(),
                scoped_maximum_delay: scoped_flow.maximum_delay(),
            });
        }
    }
    Ok(())
}

fn remote_flow_covers_scoped_flow(
    merged_flow: ParallelRemoteFlowRecord,
    scoped_flow: ParallelRemoteFlowRecord,
) -> bool {
    merged_flow.send_count() >= scoped_flow.send_count()
        && merged_flow.first_tick() <= scoped_flow.first_tick()
        && merged_flow.last_tick() >= scoped_flow.last_tick()
        && remote_flow_delay_covers_scoped_flow(merged_flow, scoped_flow)
}

fn remote_flow_delay_covers_scoped_flow(
    merged_flow: ParallelRemoteFlowRecord,
    scoped_flow: ParallelRemoteFlowRecord,
) -> bool {
    match (merged_flow.delay_bounds(), scoped_flow.delay_bounds()) {
        (_, None) => true,
        (
            Some((merged_minimum_delay, merged_maximum_delay)),
            Some((scoped_minimum_delay, scoped_maximum_delay)),
        ) => {
            merged_minimum_delay <= scoped_minimum_delay
                && merged_maximum_delay >= scoped_maximum_delay
        }
        (None, Some(_)) => false,
    }
}

fn validate_remote_traffic_flow_evidence(
    scope: WorkloadParallelRemoteFlowScope,
    flow: ParallelRemoteFlowRecord,
) -> Result<(), WorkloadError> {
    if flow.source() == flow.target() {
        return Err(WorkloadError::InvalidParallelRemoteTrafficFlowEndpoints {
            scope,
            source: flow.source().index(),
            target: flow.target().index(),
            send_count: flow.send_count(),
            first_tick: flow.first_tick(),
            last_tick: flow.last_tick(),
        });
    }
    if flow.first_tick() > flow.last_tick() {
        return Err(WorkloadError::InvalidParallelRemoteTrafficFlowTiming {
            scope,
            source: flow.source().index(),
            target: flow.target().index(),
            send_count: flow.send_count(),
            first_tick: flow.first_tick(),
            last_tick: flow.last_tick(),
        });
    }
    if let Some((minimum_delay, maximum_delay)) = flow.delay_bounds() {
        if minimum_delay > maximum_delay {
            return Err(WorkloadError::InvalidParallelRemoteTrafficFlowDelayBounds {
                scope,
                source: flow.source().index(),
                target: flow.target().index(),
                minimum_delay,
                maximum_delay,
            });
        }
    }
    Ok(())
}

fn validate_remote_traffic_send_evidence(
    scope: WorkloadParallelRemoteFlowScope,
    send: ParallelRemoteSendRecord,
) -> Result<(), WorkloadError> {
    if send.source() == send.target() {
        return Err(WorkloadError::InvalidParallelRemoteTrafficSendEndpoints {
            scope,
            source: send.source().index(),
            target: send.target().index(),
            source_tick: send.source_tick(),
            delivery_tick: send.delivery_tick(),
            order: send.order(),
        });
    }
    if send.delivery_tick() < send.source_tick() {
        return Err(WorkloadError::InvalidParallelRemoteTrafficSendTiming {
            scope,
            source: send.source().index(),
            target: send.target().index(),
            source_tick: send.source_tick(),
            delivery_tick: send.delivery_tick(),
            order: send.order(),
        });
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
        WorkloadParallelRemoteFlowScope::GpuDmaScheduler => {
            summary.gpu_dma_scheduler_remote_flows().to_vec()
        }
        WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
            summary.accelerator_dma_scheduler_remote_flows().to_vec()
        }
        WorkloadParallelRemoteFlowScope::DmaScheduler => merge_explicit_parallel_remote_flows(
            summary
                .gpu_dma_scheduler_remote_flows()
                .iter()
                .copied()
                .chain(
                    summary
                        .accelerator_dma_scheduler_remote_flows()
                        .iter()
                        .copied(),
                ),
        ),
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
                )
                .chain(summary.gpu_dma_scheduler_remote_flows().iter().copied())
                .chain(
                    summary
                        .accelerator_dma_scheduler_remote_flows()
                        .iter()
                        .copied(),
                ),
        ),
    }
}

fn raw_explicit_parallel_remote_flows_for_scope(
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
        WorkloadParallelRemoteFlowScope::GpuDmaScheduler => {
            summary.gpu_dma_scheduler_remote_flows().to_vec()
        }
        WorkloadParallelRemoteFlowScope::AcceleratorDmaScheduler => {
            summary.accelerator_dma_scheduler_remote_flows().to_vec()
        }
        WorkloadParallelRemoteFlowScope::DmaScheduler => summary
            .gpu_dma_scheduler_remote_flows()
            .iter()
            .copied()
            .chain(
                summary
                    .accelerator_dma_scheduler_remote_flows()
                    .iter()
                    .copied(),
            )
            .collect(),
        WorkloadParallelRemoteFlowScope::FullSystem => summary
            .parallel_scheduler_remote_flows()
            .iter()
            .copied()
            .chain(
                summary
                    .data_cache_parallel_scheduler_remote_flows()
                    .iter()
                    .copied(),
            )
            .chain(summary.gpu_dma_scheduler_remote_flows().iter().copied())
            .chain(
                summary
                    .accelerator_dma_scheduler_remote_flows()
                    .iter()
                    .copied(),
            )
            .collect(),
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

fn remote_traffic_missing_sends_mismatch(
    scope: WorkloadParallelRemoteFlowScope,
    flow: ParallelRemoteFlowRecord,
) -> WorkloadError {
    WorkloadError::ParallelRemoteTrafficConsistencyMismatch(Box::new(
        WorkloadParallelRemoteTrafficConsistencyMismatch {
            scope,
            source: flow.source().index(),
            target: flow.target().index(),
            flow_send_count: flow.send_count(),
            send_record_count: 0,
            flow_first_tick: flow.first_tick(),
            send_first_tick: None,
            flow_last_tick: flow.last_tick(),
            send_last_tick: None,
            flow_minimum_delay: flow.minimum_delay(),
            send_minimum_delay: None,
            flow_maximum_delay: flow.maximum_delay(),
            send_maximum_delay: None,
        },
    ))
}

fn remote_traffic_missing_flow_mismatch(
    scope: WorkloadParallelRemoteFlowScope,
    source: PartitionId,
    target: PartitionId,
    sends: RemoteSendObservation,
) -> WorkloadError {
    WorkloadError::MissingParallelRemoteTrafficAggregateFlow {
        scope,
        source: source.index(),
        target: target.index(),
        send_record_count: sends.send_count,
        send_first_tick: sends.first_tick,
        send_last_tick: sends.last_tick,
        send_minimum_delay: sends.minimum_delay,
        send_maximum_delay: sends.maximum_delay,
    }
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
        validate_remote_flow_scope_evidence(summary, expected.scope())?;
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

fn partition_indexes(partitions: &[PartitionId]) -> Vec<u32> {
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

    for scope in PARALLEL_REMOTE_FLOW_SCOPES {
        let expected_for_scope =
            expected_parallel_remote_flow_timings_for_scope(expected_timings, scope);
        if !expected_for_scope.is_empty() {
            validate_remote_flow_scope_evidence(summary, scope)?;
        }
    }
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
