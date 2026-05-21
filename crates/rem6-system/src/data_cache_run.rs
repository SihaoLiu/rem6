use std::collections::{BTreeMap, BTreeSet};

use rem6_coherence::ParallelCoherenceRunSummary;
use rem6_kernel::{DeadlockDiagnostic, Tick, WaitForEdge, WaitForEdgeKind, WaitForNode};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn data_cache_runs(&self) -> &[ParallelCoherenceRunSummary] {
        &self.data_cache_runs
    }

    pub fn data_cache_run_count(&self) -> usize {
        self.data_cache_runs.len()
    }

    pub fn initial_data_cache_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.data_cache_runs
            .iter()
            .flat_map(|run| run.initial_wait_for_edges().iter().cloned())
            .collect()
    }

    pub fn remaining_data_cache_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.data_cache_runs
            .iter()
            .flat_map(|run| run.remaining_wait_for_edges().iter().cloned())
            .collect()
    }

    pub fn data_cache_wait_for_edges(&self) -> Vec<WaitForEdge> {
        self.remaining_data_cache_wait_for_edges()
    }

    pub fn initial_data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::initial_wait_for_edge_count)
            .sum()
    }

    pub fn remaining_data_cache_wait_for_edge_count(&self) -> usize {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::remaining_wait_for_edge_count)
            .sum()
    }

    pub fn data_cache_wait_for_edge_count(&self) -> usize {
        self.remaining_data_cache_wait_for_edge_count()
    }

    pub fn has_data_cache_wait_for_edges(&self) -> bool {
        self.data_cache_wait_for_edge_count() != 0
    }

    pub fn initial_data_cache_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.data_cache_runs
            .iter()
            .flat_map(ParallelCoherenceRunSummary::initial_wait_for_blocked_nodes)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn remaining_data_cache_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.data_cache_runs
            .iter()
            .flat_map(ParallelCoherenceRunSummary::remaining_wait_for_blocked_nodes)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn data_cache_wait_for_blocked_nodes(&self) -> Vec<WaitForNode> {
        self.remaining_data_cache_wait_for_blocked_nodes()
    }

    pub fn initial_data_cache_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        let mut counts = BTreeMap::new();
        for run in &self.data_cache_runs {
            merge_counts(&mut counts, run.initial_wait_for_edge_kind_counts());
        }
        counts
    }

    pub fn remaining_data_cache_wait_for_edge_kind_counts(
        &self,
    ) -> BTreeMap<WaitForEdgeKind, usize> {
        let mut counts = BTreeMap::new();
        for run in &self.data_cache_runs {
            merge_counts(&mut counts, run.remaining_wait_for_edge_kind_counts());
        }
        counts
    }

    pub fn data_cache_wait_for_edge_kind_counts(&self) -> BTreeMap<WaitForEdgeKind, usize> {
        self.remaining_data_cache_wait_for_edge_kind_counts()
    }

    pub fn initial_data_cache_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.data_cache_runs
            .iter()
            .map(|run| run.initial_wait_for_edge_count_by_kind(kind))
            .sum()
    }

    pub fn remaining_data_cache_wait_for_edge_count_by_kind(&self, kind: WaitForEdgeKind) -> usize {
        self.data_cache_runs
            .iter()
            .map(|run| run.remaining_wait_for_edge_count_by_kind(kind))
            .sum()
    }

    pub fn initial_data_cache_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        oldest_edge(self.initial_data_cache_wait_for_edges())
    }

    pub fn remaining_data_cache_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        oldest_edge(self.remaining_data_cache_wait_for_edges())
    }

    pub fn data_cache_oldest_wait_edge(&self) -> Option<WaitForEdge> {
        self.remaining_data_cache_oldest_wait_edge()
    }

    pub fn initial_data_cache_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        newest_edge(self.initial_data_cache_wait_for_edges())
    }

    pub fn remaining_data_cache_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        newest_edge(self.remaining_data_cache_wait_for_edges())
    }

    pub fn data_cache_newest_observed_wait_edge(&self) -> Option<WaitForEdge> {
        self.remaining_data_cache_newest_observed_wait_edge()
    }

    pub fn initial_data_cache_total_wait_observation_count(&self) -> u64 {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::initial_total_wait_observation_count)
            .sum()
    }

    pub fn remaining_data_cache_total_wait_observation_count(&self) -> u64 {
        self.data_cache_runs
            .iter()
            .map(ParallelCoherenceRunSummary::remaining_total_wait_observation_count)
            .sum()
    }

    pub fn data_cache_total_wait_observation_count(&self) -> u64 {
        self.remaining_data_cache_total_wait_observation_count()
    }

    pub fn initial_data_cache_first_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::initial_first_wait_tick)
            .min()
    }

    pub fn remaining_data_cache_first_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::remaining_first_wait_tick)
            .min()
    }

    pub fn data_cache_first_wait_tick(&self) -> Option<Tick> {
        self.remaining_data_cache_first_wait_tick()
    }

    pub fn initial_data_cache_last_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::initial_last_wait_tick)
            .max()
    }

    pub fn remaining_data_cache_last_wait_tick(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::remaining_last_wait_tick)
            .max()
    }

    pub fn data_cache_last_wait_tick(&self) -> Option<Tick> {
        self.remaining_data_cache_last_wait_tick()
    }

    pub fn initial_data_cache_longest_observed_wait_span(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::initial_longest_observed_wait_span)
            .max()
    }

    pub fn remaining_data_cache_longest_observed_wait_span(&self) -> Option<Tick> {
        self.data_cache_runs
            .iter()
            .filter_map(ParallelCoherenceRunSummary::remaining_longest_observed_wait_span)
            .max()
    }

    pub fn data_cache_longest_observed_wait_span(&self) -> Option<Tick> {
        self.remaining_data_cache_longest_observed_wait_span()
    }

    pub fn initial_data_cache_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.data_cache_runs
            .iter()
            .filter_map(|run| run.initial_deadlock_diagnostic().cloned())
            .collect()
    }

    pub fn remaining_data_cache_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.data_cache_runs
            .iter()
            .filter_map(|run| run.remaining_deadlock_diagnostic().cloned())
            .collect()
    }

    pub fn data_cache_deadlock_diagnostics(&self) -> Vec<DeadlockDiagnostic> {
        self.remaining_data_cache_deadlock_diagnostics()
    }

    pub fn initial_data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.initial_data_cache_deadlock_diagnostics().len()
    }

    pub fn remaining_data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.remaining_data_cache_deadlock_diagnostics().len()
    }

    pub fn data_cache_deadlock_diagnostic_count(&self) -> usize {
        self.remaining_data_cache_deadlock_diagnostic_count()
    }
}

fn merge_counts(
    counts: &mut BTreeMap<WaitForEdgeKind, usize>,
    run_counts: BTreeMap<WaitForEdgeKind, usize>,
) {
    for (kind, count) in run_counts {
        *counts.entry(kind).or_insert(0) += count;
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
