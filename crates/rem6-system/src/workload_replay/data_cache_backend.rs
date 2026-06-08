use std::collections::{BTreeMap, BTreeSet};

use rem6_cache::{
    ChiCacheControllerSnapshot, MesiCacheControllerSnapshot, MoesiCacheControllerSnapshot,
    MsiCacheControllerSnapshot,
};
use rem6_coherence::{
    ChiHarnessError, LineBackingStore, MesiHarnessError, MoesiHarnessError,
    PartitionedCacheAgentConfig, PartitionedChiDirectoryLineHarness,
    PartitionedChiDirectoryLineHarnessSnapshot, PartitionedDirectoryLineHarness,
    PartitionedDirectoryLineHarnessSnapshot, PartitionedDramMemoryConfig,
    PartitionedMesiDirectoryLineHarness, PartitionedMesiDirectoryLineHarnessSnapshot,
    PartitionedMoesiDirectoryLineHarness, PartitionedMoesiDirectoryLineHarnessSnapshot,
};
use rem6_cpu::{HtmFailureCause, HtmTransactionUid};
use rem6_directory::{
    ChiDirectoryLineState, DirectoryLineState, MesiDirectoryLineState, MoesiDirectoryLineState,
};
use rem6_dram::DramMemorySnapshot;
use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{
    Address, CacheLineLayout, MemoryOperation, MemoryRequestId, MemoryResponse, MemoryTargetId,
};
use rem6_protocol_chi::{ChiCacheLine, ChiState};
use rem6_protocol_mesi::{MesiCacheLine, MesiState};
use rem6_protocol_moesi::{MoesiCacheLine, MoesiState};
use rem6_protocol_msi::{MsiCacheLine, MsiState};
use rem6_traffic::{
    TrafficTraceCacheEvent, TrafficTraceDiagnosticEvent, TrafficTraceErrorEvent,
    TrafficTraceHtmEvent, TrafficTraceResponseEvent, TrafficTraceSyncEvent,
};
use rem6_transport::{MemoryRouteId, RequestDelivery, TargetOutcome, TransportEndpointId};
use rem6_workload::{WorkloadDataCacheProtocol, WorkloadRiscvDataCache};

use super::cache_response::data_cache_response_result;
use super::RiscvWorkloadReplayError;
use crate::{
    RiscvDataCacheControllerError, RiscvDataCacheControllerErrorRecord, RiscvDataCacheProtocol,
    RiscvDataCacheRunRecord, RiscvTraceDiagnosticRecord, RiscvTraceErrorRecord,
    RiscvTraceHtmAccessKind, RiscvTraceHtmAccessRecord,
};

enum WorkloadDataCacheHarness {
    Msi(PartitionedDirectoryLineHarness),
    Mesi(PartitionedMesiDirectoryLineHarness),
    Moesi(PartitionedMoesiDirectoryLineHarness),
    Chi(PartitionedChiDirectoryLineHarness),
}

enum WorkloadDataCacheRollbackSnapshot {
    Msi(PartitionedDirectoryLineHarnessSnapshot),
    Mesi(PartitionedMesiDirectoryLineHarnessSnapshot),
    Moesi(PartitionedMoesiDirectoryLineHarnessSnapshot),
    Chi(PartitionedChiDirectoryLineHarnessSnapshot),
}

struct WorkloadDataCacheLineRollbackSnapshot {
    line: Address,
    snapshot: WorkloadDataCacheRollbackSnapshot,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct WorkloadDataCacheRollbackKey {
    route: MemoryRouteId,
    transaction_uid: HtmTransactionUid,
}

impl WorkloadDataCacheRollbackKey {
    const fn new(route: MemoryRouteId, transaction_uid: HtmTransactionUid) -> Self {
        Self {
            route,
            transaction_uid,
        }
    }
}

struct WorkloadDataCacheRollback {
    snapshots: BTreeMap<Address, WorkloadDataCacheLineRollbackSnapshot>,
    written_lines: BTreeSet<Address>,
}

impl WorkloadDataCacheRollback {
    fn new(snapshots: BTreeMap<Address, WorkloadDataCacheLineRollbackSnapshot>) -> Self {
        Self {
            snapshots,
            written_lines: BTreeSet::new(),
        }
    }

    fn mark_written(&mut self, line: Address) {
        self.written_lines.insert(line);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct WorkloadTraceCacheApplication {
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    line: Address,
}

impl WorkloadTraceCacheApplication {
    const fn new(protocol: RiscvDataCacheProtocol, target: MemoryTargetId, line: Address) -> Self {
        Self {
            protocol,
            target,
            line,
        }
    }

    pub(super) const fn protocol(self) -> RiscvDataCacheProtocol {
        self.protocol
    }

    pub(super) const fn target(self) -> MemoryTargetId {
        self.target
    }

    pub(super) const fn line(self) -> Address {
        self.line
    }
}

#[derive(Default)]
struct WorkloadDataCacheHtmAccessSet {
    read_lines: BTreeSet<Address>,
    write_lines: BTreeSet<Address>,
    memory_conflict: bool,
}

impl WorkloadDataCacheHtmAccessSet {
    fn record_read(&mut self, line: Address) {
        self.read_lines.insert(line);
    }

    fn record_write(&mut self, line: Address) {
        self.write_lines.insert(line);
    }

    fn conflicts_with_write(&self, line: Address) -> bool {
        self.read_lines.contains(&line) || self.write_lines.contains(&line)
    }

    fn mark_memory_conflict(&mut self) {
        self.memory_conflict = true;
    }

    fn abort_cause(&self) -> HtmFailureCause {
        if self.memory_conflict {
            HtmFailureCause::Memory
        } else {
            HtmFailureCause::Other
        }
    }
}

pub(super) enum WorkloadDataCacheLineMemory {
    Line(Vec<u8>),
    Dram(Box<PartitionedDramMemoryConfig>),
}

pub(super) struct WorkloadDataCacheLineBackend {
    protocol: RiscvDataCacheProtocol,
    target: MemoryTargetId,
    layout: CacheLineLayout,
    line: Address,
    harness: WorkloadDataCacheHarness,
    records: Vec<RiscvDataCacheRunRecord>,
    trace_diagnostic_records: Vec<RiscvTraceDiagnosticRecord>,
    trace_error_records: Vec<RiscvTraceErrorRecord>,
    trace_htm_access_records: Vec<RiscvTraceHtmAccessRecord>,
    error: Option<RiscvWorkloadReplayError>,
}

impl WorkloadDataCacheLineBackend {
    pub(super) fn new(
        config: &WorkloadRiscvDataCache,
        layout: CacheLineLayout,
        line_address: Address,
        memory: WorkloadDataCacheLineMemory,
        agents: Vec<PartitionedCacheAgentConfig>,
    ) -> Result<Self, RiscvWorkloadReplayError> {
        let target = MemoryTargetId::new(config.memory_target());
        let line = layout.line_address(line_address);
        let directory_partition = PartitionId::new(config.directory_partition());
        let directory_endpoint = TransportEndpointId::new(config.directory_endpoint())
            .map_err(RiscvWorkloadReplayError::Transport)?;
        let (protocol, harness) = match config.protocol() {
            WorkloadDataCacheProtocol::Msi => (
                RiscvDataCacheProtocol::Msi,
                WorkloadDataCacheHarness::Msi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            PartitionedDirectoryLineHarness::new(
                                layout,
                                line,
                                LineBackingStore::new(layout, line, line_data)
                                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?,
                                directory_partition,
                                directory_endpoint,
                                agents,
                            )
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                *memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Mesi => (
                RiscvDataCacheProtocol::Mesi,
                WorkloadDataCacheHarness::Mesi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            LineBackingStore::new(layout, line, line_data)
                                .map_err(MesiHarnessError::Backing)
                                .and_then(|backing| {
                                    PartitionedMesiDirectoryLineHarness::new(
                                        layout,
                                        line,
                                        backing,
                                        directory_partition,
                                        directory_endpoint,
                                        agents,
                                    )
                                })
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedMesiDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                *memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Moesi => (
                RiscvDataCacheProtocol::Moesi,
                WorkloadDataCacheHarness::Moesi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            LineBackingStore::new(layout, line, line_data)
                                .map_err(MoesiHarnessError::Backing)
                                .and_then(|backing| {
                                    PartitionedMoesiDirectoryLineHarness::new(
                                        layout,
                                        line,
                                        backing,
                                        directory_partition,
                                        directory_endpoint,
                                        agents,
                                    )
                                })
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedMoesiDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                *memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::MoesiDataCache)?,
                ),
            ),
            WorkloadDataCacheProtocol::Chi => (
                RiscvDataCacheProtocol::Chi,
                WorkloadDataCacheHarness::Chi(
                    match memory {
                        WorkloadDataCacheLineMemory::Line(line_data) => {
                            LineBackingStore::new(layout, line, line_data)
                                .map_err(ChiHarnessError::Backing)
                                .and_then(|backing| {
                                    PartitionedChiDirectoryLineHarness::new(
                                        layout,
                                        line,
                                        backing,
                                        directory_partition,
                                        directory_endpoint,
                                        agents,
                                    )
                                })
                        }
                        WorkloadDataCacheLineMemory::Dram(memory) => {
                            PartitionedChiDirectoryLineHarness::new_with_dram_memory(
                                layout,
                                line,
                                directory_partition,
                                directory_endpoint,
                                *memory,
                                agents,
                            )
                        }
                    }
                    .map_err(RiscvWorkloadReplayError::ChiDataCache)?,
                ),
            ),
        };

        Ok(Self {
            protocol,
            target,
            layout,
            line,
            harness,
            records: Vec::new(),
            trace_diagnostic_records: Vec::new(),
            trace_error_records: Vec::new(),
            trace_htm_access_records: Vec::new(),
            error: None,
        })
    }

    fn records(&self) -> Vec<RiscvDataCacheRunRecord> {
        self.records.clone()
    }

    fn trace_diagnostic_records(&self) -> Vec<RiscvTraceDiagnosticRecord> {
        self.trace_diagnostic_records.clone()
    }

    fn trace_error_records(&self) -> Vec<RiscvTraceErrorRecord> {
        self.trace_error_records.clone()
    }

    fn trace_htm_access_records(&self) -> Vec<RiscvTraceHtmAccessRecord> {
        self.trace_htm_access_records.clone()
    }

    fn take_error(&mut self) -> Option<RiscvWorkloadReplayError> {
        self.error.take()
    }

    fn target(&self) -> MemoryTargetId {
        self.target
    }

    fn line(&self) -> Address {
        self.line
    }

    fn accepts_delivery(&self, delivery: &RequestDelivery) -> bool {
        delivery.request().operation() != MemoryOperation::InstructionFetch
    }

    fn accepts_trace_cache_event(&self, event: TrafficTraceCacheEvent) -> bool {
        event.is_flush() && self.layout.line_address(event.address()) == self.line
    }

    fn accepts_trace_response_event(&self, event: TrafficTraceResponseEvent) -> bool {
        event.address().is_some_and(|address| {
            (event.invalidates_line() || event.cleans_line())
                && self.layout.line_address(address) == self.line
        })
    }

    fn accepts_trace_error_event(
        &self,
        event: TrafficTraceErrorEvent,
        fallback_address: Option<Address>,
    ) -> bool {
        event
            .address()
            .or(fallback_address)
            .is_some_and(|address| self.layout.line_address(address) == self.line)
    }

    fn accepts_trace_htm_access_event(&self, event: TrafficTraceResponseEvent) -> bool {
        event.address().is_some_and(|address| {
            self.layout.line_address(address) == self.line
                && event.size_bytes().is_some()
                && !event.is_prefetch()
                && (event.is_read() || event.is_write())
        })
    }

    fn accepts_trace_diagnostic_event(&self, event: TrafficTraceDiagnosticEvent) -> bool {
        event.address().is_some_and(|address| {
            event.is_print() && self.layout.line_address(address) == self.line
        })
    }

    fn final_line_data(&self) -> Result<Vec<u8>, RiscvWorkloadReplayError> {
        match &self.harness {
            WorkloadDataCacheHarness::Msi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?;
                if let Some(data) = snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                {
                    return Ok(data);
                }
                snapshot
                    .backing()
                    .map(|backing| backing.data().to_vec())
                    .or_else(|| {
                        snapshot
                            .dram_memory()
                            .and_then(|dram| dram_snapshot_line_data(dram, self.target, self.line))
                    })
                    .ok_or(RiscvWorkloadReplayError::MissingDataCacheLine)
            }
            WorkloadDataCacheHarness::Mesi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)?;
                Ok(snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                    .unwrap_or_else(|| snapshot.backing().data().to_vec()))
            }
            WorkloadDataCacheHarness::Moesi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MoesiDataCache)?;
                Ok(snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                    .unwrap_or_else(|| snapshot.backing().data().to_vec()))
            }
            WorkloadDataCacheHarness::Chi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::ChiDataCache)?;
                Ok(snapshot
                    .caches()
                    .values()
                    .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
                    .unwrap_or_else(|| snapshot.backing().data().to_vec()))
            }
        }
    }

    fn apply_trace_cache_event(
        &mut self,
        event: TrafficTraceCacheEvent,
    ) -> Option<WorkloadTraceCacheApplication> {
        if !self.accepts_trace_cache_event(event) {
            return None;
        }

        if let Err(error) = self.try_invalidate_trace_line() {
            let record = RiscvDataCacheControllerErrorRecord::from_trace_cache_event(
                event.tick(),
                event,
                self.protocol,
                self.target,
                self.line,
                error,
            );
            self.error = Some(RiscvWorkloadReplayError::DataCacheController {
                record: Box::new(record),
            });
        }
        Some(WorkloadTraceCacheApplication::new(
            self.protocol,
            self.target,
            self.line,
        ))
    }

    fn apply_trace_response_event(&mut self, event: TrafficTraceResponseEvent) -> bool {
        if !self.accepts_trace_response_event(event) {
            return false;
        }

        let result = if event.invalidates_line() {
            self.try_invalidate_trace_line()
        } else if event.cleans_line() {
            self.try_clean_trace_line()
        } else {
            Ok(())
        };
        if let Err(error) = result {
            let record = RiscvDataCacheControllerErrorRecord::from_trace_response_event(
                event.tick(),
                event,
                self.protocol,
                self.target,
                self.line,
                error,
            );
            self.error = Some(RiscvWorkloadReplayError::DataCacheController {
                record: Box::new(record),
            });
        }
        true
    }

    fn record_trace_error_event(
        &mut self,
        tick: Tick,
        request_id: MemoryRequestId,
        event: TrafficTraceErrorEvent,
        fallback_address: Option<Address>,
    ) -> Option<RiscvTraceErrorRecord> {
        if !self.accepts_trace_error_event(event, fallback_address) {
            return None;
        }

        let record = RiscvTraceErrorRecord::from_trace_error(
            tick,
            request_id,
            self.protocol,
            self.target,
            self.layout,
            event,
            fallback_address,
        )?;
        self.trace_error_records.push(record);
        Some(record)
    }

    fn record_trace_htm_access_event(
        &mut self,
        tick: Tick,
        transaction_uid: HtmTransactionUid,
        event: TrafficTraceResponseEvent,
    ) -> Vec<RiscvTraceHtmAccessRecord> {
        if !self.accepts_trace_htm_access_event(event) {
            return Vec::new();
        }

        let mut records = Vec::new();
        if event.is_read() {
            if let Some(record) = RiscvTraceHtmAccessRecord::from_trace_response(
                RiscvTraceHtmAccessKind::ReadSet,
                tick,
                transaction_uid,
                self.protocol,
                self.target,
                self.layout,
                event,
            ) {
                self.trace_htm_access_records.push(record);
                records.push(record);
            }
        }
        if event.is_write() {
            if let Some(record) = RiscvTraceHtmAccessRecord::from_trace_response(
                RiscvTraceHtmAccessKind::WriteSet,
                tick,
                transaction_uid,
                self.protocol,
                self.target,
                self.layout,
                event,
            ) {
                self.trace_htm_access_records.push(record);
                records.push(record);
            }
        }
        records
    }

    fn htm_rollback_snapshot(
        &self,
    ) -> Result<WorkloadDataCacheLineRollbackSnapshot, RiscvDataCacheControllerError> {
        let snapshot = match &self.harness {
            WorkloadDataCacheHarness::Msi(harness) => WorkloadDataCacheRollbackSnapshot::Msi(
                harness
                    .quiescent_snapshot()
                    .map_err(RiscvDataCacheControllerError::Msi)?,
            ),
            WorkloadDataCacheHarness::Mesi(harness) => WorkloadDataCacheRollbackSnapshot::Mesi(
                harness
                    .quiescent_snapshot()
                    .map_err(RiscvDataCacheControllerError::Mesi)?,
            ),
            WorkloadDataCacheHarness::Moesi(harness) => WorkloadDataCacheRollbackSnapshot::Moesi(
                harness
                    .quiescent_snapshot()
                    .map_err(RiscvDataCacheControllerError::Moesi)?,
            ),
            WorkloadDataCacheHarness::Chi(harness) => WorkloadDataCacheRollbackSnapshot::Chi(
                harness
                    .quiescent_snapshot()
                    .map_err(RiscvDataCacheControllerError::Chi)?,
            ),
        };
        Ok(WorkloadDataCacheLineRollbackSnapshot {
            line: self.line,
            snapshot,
        })
    }

    fn restore_htm_rollback_snapshot_from_event(
        &mut self,
        tick: Tick,
        event: TrafficTraceHtmEvent,
        rollback: &WorkloadDataCacheLineRollbackSnapshot,
    ) -> bool {
        if rollback.line != self.line {
            return false;
        }
        let result = match (&mut self.harness, &rollback.snapshot) {
            (
                WorkloadDataCacheHarness::Msi(harness),
                WorkloadDataCacheRollbackSnapshot::Msi(snapshot),
            ) => harness
                .restore_quiescent(snapshot)
                .map_err(RiscvDataCacheControllerError::Msi),
            (
                WorkloadDataCacheHarness::Mesi(harness),
                WorkloadDataCacheRollbackSnapshot::Mesi(snapshot),
            ) => harness
                .restore_quiescent(snapshot)
                .map_err(RiscvDataCacheControllerError::Mesi),
            (
                WorkloadDataCacheHarness::Moesi(harness),
                WorkloadDataCacheRollbackSnapshot::Moesi(snapshot),
            ) => harness
                .restore_quiescent(snapshot)
                .map_err(RiscvDataCacheControllerError::Moesi),
            (
                WorkloadDataCacheHarness::Chi(harness),
                WorkloadDataCacheRollbackSnapshot::Chi(snapshot),
            ) => harness
                .restore_quiescent(snapshot)
                .map_err(RiscvDataCacheControllerError::Chi),
            _ => return false,
        };
        if let Err(error) = result {
            let record = RiscvDataCacheControllerErrorRecord::from_trace_htm_event(
                tick,
                event,
                self.protocol,
                self.target,
                self.line,
                MemoryOperation::NoAccess,
                error,
            );
            self.error = Some(RiscvWorkloadReplayError::DataCacheController {
                record: Box::new(record),
            });
        }
        true
    }

    fn apply_trace_diagnostic_event(
        &mut self,
        tick: Tick,
        event: TrafficTraceDiagnosticEvent,
    ) -> Option<RiscvTraceDiagnosticRecord> {
        if !self.accepts_trace_diagnostic_event(event) {
            return None;
        }

        match self.trace_diagnostic_record(tick, event) {
            Ok(Some(record)) => {
                self.trace_diagnostic_records.push(record);
                return Some(record);
            }
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
        None
    }

    fn trace_diagnostic_record(
        &self,
        tick: Tick,
        event: TrafficTraceDiagnosticEvent,
    ) -> Result<Option<RiscvTraceDiagnosticRecord>, RiscvWorkloadReplayError> {
        let Some(address) = event.address() else {
            return Ok(None);
        };
        let line = self.layout.line_address(address);
        if !event.is_print() || line != self.line {
            return Ok(None);
        }

        let (cached_copy_count, backing_line_present) = match &self.harness {
            WorkloadDataCacheHarness::Msi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)?;
                (
                    snapshot
                        .caches()
                        .values()
                        .filter(|cache| cache.cached_data().is_some())
                        .count(),
                    snapshot.backing().is_some()
                        || snapshot.dram_memory().is_some_and(|dram| {
                            dram_snapshot_line_data(dram, self.target, self.line).is_some()
                        }),
                )
            }
            WorkloadDataCacheHarness::Mesi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)?;
                (
                    snapshot
                        .caches()
                        .values()
                        .filter(|cache| cache.cached_data().is_some())
                        .count(),
                    snapshot.backing().line_address() == self.line,
                )
            }
            WorkloadDataCacheHarness::Moesi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::MoesiDataCache)?;
                (
                    snapshot
                        .caches()
                        .values()
                        .filter(|cache| cache.cached_data().is_some())
                        .count(),
                    snapshot.backing().line_address() == self.line,
                )
            }
            WorkloadDataCacheHarness::Chi(harness) => {
                let snapshot = harness
                    .quiescent_snapshot()
                    .map_err(RiscvWorkloadReplayError::ChiDataCache)?;
                (
                    snapshot
                        .caches()
                        .values()
                        .filter(|cache| cache.cached_data().is_some())
                        .count(),
                    snapshot.backing().line_address() == self.line,
                )
            }
        };

        Ok(Some(RiscvTraceDiagnosticRecord::data_cache_line(
            tick,
            self.protocol,
            self.target,
            address,
            self.line,
            cached_copy_count,
            backing_line_present,
        )))
    }

    fn invalidate_trace_line_from_sync(&mut self, tick: Tick, event: TrafficTraceSyncEvent) {
        let result = self.try_invalidate_trace_line();
        if let Err(error) = result {
            let record = RiscvDataCacheControllerErrorRecord::from_trace_sync_event(
                tick,
                event,
                self.protocol,
                self.target,
                self.line,
                error,
            );
            self.error = Some(RiscvWorkloadReplayError::DataCacheController {
                record: Box::new(record),
            });
        }
    }

    fn try_invalidate_trace_line(&mut self) -> Result<(), RiscvDataCacheControllerError> {
        match &mut self.harness {
            WorkloadDataCacheHarness::Msi(harness) => {
                flush_msi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Msi)
            }
            WorkloadDataCacheHarness::Mesi(harness) => {
                flush_mesi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Mesi)
            }
            WorkloadDataCacheHarness::Moesi(harness) => {
                flush_moesi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Moesi)
            }
            WorkloadDataCacheHarness::Chi(harness) => {
                flush_chi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Chi)
            }
        }
    }

    fn try_clean_trace_line(&mut self) -> Result<(), RiscvDataCacheControllerError> {
        match &mut self.harness {
            WorkloadDataCacheHarness::Msi(harness) => {
                clean_msi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Msi)
            }
            WorkloadDataCacheHarness::Mesi(harness) => {
                clean_mesi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Mesi)
            }
            WorkloadDataCacheHarness::Moesi(harness) => {
                clean_moesi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Moesi)
            }
            WorkloadDataCacheHarness::Chi(harness) => {
                clean_chi_harness(harness, self.target, self.line)
                    .map_err(RiscvDataCacheControllerError::Chi)
            }
        }
    }

    fn respond(&mut self, delivery: &RequestDelivery) -> Option<TargetOutcome> {
        if !self.accepts_delivery(delivery) {
            return None;
        }
        if delivery.request().line_address() != self.line {
            return None;
        }
        let outcome = match &mut self.harness {
            WorkloadDataCacheHarness::Msi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvDataCacheControllerError::Msi,
            ),
            WorkloadDataCacheHarness::Mesi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvDataCacheControllerError::Mesi,
            ),
            WorkloadDataCacheHarness::Moesi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvDataCacheControllerError::Moesi,
            ),
            WorkloadDataCacheHarness::Chi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvDataCacheControllerError::Chi,
            ),
        };
        Some(match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                let record = RiscvDataCacheControllerErrorRecord::from_request(
                    delivery.tick(),
                    delivery.request(),
                    self.protocol,
                    self.target,
                    error,
                );
                self.error = Some(RiscvWorkloadReplayError::DataCacheController {
                    record: Box::new(record),
                });
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            }
        })
    }
}

pub(super) struct WorkloadDataCacheBackend {
    lines: BTreeMap<Address, WorkloadDataCacheLineBackend>,
    htm_rollbacks: BTreeMap<WorkloadDataCacheRollbackKey, WorkloadDataCacheRollback>,
    htm_access_sets: BTreeMap<WorkloadDataCacheRollbackKey, WorkloadDataCacheHtmAccessSet>,
}

impl WorkloadDataCacheBackend {
    pub(super) fn new<I>(lines: I) -> Self
    where
        I: IntoIterator<Item = WorkloadDataCacheLineBackend>,
    {
        Self {
            lines: lines.into_iter().map(|line| (line.line(), line)).collect(),
            htm_rollbacks: BTreeMap::new(),
            htm_access_sets: BTreeMap::new(),
        }
    }

    pub(super) fn records(&self) -> Vec<RiscvDataCacheRunRecord> {
        self.lines
            .values()
            .flat_map(WorkloadDataCacheLineBackend::records)
            .collect()
    }

    pub(super) fn trace_diagnostic_records(&self) -> Vec<RiscvTraceDiagnosticRecord> {
        self.lines
            .values()
            .flat_map(WorkloadDataCacheLineBackend::trace_diagnostic_records)
            .collect()
    }

    pub(super) fn trace_error_records(&self) -> Vec<RiscvTraceErrorRecord> {
        let mut records = self
            .lines
            .values()
            .flat_map(WorkloadDataCacheLineBackend::trace_error_records)
            .collect::<Vec<_>>();
        records.sort_by_key(|record| (record.tick(), record.sequence(), record.line().get()));
        records
    }

    pub(super) fn trace_htm_access_records(&self) -> Vec<RiscvTraceHtmAccessRecord> {
        let mut records = self
            .lines
            .values()
            .flat_map(WorkloadDataCacheLineBackend::trace_htm_access_records)
            .collect::<Vec<_>>();
        records.sort_by_key(|record| (record.tick(), record.sequence(), record.line().get()));
        records
    }

    pub(super) fn take_error(&mut self) -> Option<RiscvWorkloadReplayError> {
        self.lines
            .values_mut()
            .find_map(WorkloadDataCacheLineBackend::take_error)
    }

    pub(super) fn apply_trace_cache_event(
        &mut self,
        event: TrafficTraceCacheEvent,
    ) -> Option<WorkloadTraceCacheApplication> {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_cache_event(event))
            .and_then(|line| line.apply_trace_cache_event(event))
    }

    pub(super) fn invalidate_trace_l1_from_sync(
        &mut self,
        tick: Tick,
        event: TrafficTraceSyncEvent,
    ) -> usize {
        let mut invalidated_line_count = 0;
        for line in self.lines.values_mut() {
            line.invalidate_trace_line_from_sync(tick, event);
            invalidated_line_count += 1;
        }
        invalidated_line_count
    }

    pub(super) fn apply_trace_response_event(&mut self, event: TrafficTraceResponseEvent) -> bool {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_response_event(event))
            .is_some_and(|line| line.apply_trace_response_event(event))
    }

    pub(super) fn record_trace_error_event(
        &mut self,
        tick: Tick,
        request_id: MemoryRequestId,
        event: TrafficTraceErrorEvent,
        fallback_address: Option<Address>,
    ) -> Option<RiscvTraceErrorRecord> {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_error_event(event, fallback_address))
            .and_then(|line| {
                line.record_trace_error_event(tick, request_id, event, fallback_address)
            })
    }

    pub(super) fn record_trace_htm_access_event(
        &mut self,
        tick: Tick,
        route: MemoryRouteId,
        transaction_uid: HtmTransactionUid,
        event: TrafficTraceResponseEvent,
        data_cache_response_applied: bool,
    ) -> Vec<RiscvTraceHtmAccessRecord> {
        let Some(line_address) = self
            .lines
            .iter()
            .find(|(_, line)| line.accepts_trace_htm_access_event(event))
            .map(|(line_address, _)| *line_address)
        else {
            return Vec::new();
        };
        let Some(line) = self.lines.get_mut(&line_address) else {
            return Vec::new();
        };
        let records = line.record_trace_htm_access_event(tick, transaction_uid, event);
        let recorded = !records.is_empty();
        let key = WorkloadDataCacheRollbackKey::new(route, transaction_uid);
        if let Some(access_set) = self.htm_access_sets.get_mut(&key) {
            if event.is_read() {
                access_set.record_read(line_address);
            }
            if event.is_write() && data_cache_response_applied {
                access_set.record_write(line_address);
            }
        }
        if recorded && event.is_write() && data_cache_response_applied {
            if let Some(rollback) = self.htm_rollbacks.get_mut(&key) {
                rollback.mark_written(line_address);
            }
            self.mark_trace_htm_write_conflicts(route, Some(transaction_uid), line_address);
        }
        records
    }

    pub(super) fn record_trace_htm_write_conflict_event(
        &mut self,
        route: MemoryRouteId,
        transaction_uid: Option<HtmTransactionUid>,
        event: TrafficTraceResponseEvent,
        data_cache_response_applied: bool,
    ) -> bool {
        if !event.is_write() || !data_cache_response_applied {
            return false;
        }
        let Some(line_address) = self
            .lines
            .iter()
            .find(|(_, line)| line.accepts_trace_htm_access_event(event))
            .map(|(line_address, _)| *line_address)
        else {
            return false;
        };
        self.mark_trace_htm_write_conflicts(route, transaction_uid, line_address);
        true
    }

    fn mark_trace_htm_write_conflicts(
        &mut self,
        route: MemoryRouteId,
        transaction_uid: Option<HtmTransactionUid>,
        line: Address,
    ) {
        let writer = transaction_uid.map(|uid| WorkloadDataCacheRollbackKey::new(route, uid));
        let conflicting = self
            .htm_access_sets
            .iter()
            .filter(|(key, access_set)| {
                Some(**key) != writer && access_set.conflicts_with_write(line)
            })
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in &conflicting {
            if let Some(access_set) = self.htm_access_sets.get_mut(key) {
                access_set.mark_memory_conflict();
            }
        }
    }

    pub(super) fn trace_htm_abort_cause(
        &self,
        route: MemoryRouteId,
        transaction_uid: HtmTransactionUid,
    ) -> HtmFailureCause {
        let key = WorkloadDataCacheRollbackKey::new(route, transaction_uid);
        self.htm_access_sets.get(&key).map_or(
            HtmFailureCause::Other,
            WorkloadDataCacheHtmAccessSet::abort_cause,
        )
    }

    pub(super) fn capture_trace_htm_rollback_from_event(
        &mut self,
        route: MemoryRouteId,
        transaction_uid: HtmTransactionUid,
        tick: Tick,
        event: TrafficTraceHtmEvent,
    ) -> bool {
        let key = WorkloadDataCacheRollbackKey::new(route, transaction_uid);
        if self.htm_rollbacks.contains_key(&key) {
            return false;
        }

        let mut snapshots = BTreeMap::new();
        for line in self.lines.values_mut() {
            match line.htm_rollback_snapshot() {
                Ok(snapshot) => {
                    snapshots.insert(snapshot.line, snapshot);
                }
                Err(error) => {
                    let record = RiscvDataCacheControllerErrorRecord::from_trace_htm_event(
                        tick,
                        event,
                        line.protocol,
                        line.target,
                        line.line,
                        MemoryOperation::NoAccess,
                        error,
                    );
                    line.error = Some(RiscvWorkloadReplayError::DataCacheController {
                        record: Box::new(record),
                    });
                    return false;
                }
            }
        }
        self.htm_rollbacks
            .insert(key, WorkloadDataCacheRollback::new(snapshots));
        self.htm_access_sets
            .insert(key, WorkloadDataCacheHtmAccessSet::default());
        true
    }

    pub(super) fn restore_trace_htm_rollback_from_event(
        &mut self,
        route: MemoryRouteId,
        transaction_uid: HtmTransactionUid,
        tick: Tick,
        event: TrafficTraceHtmEvent,
    ) -> bool {
        let key = WorkloadDataCacheRollbackKey::new(route, transaction_uid);
        self.htm_access_sets.remove(&key);
        let Some(rollback) = self.htm_rollbacks.remove(&key) else {
            return false;
        };
        for line_address in rollback.written_lines {
            if let Some(snapshot) = rollback.snapshots.get(&line_address) {
                if let Some(line) = self.lines.get_mut(&snapshot.line) {
                    line.restore_htm_rollback_snapshot_from_event(tick, event, snapshot);
                }
            }
        }
        true
    }

    pub(super) fn discard_trace_htm_transaction(
        &mut self,
        route: MemoryRouteId,
        transaction_uid: HtmTransactionUid,
    ) -> bool {
        let key = WorkloadDataCacheRollbackKey::new(route, transaction_uid);
        let removed_access_set = self.htm_access_sets.remove(&key).is_some();
        let removed_rollback = self.htm_rollbacks.remove(&key).is_some();
        removed_access_set || removed_rollback
    }

    pub(super) fn apply_trace_diagnostic_event(
        &mut self,
        tick: Tick,
        event: TrafficTraceDiagnosticEvent,
    ) -> Option<RiscvTraceDiagnosticRecord> {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_diagnostic_event(event))
            .and_then(|line| line.apply_trace_diagnostic_event(tick, event))
    }

    pub(super) fn final_lines(
        &self,
    ) -> Result<Vec<(MemoryTargetId, Address, Vec<u8>)>, RiscvWorkloadReplayError> {
        self.lines
            .values()
            .map(|line| Ok((line.target(), line.line(), line.final_line_data()?)))
            .collect()
    }

    pub(super) fn respond(&mut self, delivery: &RequestDelivery) -> Option<TargetOutcome> {
        self.lines
            .get_mut(&delivery.request().line_address())
            .and_then(|line| line.respond(delivery))
    }
}

fn dram_snapshot_line_data(
    snapshot: &DramMemorySnapshot,
    target: MemoryTargetId,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .store()
        .partitions()
        .iter()
        .find(|partition| partition.target() == target)
        .and_then(|partition| {
            partition
                .lines()
                .iter()
                .find(|candidate| candidate.line() == line)
        })
        .map(|line| line.data().to_vec())
}

fn flush_msi_harness(
    harness: &mut PartitionedDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), rem6_coherence::HarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = msi_flush_data(&snapshot, target, line)
        .ok_or(rem6_coherence::HarnessError::MissingBackingMemory { line })?;
    let backing = replace_optional_backing(snapshot.backing(), data.clone())?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(rem6_coherence::HarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                MsiCacheControllerSnapshot::new(
                    MsiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let flushed = PartitionedDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        DirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&flushed)
}

fn flush_mesi_harness(
    harness: &mut PartitionedMesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = mesi_flush_data(&snapshot, line).ok_or(MesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MesiHarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                MesiCacheControllerSnapshot::new(
                    MesiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let flushed = PartitionedMesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        MesiDirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&flushed)
}

fn flush_moesi_harness(
    harness: &mut PartitionedMoesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MoesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = moesi_flush_data(&snapshot, line).ok_or(MoesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MoesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MoesiHarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                MoesiCacheControllerSnapshot::new(
                    MoesiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let flushed = PartitionedMoesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        MoesiDirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&flushed)
}

fn flush_chi_harness(
    harness: &mut PartitionedChiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), ChiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = chi_flush_data(&snapshot, line).ok_or(ChiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(ChiHarnessError::Backing)?;
    let dram_memory =
        replace_optional_dram_controller_line(snapshot.dram_memory(), target, line, data)
            .map_err(ChiHarnessError::Dram)?;
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| {
            (
                *agent,
                ChiCacheControllerSnapshot::new(
                    ChiCacheLine::new(*agent, cache.line()),
                    cache.layout(),
                    cache.next_sequence(),
                    None,
                    None,
                ),
            )
        })
        .collect();
    let flushed = rem6_coherence::PartitionedChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        ChiDirectoryLineState::new(snapshot.directory().line()),
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
    );
    harness.restore_quiescent(&flushed)
}

fn clean_msi_harness(
    harness: &mut PartitionedDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), rem6_coherence::HarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = msi_flush_data(&snapshot, target, line)
        .ok_or(rem6_coherence::HarnessError::MissingBackingMemory { line })?;
    let backing = replace_optional_backing(snapshot.backing(), data.clone())?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(rem6_coherence::HarnessError::Dram)?;
    let directory = clean_msi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_msi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = PartitionedDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&cleaned)
}

fn clean_mesi_harness(
    harness: &mut PartitionedMesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = mesi_flush_data(&snapshot, line).ok_or(MesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MesiHarnessError::Dram)?;
    let directory = clean_mesi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_mesi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = PartitionedMesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&cleaned)
}

fn clean_moesi_harness(
    harness: &mut PartitionedMoesiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), MoesiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = moesi_flush_data(&snapshot, line).ok_or(MoesiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(MoesiHarnessError::Backing)?;
    let dram_memory = replace_optional_dram_line(snapshot.dram_memory(), target, line, data)
        .map_err(MoesiHarnessError::Dram)?;
    let directory = clean_moesi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_moesi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = PartitionedMoesiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.fabric_lanes().map(<[_]>::to_vec),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
        snapshot.dram_accesses(),
        snapshot.parallel_runs().to_vec(),
    );
    harness.restore_quiescent(&cleaned)
}

fn clean_chi_harness(
    harness: &mut PartitionedChiDirectoryLineHarness,
    target: MemoryTargetId,
    line: Address,
) -> Result<(), ChiHarnessError> {
    let snapshot = harness.quiescent_snapshot()?;
    let data = chi_flush_data(&snapshot, line).ok_or(ChiHarnessError::Backing(
        rem6_coherence::HarnessError::MissingBackingMemory { line },
    ))?;
    let backing =
        replace_backing(snapshot.backing(), data.clone()).map_err(ChiHarnessError::Backing)?;
    let dram_memory =
        replace_optional_dram_controller_line(snapshot.dram_memory(), target, line, data)
            .map_err(ChiHarnessError::Dram)?;
    let directory = clean_chi_directory(snapshot.directory());
    let caches = snapshot
        .caches()
        .iter()
        .map(|(agent, cache)| (*agent, clean_chi_cache_snapshot(*agent, cache)))
        .collect();
    let cleaned = rem6_coherence::PartitionedChiDirectoryLineHarnessSnapshot::new(
        snapshot.line(),
        snapshot.scheduler().clone(),
        directory,
        caches,
        backing,
        dram_memory,
        snapshot.dram_qos().cloned(),
        snapshot.trace(),
        snapshot.cpu_responses(),
        snapshot.directory_decisions(),
    );
    harness.restore_quiescent(&cleaned)
}

fn clean_msi_directory(snapshot: &DirectoryLineState) -> DirectoryLineState {
    let mut clean = DirectoryLineState::new(snapshot.line());
    if let Some(owner) = snapshot.owner() {
        clean = clean.with_sharer(owner);
    }
    for sharer in snapshot.sharers() {
        clean = clean.with_sharer(*sharer);
    }
    clean
}

fn clean_mesi_directory(snapshot: &MesiDirectoryLineState) -> MesiDirectoryLineState {
    let mut clean = MesiDirectoryLineState::new(snapshot.line());
    if let Some((owner, state)) = snapshot.owner() {
        clean = clean.with_owner(owner, clean_mesi_state(state));
    }
    for sharer in snapshot.sharers() {
        clean = clean.with_sharer(*sharer);
    }
    clean
}

fn clean_moesi_directory(snapshot: &MoesiDirectoryLineState) -> MoesiDirectoryLineState {
    let mut clean = MoesiDirectoryLineState::new(snapshot.line());
    if let Some((owner, state)) = snapshot.owner() {
        match clean_moesi_state(state) {
            MoesiState::Shared => clean = clean.with_sharer(owner),
            state => clean = clean.with_owner(owner, state),
        }
    }
    for sharer in snapshot.sharers() {
        clean = clean.with_sharer(*sharer);
    }
    clean
}

fn clean_chi_directory(snapshot: &ChiDirectoryLineState) -> ChiDirectoryLineState {
    let mut clean = ChiDirectoryLineState::new(snapshot.line());
    if let (Some(owner), Some(state)) = (snapshot.unique_owner(), snapshot.unique_owner_state()) {
        clean = clean.with_unique_owner(owner, clean_chi_state(state));
    }
    for (sharer, state) in snapshot.sharers() {
        clean = clean.with_sharer(*sharer, clean_chi_state(*state));
    }
    clean
}

fn clean_msi_cache_snapshot(
    agent: rem6_memory::AgentId,
    snapshot: &MsiCacheControllerSnapshot,
) -> MsiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = MsiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_msi_state(snapshot.state()))
        .expect("clean MSI trace response selects a stable cache state");
    MsiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_mesi_cache_snapshot(
    agent: rem6_memory::AgentId,
    snapshot: &MesiCacheControllerSnapshot,
) -> MesiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = MesiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_mesi_state(snapshot.state()))
        .expect("clean MESI trace response selects a stable cache state");
    MesiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_moesi_cache_snapshot(
    agent: rem6_memory::AgentId,
    snapshot: &MoesiCacheControllerSnapshot,
) -> MoesiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = MoesiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_moesi_state(snapshot.state()))
        .expect("clean MOESI trace response selects a stable cache state");
    MoesiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_chi_cache_snapshot(
    agent: rem6_memory::AgentId,
    snapshot: &ChiCacheControllerSnapshot,
) -> ChiCacheControllerSnapshot {
    let data = snapshot.cached_data().map(<[_]>::to_vec);
    let mut line = ChiCacheLine::new(agent, snapshot.line());
    line.force_state(clean_chi_state(snapshot.state()))
        .expect("clean CHI trace response selects a stable cache state");
    ChiCacheControllerSnapshot::new(
        line,
        snapshot.layout(),
        snapshot.next_sequence(),
        data,
        None,
    )
}

fn clean_msi_state(state: MsiState) -> MsiState {
    match state {
        MsiState::Modified => MsiState::Shared,
        state => state,
    }
}

fn clean_mesi_state(state: MesiState) -> MesiState {
    match state {
        MesiState::Modified => MesiState::Exclusive,
        state => state,
    }
}

fn clean_moesi_state(state: MoesiState) -> MoesiState {
    match state {
        MoesiState::Modified => MoesiState::Exclusive,
        MoesiState::Owned => MoesiState::Shared,
        state => state,
    }
}

fn clean_chi_state(state: ChiState) -> ChiState {
    match state {
        ChiState::SharedDirty => ChiState::SharedClean,
        ChiState::UniqueDirty => ChiState::UniqueClean,
        state => state,
    }
}

fn msi_flush_data(
    snapshot: &PartitionedDirectoryLineHarnessSnapshot,
    target: MemoryTargetId,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| snapshot.backing().map(|backing| backing.data().to_vec()))
        .or_else(|| {
            snapshot
                .dram_memory()
                .and_then(|dram| dram_snapshot_line_data(dram, target, line))
        })
}

fn mesi_flush_data(
    snapshot: &PartitionedMesiDirectoryLineHarnessSnapshot,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| {
            (snapshot.backing().line_address() == line).then(|| snapshot.backing().data().to_vec())
        })
}

fn moesi_flush_data(
    snapshot: &PartitionedMoesiDirectoryLineHarnessSnapshot,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| {
            (snapshot.backing().line_address() == line).then(|| snapshot.backing().data().to_vec())
        })
}

fn chi_flush_data(
    snapshot: &rem6_coherence::PartitionedChiDirectoryLineHarnessSnapshot,
    line: Address,
) -> Option<Vec<u8>> {
    snapshot
        .caches()
        .values()
        .find_map(|cache| cache.cached_data().map(<[u8]>::to_vec))
        .or_else(|| {
            (snapshot.backing().line_address() == line).then(|| snapshot.backing().data().to_vec())
        })
}

fn replace_backing(
    backing: &LineBackingStore,
    data: Vec<u8>,
) -> Result<LineBackingStore, rem6_coherence::HarnessError> {
    let mut backing = backing.clone();
    backing.replace_data(data)?;
    Ok(backing)
}

fn replace_optional_backing(
    backing: Option<&LineBackingStore>,
    data: Vec<u8>,
) -> Result<Option<LineBackingStore>, rem6_coherence::HarnessError> {
    backing
        .map(|backing| replace_backing(backing, data))
        .transpose()
}

fn replace_optional_dram_line(
    snapshot: Option<&DramMemorySnapshot>,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<Option<DramMemorySnapshot>, rem6_dram::DramMemoryError> {
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    let mut dram = rem6_dram::DramMemoryController::from_snapshot(snapshot)?;
    dram.insert_line(target, line, data)?;
    Ok(Some(dram.snapshot()))
}

fn replace_optional_dram_controller_line(
    snapshot: Option<&rem6_dram::DramMemoryController>,
    target: MemoryTargetId,
    line: Address,
    data: Vec<u8>,
) -> Result<Option<rem6_dram::DramMemoryController>, rem6_dram::DramMemoryError> {
    let Some(snapshot) = snapshot else {
        return Ok(None);
    };
    let mut dram = snapshot.clone();
    dram.insert_line(target, line, data)?;
    Ok(Some(dram))
}

#[cfg(test)]
#[path = "data_cache_backend_htm_conflict_tests.rs"]
mod htm_conflict_tests;

#[cfg(test)]
#[path = "data_cache_backend_trace_tests.rs"]
mod trace_tests;
