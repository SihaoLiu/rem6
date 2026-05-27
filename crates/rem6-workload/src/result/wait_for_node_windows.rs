use std::collections::BTreeMap;

use rem6_kernel::WaitForNode;

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadWaitForBlockedNodeWindow {
    node: WaitForNode,
    edge_count: usize,
    first_tick: u64,
    last_tick: u64,
}

impl WorkloadWaitForBlockedNodeWindow {
    pub fn new(node: WaitForNode, edge_count: usize, first_tick: u64, last_tick: u64) -> Self {
        let stored_first_tick = if first_tick <= last_tick {
            first_tick
        } else {
            last_tick
        };
        let stored_last_tick = if first_tick <= last_tick {
            last_tick
        } else {
            first_tick
        };
        Self {
            node,
            edge_count,
            first_tick: stored_first_tick,
            last_tick: stored_last_tick,
        }
    }

    pub const fn node(&self) -> &WaitForNode {
        &self.node
    }

    pub const fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub const fn first_tick(&self) -> u64 {
        self.first_tick
    }

    pub const fn last_tick(&self) -> u64 {
        self.last_tick
    }

    pub const fn is_empty(&self) -> bool {
        self.edge_count == 0
    }

    pub(crate) fn merge(&mut self, other: Self) {
        debug_assert_eq!(self.node, other.node);
        self.edge_count = self.edge_count.saturating_add(other.edge_count);
        self.first_tick = self.first_tick.min(other.first_tick);
        self.last_tick = self.last_tick.max(other.last_tick);
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadWaitForTargetNodeWindow {
    node: WaitForNode,
    edge_count: usize,
    first_tick: u64,
    last_tick: u64,
}

impl WorkloadWaitForTargetNodeWindow {
    pub fn new(node: WaitForNode, edge_count: usize, first_tick: u64, last_tick: u64) -> Self {
        let stored_first_tick = if first_tick <= last_tick {
            first_tick
        } else {
            last_tick
        };
        let stored_last_tick = if first_tick <= last_tick {
            last_tick
        } else {
            first_tick
        };
        Self {
            node,
            edge_count,
            first_tick: stored_first_tick,
            last_tick: stored_last_tick,
        }
    }

    pub const fn node(&self) -> &WaitForNode {
        &self.node
    }

    pub const fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub const fn first_tick(&self) -> u64 {
        self.first_tick
    }

    pub const fn last_tick(&self) -> u64 {
        self.last_tick
    }

    pub const fn is_empty(&self) -> bool {
        self.edge_count == 0
    }

    pub(crate) fn merge(&mut self, other: Self) {
        debug_assert_eq!(self.node, other.node);
        self.edge_count = self.edge_count.saturating_add(other.edge_count);
        self.first_tick = self.first_tick.min(other.first_tick);
        self.last_tick = self.last_tick.max(other.last_tick);
    }
}

pub(super) fn collect_wait_for_blocked_node_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
) -> Vec<WorkloadWaitForBlockedNodeWindow> {
    let mut by_node = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        by_node
            .entry(window.node().clone())
            .and_modify(|stored: &mut WorkloadWaitForBlockedNodeWindow| {
                stored.merge(window.clone())
            })
            .or_insert(window);
    }
    by_node.into_values().collect()
}

pub(super) fn merge_wait_for_blocked_node_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
) -> Vec<WorkloadWaitForBlockedNodeWindow> {
    collect_wait_for_blocked_node_windows(windows)
}

pub(super) fn merge_wait_for_blocked_node_windows_by_strongest(
    windows: impl IntoIterator<Item = WorkloadWaitForBlockedNodeWindow>,
) -> Vec<WorkloadWaitForBlockedNodeWindow> {
    let mut by_node: BTreeMap<WaitForNode, WorkloadWaitForBlockedNodeWindow> = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        let node = window.node().clone();
        if let Some(stored) = by_node.get_mut(&node) {
            *stored = WorkloadWaitForBlockedNodeWindow::new(
                node,
                stored.edge_count().max(window.edge_count()),
                stored.first_tick().min(window.first_tick()),
                stored.last_tick().max(window.last_tick()),
            );
        } else {
            by_node.insert(node, window);
        }
    }
    by_node.into_values().collect()
}

pub(super) fn wait_for_blocked_node_window(
    windows: &[WorkloadWaitForBlockedNodeWindow],
    node: &WaitForNode,
) -> Option<WorkloadWaitForBlockedNodeWindow> {
    windows.iter().find(|window| window.node() == node).cloned()
}

pub(super) fn wait_for_blocked_node_window_count_sum(
    windows: &[WorkloadWaitForBlockedNodeWindow],
) -> usize {
    windows
        .iter()
        .map(WorkloadWaitForBlockedNodeWindow::edge_count)
        .sum()
}

pub(super) fn collect_wait_for_target_node_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
) -> Vec<WorkloadWaitForTargetNodeWindow> {
    let mut by_node = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        by_node
            .entry(window.node().clone())
            .and_modify(|stored: &mut WorkloadWaitForTargetNodeWindow| stored.merge(window.clone()))
            .or_insert(window);
    }
    by_node.into_values().collect()
}

pub(super) fn merge_wait_for_target_node_windows(
    windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
) -> Vec<WorkloadWaitForTargetNodeWindow> {
    collect_wait_for_target_node_windows(windows)
}

pub(super) fn merge_wait_for_target_node_windows_by_strongest(
    windows: impl IntoIterator<Item = WorkloadWaitForTargetNodeWindow>,
) -> Vec<WorkloadWaitForTargetNodeWindow> {
    let mut by_node: BTreeMap<WaitForNode, WorkloadWaitForTargetNodeWindow> = BTreeMap::new();
    for window in windows {
        if window.is_empty() {
            continue;
        }
        let node = window.node().clone();
        if let Some(stored) = by_node.get_mut(&node) {
            *stored = WorkloadWaitForTargetNodeWindow::new(
                node,
                stored.edge_count().max(window.edge_count()),
                stored.first_tick().min(window.first_tick()),
                stored.last_tick().max(window.last_tick()),
            );
        } else {
            by_node.insert(node, window);
        }
    }
    by_node.into_values().collect()
}

pub(super) fn wait_for_target_node_window(
    windows: &[WorkloadWaitForTargetNodeWindow],
    node: &WaitForNode,
) -> Option<WorkloadWaitForTargetNodeWindow> {
    windows.iter().find(|window| window.node() == node).cloned()
}

pub(super) fn wait_for_target_node_window_count_sum(
    windows: &[WorkloadWaitForTargetNodeWindow],
) -> usize {
    windows
        .iter()
        .map(WorkloadWaitForTargetNodeWindow::edge_count)
        .sum()
}
