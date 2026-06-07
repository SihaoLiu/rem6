use rem6_kernel::Tick;
use rem6_memory::{Address, CacheLineLayout, MemoryRequestId, MemoryTargetId};
use rem6_traffic::{TrafficTraceErrorEvent, TrafficTraceErrorKind};

use crate::{RiscvDataCacheProtocol, RiscvSystemRun};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvTraceErrorRecord {
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    request_id: MemoryRequestId,
    error: TrafficTraceErrorKind,
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    address: Address,
    line: Address,
    size_bytes: Option<u64>,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl RiscvTraceErrorRecord {
    #[allow(clippy::too_many_arguments)]
    const fn new(
        tick: Tick,
        trace_tick: Tick,
        sequence: u64,
        request_id: MemoryRequestId,
        error: TrafficTraceErrorKind,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        address: Address,
        line: Address,
        size_bytes: Option<u64>,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            tick,
            trace_tick,
            sequence,
            request_id,
            error,
            protocol,
            target,
            address,
            line,
            size_bytes,
            trace_packet_id,
            trace_pc,
        }
    }

    pub(crate) fn from_trace_error(
        tick: Tick,
        request_id: MemoryRequestId,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        layout: CacheLineLayout,
        event: TrafficTraceErrorEvent,
        fallback_address: Option<Address>,
    ) -> Option<Self> {
        let address = event.address().or(fallback_address)?;
        Some(Self::new(
            tick,
            event.tick(),
            event.sequence(),
            request_id,
            event.kind(),
            protocol,
            target,
            address,
            layout.line_address(address),
            event.size_bytes(),
            event.trace_packet_id(),
            event.trace_pc(),
        ))
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

    pub const fn request_id(&self) -> MemoryRequestId {
        self.request_id
    }

    pub const fn error(&self) -> TrafficTraceErrorKind {
        self.error
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

    pub const fn size_bytes(&self) -> Option<u64> {
        self.size_bytes
    }

    pub const fn trace_packet_id(&self) -> Option<u64> {
        self.trace_packet_id
    }

    pub const fn trace_pc(&self) -> Option<Address> {
        self.trace_pc
    }
}

impl RiscvSystemRun {
    pub fn with_data_cache_error_records(
        mut self,
        data_cache_error_records: Vec<RiscvTraceErrorRecord>,
    ) -> Self {
        self.data_cache_error_records = data_cache_error_records;
        self
    }

    pub fn data_cache_error_records(&self) -> &[RiscvTraceErrorRecord] {
        &self.data_cache_error_records
    }

    pub fn data_cache_error_count(&self) -> usize {
        self.data_cache_error_records.len()
    }

    pub fn has_data_cache_errors(&self) -> bool {
        !self.data_cache_error_records.is_empty()
    }

    pub fn with_trace_error_records(
        mut self,
        trace_error_records: Vec<RiscvTraceErrorRecord>,
    ) -> Self {
        self.trace_error_records = trace_error_records;
        self
    }

    pub fn trace_error_records(&self) -> &[RiscvTraceErrorRecord] {
        &self.trace_error_records
    }

    pub fn trace_error_count(&self) -> usize {
        self.trace_error_records.len()
    }

    pub fn has_trace_errors(&self) -> bool {
        !self.trace_error_records.is_empty()
    }
}
