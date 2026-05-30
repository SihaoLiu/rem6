use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::MsiCacheControllerSnapshot;
use rem6_directory::DirectoryLineState;
use rem6_dram::{DramMemoryController, DramMemorySnapshot};
use rem6_fabric::{FabricLaneSnapshot, FabricModel};
use rem6_kernel::SchedulerSnapshot;
use rem6_memory::AgentId;
use rem6_protocol_msi::MsiLineId;
use rem6_transport::{MemoryTrace, MemoryTraceEvent};

use crate::{
    map_cache_error, CpuResponseRecord, DirectoryDecisionRecord, DramMemoryAccessRecord,
    HarnessError, LineBackingStore, ParallelCoherenceRunHistory, ParallelCoherenceRunSummary,
    PartitionedDirectoryLineHarness,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedDirectoryLineHarnessSnapshot {
    line: MsiLineId,
    scheduler: SchedulerSnapshot,
    directory: DirectoryLineState,
    caches: BTreeMap<AgentId, MsiCacheControllerSnapshot>,
    backing: Option<LineBackingStore>,
    dram_memory: Option<DramMemorySnapshot>,
    fabric_lanes: Option<Vec<FabricLaneSnapshot>>,
    trace: Vec<MemoryTraceEvent>,
    cpu_responses: Vec<CpuResponseRecord>,
    directory_decisions: Vec<DirectoryDecisionRecord>,
    dram_accesses: Vec<DramMemoryAccessRecord>,
    parallel_runs: Vec<ParallelCoherenceRunSummary>,
}

impl PartitionedDirectoryLineHarnessSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line: MsiLineId,
        scheduler: SchedulerSnapshot,
        directory: DirectoryLineState,
        caches: BTreeMap<AgentId, MsiCacheControllerSnapshot>,
        backing: Option<LineBackingStore>,
        dram_memory: Option<DramMemorySnapshot>,
        fabric_lanes: Option<Vec<FabricLaneSnapshot>>,
        trace: Vec<MemoryTraceEvent>,
        cpu_responses: Vec<CpuResponseRecord>,
        directory_decisions: Vec<DirectoryDecisionRecord>,
        dram_accesses: Vec<DramMemoryAccessRecord>,
        parallel_runs: Vec<ParallelCoherenceRunSummary>,
    ) -> Self {
        Self {
            line,
            scheduler,
            directory,
            caches,
            backing,
            dram_memory,
            fabric_lanes,
            trace,
            cpu_responses,
            directory_decisions,
            dram_accesses,
            parallel_runs,
        }
    }

    pub const fn line(&self) -> MsiLineId {
        self.line
    }

    pub const fn scheduler(&self) -> &SchedulerSnapshot {
        &self.scheduler
    }

    pub const fn directory(&self) -> &DirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, MsiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> Option<&LineBackingStore> {
        self.backing.as_ref()
    }

    pub const fn dram_memory(&self) -> Option<&DramMemorySnapshot> {
        self.dram_memory.as_ref()
    }

    pub fn fabric_lanes(&self) -> Option<&[FabricLaneSnapshot]> {
        self.fabric_lanes.as_deref()
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.clone()
    }

    pub fn cpu_responses(&self) -> Vec<CpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> Vec<DirectoryDecisionRecord> {
        self.directory_decisions.clone()
    }

    pub fn dram_accesses(&self) -> Vec<DramMemoryAccessRecord> {
        self.dram_accesses.clone()
    }

    pub fn parallel_runs(&self) -> &[ParallelCoherenceRunSummary] {
        &self.parallel_runs
    }

    pub fn parallel_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::from_runs(&self.parallel_runs)
    }
}

impl PartitionedDirectoryLineHarness {
    pub fn quiescent_snapshot(
        &self,
    ) -> Result<PartitionedDirectoryLineHarnessSnapshot, HarnessError> {
        let scheduler = self
            .scheduler
            .quiescent_snapshot()
            .map_err(HarnessError::Scheduler)?;
        Ok(PartitionedDirectoryLineHarnessSnapshot::new(
            self.line,
            scheduler,
            self.directory
                .lock()
                .expect("directory lock")
                .line_state(self.line),
            self.caches
                .iter()
                .map(|(agent, cache)| (*agent, cache.lock().expect("cache lock").snapshot()))
                .collect(),
            self.backing
                .as_ref()
                .map(|backing| backing.lock().expect("backing lock").clone()),
            self.dram_memory
                .as_ref()
                .map(|dram| dram.lock().expect("DRAM memory lock").snapshot()),
            self.fabric
                .as_ref()
                .map(|fabric| fabric.lock().expect("fabric lock").lane_snapshots()),
            self.trace.snapshot(),
            self.cpu_responses.lock().expect("response lock").clone(),
            self.directory_decisions
                .lock()
                .expect("decision lock")
                .clone(),
            self.dram_accesses.lock().expect("DRAM access lock").clone(),
            self.parallel_runs.clone(),
        ))
    }

    pub fn restore_quiescent(
        &mut self,
        snapshot: &PartitionedDirectoryLineHarnessSnapshot,
    ) -> Result<(), HarnessError> {
        self.validate_quiescent_snapshot_identity(snapshot)?;
        self.scheduler
            .quiescent_snapshot()
            .map_err(HarnessError::Scheduler)?;

        let mut caches = BTreeMap::new();
        for (agent, current) in &self.caches {
            let cache_snapshot = snapshot
                .caches()
                .get(agent)
                .ok_or(HarnessError::UnknownCache { agent: *agent })?;
            let mut cache = current.lock().expect("cache lock").clone();
            cache.restore(cache_snapshot).map_err(map_cache_error)?;
            caches.insert(*agent, Arc::new(Mutex::new(cache)));
        }

        let backing = match (&self.backing, snapshot.backing()) {
            (Some(_), Some(backing)) => Some(Arc::new(Mutex::new(backing.clone()))),
            (None, None) => None,
            _ => {
                return Err(HarnessError::SnapshotResourceMismatch {
                    resource: "backing",
                });
            }
        };
        let dram_memory = restored_dram_memory(&self.dram_memory, snapshot.dram_memory())?;
        validate_fabric_snapshot(&self.fabric, snapshot.fabric_lanes())?;

        self.scheduler
            .restore_quiescent(snapshot.scheduler())
            .map_err(HarnessError::Scheduler)?;
        self.directory
            .lock()
            .expect("directory lock")
            .restore_line_state(snapshot.directory());
        self.caches = caches;
        self.backing = backing;
        self.dram_memory = dram_memory;
        if let Some(fabric) = &self.fabric {
            let lanes = snapshot
                .fabric_lanes()
                .ok_or(HarnessError::SnapshotResourceMismatch { resource: "fabric" })?;
            fabric
                .lock()
                .expect("fabric lock")
                .restore_lane_snapshots(lanes.iter().cloned())
                .map_err(HarnessError::Fabric)?;
        }
        self.trace = MemoryTrace::from_events(snapshot.trace.clone());
        *self.cpu_responses.lock().expect("response lock") = snapshot.cpu_responses.clone();
        *self.directory_decisions.lock().expect("decision lock") =
            snapshot.directory_decisions.clone();
        *self.dram_accesses.lock().expect("DRAM access lock") = snapshot.dram_accesses.clone();
        self.parallel_runs = snapshot.parallel_runs.clone();
        Ok(())
    }

    fn validate_quiescent_snapshot_identity(
        &self,
        snapshot: &PartitionedDirectoryLineHarnessSnapshot,
    ) -> Result<(), HarnessError> {
        if self.line != snapshot.line() {
            return Err(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            });
        }
        if let Some(backing) = snapshot.backing() {
            if backing.line_address() != self.line.address() {
                return Err(HarnessError::WrongLine {
                    expected: self.line.address(),
                    actual: backing.line_address(),
                });
            }
        }
        if snapshot.directory().line() != self.line {
            return Err(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.directory().line().address(),
            });
        }
        for agent in self.caches.keys() {
            if !snapshot.caches().contains_key(agent) {
                return Err(HarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.caches().keys() {
            if !self.caches.contains_key(agent) {
                return Err(HarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

fn restored_dram_memory(
    current: &Option<Arc<Mutex<DramMemoryController>>>,
    snapshot: Option<&DramMemorySnapshot>,
) -> Result<Option<Arc<Mutex<DramMemoryController>>>, HarnessError> {
    match (current, snapshot) {
        (Some(_), Some(snapshot)) => {
            let controller =
                DramMemoryController::from_snapshot(snapshot).map_err(HarnessError::Dram)?;
            Ok(Some(Arc::new(Mutex::new(controller))))
        }
        (None, None) => Ok(None),
        _ => Err(HarnessError::SnapshotResourceMismatch { resource: "dram" }),
    }
}

fn validate_fabric_snapshot(
    current: &Option<Arc<Mutex<FabricModel>>>,
    snapshot: Option<&[FabricLaneSnapshot]>,
) -> Result<(), HarnessError> {
    match (current, snapshot) {
        (Some(_), Some(snapshot)) => {
            let mut fabric = FabricModel::new();
            fabric
                .restore_lane_snapshots(snapshot.iter().cloned())
                .map_err(HarnessError::Fabric)?;
            Ok(())
        }
        (None, None) => Ok(()),
        _ => Err(HarnessError::SnapshotResourceMismatch { resource: "fabric" }),
    }
}
