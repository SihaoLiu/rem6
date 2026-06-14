use rem6_kernel::Tick;
use rem6_memory::Address;

use crate::{GpuKernelId, GpuWorkgroupId};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum GpuMemoryAccessKind {
    Read,
    Write,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct GpuCoalescedMemoryAccessContext {
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
    compute_unit: u32,
    slot: u32,
    completed_at: Tick,
}

impl GpuCoalescedMemoryAccessContext {
    pub const fn new(
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
        completed_at: Tick,
    ) -> Self {
        Self {
            kernel,
            workgroup,
            compute_unit,
            slot,
            completed_at,
        }
    }

    pub const fn kernel(self) -> GpuKernelId {
        self.kernel
    }

    pub const fn workgroup(self) -> GpuWorkgroupId {
        self.workgroup
    }

    pub const fn compute_unit(self) -> u32 {
        self.compute_unit
    }

    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn completed_at(self) -> Tick {
        self.completed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuCoalescedMemoryAccess {
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
    compute_unit: u32,
    slot: u32,
    completed_at: Tick,
    instruction_index: usize,
    kind: GpuMemoryAccessKind,
    line: Address,
    access_count: u32,
    byte_count: u64,
}

impl GpuCoalescedMemoryAccess {
    pub const fn new(
        context: GpuCoalescedMemoryAccessContext,
        instruction_index: usize,
        kind: GpuMemoryAccessKind,
        line: Address,
        access_count: u32,
        byte_count: u64,
    ) -> Self {
        Self {
            kernel: context.kernel,
            workgroup: context.workgroup,
            compute_unit: context.compute_unit,
            slot: context.slot,
            completed_at: context.completed_at,
            instruction_index,
            kind,
            line,
            access_count,
            byte_count,
        }
    }

    pub const fn context(&self) -> GpuCoalescedMemoryAccessContext {
        GpuCoalescedMemoryAccessContext::new(
            self.kernel,
            self.workgroup,
            self.compute_unit,
            self.slot,
            self.completed_at,
        )
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

    pub const fn completed_at(&self) -> Tick {
        self.completed_at
    }

    pub const fn instruction_index(&self) -> usize {
        self.instruction_index
    }

    pub const fn kind(&self) -> GpuMemoryAccessKind {
        self.kind
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn access_count(&self) -> u32 {
        self.access_count
    }

    pub const fn byte_count(&self) -> u64 {
        self.byte_count
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct GpuCoalescedMemoryAccessDelta {
    instruction_index: usize,
    kind: GpuMemoryAccessKind,
    line: Address,
    access_count: u32,
    byte_count: u64,
}

impl GpuCoalescedMemoryAccessDelta {
    pub(crate) const fn new(
        instruction_index: usize,
        kind: GpuMemoryAccessKind,
        line: Address,
        access_count: u32,
        byte_count: u64,
    ) -> Self {
        Self {
            instruction_index,
            kind,
            line,
            access_count,
            byte_count,
        }
    }

    pub(crate) const fn instruction_index(self) -> usize {
        self.instruction_index
    }

    pub(crate) const fn kind(self) -> GpuMemoryAccessKind {
        self.kind
    }

    pub(crate) const fn line(self) -> Address {
        self.line
    }

    pub(crate) const fn access_count(self) -> u32 {
        self.access_count
    }

    pub(crate) const fn byte_count(self) -> u64 {
        self.byte_count
    }
}
