use rem6_kernel::Tick;

use crate::{
    AcceleratorCommand, AcceleratorCompletion, AcceleratorDmaCompletion,
    AcceleratorPendingDmaWrite, AcceleratorTraceEvent,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorEngineSnapshot {
    lane_busy_until: Vec<Tick>,
    queued_commands: Vec<AcceleratorQueuedCommandSnapshot>,
    trace: Vec<AcceleratorTraceEvent>,
    completed: Vec<AcceleratorCompletion>,
    pending_dma_writes: Vec<AcceleratorPendingDmaWrite>,
    dma_completions: Vec<AcceleratorDmaCompletion>,
}

impl AcceleratorEngineSnapshot {
    pub fn new(
        lane_busy_until: Vec<Tick>,
        trace: Vec<AcceleratorTraceEvent>,
        completed: Vec<AcceleratorCompletion>,
        pending_dma_writes: Vec<AcceleratorPendingDmaWrite>,
        dma_completions: Vec<AcceleratorDmaCompletion>,
    ) -> Self {
        Self {
            lane_busy_until,
            queued_commands: Vec::new(),
            trace,
            completed,
            pending_dma_writes,
            dma_completions,
        }
    }

    pub fn with_queued_commands(
        mut self,
        queued_commands: Vec<AcceleratorQueuedCommandSnapshot>,
    ) -> Self {
        self.queued_commands = queued_commands;
        self
    }

    pub fn lane_busy_until(&self) -> &[Tick] {
        &self.lane_busy_until
    }

    pub fn lane_count(&self) -> usize {
        self.lane_busy_until.len()
    }

    pub fn queued_commands(&self) -> &[AcceleratorQueuedCommandSnapshot] {
        &self.queued_commands
    }

    pub fn has_queued_commands(&self) -> bool {
        !self.queued_commands.is_empty()
    }

    pub fn trace(&self) -> &[AcceleratorTraceEvent] {
        &self.trace
    }

    pub fn completed(&self) -> &[AcceleratorCompletion] {
        &self.completed
    }

    pub fn pending_dma_writes(&self) -> &[AcceleratorPendingDmaWrite] {
        &self.pending_dma_writes
    }

    pub fn has_pending_dma_writes(&self) -> bool {
        !self.pending_dma_writes.is_empty()
    }

    pub fn dma_completions(&self) -> &[AcceleratorDmaCompletion] {
        &self.dma_completions
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorQueuedCommandSnapshot {
    command: AcceleratorCommand,
    lane: u32,
    queued_at: Tick,
    started_at: Tick,
    completed_at: Tick,
}

impl AcceleratorQueuedCommandSnapshot {
    pub const fn new(
        command: AcceleratorCommand,
        lane: u32,
        queued_at: Tick,
        started_at: Tick,
        completed_at: Tick,
    ) -> Self {
        Self {
            command,
            lane,
            queued_at,
            started_at,
            completed_at,
        }
    }

    pub const fn command(&self) -> &AcceleratorCommand {
        &self.command
    }

    pub const fn lane(&self) -> u32 {
        self.lane
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
