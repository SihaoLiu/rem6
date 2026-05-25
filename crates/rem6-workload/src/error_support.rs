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

impl Error for WorkloadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Boot(error) => Some(error),
            Self::Memory(error) => Some(error),
            _ => None,
        }
    }
}
