use rem6_kernel::Tick;
use rem6_memory::Address;
use rem6_traffic::{TrafficTraceErrorKind, TrafficTraceSyncEvent, TrafficTraceSyncKind};

use crate::TrafficTraceReplayOrder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RiscvWorkloadTraceSyncOutcome {
    Ack,
    Failure { error: TrafficTraceErrorKind },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceSyncRecord {
    completion_tick: Tick,
    trace_tick: Tick,
    trace_sequence: u64,
    source_tick: Tick,
    source_sequence: u64,
    kind: TrafficTraceSyncKind,
    kernel_sync: bool,
    invalidates_l1: bool,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
    outcome: RiscvWorkloadTraceSyncOutcome,
}

impl RiscvWorkloadTraceSyncRecord {
    pub(super) fn ack(
        completion_tick: Tick,
        sync: TrafficTraceSyncEvent,
        trace_order: TrafficTraceReplayOrder,
    ) -> Self {
        Self::new(
            completion_tick,
            sync,
            trace_order,
            RiscvWorkloadTraceSyncOutcome::Ack,
        )
    }

    pub(super) fn failure(
        completion_tick: Tick,
        sync: TrafficTraceSyncEvent,
        trace_order: TrafficTraceReplayOrder,
        error: TrafficTraceErrorKind,
    ) -> Self {
        Self::new(
            completion_tick,
            sync,
            trace_order,
            RiscvWorkloadTraceSyncOutcome::Failure { error },
        )
    }

    fn new(
        completion_tick: Tick,
        sync: TrafficTraceSyncEvent,
        trace_order: TrafficTraceReplayOrder,
        outcome: RiscvWorkloadTraceSyncOutcome,
    ) -> Self {
        Self {
            completion_tick,
            trace_tick: trace_order.tick(),
            trace_sequence: trace_order.sequence(),
            source_tick: sync.tick(),
            source_sequence: sync.sequence(),
            kind: sync.kind(),
            kernel_sync: sync.kernel_sync(),
            invalidates_l1: sync.invalidates_l1(),
            trace_packet_id: sync.trace_packet_id(),
            trace_pc: sync.trace_pc(),
            outcome,
        }
    }

    pub const fn completion_tick(&self) -> Tick {
        self.completion_tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn trace_sequence(&self) -> u64 {
        self.trace_sequence
    }

    pub const fn source_tick(&self) -> Tick {
        self.source_tick
    }

    pub const fn source_sequence(&self) -> u64 {
        self.source_sequence
    }

    pub const fn kind(&self) -> TrafficTraceSyncKind {
        self.kind
    }

    pub const fn kernel_sync(&self) -> bool {
        self.kernel_sync
    }

    pub const fn invalidates_l1(&self) -> bool {
        self.invalidates_l1
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }

    pub const fn outcome(&self) -> &RiscvWorkloadTraceSyncOutcome {
        &self.outcome
    }
}
