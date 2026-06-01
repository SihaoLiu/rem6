use std::collections::BTreeMap;

use rem6_kernel::WaitForEdgeKind;

use crate::{WorkloadError, WorkloadParallelDiagnosticScope};

use super::wait_for_node_windows::{
    wait_for_blocked_node_window_count_sum, wait_for_target_node_window_count_sum,
};
use super::{
    WorkloadWaitForBlockedNodeWindow, WorkloadWaitForEdgeKindWindow,
    WorkloadWaitForTargetNodeWindow,
};

pub(super) fn collect_wait_for_edge_kind_counts(
    counts: impl IntoIterator<Item = (WaitForEdgeKind, usize)>,
) -> BTreeMap<WaitForEdgeKind, usize> {
    let mut by_kind = BTreeMap::new();
    for (kind, count) in counts {
        if count == 0 {
            continue;
        }
        let stored = by_kind.entry(kind).or_insert(0usize);
        *stored = stored.saturating_add(count);
    }
    by_kind
}

pub(super) fn validate_wait_for_edge_count_summary(
    scope: WorkloadParallelDiagnosticScope,
    wait_for_edge_count: usize,
    edge_kind_counts: &BTreeMap<WaitForEdgeKind, usize>,
    edge_kind_windows: &[WorkloadWaitForEdgeKindWindow],
    blocked_node_windows: &[WorkloadWaitForBlockedNodeWindow],
    target_node_windows: &[WorkloadWaitForTargetNodeWindow],
) -> Result<(), WorkloadError> {
    let evidence_edge_count = wait_for_edge_kind_count_sum(edge_kind_counts)
        .max(wait_for_edge_kind_window_count_sum(edge_kind_windows))
        .max(wait_for_blocked_node_window_count_sum(blocked_node_windows))
        .max(wait_for_target_node_window_count_sum(target_node_windows));
    if wait_for_edge_count < evidence_edge_count {
        return Err(WorkloadError::InvalidParallelWaitForEdgeCountSummary {
            scope,
            wait_for_edge_count,
            evidence_edge_count,
        });
    }
    Ok(())
}

pub(super) fn validate_wait_for_edge_kind_window_summary(
    scope: WorkloadParallelDiagnosticScope,
    counts: &BTreeMap<WaitForEdgeKind, usize>,
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> Result<(), WorkloadError> {
    for window in windows {
        let edge_kind_count = wait_for_edge_kind_count(counts, window.kind());
        if edge_kind_count < window.edge_count() {
            return Err(WorkloadError::InvalidParallelWaitForEdgeKindWindowSummary {
                scope,
                kind: window.kind(),
                edge_kind_count,
                window_edge_count: window.edge_count(),
            });
        }
    }
    Ok(())
}

pub(super) fn validate_wait_for_edge_kind_count_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &BTreeMap<WaitForEdgeKind, usize>,
    scoped: &BTreeMap<WaitForEdgeKind, usize>,
) -> Result<(), WorkloadError> {
    for (kind, scoped_edge_count) in scoped {
        let Some(merged_edge_count) = merged.get(kind).copied() else {
            continue;
        };
        if merged_edge_count < *scoped_edge_count {
            return Err(
                WorkloadError::InvalidParallelWaitForEdgeKindCountMergeSummary {
                    scope,
                    kind: *kind,
                    merged_edge_count,
                    scoped_edge_count: *scoped_edge_count,
                },
            );
        }
    }
    Ok(())
}

pub(super) fn validate_wait_for_edge_kind_window_merge_summary(
    scope: WorkloadParallelDiagnosticScope,
    merged: &[WorkloadWaitForEdgeKindWindow],
    scoped: &[WorkloadWaitForEdgeKindWindow],
) -> Result<(), WorkloadError> {
    for scoped_window in scoped {
        let Some(merged_window) = wait_for_edge_kind_window(merged, scoped_window.kind()) else {
            continue;
        };
        if merged_window.edge_count() < scoped_window.edge_count()
            || merged_window.first_tick() > scoped_window.first_tick()
            || merged_window.last_tick() < scoped_window.last_tick()
        {
            return Err(
                WorkloadError::InvalidParallelWaitForEdgeKindWindowMergeSummary {
                    scope,
                    kind: scoped_window.kind(),
                    merged_edge_count: merged_window.edge_count(),
                    scoped_edge_count: scoped_window.edge_count(),
                    merged_first_tick: merged_window.first_tick(),
                    scoped_first_tick: scoped_window.first_tick(),
                    merged_last_tick: merged_window.last_tick(),
                    scoped_last_tick: scoped_window.last_tick(),
                },
            );
        }
    }
    Ok(())
}

pub(super) fn wait_for_edge_kind_count(
    counts: &BTreeMap<WaitForEdgeKind, usize>,
    kind: WaitForEdgeKind,
) -> usize {
    counts.get(&kind).copied().unwrap_or(0)
}

pub(super) fn wait_for_edge_kind_count_sum(counts: &BTreeMap<WaitForEdgeKind, usize>) -> usize {
    counts.values().copied().sum()
}

pub(super) fn merge_wait_for_edge_kind_counts<'a>(
    maps: impl IntoIterator<Item = &'a BTreeMap<WaitForEdgeKind, usize>>,
) -> BTreeMap<WaitForEdgeKind, usize> {
    let mut merged = BTreeMap::new();
    for map in maps {
        for (kind, count) in map {
            let stored = merged.entry(*kind).or_insert(0usize);
            *stored = stored.saturating_add(*count);
        }
    }
    merged
}

pub(super) fn collect_wait_for_edge_kind_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    let mut by_kind = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        by_kind
            .entry(window.kind())
            .and_modify(|stored: &mut WorkloadWaitForEdgeKindWindow| stored.merge(window))
            .or_insert(window);
    }
    by_kind.into_values().collect()
}

pub(super) fn merge_wait_for_edge_kind_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    collect_wait_for_edge_kind_windows(windows)
}

pub(super) fn merge_wait_for_edge_kind_windows_by_strongest(
    windows: impl IntoIterator<Item = WorkloadWaitForEdgeKindWindow>,
) -> Vec<WorkloadWaitForEdgeKindWindow> {
    let mut by_kind = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        by_kind
            .entry(window.kind())
            .and_modify(|stored: &mut WorkloadWaitForEdgeKindWindow| {
                *stored = WorkloadWaitForEdgeKindWindow::new(
                    stored.kind(),
                    stored.edge_count().max(window.edge_count()),
                    stored.first_tick().min(window.first_tick()),
                    stored.last_tick().max(window.last_tick()),
                );
            })
            .or_insert(window);
    }
    by_kind.into_values().collect()
}

pub(super) fn wait_for_edge_kind_window(
    windows: &[WorkloadWaitForEdgeKindWindow],
    kind: WaitForEdgeKind,
) -> Option<WorkloadWaitForEdgeKindWindow> {
    windows.iter().copied().find(|window| window.kind() == kind)
}

pub(super) fn wait_for_edge_kind_window_count_sum(
    windows: &[WorkloadWaitForEdgeKindWindow],
) -> usize {
    windows
        .iter()
        .map(WorkloadWaitForEdgeKindWindow::edge_count)
        .sum()
}

pub(super) fn merge_wait_for_edge_kind_counts_from_windows(
    counts: &mut BTreeMap<WaitForEdgeKind, usize>,
    windows: &[WorkloadWaitForEdgeKindWindow],
) {
    for window in windows {
        counts
            .entry(window.kind())
            .and_modify(|count| *count = (*count).max(window.edge_count()))
            .or_insert(window.edge_count());
    }
}
