use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{PartitionId, Tick};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaitForNode {
    Partition(PartitionId),
    Component(String),
    Resource(String),
    Transaction(String),
}

impl WaitForNode {
    pub const fn partition(partition: PartitionId) -> Self {
        Self::Partition(partition)
    }

    pub fn component(label: impl Into<String>) -> Result<Self, WaitForGraphError> {
        Ok(Self::Component(validate_label(label.into())?))
    }

    pub fn resource(label: impl Into<String>) -> Result<Self, WaitForGraphError> {
        Ok(Self::Resource(validate_label(label.into())?))
    }

    pub fn transaction(label: impl Into<String>) -> Result<Self, WaitForGraphError> {
        Ok(Self::Transaction(validate_label(label.into())?))
    }
}

impl fmt::Display for WaitForNode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Partition(partition) => write!(formatter, "partition:{}", partition.index()),
            Self::Component(label) => write!(formatter, "component:{label}"),
            Self::Resource(label) => write!(formatter, "resource:{label}"),
            Self::Transaction(label) => write!(formatter, "transaction:{label}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WaitForEdgeKind {
    Resource,
    Message,
    Protocol,
    Queue,
    Credit,
    HostAction,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitForEdge {
    source: WaitForNode,
    target: WaitForNode,
    kind: WaitForEdgeKind,
    first_observed_tick: Tick,
    last_observed_tick: Tick,
    observation_count: u64,
}

impl WaitForEdge {
    fn new(source: WaitForNode, target: WaitForNode, kind: WaitForEdgeKind, tick: Tick) -> Self {
        Self {
            source,
            target,
            kind,
            first_observed_tick: tick,
            last_observed_tick: tick,
            observation_count: 1,
        }
    }

    fn observe(&mut self, tick: Tick) {
        self.first_observed_tick = self.first_observed_tick.min(tick);
        self.last_observed_tick = self.last_observed_tick.max(tick);
        self.observation_count += 1;
    }

    pub const fn source(&self) -> &WaitForNode {
        &self.source
    }

    pub const fn target(&self) -> &WaitForNode {
        &self.target
    }

    pub const fn kind(&self) -> WaitForEdgeKind {
        self.kind
    }

    pub const fn first_observed_tick(&self) -> Tick {
        self.first_observed_tick
    }

    pub const fn last_observed_tick(&self) -> Tick {
        self.last_observed_tick
    }

    pub const fn observation_count(&self) -> u64 {
        self.observation_count
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DeadlockDiagnostic {
    cycle_edges: Vec<WaitForEdge>,
    cycle_nodes: Vec<WaitForNode>,
    edge_kinds: Vec<WaitForEdgeKind>,
    first_observed_tick: Tick,
    last_observed_tick: Tick,
}

impl DeadlockDiagnostic {
    fn new(cycle_edges: Vec<WaitForEdge>) -> Self {
        let mut cycle_nodes = Vec::new();
        if let Some(first) = cycle_edges.first() {
            cycle_nodes.push(first.source.clone());
            cycle_nodes.extend(cycle_edges.iter().map(|edge| edge.target.clone()));
        }
        let edge_kinds = cycle_edges.iter().map(WaitForEdge::kind).collect();
        let first_observed_tick = cycle_edges
            .iter()
            .map(WaitForEdge::first_observed_tick)
            .min()
            .unwrap_or(0);
        let last_observed_tick = cycle_edges
            .iter()
            .map(WaitForEdge::last_observed_tick)
            .max()
            .unwrap_or(0);

        Self {
            cycle_edges,
            cycle_nodes,
            edge_kinds,
            first_observed_tick,
            last_observed_tick,
        }
    }

    pub fn cycle_edges(&self) -> &[WaitForEdge] {
        &self.cycle_edges
    }

    pub fn cycle_nodes(&self) -> &[WaitForNode] {
        &self.cycle_nodes
    }

    pub fn edge_kinds(&self) -> &[WaitForEdgeKind] {
        &self.edge_kinds
    }

    pub fn edge_count(&self) -> usize {
        self.cycle_edges.len()
    }

    pub const fn first_observed_tick(&self) -> Tick {
        self.first_observed_tick
    }

    pub const fn last_observed_tick(&self) -> Tick {
        self.last_observed_tick
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaitForGraphError {
    EmptyNodeLabel,
    InvalidNodeLabel { label: String },
    SelfWait,
}

impl fmt::Display for WaitForGraphError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyNodeLabel => write!(formatter, "wait-for node label must not be empty"),
            Self::InvalidNodeLabel { label } => {
                write!(formatter, "wait-for node label {label:?} is invalid")
            }
            Self::SelfWait => write!(formatter, "wait-for edge source and target must differ"),
        }
    }
}

impl Error for WaitForGraphError {}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WaitForGraph {
    edges: BTreeMap<WaitForEdgeKey, WaitForEdge>,
}

impl WaitForGraph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record_wait(
        &mut self,
        source: WaitForNode,
        target: WaitForNode,
        kind: WaitForEdgeKind,
        tick: Tick,
    ) -> Result<(), WaitForGraphError> {
        if source == target {
            return Err(WaitForGraphError::SelfWait);
        }
        let key = WaitForEdgeKey::new(source.clone(), target.clone(), kind);
        if let Some(edge) = self.edges.get_mut(&key) {
            edge.observe(tick);
        } else {
            self.edges
                .insert(key, WaitForEdge::new(source, target, kind, tick));
        }
        Ok(())
    }

    pub fn remove_wait(
        &mut self,
        source: &WaitForNode,
        target: &WaitForNode,
        kind: WaitForEdgeKind,
    ) -> bool {
        self.edges
            .remove(&WaitForEdgeKey::new(source.clone(), target.clone(), kind))
            .is_some()
    }

    pub fn clear_waits_from(&mut self, source: &WaitForNode) -> usize {
        let before = self.edges.len();
        self.edges.retain(|key, _| &key.source != source);
        before - self.edges.len()
    }

    pub fn clear_waits_to(&mut self, target: &WaitForNode) -> usize {
        let before = self.edges.len();
        self.edges.retain(|key, _| &key.target != target);
        before - self.edges.len()
    }

    pub fn dependencies(&self, source: &WaitForNode) -> Vec<WaitForEdge> {
        self.edges
            .iter()
            .filter(|(key, _)| &key.source == source)
            .map(|(_, edge)| edge.clone())
            .collect()
    }

    pub fn dependents(&self, target: &WaitForNode) -> Vec<WaitForEdge> {
        self.edges
            .iter()
            .filter(|(key, _)| &key.target == target)
            .map(|(_, edge)| edge.clone())
            .collect()
    }

    pub fn blocked_nodes(&self) -> Vec<WaitForNode> {
        self.edges
            .keys()
            .map(|key| key.source.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    pub fn deadlock_diagnostic(&self) -> Option<DeadlockDiagnostic> {
        let nodes = self.nodes();
        let mut finished = BTreeSet::new();
        for node in nodes {
            let mut path_nodes = Vec::new();
            let mut path_edges = Vec::new();
            if let Some(edges) =
                self.find_cycle_from(&node, &mut path_nodes, &mut path_edges, &mut finished)
            {
                return Some(DeadlockDiagnostic::new(edges));
            }
        }
        None
    }

    fn nodes(&self) -> Vec<WaitForNode> {
        self.edges
            .values()
            .flat_map(|edge| [edge.source.clone(), edge.target.clone()])
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect()
    }

    fn find_cycle_from(
        &self,
        node: &WaitForNode,
        path_nodes: &mut Vec<WaitForNode>,
        path_edges: &mut Vec<WaitForEdge>,
        finished: &mut BTreeSet<WaitForNode>,
    ) -> Option<Vec<WaitForEdge>> {
        if finished.contains(node) {
            return None;
        }
        path_nodes.push(node.clone());
        for edge in self.dependencies(node) {
            if let Some(index) = path_nodes
                .iter()
                .position(|path_node| path_node == edge.target())
            {
                let mut cycle = path_edges[index..].to_vec();
                cycle.push(edge);
                path_nodes.pop();
                return Some(cycle);
            }

            path_edges.push(edge.clone());
            if let Some(cycle) =
                self.find_cycle_from(edge.target(), path_nodes, path_edges, finished)
            {
                path_nodes.pop();
                return Some(cycle);
            }
            path_edges.pop();
        }
        path_nodes.pop();
        finished.insert(node.clone());
        None
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct WaitForEdgeKey {
    source: WaitForNode,
    target: WaitForNode,
    kind: WaitForEdgeKind,
}

impl WaitForEdgeKey {
    fn new(source: WaitForNode, target: WaitForNode, kind: WaitForEdgeKind) -> Self {
        Self {
            source,
            target,
            kind,
        }
    }
}

fn validate_label(label: String) -> Result<String, WaitForGraphError> {
    if label.is_empty() {
        return Err(WaitForGraphError::EmptyNodeLabel);
    }
    if !label
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b':'))
    {
        return Err(WaitForGraphError::InvalidNodeLabel { label });
    }
    Ok(label)
}
