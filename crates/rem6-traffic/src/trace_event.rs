use rem6_memory::{AccessSize, Address};

use crate::common::TrafficRequestEvent;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceSyncKind {
    MemFence,
    MemSync,
}

impl TrafficTraceSyncKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::MemFence => "MemFenceReq",
            Self::MemSync => "MemSyncReq",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceTlbKind {
    ExternalSync,
}

impl TrafficTraceTlbKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::ExternalSync => "TlbiExtSync",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceHtmKind {
    Request,
    Abort,
}

impl TrafficTraceHtmKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::Request => "HTMReq",
            Self::Abort => "HTMAbort",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TrafficTraceDiagnosticKind {
    Print,
}

impl TrafficTraceDiagnosticKind {
    pub const fn gem5_name(self) -> &'static str {
        match self {
            Self::Print => "PrintReq",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceSyncEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceSyncKind,
    kernel_sync: bool,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceSyncEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceSyncKind,
        kernel_sync: bool,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            kernel_sync,
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceSyncKind {
        self.kind
    }

    pub const fn kernel_sync(self) -> bool {
        self.kernel_sync
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceTlbEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceTlbKind,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceTlbEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceTlbKind,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceTlbKind {
        self.kind
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceDiagnosticEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceDiagnosticKind,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceDiagnosticEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceDiagnosticKind,
        address: Option<Address>,
        size: Option<AccessSize>,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            size_bytes: match size {
                Some(size) => Some(size.bytes()),
                None => None,
            },
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceDiagnosticKind {
        self.kind
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TrafficTraceHtmEvent {
    tick: u64,
    sequence: u64,
    kind: TrafficTraceHtmKind,
    address: Option<Address>,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl TrafficTraceHtmEvent {
    pub(crate) const fn new(
        tick: u64,
        sequence: u64,
        kind: TrafficTraceHtmKind,
        address: Option<Address>,
        size: Option<AccessSize>,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            sequence,
            kind,
            address,
            size_bytes: match size {
                Some(size) => Some(size.bytes()),
                None => None,
            },
            trace_packet_id,
            trace_pc,
        }
    }

    pub const fn tick(self) -> u64 {
        self.tick
    }

    pub const fn sequence(self) -> u64 {
        self.sequence
    }

    pub const fn kind(self) -> TrafficTraceHtmKind {
        self.kind
    }

    pub const fn address(self) -> Option<Address> {
        self.address
    }

    pub const fn size_bytes(self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TrafficTraceEvent {
    Request(TrafficRequestEvent),
    Sync(TrafficTraceSyncEvent),
    Tlb(TrafficTraceTlbEvent),
    Htm(TrafficTraceHtmEvent),
    Diagnostic(TrafficTraceDiagnosticEvent),
}

impl TrafficTraceEvent {
    pub const fn tick(&self) -> u64 {
        match self {
            Self::Request(event) => event.tick(),
            Self::Sync(event) => event.tick(),
            Self::Tlb(event) => event.tick(),
            Self::Htm(event) => event.tick(),
            Self::Diagnostic(event) => event.tick(),
        }
    }

    pub const fn sequence(&self) -> u64 {
        match self {
            Self::Request(event) => event.sequence(),
            Self::Sync(event) => event.sequence(),
            Self::Tlb(event) => event.sequence(),
            Self::Htm(event) => event.sequence(),
            Self::Diagnostic(event) => event.sequence(),
        }
    }
}
