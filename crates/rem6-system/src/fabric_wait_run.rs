use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{
    DeadlockDiagnostic, Tick, WaitForBlockedNodeWindow, WaitForEdge, WaitForEdgeKind, WaitForGraph,
    WaitForNode, WaitForTargetNodeWindow,
};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn with_fabric_wait_for(mut self, fabric_wait_for: WaitForGraph) -> Self {
        self.fabric_wait_for = fabric_wait_for;
        self
    }

    pub fn with_dram_wait_for(mut self, dram_wait_for: WaitForGraph) -> Self {
        self.dram_wait_for = dram_wait_for;
        self
    }

    pub fn fabric_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.fabric_wait_for.edges()
    }

    pub fn fabric_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for.edge_count()
    }

    pub fn has_fabric_wait_for_edges(&self) -> bool {
        self.fabric_wait_for_edge_count() != 0
    }

    pub fn fabric_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.fabric_wait_for.blocked_nodes()
    }

    pub fn fabric_wait_for_blocked_node_windows(&self) -> Vec<WaitForBlockedNodeWindow> {
        self.fabric_wait_for.blocked_node_windows()
    }

    pub fn fabric_wait_for_target_nodes(&self) -> Vec<WaitForNode> {
        target_nodes_from_windows(self.fabric_wait_for_target_node_windows())
    }

    pub fn fabric_wait_for_target_node_windows(&self) -> Vec<WaitForTargetNodeWindow> {
        self.fabric_wait_for.target_node_windows()
    }

    pub fn fabric_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        let mut counts = BTreeMap::new();
        for edge in self.fabric_wait_for_edges() {
            *counts.entry(edge.kind()).or_insert(0) += 1;
        }
        counts
    }

    pub fn fabric_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.fabric_wait_for_edges()
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
    }

    pub fn fabric_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        oldest_edge(self.fabric_wait_for_edges())
    }

    pub fn fabric_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        newest_edge(self.fabric_wait_for_edges())
    }

    pub fn fabric_total_wait_observation_count(&self) -> u64 {
        self.fabric_wait_for_edges()
            .iter()
            .map(WaitForEdge::observation_count)
            .sum()
    }

    pub fn fabric_first_wait_tick(&self) -> Option<Tick> {
        self.fabric_wait_for_edges()
            .iter()
            .map(WaitForEdge::first_observed_tick)
            .min()
    }

    pub fn fabric_last_wait_tick(&self) -> Option<Tick> {
        self.fabric_wait_for_edges()
            .iter()
            .map(WaitForEdge::last_observed_tick)
            .max()
    }

    pub fn fabric_longest_observed_wait_span(&self) -> Option<Tick> {
        self.fabric_wait_for_edges()
            .iter()
            .map(|edge| {
                edge.last_observed_tick()
                    .saturating_sub(edge.first_observed_tick())
            })
            .max()
    }

    pub fn fabric_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.fabric_wait_for
            .deadlock_diagnostic()
            .into_iter()
            .collect()
    }

    pub fn fabric_deadlock_diagnostic_count(&self) -> usize {
        self.fabric_deadlock_diagnostics().len()
    }

    pub fn dram_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.dram_wait_for.edges()
    }

    pub fn dram_wait_for_edge_count(&self) -> usize {
        self.dram_wait_for.edge_count()
    }

    pub fn has_dram_wait_for_edges(&self) -> bool {
        self.dram_wait_for_edge_count() != 0
    }

    pub fn dram_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.dram_wait_for.blocked_nodes()
    }

    pub fn dram_wait_for_blocked_node_windows(&self) -> Vec<WaitForBlockedNodeWindow> {
        self.dram_wait_for.blocked_node_windows()
    }

    pub fn dram_wait_for_target_nodes(&self) -> Vec<WaitForNode> {
        target_nodes_from_windows(self.dram_wait_for_target_node_windows())
    }

    pub fn dram_wait_for_target_node_windows(&self) -> Vec<WaitForTargetNodeWindow> {
        self.dram_wait_for.target_node_windows()
    }

    pub fn dram_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        let mut counts = BTreeMap::new();
        for edge in self.dram_wait_for_edges() {
            *counts.entry(edge.kind()).or_insert(0) += 1;
        }
        counts
    }

    pub fn dram_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.dram_wait_for_edges()
            .into_iter()
            .filter(|edge| edge.kind() == kind)
            .count()
    }

    pub fn dram_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        oldest_edge(self.dram_wait_for_edges())
    }

    pub fn dram_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        newest_edge(self.dram_wait_for_edges())
    }

    pub fn dram_total_wait_observation_count(&self) -> u64 {
        self.dram_wait_for_edges()
            .iter()
            .map(WaitForEdge::observation_count)
            .sum()
    }

    pub fn dram_first_wait_tick(&self) -> Option<Tick> {
        self.dram_wait_for_edges()
            .iter()
            .map(WaitForEdge::first_observed_tick)
            .min()
    }

    pub fn dram_last_wait_tick(&self) -> Option<Tick> {
        self.dram_wait_for_edges()
            .iter()
            .map(WaitForEdge::last_observed_tick)
            .max()
    }

    pub fn dram_longest_observed_wait_span(&self) -> Option<Tick> {
        self.dram_wait_for_edges()
            .iter()
            .map(|edge| {
                edge.last_observed_tick()
                    .saturating_sub(edge.first_observed_tick())
            })
            .max()
    }

    pub fn dram_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.dram_wait_for
            .deadlock_diagnostic()
            .into_iter()
            .collect()
    }

    pub fn dram_deadlock_diagnostic_count(&self) -> usize {
        self.dram_deadlock_diagnostics().len()
    }

    pub fn resource_wait_for_edges(&self) -> Vec<WaitForEdge> {
        let mut edges = self.fabric_wait_for_edges();
        edges.extend(self.dram_wait_for_edges());
        edges
    }

    pub fn resource_wait_for_edge_count(&self) -> usize {
        self.fabric_wait_for_edge_count() + self.dram_wait_for_edge_count()
    }

    pub fn has_resource_wait_for_edges(&self) -> bool {
        self.resource_wait_for_edge_count() != 0
    }

    pub fn resource_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        blocked_nodes_from_edges(self.resource_wait_for_edges())
    }

    pub fn resource_wait_for_blocked_node_windows(&self) -> Vec<WaitForBlockedNodeWindow> {
        WaitForBlockedNodeWindow::from_edges(self.resource_wait_for_edges())
    }

    pub fn resource_wait_for_target_nodes(&self) -> Vec<WaitForNode> {
        target_nodes_from_windows(self.resource_wait_for_target_node_windows())
    }

    pub fn resource_wait_for_target_node_windows(&self) -> Vec<WaitForTargetNodeWindow> {
        WaitForTargetNodeWindow::from_edges(self.resource_wait_for_edges())
    }

    pub fn resource_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        wait_for_edge_kind_counts(self.resource_wait_for_edges())
    }

    pub fn resource_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_count_by_kind(self.resource_wait_for_edges(), kind)
    }

    pub fn resource_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        let mut diagnostics = self.fabric_deadlock_diagnostics();
        diagnostics.extend(self.dram_deadlock_diagnostics());
        append_combined_deadlock(&mut diagnostics, self.resource_wait_for_edges());
        diagnostics
    }

    pub fn resource_deadlock_diagnostic_count(&self) -> usize {
        self.resource_deadlock_diagnostics().len()
    }

    pub fn has_resource_deadlock_diagnostics(&self) -> bool {
        self.resource_deadlock_diagnostic_count() != 0
    }

    pub fn full_system_wait_for_edges(&self) -> Vec<WaitForEdge> {
        let mut edges = self.resource_wait_for_edges();
        edges.extend(self.data_cache_wait_for_edges());
        edges
    }

    pub fn full_system_wait_for_edge_count(&self) -> usize {
        self.resource_wait_for_edge_count() + self.data_cache_wait_for_edge_count()
    }

    pub fn has_full_system_wait_for_edges(&self) -> bool {
        self.full_system_wait_for_edge_count() != 0
    }

    pub fn full_system_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        blocked_nodes_from_edges(self.full_system_wait_for_edges())
    }

    pub fn full_system_wait_for_blocked_node_windows(&self) -> Vec<WaitForBlockedNodeWindow> {
        WaitForBlockedNodeWindow::from_edges(self.full_system_wait_for_edges())
    }

    pub fn full_system_wait_for_target_nodes(&self) -> Vec<WaitForNode> {
        target_nodes_from_windows(self.full_system_wait_for_target_node_windows())
    }

    pub fn full_system_wait_for_target_node_windows(&self) -> Vec<WaitForTargetNodeWindow> {
        WaitForTargetNodeWindow::from_edges(self.full_system_wait_for_edges())
    }

    pub fn full_system_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        wait_for_edge_kind_counts(self.full_system_wait_for_edges())
    }

    pub fn full_system_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        wait_for_edge_count_by_kind(self.full_system_wait_for_edges(), kind)
    }

    pub fn full_system_first_wait_tick(&self) -> Option<Tick> {
        self.full_system_wait_for_edges()
            .iter()
            .map(WaitForEdge::first_observed_tick)
            .min()
    }

    pub fn full_system_last_wait_tick(&self) -> Option<Tick> {
        self.full_system_wait_for_edges()
            .iter()
            .map(WaitForEdge::last_observed_tick)
            .max()
    }

    pub fn full_system_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        let mut diagnostics = self.resource_deadlock_diagnostics();
        diagnostics.extend(self.data_cache_deadlock_diagnostics());
        append_combined_deadlock(&mut diagnostics, self.full_system_wait_for_edges());
        diagnostics
    }

    pub fn full_system_deadlock_diagnostic_count(&self) -> usize {
        self.full_system_deadlock_diagnostics().len()
    }

    pub fn has_full_system_deadlock_diagnostics(&self) -> bool {
        self.full_system_deadlock_diagnostic_count() != 0
    }
}

fn oldest_edge(edges: Vec<WaitForEdge>) -> Option<WaitForEdge> {
    edges
        .into_iter()
        .min_by_key(|edge| (edge.first_observed_tick(), edge.last_observed_tick()))
}

fn newest_edge(edges: Vec<WaitForEdge>) -> Option<WaitForEdge> {
    edges
        .into_iter()
        .max_by_key(|edge| (edge.last_observed_tick(), edge.first_observed_tick()))
}

fn blocked_nodes_from_edges(edges: Vec<WaitForEdge>) -> Vec<WaitForNode> {
    WaitForBlockedNodeWindow::from_edges(edges)
        .into_iter()
        .map(|window| window.node().clone())
        .collect()
}

fn target_nodes_from_windows(windows: Vec<WaitForTargetNodeWindow>) -> Vec<WaitForNode> {
    windows
        .into_iter()
        .map(|window| window.node().clone())
        .collect()
}

fn wait_for_edge_kind_counts(edges: Vec<WaitForEdge>) -> BTreeMap<WaitForEdgeKind, usize> {
    let mut counts = BTreeMap::new();
    for edge in edges {
        *counts.entry(edge.kind()).or_insert(0) += 1;
    }
    counts
}

fn wait_for_edge_count_by_kind(edges: Vec<WaitForEdge>, kind: WaitForEdgeKind) -> usize {
    edges.into_iter().filter(|edge| edge.kind() == kind).count()
}

fn append_combined_deadlock(diagnostics: &mut Vec<DeadlockDiagnostic>, edges: Vec<WaitForEdge>) {
    let graph = graph_from_edges(edges);
    let Some(diagnostic) = graph.deadlock_diagnostic() else {
        return;
    };
    if diagnostics
        .iter()
        .all(|existing| diagnostic_edge_key_set(existing) != diagnostic_edge_key_set(&diagnostic))
    {
        diagnostics.push(diagnostic);
    }
}

fn graph_from_edges(edges: Vec<WaitForEdge>) -> WaitForGraph {
    let mut graph = WaitForGraph::new();
    for edge in edges {
        graph
            .record_wait(
                edge.source().clone(),
                edge.target().clone(),
                edge.kind(),
                edge.first_observed_tick(),
            )
            .expect("existing wait-for edge is valid");
        if edge.last_observed_tick() != edge.first_observed_tick() {
            graph
                .record_wait(
                    edge.source().clone(),
                    edge.target().clone(),
                    edge.kind(),
                    edge.last_observed_tick(),
                )
                .expect("existing wait-for edge is valid");
        }
    }
    graph
}

fn diagnostic_edge_key_set(
    diagnostic: &DeadlockDiagnostic,
) -> BTreeSet<(WaitForNode, WaitForNode, WaitForEdgeKind)> {
    diagnostic
        .cycle_edges()
        .iter()
        .map(|edge| (edge.source().clone(), edge.target().clone(), edge.kind()))
        .collect()
}
