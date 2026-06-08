use rem6_kernel::Tick;
use rem6_memory::{Address, MemoryTargetId};
use rem6_traffic::{
    TrafficTraceCacheEvent, TrafficTraceCacheKind, TrafficTraceControlFailureRecord,
    TrafficTraceControlFailureSource, TrafficTraceErrorKind, TrafficTraceTlbEvent,
    TrafficTraceTlbKind,
};

use crate::RiscvDataCacheProtocol;

use super::data_cache_backend::WorkloadTraceCacheApplication;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceTlbSyncRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    kind: TrafficTraceTlbKind,
    flushed_entry_count: usize,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl RiscvWorkloadTraceTlbSyncRecord {
    pub(super) fn from_trace_tlb_event(
        tick: Tick,
        event: TrafficTraceTlbEvent,
        flushed_entry_count: usize,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            kind: event.kind(),
            flushed_entry_count,
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
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

    pub const fn kind(&self) -> TrafficTraceTlbKind {
        self.kind
    }

    pub const fn flushed_entry_count(&self) -> usize {
        self.flushed_entry_count
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceSidebandFailureRecord {
    tick: Tick,
    source: TrafficTraceControlFailureSource,
    error: TrafficTraceErrorKind,
}

impl RiscvWorkloadTraceSidebandFailureRecord {
    pub(super) fn from_control_failure(
        tick: Tick,
        record: TrafficTraceControlFailureRecord,
    ) -> Option<Self> {
        let source = record.source()?;
        if !control_failure_source_is_non_response_sideband(source) {
            return None;
        }
        Some(Self {
            tick,
            source,
            error: record.failure().error(),
        })
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn source(&self) -> TrafficTraceControlFailureSource {
        self.source
    }

    pub const fn error(&self) -> TrafficTraceErrorKind {
        self.error
    }

    pub const fn trace_tick(&self) -> Tick {
        match self.source {
            TrafficTraceControlFailureSource::Sync(source) => source.tick(),
            TrafficTraceControlFailureSource::Tlb(source) => source.tick(),
            TrafficTraceControlFailureSource::Cache(source) => source.tick(),
            TrafficTraceControlFailureSource::Htm(source) => source.tick(),
            TrafficTraceControlFailureSource::Diagnostic(source) => source.tick(),
        }
    }

    pub const fn sequence(&self) -> u64 {
        match self.source {
            TrafficTraceControlFailureSource::Sync(source) => source.sequence(),
            TrafficTraceControlFailureSource::Tlb(source) => source.sequence(),
            TrafficTraceControlFailureSource::Cache(source) => source.sequence(),
            TrafficTraceControlFailureSource::Htm(source) => source.sequence(),
            TrafficTraceControlFailureSource::Diagnostic(source) => source.sequence(),
        }
    }

    pub const fn address(&self) -> Option<Address> {
        match self.source {
            TrafficTraceControlFailureSource::Sync(_)
            | TrafficTraceControlFailureSource::Tlb(_) => None,
            TrafficTraceControlFailureSource::Cache(source) => Some(source.address()),
            TrafficTraceControlFailureSource::Htm(source) => source.address(),
            TrafficTraceControlFailureSource::Diagnostic(source) => source.address(),
        }
    }

    pub const fn size_bytes(&self) -> Option<u64> {
        match self.source {
            TrafficTraceControlFailureSource::Sync(_)
            | TrafficTraceControlFailureSource::Tlb(_) => None,
            TrafficTraceControlFailureSource::Cache(source) => Some(source.size_bytes()),
            TrafficTraceControlFailureSource::Htm(source) => source.size_bytes(),
            TrafficTraceControlFailureSource::Diagnostic(source) => source.size_bytes(),
        }
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        match self.source {
            TrafficTraceControlFailureSource::Sync(source) => source.trace_packet_id(),
            TrafficTraceControlFailureSource::Tlb(source) => source.trace_packet_id(),
            TrafficTraceControlFailureSource::Cache(source) => source.trace_packet_id(),
            TrafficTraceControlFailureSource::Htm(source) => source.trace_packet_id(),
            TrafficTraceControlFailureSource::Diagnostic(source) => source.trace_packet_id(),
        }
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        match self.source {
            TrafficTraceControlFailureSource::Sync(source) => source.trace_pc(),
            TrafficTraceControlFailureSource::Tlb(source) => source.trace_pc(),
            TrafficTraceControlFailureSource::Cache(source) => source.trace_pc(),
            TrafficTraceControlFailureSource::Htm(source) => source.trace_pc(),
            TrafficTraceControlFailureSource::Diagnostic(source) => source.trace_pc(),
        }
    }
}

fn control_failure_source_is_non_response_sideband(
    source: TrafficTraceControlFailureSource,
) -> bool {
    match source {
        TrafficTraceControlFailureSource::Sync(_) => false,
        TrafficTraceControlFailureSource::Tlb(_)
        | TrafficTraceControlFailureSource::Cache(_)
        | TrafficTraceControlFailureSource::Diagnostic(_) => true,
        TrafficTraceControlFailureSource::Htm(source) => !source.requires_response(),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvWorkloadTraceCacheFlushRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    kind: TrafficTraceCacheKind,
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    address: Address,
    line: Address,
    size_bytes: u64,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl RiscvWorkloadTraceCacheFlushRecord {
    pub(super) fn from_trace_cache_event(
        tick: Tick,
        event: TrafficTraceCacheEvent,
        application: WorkloadTraceCacheApplication,
    ) -> Self {
        Self {
            tick,
            trace_tick: event.tick(),
            sequence: event.sequence(),
            kind: event.kind(),
            protocol: application.protocol(),
            target: application.target(),
            address: event.address(),
            line: application.line(),
            size_bytes: event.size_bytes(),
            trace_packet_id: event.trace_packet_id(),
            trace_pc: event.trace_pc(),
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

    pub const fn kind(&self) -> TrafficTraceCacheKind {
        self.kind
    }

    pub const fn protocol(&self) -> RiscvDataCacheProtocol {
        self.protocol
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn address(&self) -> Address {
        self.address
    }

    pub const fn line(&self) -> Address {
        self.line
    }

    pub const fn size_bytes(&self) -> u64 {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }
}
