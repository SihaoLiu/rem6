use rem6_kernel::Tick;
use rem6_memory::{Address, MemoryTargetId};
use rem6_traffic::{
    TrafficTraceCacheEvent, TrafficTraceCacheKind, TrafficTraceTlbEvent, TrafficTraceTlbKind,
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
