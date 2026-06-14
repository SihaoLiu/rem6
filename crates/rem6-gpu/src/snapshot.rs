use rem6_kernel::Tick;

use crate::{
    GpuDmaCompletion, GpuIsaProgram, GpuKernelId, GpuPendingDmaWrite, GpuTraceEvent,
    GpuWorkgroupCompletion, GpuWorkgroupId,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuDeviceSnapshot {
    slots: Vec<GpuSlotSnapshot>,
    trace: Vec<GpuTraceEvent>,
    completions: Vec<GpuWorkgroupCompletion>,
    pending_dma_writes: Vec<GpuPendingDmaWrite>,
    dma_completions: Vec<GpuDmaCompletion>,
    queued_isa_programs: Vec<GpuQueuedIsaProgramSnapshot>,
}

impl GpuDeviceSnapshot {
    pub fn new(
        slots: Vec<GpuSlotSnapshot>,
        trace: Vec<GpuTraceEvent>,
        completions: Vec<GpuWorkgroupCompletion>,
        pending_dma_writes: Vec<GpuPendingDmaWrite>,
        dma_completions: Vec<GpuDmaCompletion>,
    ) -> Self {
        Self {
            slots,
            trace,
            completions,
            pending_dma_writes,
            dma_completions,
            queued_isa_programs: Vec::new(),
        }
    }

    pub fn with_queued_isa_programs(
        mut self,
        queued_isa_programs: Vec<GpuQueuedIsaProgramSnapshot>,
    ) -> Self {
        self.queued_isa_programs = queued_isa_programs;
        self
    }

    pub fn slots(&self) -> &[GpuSlotSnapshot] {
        &self.slots
    }

    pub fn slot_count(&self) -> usize {
        self.slots.len()
    }

    pub fn has_queued_workgroups(&self) -> bool {
        self.slots.iter().any(|slot| !slot.queued().is_empty())
    }

    pub fn trace(&self) -> &[GpuTraceEvent] {
        &self.trace
    }

    pub fn completions(&self) -> &[GpuWorkgroupCompletion] {
        &self.completions
    }

    pub fn pending_dma_writes(&self) -> &[GpuPendingDmaWrite] {
        &self.pending_dma_writes
    }

    pub fn has_pending_dma_writes(&self) -> bool {
        !self.pending_dma_writes.is_empty()
    }

    pub fn dma_completions(&self) -> &[GpuDmaCompletion] {
        &self.dma_completions
    }

    pub fn queued_isa_programs(&self) -> &[GpuQueuedIsaProgramSnapshot] {
        &self.queued_isa_programs
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuSlotSnapshot {
    available_at: Tick,
    pump_scheduled: bool,
    queued: Vec<GpuQueuedWorkgroupSnapshot>,
}

impl GpuSlotSnapshot {
    pub fn new(
        available_at: Tick,
        pump_scheduled: bool,
        queued: Vec<GpuQueuedWorkgroupSnapshot>,
    ) -> Self {
        Self {
            available_at,
            pump_scheduled,
            queued,
        }
    }

    pub const fn available_at(&self) -> Tick {
        self.available_at
    }

    pub const fn pump_scheduled(&self) -> bool {
        self.pump_scheduled
    }

    pub fn queued(&self) -> &[GpuQueuedWorkgroupSnapshot] {
        &self.queued
    }

    pub fn is_idle(&self) -> bool {
        !self.pump_scheduled && self.queued.is_empty()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GpuQueuedWorkgroupSnapshot {
    kernel: GpuKernelId,
    workgroup: GpuWorkgroupId,
    compute_unit: u32,
    slot: u32,
    queued_at: Tick,
    started_at: Tick,
    completed_at: Tick,
}

impl GpuQueuedWorkgroupSnapshot {
    pub const fn new(
        kernel: GpuKernelId,
        workgroup: GpuWorkgroupId,
        compute_unit: u32,
        slot: u32,
        queued_at: Tick,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            kernel,
            workgroup,
            compute_unit,
            slot,
            queued_at,
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

    pub const fn queued_at(&self) -> Tick {
        self.queued_at
    }

    pub const fn started_at(&self) -> Tick {
        self.started_at
    }

    pub const fn completed_at(&self) -> Tick {
        self.completed_at
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuQueuedIsaProgramSnapshot {
    slot_index: usize,
    queue_index: usize,
    isa_program: GpuIsaProgram,
}

impl GpuQueuedIsaProgramSnapshot {
    pub fn new(slot_index: usize, queue_index: usize, isa_program: GpuIsaProgram) -> Self {
        Self {
            slot_index,
            queue_index,
            isa_program,
        }
    }

    pub const fn slot_index(&self) -> usize {
        self.slot_index
    }

    pub const fn queue_index(&self) -> usize {
        self.queue_index
    }

    pub fn isa_program(&self) -> &GpuIsaProgram {
        &self.isa_program
    }
}
