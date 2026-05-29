use rem6_kernel::{PartitionId, Tick};
use rem6_memory::MemoryRequestId;

use crate::{GpuDmaId, GpuKernelId, GpuWorkgroupId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuTraceEvent {
    tick: Tick,
    kind: GpuTraceKind,
}

impl GpuTraceEvent {
    pub const fn new(tick: Tick, kind: GpuTraceKind) -> Self {
        Self { tick, kind }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn kind(&self) -> &GpuTraceKind {
        &self.kind
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuTraceKind {
    LaunchSubmitted {
        kernel: GpuKernelId,
        source: PartitionId,
        target: PartitionId,
    },
    LaunchAccepted {
        kernel: GpuKernelId,
        workgroups: u32,
    },
    WorkgroupStarted {
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
        complete_at: Tick,
    },
    WorkgroupCompleted {
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
    },
    DmaReadIssued {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
    DmaReadCompleted {
        transfer: GpuDmaId,
        request: MemoryRequestId,
        bytes: u64,
    },
    DmaWriteIssued {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
    DmaWriteCompleted {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuWorkgroupCompletion {
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
    compute_unit: u32,
    slot: u32,
    started_at: Tick,
    completed_at: Tick,
}

impl GpuWorkgroupCompletion {
    pub const fn new(
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            kernel,
            workgroup,
            compute_unit,
            slot,
            started_at,
            completed_at,
        }
    }

    pub const fn kernel(&self) -> GpuKernelId {
        self.kernel
    }

    pub const fn workgroup(&self) -> GpuWorkgroupId {
        self.workgroup
    }

    pub const fn compute_unit(&self) -> u32 {
        self.compute_unit
    }

    pub const fn slot(&self) -> u32 {
        self.slot
    }

    pub const fn started_at(&self) -> Tick {
        self.started_at
    }

    pub const fn completed_at(&self) -> Tick {
        self.completed_at
    }
}
