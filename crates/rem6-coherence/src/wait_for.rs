use std::sync::{Arc, Mutex};

use rem6_kernel::{DeadlockDiagnostic, Tick, WaitForEdgeKind, WaitForGraph, WaitForNode};
use rem6_memory::{AgentId, MemoryRequestId};

use crate::PartitionedDirectoryLineHarness;

#[derive(Clone, Debug, Default)]
pub(crate) struct CoherenceWaitFor {
    graph: Arc<Mutex<WaitForGraph>>,
}

impl CoherenceWaitFor {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn graph(&self) -> WaitForGraph {
        self.graph.lock().expect("wait-for graph lock").clone()
    }

    pub(crate) fn deadlock_diagnostic(&self) -> Option<DeadlockDiagnostic> {
        self.graph
            .lock()
            .expect("wait-for graph lock")
            .deadlock_diagnostic()
    }

    pub(crate) fn record_cache_busy(
        &self,
        agent: AgentId,
        line_address: u64,
        request: MemoryRequestId,
        tick: Tick,
    ) {
        self.graph
            .lock()
            .expect("wait-for graph lock")
            .record_wait(
                request_node(request),
                cache_line_node(agent, line_address),
                WaitForEdgeKind::Queue,
                tick,
            )
            .expect("coherence wait-for labels are generated from typed ids");
    }

    pub(crate) fn clear_cache_line(&self, agent: AgentId, line_address: u64) -> usize {
        self.graph
            .lock()
            .expect("wait-for graph lock")
            .clear_waits_to(&cache_line_node(agent, line_address))
    }
}

impl PartitionedDirectoryLineHarness {
    pub fn wait_for_graph(&self) -> WaitForGraph {
        self.wait_for.graph()
    }

    pub fn deadlock_diagnostic(&self) -> Option<DeadlockDiagnostic> {
        self.wait_for.deadlock_diagnostic()
    }
}

fn request_node(request: MemoryRequestId) -> WaitForNode {
    WaitForNode::transaction(format!(
        "memory.{}.{}",
        request.agent().get(),
        request.sequence()
    ))
    .expect("request wait-for label is generated from numeric ids")
}

fn cache_line_node(agent: AgentId, line_address: u64) -> WaitForNode {
    WaitForNode::resource(format!("cache.{}.line.{line_address:x}", agent.get()))
        .expect("cache wait-for label is generated from numeric ids")
}
