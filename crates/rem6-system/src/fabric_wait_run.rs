use std::collections::BTreeMap;

use rem6_kernel::{
    DeadlockDiagnostic, Tick, WaitForEdge, WaitForEdgeKind, WaitForGraph, WaitForNode,
};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn with_fabric_wait_for(mut self, fabric_wait_for: WaitForGraph) -> Self {
        self.fabric_wait_for = fabric_wait_for;
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
