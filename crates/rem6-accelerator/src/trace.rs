use rem6_kernel::{PartitionId, Tick};
use rem6_memory::MemoryRequestId;

use crate::AcceleratorCommandId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorTraceEvent {
    tick: Tick,
    kind: AcceleratorTraceKind,
}

impl AcceleratorTraceEvent {
    pub const fn new(tick: Tick, kind: AcceleratorTraceKind) -> Self {
        Self { tick, kind }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn kind(&self) -> &AcceleratorTraceKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceleratorTraceKind {
    Submitted {
        command: AcceleratorCommandId,
        source: PartitionId,
        target: PartitionId,
    },
    Started {
        command: AcceleratorCommandId,
        lane: u32,
        complete_at: Tick,
    },
    Completed {
        command: AcceleratorCommandId,
        lane: u32,
    },
    DmaReadIssued {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
    DmaReadCompleted {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
        bytes: u64,
    },
    DmaWriteIssued {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
    DmaWriteCompleted {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
}
