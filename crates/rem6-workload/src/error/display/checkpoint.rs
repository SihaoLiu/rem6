use std::fmt;

use crate::WorkloadError;

pub(super) fn format_checkpoint_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::DuplicateExpectedCheckpointManifestSummary { label } => write!(
            formatter,
            "duplicate checkpoint manifest summary expectation for {label}"
        ),
        WorkloadError::DuplicateExpectedCheckpointRestoreManifestSummary { label } => write!(
            formatter,
            "duplicate checkpoint restore manifest summary expectation for {label}"
        ),
        WorkloadError::DuplicateExpectedCheckpointComponentSummary { label, component } => write!(
            formatter,
            "duplicate checkpoint component summary expectation for {label}:{component}"
        ),
        WorkloadError::DuplicateExpectedCheckpointRestoreComponentSummary { label, component } => {
            write!(
                formatter,
                "duplicate checkpoint restore component summary expectation for {label}:{component}"
            )
        }
        WorkloadError::MissingCheckpointManifestSummary { label } => write!(
            formatter,
            "checkpoint manifest summary for {label} was not recorded"
        ),
        WorkloadError::MissingCheckpointRestoreManifestSummary { label } => write!(
            formatter,
            "checkpoint restore manifest summary for {label} was not recorded"
        ),
        WorkloadError::MissingCheckpointComponentSummary { label, component } => write!(
            formatter,
            "checkpoint component summary for {label}:{component} was not recorded"
        ),
        WorkloadError::MissingCheckpointRestoreComponentSummary { label, component } => write!(
            formatter,
            "checkpoint restore component summary for {label}:{component} was not recorded"
        ),
        WorkloadError::MissingCheckpointComponentChunkSummary {
            label,
            component,
            chunk,
        } => write!(
            formatter,
            "checkpoint component summary for {label}:{component} did not record chunk {chunk}"
        ),
        WorkloadError::MissingCheckpointRestoreComponentChunkSummary {
            label,
            component,
            chunk,
        } => write!(
            formatter,
            "checkpoint restore component summary for {label}:{component} did not record chunk {chunk}"
        ),
        WorkloadError::CheckpointManifestSummaryBelowMinimum {
            label,
            minimum_component_count,
            actual_component_count,
            minimum_chunk_count,
            actual_chunk_count,
            minimum_payload_bytes,
            actual_payload_bytes,
        } => write!(
            formatter,
            "checkpoint manifest summary for {label} has components {actual_component_count}/{minimum_component_count}, chunks {actual_chunk_count}/{minimum_chunk_count}, payload bytes {actual_payload_bytes}/{minimum_payload_bytes}"
        ),
        WorkloadError::CheckpointRestoreManifestSummaryBelowMinimum {
            label,
            minimum_component_count,
            actual_component_count,
            minimum_chunk_count,
            actual_chunk_count,
            minimum_payload_bytes,
            actual_payload_bytes,
        } => write!(
            formatter,
            "checkpoint restore manifest summary for {label} has components {actual_component_count}/{minimum_component_count}, chunks {actual_chunk_count}/{minimum_chunk_count}, payload bytes {actual_payload_bytes}/{minimum_payload_bytes}"
        ),
        WorkloadError::CheckpointComponentSummaryBelowMinimum {
            label,
            component,
            minimum_chunk_count,
            actual_chunk_count,
            minimum_payload_bytes,
            actual_payload_bytes,
        } => write!(
            formatter,
            "checkpoint component summary for {label}:{component} has chunks {actual_chunk_count}/{minimum_chunk_count}, payload bytes {actual_payload_bytes}/{minimum_payload_bytes}"
        ),
        WorkloadError::CheckpointRestoreComponentSummaryBelowMinimum {
            label,
            component,
            minimum_chunk_count,
            actual_chunk_count,
            minimum_payload_bytes,
            actual_payload_bytes,
        } => write!(
            formatter,
            "checkpoint restore component summary for {label}:{component} has chunks {actual_chunk_count}/{minimum_chunk_count}, payload bytes {actual_payload_bytes}/{minimum_payload_bytes}"
        ),
        _ => unreachable!("checkpoint formatter called with non-checkpoint workload error"),
    }
}
