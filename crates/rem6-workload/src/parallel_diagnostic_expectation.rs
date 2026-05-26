use rem6_kernel::{Tick, WaitForEdgeKind, WaitForNode};

use crate::{
    WorkloadError, WorkloadParallelExecutionSummary, WorkloadWaitForBlockedNodeWindow,
    WorkloadWaitForEdgeKindWindow, WorkloadWaitForTargetNodeWindow,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelDiagnosticScope {
    Resource,
    DataCache,
    Compute,
    Dma,
    FullSystem,
}

impl WorkloadParallelDiagnosticScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Resource => "resource",
            Self::DataCache => "data-cache",
            Self::Compute => "compute",
            Self::Dma => "dma",
            Self::FullSystem => "full-system",
        }
    }

    pub(crate) const fn sort_rank(self) -> u8 {
        match self {
            Self::Resource => 0,
            Self::DataCache => 1,
            Self::Compute => 2,
            Self::Dma => 3,
            Self::FullSystem => 4,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedCleanParallelDiagnostics {
    scope: WorkloadParallelDiagnosticScope,
    livelock_transition_threshold: Option<u64>,
}

impl WorkloadExpectedCleanParallelDiagnostics {
    pub const fn new(scope: WorkloadParallelDiagnosticScope) -> Self {
        Self {
            scope,
            livelock_transition_threshold: None,
        }
    }

    pub const fn scope(self) -> WorkloadParallelDiagnosticScope {
        self.scope
    }

    pub fn with_livelock_transition_threshold(
        mut self,
        threshold: u64,
    ) -> Result<Self, WorkloadError> {
        if threshold == 0 {
            return Err(WorkloadError::ZeroExpectedLivelockTransitionThreshold {
                scope: self.scope,
            });
        }
        self.livelock_transition_threshold = Some(threshold);
        Ok(self)
    }

    pub const fn livelock_transition_threshold(self) -> Option<u64> {
        self.livelock_transition_threshold
    }

    pub(crate) const fn sort_key(self) -> u8 {
        self.scope.sort_rank()
    }

    pub(crate) fn actual_counts(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> (usize, usize, usize) {
        match self.scope {
            WorkloadParallelDiagnosticScope::Resource => (
                summary.resource_wait_for_edge_count(),
                summary.resource_deadlock_diagnostic_count(),
                0,
            ),
            WorkloadParallelDiagnosticScope::DataCache => (
                summary.data_cache_wait_for_edge_count(),
                summary.data_cache_deadlock_diagnostic_count(),
                summary.data_cache_parallel_scheduler_livelock_diagnostic_count(),
            ),
            WorkloadParallelDiagnosticScope::Compute => (
                summary.compute_wait_for_edge_count(),
                summary.compute_deadlock_diagnostic_count(),
                0,
            ),
            WorkloadParallelDiagnosticScope::Dma => (
                summary.dma_wait_for_edge_count(),
                summary.dma_deadlock_diagnostic_count(),
                0,
            ),
            WorkloadParallelDiagnosticScope::FullSystem => (
                summary.full_system_wait_for_edge_count(),
                summary.full_system_deadlock_diagnostic_count(),
                summary.full_system_livelock_diagnostic_count(),
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelWaitForEdgeKindCount {
    scope: WorkloadParallelDiagnosticScope,
    kind: WaitForEdgeKind,
    minimum_edge_count: usize,
}

impl WorkloadExpectedParallelWaitForEdgeKindCount {
    pub fn new(
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
        minimum_edge_count: usize,
    ) -> Result<Self, WorkloadError> {
        if minimum_edge_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelWaitForEdgeKindCount { scope, kind });
        }
        Ok(Self {
            scope,
            kind,
            minimum_edge_count,
        })
    }

    pub const fn scope(self) -> WorkloadParallelDiagnosticScope {
        self.scope
    }

    pub const fn kind(self) -> WaitForEdgeKind {
        self.kind
    }

    pub const fn minimum_edge_count(self) -> usize {
        self.minimum_edge_count
    }

    pub(crate) const fn sort_key(self) -> (u8, WaitForEdgeKind) {
        (self.scope.sort_rank(), self.kind)
    }

    pub(crate) fn actual_count(self, summary: &WorkloadParallelExecutionSummary) -> usize {
        match self.scope {
            WorkloadParallelDiagnosticScope::Resource => {
                summary.resource_wait_for_edge_count_by_kind(self.kind)
            }
            WorkloadParallelDiagnosticScope::DataCache => {
                summary.data_cache_wait_for_edge_count_by_kind(self.kind)
            }
            WorkloadParallelDiagnosticScope::Compute => {
                summary.compute_wait_for_edge_count_by_kind(self.kind)
            }
            WorkloadParallelDiagnosticScope::Dma => {
                summary.dma_wait_for_edge_count_by_kind(self.kind)
            }
            WorkloadParallelDiagnosticScope::FullSystem => {
                summary.full_system_wait_for_edge_count_by_kind(self.kind)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelWaitForEdgeKindWindow {
    scope: WorkloadParallelDiagnosticScope,
    kind: WaitForEdgeKind,
    edge_count: usize,
    first_tick: Tick,
    last_tick: Tick,
}

impl WorkloadExpectedParallelWaitForEdgeKindWindow {
    pub fn new(
        scope: WorkloadParallelDiagnosticScope,
        kind: WaitForEdgeKind,
        edge_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Result<Self, WorkloadError> {
        if edge_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelWaitForEdgeKindWindow { scope, kind });
        }
        if first_tick > last_tick {
            return Err(
                WorkloadError::InvalidExpectedParallelWaitForEdgeKindWindow {
                    scope,
                    kind,
                    first_tick,
                    last_tick,
                },
            );
        }
        Ok(Self {
            scope,
            kind,
            edge_count,
            first_tick,
            last_tick,
        })
    }

    pub const fn scope(self) -> WorkloadParallelDiagnosticScope {
        self.scope
    }

    pub const fn kind(self) -> WaitForEdgeKind {
        self.kind
    }

    pub const fn edge_count(self) -> usize {
        self.edge_count
    }

    pub const fn first_tick(self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(self) -> Tick {
        self.last_tick
    }

    pub(crate) const fn sort_key(self) -> (u8, WaitForEdgeKind) {
        (self.scope.sort_rank(), self.kind)
    }

    pub(crate) fn actual_window(
        self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<WorkloadWaitForEdgeKindWindow> {
        match self.scope {
            WorkloadParallelDiagnosticScope::Resource => {
                summary.resource_wait_for_edge_kind_window(self.kind)
            }
            WorkloadParallelDiagnosticScope::DataCache => {
                summary.data_cache_wait_for_edge_kind_window(self.kind)
            }
            WorkloadParallelDiagnosticScope::Compute => {
                summary.compute_wait_for_edge_kind_window(self.kind)
            }
            WorkloadParallelDiagnosticScope::Dma => {
                summary.dma_wait_for_edge_kind_window(self.kind)
            }
            WorkloadParallelDiagnosticScope::FullSystem => {
                summary.full_system_wait_for_edge_kind_window(self.kind)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelWaitForBlockedNodeWindow {
    scope: WorkloadParallelDiagnosticScope,
    node: WaitForNode,
    edge_count: usize,
    first_tick: Tick,
    last_tick: Tick,
}

impl WorkloadExpectedParallelWaitForBlockedNodeWindow {
    pub fn new(
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
        edge_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Result<Self, WorkloadError> {
        if edge_count == 0 {
            return Err(
                WorkloadError::ZeroExpectedParallelWaitForBlockedNodeWindow { scope, node },
            );
        }
        if first_tick > last_tick {
            return Err(
                WorkloadError::InvalidExpectedParallelWaitForBlockedNodeWindow {
                    scope,
                    node,
                    first_tick,
                    last_tick,
                },
            );
        }
        Ok(Self {
            scope,
            node,
            edge_count,
            first_tick,
            last_tick,
        })
    }

    pub const fn scope(&self) -> WorkloadParallelDiagnosticScope {
        self.scope
    }

    pub const fn node(&self) -> &WaitForNode {
        &self.node
    }

    pub const fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub const fn first_tick(&self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub(crate) fn sort_key(&self) -> (u8, WaitForNode) {
        (self.scope.sort_rank(), self.node.clone())
    }

    pub(crate) fn actual_window(
        &self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<WorkloadWaitForBlockedNodeWindow> {
        match self.scope {
            WorkloadParallelDiagnosticScope::Resource => {
                summary.resource_wait_for_blocked_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::DataCache => {
                summary.data_cache_wait_for_blocked_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::Compute => {
                summary.compute_wait_for_blocked_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::Dma => {
                summary.dma_wait_for_blocked_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::FullSystem => {
                summary.full_system_wait_for_blocked_node_window(&self.node)
            }
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct WorkloadExpectedParallelWaitForTargetNodeWindow {
    scope: WorkloadParallelDiagnosticScope,
    node: WaitForNode,
    edge_count: usize,
    first_tick: Tick,
    last_tick: Tick,
}

impl WorkloadExpectedParallelWaitForTargetNodeWindow {
    pub fn new(
        scope: WorkloadParallelDiagnosticScope,
        node: WaitForNode,
        edge_count: usize,
        first_tick: Tick,
        last_tick: Tick,
    ) -> Result<Self, WorkloadError> {
        if edge_count == 0 {
            return Err(WorkloadError::ZeroExpectedParallelWaitForTargetNodeWindow { scope, node });
        }
        if first_tick > last_tick {
            return Err(
                WorkloadError::InvalidExpectedParallelWaitForTargetNodeWindow {
                    scope,
                    node,
                    first_tick,
                    last_tick,
                },
            );
        }
        Ok(Self {
            scope,
            node,
            edge_count,
            first_tick,
            last_tick,
        })
    }

    pub const fn scope(&self) -> WorkloadParallelDiagnosticScope {
        self.scope
    }

    pub const fn node(&self) -> &WaitForNode {
        &self.node
    }

    pub const fn edge_count(&self) -> usize {
        self.edge_count
    }

    pub const fn first_tick(&self) -> Tick {
        self.first_tick
    }

    pub const fn last_tick(&self) -> Tick {
        self.last_tick
    }

    pub(crate) fn sort_key(&self) -> (u8, WaitForNode) {
        (self.scope.sort_rank(), self.node.clone())
    }

    pub(crate) fn actual_window(
        &self,
        summary: &WorkloadParallelExecutionSummary,
    ) -> Option<WorkloadWaitForTargetNodeWindow> {
        match self.scope {
            WorkloadParallelDiagnosticScope::Resource => {
                summary.resource_wait_for_target_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::DataCache => {
                summary.data_cache_wait_for_target_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::Compute => {
                summary.compute_wait_for_target_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::Dma => {
                summary.dma_wait_for_target_node_window(&self.node)
            }
            WorkloadParallelDiagnosticScope::FullSystem => {
                summary.full_system_wait_for_target_node_window(&self.node)
            }
        }
    }
}
