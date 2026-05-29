use rem6_kernel::{PartitionId, Tick};

use crate::AcceleratorError;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceleratorEngineId(u32);

impl AcceleratorEngineId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AcceleratorCommandId(u64);

impl AcceleratorCommandId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AcceleratorWaitForMarker {
    offset: usize,
}

impl AcceleratorWaitForMarker {
    pub(crate) const fn new(offset: usize) -> Self {
        Self { offset }
    }

    pub const fn offset(self) -> usize {
        self.offset
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceleratorCommandKind {
    GpuKernel { workgroups: u32 },
    NpuInference { tiles: u32 },
    DmaCopy { bytes: u64 },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorCommand {
    id: AcceleratorCommandId,
    kind: AcceleratorCommandKind,
    execution_latency: Tick,
}

impl AcceleratorCommand {
    pub fn new(
        id: AcceleratorCommandId,
        kind: AcceleratorCommandKind,
        execution_latency: Tick,
    ) -> Result<Self, AcceleratorError> {
        if execution_latency == 0 {
            return Err(AcceleratorError::ZeroExecutionLatency { command: id });
        }

        Ok(Self {
            id,
            kind,
            execution_latency,
        })
    }

    pub const fn id(&self) -> AcceleratorCommandId {
        self.id
    }

    pub const fn kind(&self) -> &AcceleratorCommandKind {
        &self.kind
    }

    pub const fn execution_latency(&self) -> Tick {
        self.execution_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorEngineConfig {
    id: AcceleratorEngineId,
    partition: PartitionId,
    lanes: u32,
}

impl AcceleratorEngineConfig {
    pub fn new(
        id: AcceleratorEngineId,
        partition: PartitionId,
        lanes: u32,
    ) -> Result<Self, AcceleratorError> {
        if lanes == 0 {
            return Err(AcceleratorError::ZeroLanes { engine: id });
        }

        Ok(Self {
            id,
            partition,
            lanes,
        })
    }

    pub const fn id(&self) -> AcceleratorEngineId {
        self.id
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn lanes(&self) -> u32 {
        self.lanes
    }
}
