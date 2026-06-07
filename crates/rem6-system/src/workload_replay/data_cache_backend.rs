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
use rem6_directory::{
    ChiDirectoryLineState, DirectoryLineState, MesiDirectoryLineState, MoesiDirectoryLineState,
};
use rem6_dram::DramMemorySnapshot;
use rem6_kernel::PartitionId;
use rem6_memory::{Address, CacheLineLayout, MemoryOperation, MemoryResponse, MemoryTargetId};
use rem6_protocol_chi::ChiCacheLine;
use rem6_protocol_mesi::MesiCacheLine;
use rem6_protocol_moesi::MoesiCacheLine;
use rem6_protocol_msi::MsiCacheLine;
use rem6_traffic::{TrafficTraceCacheEvent, TrafficTraceResponseEvent};
use rem6_transport::{RequestDelivery, TargetOutcome, TransportEndpointId};
use rem6_workload::{WorkloadDataCacheProtocol, WorkloadRiscvDataCache};

use super::cache_response::data_cache_response_result;
use super::RiscvWorkloadReplayError;
use crate::{RiscvDataCacheProtocol, RiscvDataCacheRunRecord};

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
            error: None,
        })
    }

    fn records(&self) -> Vec<RiscvDataCacheRunRecord> {
        self.records.clone()
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
            event.invalidates_line() && self.layout.line_address(address) == self.line
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

        self.invalidate_trace_line();
        true
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
    use rem6_transport::TransportEndpointId;

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
}
