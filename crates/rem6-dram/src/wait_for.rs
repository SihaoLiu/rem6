use rem6_kernel::{WaitForEdgeKind, WaitForGraph, WaitForNode};
use rem6_memory::{MemoryRequestId, MemoryTargetId};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DramWaitForMarker {
    pub(crate) offset: usize,
}

impl DramWaitForMarker {
    pub(crate) const fn new(offset: usize) -> Self {
        Self { offset }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DramWaitResource {
    Bank { parallel_port: u32, bank: u32 },
    Bus { parallel_port: u32 },
    NvmReadBuffer,
    NvmWriteQueue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct DramWaitRecord {
    pub(crate) request: MemoryRequestId,
    resource: DramWaitResource,
    kind: WaitForEdgeKind,
    first_cycle: u64,
    last_cycle: u64,
}

impl DramWaitRecord {
    pub(crate) fn bank_queue(
        request: MemoryRequestId,
        parallel_port: u32,
        bank: u32,
        first_cycle: u64,
        last_cycle: u64,
    ) -> Self {
        Self {
            request,
            resource: DramWaitResource::Bank {
                parallel_port,
                bank,
            },
            kind: WaitForEdgeKind::Queue,
            first_cycle,
            last_cycle,
        }
    }

    pub(crate) fn bus_resource(
        request: MemoryRequestId,
        parallel_port: u32,
        first_cycle: u64,
        last_cycle: u64,
    ) -> Self {
        Self {
            request,
            resource: DramWaitResource::Bus { parallel_port },
            kind: WaitForEdgeKind::Resource,
            first_cycle,
            last_cycle,
        }
    }

    pub(crate) fn nvm_read_buffer(
        request: MemoryRequestId,
        first_cycle: u64,
        last_cycle: u64,
    ) -> Self {
        Self {
            request,
            resource: DramWaitResource::NvmReadBuffer,
            kind: WaitForEdgeKind::Resource,
            first_cycle,
            last_cycle,
        }
    }

    pub(crate) fn nvm_write_queue(
        request: MemoryRequestId,
        first_cycle: u64,
        last_cycle: u64,
    ) -> Self {
        Self {
            request,
            resource: DramWaitResource::NvmWriteQueue,
            kind: WaitForEdgeKind::Resource,
            first_cycle,
            last_cycle,
        }
    }
}

pub(crate) fn record_dram_wait_interval(
    graph: &mut WaitForGraph,
    wait: &DramWaitRecord,
    target: Option<MemoryTargetId>,
) {
    let source = dram_request_node(wait.request, target);
    let target = dram_resource_node(wait.resource, target);
    graph
        .record_wait(source.clone(), target.clone(), wait.kind, wait.first_cycle)
        .expect("DRAM wait-for labels are generated from typed ids");
    if wait.last_cycle != wait.first_cycle {
        graph
            .record_wait(source, target, wait.kind, wait.last_cycle)
            .expect("DRAM wait-for labels are generated from typed ids");
    }
}

fn dram_request_node(request: MemoryRequestId, target: Option<MemoryTargetId>) -> WaitForNode {
    let label = if let Some(target) = target {
        format!(
            "dram.target.{}.agent.{}.request.{}",
            target.get(),
            request.agent().get(),
            request.sequence()
        )
    } else {
        format!(
            "dram.agent.{}.request.{}",
            request.agent().get(),
            request.sequence()
        )
    };
    WaitForNode::transaction(label).expect("DRAM request wait-for label uses numeric ids")
}

fn dram_resource_node(resource: DramWaitResource, target: Option<MemoryTargetId>) -> WaitForNode {
    let label = match (target, resource) {
        (
            Some(target),
            DramWaitResource::Bank {
                parallel_port,
                bank,
            },
        ) => format!(
            "dram.target.{}.port.{}.bank.{}",
            target.get(),
            parallel_port,
            bank
        ),
        (Some(target), DramWaitResource::Bus { parallel_port }) => {
            format!("dram.target.{}.port.{}.bus", target.get(), parallel_port)
        }
        (Some(target), DramWaitResource::NvmReadBuffer) => {
            format!("dram.target.{}.nvm.read_buffer", target.get())
        }
        (Some(target), DramWaitResource::NvmWriteQueue) => {
            format!("dram.target.{}.nvm.write_queue", target.get())
        }
        (
            None,
            DramWaitResource::Bank {
                parallel_port,
                bank,
            },
        ) => format!("dram.port.{}.bank.{}", parallel_port, bank),
        (None, DramWaitResource::Bus { parallel_port }) => {
            format!("dram.port.{}.bus", parallel_port)
        }
        (None, DramWaitResource::NvmReadBuffer) => "dram.nvm.read_buffer".to_string(),
        (None, DramWaitResource::NvmWriteQueue) => "dram.nvm.write_queue".to_string(),
    };
    WaitForNode::resource(label).expect("DRAM resource wait-for label uses numeric ids")
}

pub(crate) fn merge_wait_for_graph(target: &mut WaitForGraph, source: WaitForGraph) {
    for edge in source.edges() {
        target
            .record_wait(
                edge.source().clone(),
                edge.target().clone(),
                edge.kind(),
                edge.first_observed_tick(),
            )
            .expect("merged wait-for graph already contains valid labels");
        if edge.last_observed_tick() != edge.first_observed_tick() {
            target
                .record_wait(
                    edge.source().clone(),
                    edge.target().clone(),
                    edge.kind(),
                    edge.last_observed_tick(),
                )
                .expect("merged wait-for graph already contains valid labels");
        }
    }
}
