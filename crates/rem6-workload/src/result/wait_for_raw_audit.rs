use std::collections::BTreeSet;

use crate::WorkloadError;

use super::{
    WorkloadWaitForBlockedNodeWindow, WorkloadWaitForEdgeKindWindow,
    WorkloadWaitForTargetNodeWindow,
};

pub(super) fn validate_unique_full_system_wait_for_edge_kind_windows(
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> Result<(), WorkloadError> {
    let mut seen = BTreeSet::new();
    for window in windows {
        if !window.is_empty() && !seen.insert(*window) {
            return Err(
                WorkloadError::DuplicateFullSystemWaitForEdgeKindWindowRecord {
                    kind: window.kind(),
                    edge_count: window.edge_count(),
                    first_tick: window.first_tick(),
                    last_tick: window.last_tick(),
                },
            );
        }
    }
    Ok(())
}

pub(super) fn validate_unique_full_system_wait_for_blocked_node_windows(
    windows: &[WorkloadWaitForBlockedNodeWindow],
) -> Result<(), WorkloadError> {
    let mut seen = BTreeSet::new();
    for window in windows {
        if !window.is_empty() && !seen.insert(window.clone()) {
            return Err(
                WorkloadError::DuplicateFullSystemWaitForBlockedNodeWindowRecord {
                    node: window.node().clone(),
                    edge_count: window.edge_count(),
                    first_tick: window.first_tick(),
                    last_tick: window.last_tick(),
                },
            );
        }
    }
    Ok(())
}

pub(super) fn validate_unique_full_system_wait_for_target_node_windows(
    windows: &[WorkloadWaitForTargetNodeWindow],
) -> Result<(), WorkloadError> {
    let mut seen = BTreeSet::new();
    for window in windows {
        if !window.is_empty() && !seen.insert(window.clone()) {
            return Err(
                WorkloadError::DuplicateFullSystemWaitForTargetNodeWindowRecord {
                    node: window.node().clone(),
                    edge_count: window.edge_count(),
                    first_tick: window.first_tick(),
                    last_tick: window.last_tick(),
                },
            );
        }
    }
    Ok(())
}
