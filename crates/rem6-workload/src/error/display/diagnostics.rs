use std::fmt;

use super::super::WorkloadError;

pub(super) fn format_diagnostic_error(
    error: &WorkloadError,
    formatter: &mut fmt::Formatter<'_>,
) -> fmt::Result {
    match error {
        WorkloadError::MissingParallelDiagnosticSummary { scope } => write!(
            formatter,
            "missing parallel summary for expected clean {} diagnostics",
            scope.as_str()
        ),
        WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
            scope,
            wait_for_edge_count,
            deadlock_diagnostic_count,
            livelock_diagnostic_count,
            livelock_subjects,
        } => {
            write!(
                formatter,
                "expected clean {} diagnostics, got {wait_for_edge_count} wait-for edges, {deadlock_diagnostic_count} deadlock diagnostics, and {livelock_diagnostic_count} livelock diagnostics",
                scope.as_str()
            )?;
            if !livelock_subjects.is_empty() {
                write!(
                    formatter,
                    " for livelock subjects {}",
                    livelock_subjects.join(", ")
                )?;
            }
            Ok(())
        }
        WorkloadError::ExpectedParallelWaitForEdgeKindCountBelowMinimum {
            scope,
            kind,
            minimum_edge_count,
            actual_edge_count,
        } => write!(
            formatter,
            "expected {} wait-for edge kind {kind:?} to reach at least {minimum_edge_count} edges, got {actual_edge_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelWaitForEdgeCountSummary {
            scope,
            wait_for_edge_count,
            evidence_edge_count,
        } => write!(
            formatter,
            "invalid {} wait-for summary: total edge count {wait_for_edge_count} is below typed evidence count {evidence_edge_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelWaitForEdgeKindWindowSummary {
            scope,
            kind,
            edge_kind_count,
            window_edge_count,
        } => write!(
            formatter,
            "invalid {} wait-for edge kind {kind:?} summary: kind count {edge_kind_count} is below exact window count {window_edge_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelWaitForEdgeKindCountMergeSummary {
            scope,
            kind,
            merged_edge_count,
            scoped_edge_count,
        } => write!(
            formatter,
            "invalid {} wait-for edge kind {kind:?} merge summary: merged edge count {merged_edge_count} is below scoped edge count {scoped_edge_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelWaitForEdgeKindWindowMergeSummary {
            scope,
            kind,
            merged_edge_count,
            scoped_edge_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} wait-for edge kind {kind:?} window merge summary: merged window {merged_edge_count} edges from tick {merged_first_tick} to {merged_last_tick} is weaker than scoped window {scoped_edge_count} edges from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelWaitForBlockedNodeWindowMergeSummary {
            scope,
            node,
            merged_edge_count,
            scoped_edge_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} wait-for blocked node {node} window merge summary: merged window {merged_edge_count} edges from tick {merged_first_tick} to {merged_last_tick} is weaker than scoped window {scoped_edge_count} edges from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelWaitForTargetNodeWindowMergeSummary {
            scope,
            node,
            merged_edge_count,
            scoped_edge_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} wait-for target node {node} window merge summary: merged window {merged_edge_count} edges from tick {merged_first_tick} to {merged_last_tick} is weaker than scoped window {scoped_edge_count} edges from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelDeadlockMergeSummary {
            scope,
            merged_diagnostic_count,
            scoped_diagnostic_count,
        } => write!(
            formatter,
            "invalid {} deadlock merge summary: merged diagnostic count {merged_diagnostic_count} is below scoped diagnostic count {scoped_diagnostic_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelLivelockDiagnosticCountSummary {
            scope,
            progress_transition_count,
            livelock_diagnostic_count,
        } => write!(
            formatter,
            "invalid {} livelock summary: diagnostic count {livelock_diagnostic_count} exceeds progress transition count {progress_transition_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelLivelockTransitionCountSummary {
            scope,
            progress_transition_count,
            evidence_transition_count,
        } => write!(
            formatter,
            "invalid {} livelock summary: progress transition count {progress_transition_count} is below diagnostic transition evidence count {evidence_transition_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelLivelockMergeSummary {
            scope,
            merged_evidence_count,
            scoped_evidence_count,
        } => write!(
            formatter,
            "invalid {} livelock merge summary: merged evidence count {merged_evidence_count} is below scoped evidence count {scoped_evidence_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelLivelockSubjectMergeSummary {
            scope,
            subject,
            merged_diagnostic_count,
            scoped_diagnostic_count,
            merged_transition_count,
            scoped_transition_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} livelock subject {subject} merge summary: merged diagnostics {merged_diagnostic_count} and transitions {merged_transition_count} from tick {} to {} are weaker than scoped diagnostics {scoped_diagnostic_count} and transitions {scoped_transition_count} from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str(),
            merged_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            merged_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        WorkloadError::InvalidParallelLivelockTransitionKindMergeSummary {
            scope,
            kind,
            merged_diagnostic_count,
            scoped_diagnostic_count,
            merged_transition_count,
            scoped_transition_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} livelock transition kind {kind:?} merge summary: merged diagnostics {merged_diagnostic_count} and transitions {merged_transition_count} from tick {} to {} are weaker than scoped diagnostics {scoped_diagnostic_count} and transitions {scoped_transition_count} from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str(),
            merged_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            merged_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        WorkloadError::InvalidParallelProgressTransitionMergeSummary {
            scope,
            merged_transition_count,
            scoped_transition_count,
        } => write!(
            formatter,
            "invalid {} progress-transition merge summary: merged transition count {merged_transition_count} is below scoped transition count {scoped_transition_count}",
            scope.as_str()
        ),
        WorkloadError::InvalidParallelProgressTransitionRecordMergeSummary {
            scope,
            partition,
            subject,
            kind,
            tick,
            order,
        } => write!(
            formatter,
            "invalid {} progress-transition record merge summary: scoped {kind:?} transition for {subject} on partition {} at tick {tick} with order {order} is missing from explicit merged evidence",
            scope.as_str(),
            partition.index(),
        ),
        WorkloadError::InvalidParallelProgressTransitionSubjectMergeSummary {
            scope,
            subject,
            merged_transition_count,
            scoped_transition_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} progress-transition subject {subject} merge summary: merged transitions {merged_transition_count} from tick {} to {} are weaker than scoped transitions {scoped_transition_count} from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str(),
            merged_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            merged_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        WorkloadError::InvalidParallelProgressTransitionKindMergeSummary {
            scope,
            kind,
            merged_transition_count,
            scoped_transition_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} progress-transition kind {kind:?} merge summary: merged transitions {merged_transition_count} from tick {} to {} are weaker than scoped transitions {scoped_transition_count} from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str(),
            merged_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            merged_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        WorkloadError::InvalidParallelProgressTransitionPartitionMergeSummary {
            scope,
            partition,
            merged_transition_count,
            scoped_transition_count,
            merged_first_tick,
            scoped_first_tick,
            merged_last_tick,
            scoped_last_tick,
        } => write!(
            formatter,
            "invalid {} progress-transition partition {} merge summary: merged transitions {merged_transition_count} from tick {} to {} are weaker than scoped transitions {scoped_transition_count} from tick {scoped_first_tick} to {scoped_last_tick}",
            scope.as_str(),
            partition.index(),
            merged_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            merged_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        WorkloadError::ExpectedParallelWaitForEdgeKindWindowMismatch {
            scope,
            kind,
            expected_edge_count,
            actual_edge_count,
            expected_first_tick,
            actual_first_tick,
            expected_last_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "expected {} wait-for edge kind {kind:?} window to have {expected_edge_count} edges from tick {expected_first_tick} to {expected_last_tick}, got {actual_edge_count} edges from tick {} to {}",
            scope.as_str(),
            actual_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            actual_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        WorkloadError::ExpectedParallelWaitForBlockedNodeWindowMismatch {
            scope,
            node,
            expected_edge_count,
            actual_edge_count,
            expected_first_tick,
            actual_first_tick,
            expected_last_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "expected {} wait-for blocked node {node} window to have {expected_edge_count} edges from tick {expected_first_tick} to {expected_last_tick}, got {actual_edge_count} edges from tick {} to {}",
            scope.as_str(),
            actual_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            actual_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        WorkloadError::ExpectedParallelWaitForTargetNodeWindowMismatch {
            scope,
            node,
            expected_edge_count,
            actual_edge_count,
            expected_first_tick,
            actual_first_tick,
            expected_last_tick,
            actual_last_tick,
        } => write!(
            formatter,
            "expected {} wait-for target node {node} window to have {expected_edge_count} edges from tick {expected_first_tick} to {expected_last_tick}, got {actual_edge_count} edges from tick {} to {}",
            scope.as_str(),
            actual_first_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
            actual_last_tick
                .map(|tick| tick.to_string())
                .unwrap_or_else(|| "missing".to_string()),
        ),
        _ => unreachable!("unsupported diagnostic error"),
    }
}
