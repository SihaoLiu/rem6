use std::collections::BTreeMap;

use rem6_cache::{
    ChiCacheControllerSnapshot, MesiCacheControllerSnapshot, MoesiCacheControllerSnapshot,
    MsiCacheControllerSnapshot,
};
use rem6_coherence::{
    ChiHarnessError, LineBackingStore, MesiHarnessError, MoesiHarnessError,
    PartitionedCacheAgentConfig, PartitionedChiDirectoryLineHarness,
    PartitionedDirectoryLineHarness, PartitionedDirectoryLineHarnessSnapshot,
    PartitionedDramMemoryConfig, PartitionedMesiDirectoryLineHarness,
    PartitionedMesiDirectoryLineHarnessSnapshot, PartitionedMoesiDirectoryLineHarness,
    PartitionedMoesiDirectoryLineHarnessSnapshot,
};
use rem6_cpu::HtmTransactionUid;
use rem6_directory::{
    ChiDirectoryLineState, DirectoryLineState, MesiDirectoryLineState, MoesiDirectoryLineState,
};
use rem6_dram::DramMemorySnapshot;
use rem6_kernel::{PartitionId, Tick};
use rem6_memory::{Address, CacheLineLayout, MemoryOperation, MemoryResponse, MemoryTargetId};
use rem6_protocol_chi::{ChiCacheLine, ChiState};
use rem6_protocol_mesi::{MesiCacheLine, MesiState};
use rem6_protocol_moesi::{MoesiCacheLine, MoesiState};
use rem6_protocol_msi::{MsiCacheLine, MsiState};
use rem6_traffic::{
    TrafficTraceCacheEvent, TrafficTraceDiagnosticEvent, TrafficTraceResponseEvent,
};
use rem6_transport::{RequestDelivery, TargetOutcome, TransportEndpointId};
use rem6_workload::{WorkloadDataCacheProtocol, WorkloadRiscvDataCache};

use super::cache_response::data_cache_response_result;
use super::RiscvWorkloadReplayError;
use crate::{
    RiscvDataCacheProtocol, RiscvDataCacheRunRecord, RiscvTraceDiagnosticRecord,
    RiscvTraceHtmAccessKind, RiscvTraceHtmAccessRecord,
};

enum WorkloadDataCacheHarness {
    Msi(PartitionedDirectoryLineHarness),
    Mesi(PartitionedMesiDirectoryLineHarness),
    Moesi(PartitionedMoesiDirectoryLineHarness),
    Chi(PartitionedChiDirectoryLineHarness),
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

    fn apply_trace_cache_event(&mut self, event: TrafficTraceCacheEvent) -> bool {
        if !self.accepts_trace_cache_event(event) {
            return false;
        }

        self.invalidate_trace_line();
        true
    }

    fn apply_trace_response_event(&mut self, event: TrafficTraceResponseEvent) -> bool {
        if !self.accepts_trace_response_event(event) {
            return false;
        }

        if event.invalidates_line() {
            self.invalidate_trace_line();
        } else if event.cleans_line() {
            self.clean_trace_line();
        }
        true
    }

    fn record_trace_htm_access_event(
        &mut self,
        tick: Tick,
        transaction_uid: HtmTransactionUid,
        event: TrafficTraceResponseEvent,
    ) -> bool {
        if !self.accepts_trace_htm_access_event(event) {
            return false;
        }

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
            }
        }
        true
    }

    fn apply_trace_diagnostic_event(
        &mut self,
        tick: Tick,
        event: TrafficTraceDiagnosticEvent,
    ) -> bool {
        if !self.accepts_trace_diagnostic_event(event) {
            return false;
        }

        match self.trace_diagnostic_record(tick, event) {
            Ok(Some(record)) => self.trace_diagnostic_records.push(record),
            Ok(None) => {}
            Err(error) => self.error = Some(error),
        }
        true
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

    fn invalidate_trace_line(&mut self) {
        let result = match &mut self.harness {
            WorkloadDataCacheHarness::Msi(harness) => {
                flush_msi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)
            }
            WorkloadDataCacheHarness::Mesi(harness) => {
                flush_mesi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)
            }
            WorkloadDataCacheHarness::Moesi(harness) => {
                flush_moesi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::MoesiDataCache)
            }
            WorkloadDataCacheHarness::Chi(harness) => {
                flush_chi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::ChiDataCache)
            }
        };
        if let Err(error) = result {
            self.error = Some(error);
        }
    }

    fn clean_trace_line(&mut self) {
        let result = match &mut self.harness {
            WorkloadDataCacheHarness::Msi(harness) => {
                clean_msi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::MsiDataCache)
            }
            WorkloadDataCacheHarness::Mesi(harness) => {
                clean_mesi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::MesiDataCache)
            }
            WorkloadDataCacheHarness::Moesi(harness) => {
                clean_moesi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::MoesiDataCache)
            }
            WorkloadDataCacheHarness::Chi(harness) => {
                clean_chi_harness(harness, self.target, self.line)
                    .map_err(RiscvWorkloadReplayError::ChiDataCache)
            }
        };
        if let Err(error) = result {
            self.error = Some(error);
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
                RiscvWorkloadReplayError::MsiDataCache,
            ),
            WorkloadDataCacheHarness::Mesi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvWorkloadReplayError::MesiDataCache,
            ),
            WorkloadDataCacheHarness::Moesi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvWorkloadReplayError::MoesiDataCache,
            ),
            WorkloadDataCacheHarness::Chi(harness) => data_cache_response_result(
                self.protocol,
                &mut self.records,
                harness,
                delivery,
                RiscvWorkloadReplayError::ChiDataCache,
            ),
        };
        Some(match outcome {
            Ok(outcome) => outcome,
            Err(error) => {
                self.error = Some(error);
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            }
        })
    }
}

pub(super) struct WorkloadDataCacheBackend {
    lines: BTreeMap<Address, WorkloadDataCacheLineBackend>,
}

impl WorkloadDataCacheBackend {
    pub(super) fn new<I>(lines: I) -> Self
    where
        I: IntoIterator<Item = WorkloadDataCacheLineBackend>,
    {
        Self {
            lines: lines.into_iter().map(|line| (line.line(), line)).collect(),
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

    pub(super) fn apply_trace_cache_event(&mut self, event: TrafficTraceCacheEvent) -> bool {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_cache_event(event))
            .is_some_and(|line| line.apply_trace_cache_event(event))
    }

    pub(super) fn apply_trace_response_event(&mut self, event: TrafficTraceResponseEvent) -> bool {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_response_event(event))
            .is_some_and(|line| line.apply_trace_response_event(event))
    }

    pub(super) fn record_trace_htm_access_event(
        &mut self,
        tick: Tick,
        transaction_uid: HtmTransactionUid,
        event: TrafficTraceResponseEvent,
    ) -> bool {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_htm_access_event(event))
            .is_some_and(|line| line.record_trace_htm_access_event(tick, transaction_uid, event))
    }

    pub(super) fn apply_trace_diagnostic_event(
        &mut self,
        tick: Tick,
        event: TrafficTraceDiagnosticEvent,
    ) -> bool {
        self.lines
            .values_mut()
            .find(|line| line.accepts_trace_diagnostic_event(event))
            .is_some_and(|line| line.apply_trace_diagnostic_event(tick, event))
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
mod tests {
    use rem6_memory::{AccessSize, AgentId, ByteMask, MemoryRequest, MemoryRequestId};
    use rem6_protocol_mesi::MesiState;
    use rem6_traffic::{
        TrafficTrace, TrafficTraceConfig, TrafficTraceEvent, TrafficTraceGenerator,
        TrafficTraceResponseKind,
    };
    use rem6_transport::TransportEndpointId;
    use rem6_workload::WorkloadRouteId;

    use super::*;

    fn layout() -> CacheLineLayout {
        CacheLineLayout::new(64).unwrap()
    }

    fn line_data() -> Vec<u8> {
        (0..64).collect()
    }

    fn agent(value: u32) -> AgentId {
        AgentId::new(value)
    }

    fn request_id(agent: u32, sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(agent), sequence)
    }

    fn endpoint(name: &str) -> TransportEndpointId {
        TransportEndpointId::new(name).unwrap()
    }

    fn cache_config(agent: u32) -> PartitionedCacheAgentConfig {
        PartitionedCacheAgentConfig::new(
            AgentId::new(agent),
            PartitionId::new(agent),
            endpoint(&format!("l1d{agent}")),
            2,
            3,
        )
    }

    fn write(agent: u32, sequence: u64, address: u64, data: Vec<u8>) -> MemoryRequest {
        let size = AccessSize::new(data.len() as u64).unwrap();
        MemoryRequest::write(
            request_id(agent, sequence),
            Address::new(address),
            size,
            data,
            ByteMask::full(size).unwrap(),
            layout(),
        )
        .unwrap()
    }

    fn data_cache_config(protocol: WorkloadDataCacheProtocol) -> WorkloadRiscvDataCache {
        WorkloadRiscvDataCache::new(
            protocol,
            0,
            Address::new(0x3000),
            2,
            "dir0",
            WorkloadRouteId::new("memory").unwrap(),
        )
        .unwrap()
    }

    fn clean_shared_response_event() -> TrafficTraceResponseEvent {
        let trace =
            TrafficTrace::from_gem5_packet_trace(&gem5_packet_trace(1_000, 43), 1_000).unwrap();
        let config = TrafficTraceConfig::new(AgentId::new(7), layout(), 99, trace).unwrap();
        let mut generator = TrafficTraceGenerator::new(config);
        generator.enter(0);
        match generator.next_event(0, 0).unwrap().unwrap() {
            TrafficTraceEvent::Response(event) => event,
            event => panic!("unexpected trace event: {event:?}"),
        }
    }

    fn gem5_packet_trace(tick_frequency: u64, command: u32) -> Vec<u8> {
        let mut bytes = vec![0x67, 0x65, 0x6d, 0x35];
        let mut header = Vec::new();
        append_key(&mut header, 3, 0);
        append_varint(&mut header, tick_frequency);
        append_record(&mut bytes, &header);

        let mut packet = Vec::new();
        append_key(&mut packet, 1, 0);
        append_varint(&mut packet, 4);
        append_key(&mut packet, 2, 0);
        append_varint(&mut packet, u64::from(command));
        append_key(&mut packet, 3, 0);
        append_varint(&mut packet, 0x3000);
        append_key(&mut packet, 4, 0);
        append_varint(&mut packet, 64);
        append_record(&mut bytes, &packet);

        bytes
    }

    fn append_record(bytes: &mut Vec<u8>, message: &[u8]) {
        append_varint(bytes, message.len() as u64);
        bytes.extend_from_slice(message);
    }

    fn append_key(bytes: &mut Vec<u8>, field: u32, wire_type: u8) {
        append_varint(bytes, (u64::from(field) << 3) | u64::from(wire_type));
    }

    fn append_varint(bytes: &mut Vec<u8>, mut value: u64) {
        while value >= 0x80 {
            bytes.push((value as u8) | 0x80);
            value >>= 7;
        }
        bytes.push(value as u8);
    }

    #[test]
    fn trace_flush_mesi_harness_writes_dirty_owner_to_backing() {
        let mut harness = PartitionedMesiDirectoryLineHarness::new(
            layout(),
            Address::new(0x3000),
            LineBackingStore::new(layout(), Address::new(0x3000), line_data()).unwrap(),
            PartitionId::new(2),
            endpoint("dir0"),
            [cache_config(1)],
        )
        .unwrap();

        harness
            .submit_cpu_request_parallel(agent(1), write(1, 0, 0x3008, vec![0xaa, 0xbb]))
            .unwrap();
        harness.run_until_idle_parallel_recorded().unwrap();
        let before = harness.quiescent_snapshot().unwrap();
        assert_eq!(
            &before
                .caches()
                .get(&agent(1))
                .unwrap()
                .cached_data()
                .unwrap()[8..10],
            &[0xaa, 0xbb]
        );

        flush_mesi_harness(&mut harness, MemoryTargetId::new(0), Address::new(0x3000)).unwrap();

        let snapshot = harness.quiescent_snapshot().unwrap();
        assert_eq!(&snapshot.backing().data()[8..10], &[0xaa, 0xbb]);
        let cache = snapshot.caches().get(&agent(1)).unwrap();
        assert_eq!(cache.state(), MesiState::Invalid);
        assert!(cache.cached_data().is_none());
    }

    #[test]
    fn trace_clean_shared_response_cleans_dirty_mesi_line_without_invalidating() {
        let mut backend = WorkloadDataCacheLineBackend::new(
            &data_cache_config(WorkloadDataCacheProtocol::Mesi),
            layout(),
            Address::new(0x3000),
            WorkloadDataCacheLineMemory::Line(line_data()),
            vec![cache_config(1)],
        )
        .unwrap();

        let WorkloadDataCacheHarness::Mesi(harness) = &mut backend.harness else {
            panic!("expected MESI backend harness");
        };
        harness
            .submit_cpu_request_parallel(agent(1), write(1, 0, 0x3008, vec![0xaa, 0xbb]))
            .unwrap();
        harness.run_until_idle_parallel_recorded().unwrap();
        let before = harness.quiescent_snapshot().unwrap();
        let before_cache = before.caches().get(&agent(1)).unwrap();
        assert_eq!(before_cache.state(), MesiState::Modified);
        assert_eq!(&before_cache.cached_data().unwrap()[8..10], &[0xaa, 0xbb]);

        let event = clean_shared_response_event();
        assert_eq!(event.kind(), TrafficTraceResponseKind::CleanShared);
        assert!(event.cleans_line());
        assert!(!event.invalidates_line());

        assert!(backend.apply_trace_response_event(event));

        let WorkloadDataCacheHarness::Mesi(harness) = &backend.harness else {
            panic!("expected MESI backend harness");
        };
        let snapshot = harness.quiescent_snapshot().unwrap();
        assert_eq!(&snapshot.backing().data()[8..10], &[0xaa, 0xbb]);
        let cache = snapshot.caches().get(&agent(1)).unwrap();
        assert_eq!(cache.state(), MesiState::Exclusive);
        assert_eq!(&cache.cached_data().unwrap()[8..10], &[0xaa, 0xbb]);
    }
}
