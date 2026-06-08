use rem6_cpu::{HtmTransactionUid, RiscvClusterHtmAbortOutcome, RiscvClusterHtmBeginOutcome};
use rem6_kernel::Tick;
use rem6_memory::Address;
use rem6_traffic::{TrafficTraceErrorKind, TrafficTraceHtmEvent};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceHtmBeginRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
    cluster_outcome: Option<RiscvClusterHtmBeginOutcome>,
    control_error: Option<TrafficTraceErrorKind>,
}

impl RiscvWorkloadTraceHtmBeginRecord {
    pub(super) fn new(
        tick: Tick,
        event: TrafficTraceHtmEvent,
        cluster_outcome: Option<RiscvClusterHtmBeginOutcome>,
        control_error: Option<TrafficTraceErrorKind>,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            address: event.address(),
            size_bytes: event.size_bytes(),
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
            cluster_outcome,
            control_error,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }

    pub fn cluster_outcome(&self) -> &RiscvClusterHtmBeginOutcome {
        self.cluster_outcome
            .as_ref()
            .expect("trace HTM begin record has cluster outcome")
    }

    pub fn cluster_outcome_option(&self) -> Option<&RiscvClusterHtmBeginOutcome> {
        self.cluster_outcome.as_ref()
    }

    pub const fn control_error(&self) -> Option<TrafficTraceErrorKind> {
        self.control_error
    }

    pub const fn begin_uid(&self) -> Option<HtmTransactionUid> {
        match &self.cluster_outcome {
            Some(RiscvClusterHtmBeginOutcome::Begun { begin, .. }) => Some(begin.uid()),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceHtmAbortRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
    cluster_outcome: RiscvClusterHtmAbortOutcome,
}

impl RiscvWorkloadTraceHtmAbortRecord {
    pub(super) fn new(
        tick: Tick,
        event: TrafficTraceHtmEvent,
        cluster_outcome: RiscvClusterHtmAbortOutcome,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            address: event.address(),
            size_bytes: event.size_bytes(),
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
            cluster_outcome,
        }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn trace_tick(&self) -> Tick {
        self.trace_tick
    }

    pub const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub const fn address(&self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }

    pub const fn cluster_outcome(&self) -> &RiscvClusterHtmAbortOutcome {
        &self.cluster_outcome
    }
}
