use std::error::Error;
use std::fmt;

use crate::error::WorkloadError;

pub(crate) fn format_partition_indexes(partitions: &[u32]) -> String {
    let values = partitions
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{values}]")
}

pub(crate) fn format_remote_traffic_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::ZeroExpectedParallelRemoteFlowCount {
            scope,
            source,
            target,
        } => write!(
            formatter,
            "expected {} remote flow {source}->{target} must have a positive send count",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelRemoteFlowEndpoints {
            scope,
            source,
            target,
        } => write!(
            formatter,
            "expected {} remote flow {source}->{target} must cross partitions",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelRemoteFlow {
            scope,
            source,
            target,
        } => write!(
            formatter,
            "expected {} remote flow {source}->{target} is already declared",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelRemoteSend {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} is already declared",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelRemoteSendEndpoints {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} must cross partitions",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelRemoteSendTiming {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} must not deliver before the source tick",
            scope.as_str()
        ),
        WorkloadError::MissingParallelExecutionSummary {
            scope,
            source,
            target,
            expected_send_count,
        } => write!(
            formatter,
            "missing parallel summary for expected {} remote flow {source}->{target} with {expected_send_count} sends",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelRemoteFlowCountMismatch {
            scope,
            source,
            target,
            expected_send_count,
            actual_send_count,
        } => write!(
            formatter,
            "expected {} remote flow {source}->{target} to have {expected_send_count} sends, got {actual_send_count}",
            scope.as_str()
        ),
        WorkloadError::UnexpectedParallelRemoteFlow {
            scope,
            source,
            target,
            actual_send_count,
        } => write!(
            formatter,
            "unexpected {} remote flow {source}->{target} with {actual_send_count} sends",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteSendSummary {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "missing parallel summary for expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order}",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelRemoteSendMissing {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "expected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} was not recorded",
            scope.as_str()
        ),
        WorkloadError::UnexpectedParallelRemoteSend {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "unexpected {} remote send {source}->{target} from tick {source_tick} to {delivery_tick} with order {order}",
            scope.as_str()
        ),
        _ => unreachable!("unsupported remote traffic error"),
    }
}

pub(crate) fn format_remote_endpoint_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::EmptyExpectedParallelRemoteEndpointSources { scope } => write!(
            formatter,
            "expected {} remote endpoints must declare at least one source partition",
            scope.as_str()
        ),
        WorkloadError::EmptyExpectedParallelRemoteEndpointTargets { scope } => write!(
            formatter,
            "expected {} remote endpoints must declare at least one target partition",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelRemoteEndpointOverlap {
            scope,
            source_partitions,
            target_partitions,
        } => write!(
            formatter,
            "expected {} remote endpoint source and target partitions must be disjoint, got {} and {}",
            scope.as_str(),
            format_partition_indexes(source_partitions),
            format_partition_indexes(target_partitions)
        ),
        WorkloadError::DuplicateExpectedParallelRemoteEndpoints { scope } => write!(
            formatter,
            "expected {} remote endpoints are already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteEndpointSummary {
            scope,
            expected_sources,
            expected_targets,
        } => write!(
            formatter,
            "missing parallel summary for expected {} remote endpoints from {} to {}",
            scope.as_str(),
            format_partition_indexes(expected_sources),
            format_partition_indexes(expected_targets)
        ),
        WorkloadError::ExpectedParallelRemoteEndpointsMismatch {
            scope,
            expected_sources,
            actual_sources,
            expected_targets,
            actual_targets,
        } => write!(
            formatter,
            "expected {} remote endpoints from {} to {}, got {} to {}",
            scope.as_str(),
            format_partition_indexes(expected_sources),
            format_partition_indexes(expected_targets),
            format_partition_indexes(actual_sources),
            format_partition_indexes(actual_targets)
        ),
        _ => unreachable!("unsupported remote endpoint error"),
    }
}

pub(crate) fn format_remote_delay_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::ZeroExpectedParallelRemoteDelayFloor { scope } => write!(
            formatter,
            "expected {} remote delay floor must be positive",
            scope.as_str()
        ),
        WorkloadError::ZeroExpectedParallelRemoteDelayCeiling { scope } => write!(
            formatter,
            "expected {} remote delay ceiling must be positive",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelRemoteDelayWindow {
            scope,
            minimum_delay,
            maximum_delay,
        } => write!(
            formatter,
            "expected {} remote delay floor {minimum_delay} must not exceed ceiling {maximum_delay}",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelRemoteDelayFloor { scope } => write!(
            formatter,
            "expected {} remote delay floor is already declared",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelRemoteDelayCeiling { scope } => write!(
            formatter,
            "expected {} remote delay ceiling is already declared",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelRemoteTrafficConsistency { scope } => write!(
            formatter,
            "expected {} remote traffic consistency is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteDelayFloorSummary {
            scope,
            minimum_delay,
        } => write!(
            formatter,
            "missing parallel summary for expected {} remote delay floor {minimum_delay}",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteDelayEvidence {
            scope,
            minimum_delay,
        } => write!(
            formatter,
            "missing {} remote delay evidence for expected floor {minimum_delay}",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteFlowDelayEvidence {
            scope,
            source,
            target,
            minimum_delay,
        } => write!(
            formatter,
            "missing {} remote flow delay evidence for {source}->{target} against floor {minimum_delay}",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelRemoteDelayBelowFloor {
            scope,
            source,
            target,
            minimum_delay,
            actual_minimum_delay,
        } => write!(
            formatter,
            "expected {} remote flow {source}->{target} minimum delay at least {minimum_delay}, got {actual_minimum_delay}",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteDelayCeilingSummary {
            scope,
            maximum_delay,
        } => write!(
            formatter,
            "missing parallel summary for expected {} remote delay ceiling at most {maximum_delay}",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteDelayCeilingEvidence {
            scope,
            maximum_delay,
        } => write!(
            formatter,
            "missing remote delay evidence for expected {} remote delay ceiling at most {maximum_delay}",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteFlowMaximumDelayEvidence {
            scope,
            source,
            target,
            maximum_delay,
        } => write!(
            formatter,
            "missing maximum delay evidence for expected {} remote flow {source}->{target} delay at most {maximum_delay}",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelRemoteDelayAboveCeiling {
            scope,
            source,
            target,
            maximum_delay,
            actual_maximum_delay,
        } => write!(
            formatter,
            "expected {} remote flow {source}->{target} maximum delay at most {maximum_delay}, got {actual_maximum_delay}",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteTrafficConsistencySummary { scope } => write!(
            formatter,
            "missing parallel summary for expected {} remote traffic consistency",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelRemoteTrafficSendEndpoints {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "{} remote traffic send evidence {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} must cross partitions",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelRemoteTrafficSendTiming {
            scope,
            source,
            target,
            source_tick,
            delivery_tick,
            order,
        } => write!(
            formatter,
            "{} remote traffic send evidence {source}->{target} from tick {source_tick} to {delivery_tick} with order {order} must not deliver before the source tick",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteTrafficAggregateFlow {
            scope,
            source,
            target,
            send_record_count,
            send_first_tick,
            send_last_tick,
            send_minimum_delay,
            send_maximum_delay,
        } => write!(
            formatter,
            "missing {} aggregate remote flow for exact send evidence {source}->{target} with {send_record_count} records, ticks {send_first_tick} to {send_last_tick}, delay {send_minimum_delay} to {send_maximum_delay}",
            scope.as_str()
        ),
        WorkloadError::ParallelRemoteTrafficConsistencyMismatch(mismatch) => write!(
            formatter,
            "expected {} remote traffic {source}->{target} flow evidence count {flow_send_count}, ticks {flow_first_tick} to {flow_last_tick}, delay {} to {}; send evidence count {send_record_count}, ticks {} to {}, delay {} to {}",
            mismatch.scope.as_str(),
            format_optional_tick(mismatch.flow_minimum_delay),
            format_optional_tick(mismatch.flow_maximum_delay),
            format_optional_tick(mismatch.send_first_tick),
            format_optional_tick(mismatch.send_last_tick),
            format_optional_tick(mismatch.send_minimum_delay),
            format_optional_tick(mismatch.send_maximum_delay),
            source = mismatch.source,
            target = mismatch.target,
            flow_send_count = mismatch.flow_send_count,
            send_record_count = mismatch.send_record_count,
            flow_first_tick = mismatch.flow_first_tick,
            flow_last_tick = mismatch.flow_last_tick
        ),
        WorkloadError::InvalidExpectedParallelRemoteFlowTimingWindow {
            scope,
            source,
            target,
            first_tick,
            last_tick,
        } => write!(
            formatter,
            "expected {} remote flow timing {source}->{target} first tick {first_tick} is after last tick {last_tick}",
            scope.as_str()
        ),
        WorkloadError::InvalidExpectedParallelRemoteFlowDelayBounds {
            scope,
            source,
            target,
            minimum_delay,
            maximum_delay,
        } => write!(
            formatter,
            "expected {} remote flow timing {source}->{target} minimum delay {minimum_delay} is above maximum delay {maximum_delay}",
            scope.as_str()
        ),
        WorkloadError::DuplicateExpectedParallelRemoteFlowTiming {
            scope,
            source,
            target,
        } => write!(
            formatter,
            "expected {} remote flow timing {source}->{target} is already declared",
            scope.as_str()
        ),
        WorkloadError::MissingParallelRemoteFlowTimingSummary {
            scope,
            source,
            target,
            expected_send_count,
            expected_first_tick,
            expected_last_tick,
        } => write!(
            formatter,
            "missing parallel summary for expected {} remote flow timing {source}->{target} with {expected_send_count} sends from tick {expected_first_tick} to {expected_last_tick}",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelRemoteFlowTimingMismatch {
            scope,
            source,
            target,
            expected_send_count,
            actual_send_count,
            expected_first_tick,
            actual_first_tick,
            expected_last_tick,
            actual_last_tick,
        } => {
            let actual_first_tick = actual_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "none".to_string());
            let actual_last_tick = actual_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "none".to_string());
            write!(
                formatter,
                "expected {} remote flow timing {source}->{target} to have {expected_send_count} sends from tick {expected_first_tick} to {expected_last_tick}, got {actual_send_count} sends from tick {actual_first_tick} to {actual_last_tick}",
                scope.as_str()
            )
        }
        WorkloadError::UnexpectedParallelRemoteFlowTiming {
            scope,
            source,
            target,
            actual_send_count,
            actual_first_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "unexpected {} remote flow timing {source}->{target} with {actual_send_count} sends from tick {actual_first_tick} to {actual_last_tick}",
            scope.as_str()
        ),
        WorkloadError::ExpectedParallelRemoteFlowDelayBoundsMismatch {
            scope,
            source,
            target,
            expected_minimum_delay,
            actual_minimum_delay,
            expected_maximum_delay,
            actual_maximum_delay,
        } => {
            let actual_minimum_delay = actual_minimum_delay
                .map(|delay| delay.to_string())
                .unwrap_or_else(|| "none".to_string());
            let actual_maximum_delay = actual_maximum_delay
                .map(|delay| delay.to_string())
                .unwrap_or_else(|| "none".to_string());
            write!(
                formatter,
                "expected {} remote flow timing {source}->{target} delay bounds {expected_minimum_delay} to {expected_maximum_delay}, got {actual_minimum_delay} to {actual_maximum_delay}",
                scope.as_str()
            )
        }
        _ => unreachable!("unsupported remote delay error"),
    }
}

fn format_optional_tick(tick: Option<u64>) -> String {
    tick.map(|tick| tick.to_string())
        .unwrap_or_else(|| "none".to_string())
}

impl Error for WorkloadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Boot(error) => Some(error),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}
