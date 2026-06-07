use rem6_kernel::Tick;
use rem6_memory::{Address, MemoryTargetId};

use crate::{RiscvDataCacheProtocol, RiscvSystemRun};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum RiscvTraceDiagnosticKind {
    DataCacheLine,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvTraceDiagnosticRecord {
    kind: RiscvTraceDiagnosticKind,
    tick: Tick,
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    address: Address,
    line: Address,
    cached_copy_count: usize,
    backing_line_present: bool,
}

impl RiscvTraceDiagnosticRecord {
    pub const fn data_cache_line(
        tick: Tick,
        protocol: RiscvDataCacheProtocol,
        target: MemoryTargetId,
        address: Address,
        line: Address,
        cached_copy_count: usize,
        backing_line_present: bool,
    ) -> Self {
        Self {
            kind: RiscvTraceDiagnosticKind::DataCacheLine,
            tick,
            protocol,
            target,
            address,
            line,
            cached_copy_count,
            backing_line_present,
        }
    }

    pub const fn kind(&self) -> RiscvTraceDiagnosticKind {
        self.kind
    }

    pub const fn tick(&self) -> Tick {
        self.tick
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

    pub const fn cached_copy_count(&self) -> usize {
        self.cached_copy_count
    }

    pub const fn has_cached_copy(&self) -> bool {
        self.cached_copy_count != 0
    }

    pub const fn has_backing_line(&self) -> bool {
        self.backing_line_present
    }
}

impl RiscvSystemRun {
    pub fn with_trace_diagnostic_records(
        mut self,
        trace_diagnostic_records: Vec<RiscvTraceDiagnosticRecord>,
    ) -> Self {
        self.trace_diagnostic_records = trace_diagnostic_records;
        self
    }

    pub fn trace_diagnostic_records(&self) -> &[RiscvTraceDiagnosticRecord] {
        &self.trace_diagnostic_records
    }

    pub fn trace_diagnostic_count(&self) -> usize {
        self.trace_diagnostic_records.len()
    }

    pub fn has_trace_diagnostics(&self) -> bool {
        !self.trace_diagnostic_records.is_empty()
    }
}
