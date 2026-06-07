use rem6_cpu::HtmTransactionUid;
use rem6_kernel::Tick;
use rem6_memory::{Address, CacheLineLayout, MemoryTargetId};
use rem6_traffic::TrafficTraceResponseEvent;

use crate::{RiscvDataCacheProtocol, RiscvSystemRun};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RiscvTraceHtmAccessKind {
    ReadSet,
    WriteSet,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvTraceHtmAccessRecord {
    kind: RiscvTraceHtmAccessKind,
    tick: Tick,
    trace_tick: Tick,
    sequence: u64,
    transaction_uid: HtmTransactionUid,
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    address: Address,
    line: Address,
    size_bytes: u64,
    trace_packet_id: Option<u64>,
    trace_pc: Option<Address>,
}

impl RiscvTraceHtmAccessRecord {
    #[allow(clippy::too_many_arguments)]
    const fn new(
        kind: RiscvTraceHtmAccessKind,
        tick: Tick,
        trace_tick: Tick,
        sequence: u64,
        transaction_uid: HtmTransactionUid,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        address: Address,
        line: Address,
        size_bytes: u64,
        trace_packet_id: Option<u64>,
        trace_pc: Option<Address>,
    ) -> Self {
        Self {
            kind,
            tick,
            trace_tick,
            sequence,
            transaction_uid,
            protocol,
            target,
            address,
            line,
            size_bytes,
            trace_packet_id,
            trace_pc,
        }
    }

    pub(crate) fn from_trace_response(
        kind: RiscvTraceHtmAccessKind,
        tick: Tick,
        transaction_uid: HtmTransactionUid,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        layout: CacheLineLayout,
        event: TrafficTraceResponseEvent,
    ) -> Option<Self> {
        let address = event.address()?;
        let size_bytes = event.size_bytes()?;
        Some(Self::new(
            kind,
            tick,
            event.tick(),
            event.sequence(),
            transaction_uid,
            protocol,
            target,
            address,
            layout.line_address(address),
            size_bytes,
            event.trace_packet_id(),
            event.trace_pc(),
        ))
    }

    pub const fn kind(&self) -> RiscvTraceHtmAccessKind {
        self.kind
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

    pub const fn transaction_uid(&self) -> HtmTransactionUid {
        self.transaction_uid
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

impl RiscvSystemRun {
    pub fn with_trace_htm_access_records(
        mut self,
        trace_htm_access_records: Vec<RiscvTraceHtmAccessRecord>,
    ) -> Self {
        self.trace_htm_access_records = trace_htm_access_records;
        self
    }

    pub fn trace_htm_access_records(&self) -> &[RiscvTraceHtmAccessRecord] {
        &self.trace_htm_access_records
    }

    pub fn trace_htm_access_count(&self) -> usize {
        self.trace_htm_access_records.len()
    }

    pub fn has_trace_htm_accesses(&self) -> bool {
        !self.trace_htm_access_records.is_empty()
    }
}
