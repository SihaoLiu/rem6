use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::MoesiCacheControllerSnapshot;
use rem6_directory::MoesiDirectoryLineState;
use rem6_dram::{DramMemoryController, DramMemorySnapshot};
use rem6_fabric::{FabricLaneSnapshot, FabricModel};
use rem6_kernel::SchedulerSnapshot;
use rem6_memory::AgentId;
use rem6_protocol_moesi::MoesiLineId;
use rem6_transport::{MemoryTrace, MemoryTraceEvent};

use crate::{
    DramMemoryAccessRecord, HarnessError, LineBackingStore, ParallelCoherenceRunHistory,
    ParallelCoherenceRunSummary, PartitionedDramQosState,
};

use super::{
    map_moesi_cache_error, MoesiCpuResponseRecord, MoesiDirectoryDecisionRecord, MoesiHarnessError,
    PartitionedMoesiDirectoryLineHarness,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMoesiDirectoryLineHarnessSnapshot {
    line: MoesiLineId,
    scheduler: SchedulerSnapshot,
    directory: MoesiDirectoryLineState,
    caches: BTreeMap<AgentId, MoesiCacheControllerSnapshot>,
    backing: LineBackingStore,
    dram_memory: Option<DramMemorySnapshot>,
    dram_qos: Option<PartitionedDramQosState>,
    fabric_lanes: Option<Vec<FabricLaneSnapshot>>,
    trace: Vec<MemoryTraceEvent>,
    cpu_responses: Vec<MoesiCpuResponseRecord>,
    directory_decisions: Vec<MoesiDirectoryDecisionRecord>,
    dram_accesses: Vec<DramMemoryAccessRecord>,
    parallel_runs: Vec<ParallelCoherenceRunSummary>,
}

impl PartitionedMoesiDirectoryLineHarnessSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line: MoesiLineId,
        scheduler: SchedulerSnapshot,
        directory: MoesiDirectoryLineState,
        caches: BTreeMap<AgentId, MoesiCacheControllerSnapshot>,
        backing: LineBackingStore,
        dram_memory: Option<DramMemorySnapshot>,
        dram_qos: Option<PartitionedDramQosState>,
        fabric_lanes: Option<Vec<FabricLaneSnapshot>>,
        trace: Vec<MemoryTraceEvent>,
        cpu_responses: Vec<MoesiCpuResponseRecord>,
        directory_decisions: Vec<MoesiDirectoryDecisionRecord>,
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
            dram_qos,
            fabric_lanes,
            trace,
            cpu_responses,
            directory_decisions,
            dram_accesses,
            parallel_runs,
        }
    }

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    pub const fn scheduler(&self) -> &SchedulerSnapshot {
        &self.scheduler
    }

    pub const fn directory(&self) -> &MoesiDirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, MoesiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> &LineBackingStore {
        &self.backing
    }

    pub const fn dram_memory(&self) -> Option<&DramMemorySnapshot> {
        self.dram_memory.as_ref()
    }

    pub const fn dram_qos(&self) -> Option<&PartitionedDramQosState> {
        self.dram_qos.as_ref()
    }

    pub fn fabric_lanes(&self) -> Option<&[FabricLaneSnapshot]> {
        self.fabric_lanes.as_deref()
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.clone()
    }

    pub fn cpu_responses(&self) -> Vec<MoesiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> Vec<MoesiDirectoryDecisionRecord> {
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

impl PartitionedMoesiDirectoryLineHarness {
    pub fn quiescent_snapshot(
        &self,
    ) -> Result<PartitionedMoesiDirectoryLineHarnessSnapshot, MoesiHarnessError> {
        let scheduler = self
            .scheduler
            .quiescent_snapshot()
            .map_err(MoesiHarnessError::Scheduler)?;
        Ok(PartitionedMoesiDirectoryLineHarnessSnapshot::new(
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
            self.backing.lock().expect("backing lock").clone(),
            self.dram_memory
                .as_ref()
                .map(|dram| dram.lock().expect("DRAM memory lock").snapshot()),
            self.dram_qos
                .as_ref()
                .map(|qos| qos.lock().expect("DRAM QoS lock").clone()),
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
        snapshot: &PartitionedMoesiDirectoryLineHarnessSnapshot,
    ) -> Result<(), MoesiHarnessError> {
        self.validate_quiescent_snapshot_identity(snapshot)?;
        self.scheduler
            .quiescent_snapshot()
            .map_err(MoesiHarnessError::Scheduler)?;

        let mut directory = self.directory.lock().expect("directory lock").clone();
        directory
            .restore_line_state(snapshot.directory())
            .map_err(MoesiHarnessError::Directory)?;

        let mut caches = BTreeMap::new();
        for (agent, current) in &self.caches {
            let cache_snapshot = snapshot
                .caches()
                .get(agent)
                .ok_or(MoesiHarnessError::UnknownCache { agent: *agent })?;
            let mut cache = current.lock().expect("cache lock").clone();
            cache
                .restore(cache_snapshot)
                .map_err(map_moesi_cache_error)?;
            caches.insert(*agent, Arc::new(Mutex::new(cache)));
        }

        let backing = Arc::new(Mutex::new(snapshot.backing.clone()));
        let dram_memory = restored_moesi_dram_memory(&self.dram_memory, snapshot.dram_memory())?;
        let dram_qos = restored_moesi_dram_qos(&self.dram_qos, snapshot.dram_qos())?;
        validate_moesi_fabric_snapshot(&self.fabric, snapshot.fabric_lanes())?;

        self.scheduler
            .restore_quiescent(snapshot.scheduler())
            .map_err(MoesiHarnessError::Scheduler)?;
        self.directory = Arc::new(Mutex::new(directory));
        self.caches = caches;
        self.backing = backing;
        self.dram_memory = dram_memory;
        self.dram_qos = dram_qos;
        restore_moesi_fabric_in_place(&self.fabric, snapshot.fabric_lanes())?;
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
        snapshot: &PartitionedMoesiDirectoryLineHarnessSnapshot,
    ) -> Result<(), MoesiHarnessError> {
        if self.line != snapshot.line() {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            }));
        }
        if snapshot.backing().line_address() != self.line.address() {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.backing().line_address(),
            }));
        }
        if snapshot.directory().line() != self.line {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.directory().line().address(),
            }));
        }
        for agent in self.caches.keys() {
            if !snapshot.caches().contains_key(agent) {
                return Err(MoesiHarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.caches().keys() {
            if !self.caches.contains_key(agent) {
                return Err(MoesiHarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

fn restored_moesi_dram_memory(
    current: &Option<Arc<Mutex<DramMemoryController>>>,
    snapshot: Option<&DramMemorySnapshot>,
) -> Result<Option<Arc<Mutex<DramMemoryController>>>, MoesiHarnessError> {
    match (current, snapshot) {
        (Some(_), Some(snapshot)) => {
            let controller =
                DramMemoryController::from_snapshot(snapshot).map_err(MoesiHarnessError::Dram)?;
            Ok(Some(Arc::new(Mutex::new(controller))))
        }
        (None, None) => Ok(None),
        _ => Err(MoesiHarnessError::SnapshotResourceMismatch { resource: "dram" }),
    }
}

fn restored_moesi_dram_qos(
    current: &Option<Arc<Mutex<PartitionedDramQosState>>>,
    snapshot: Option<&PartitionedDramQosState>,
) -> Result<Option<Arc<Mutex<PartitionedDramQosState>>>, MoesiHarnessError> {
    match (current, snapshot) {
        (Some(_), Some(snapshot)) => Ok(Some(Arc::new(Mutex::new(snapshot.clone())))),
        (None, None) => Ok(None),
        _ => Err(MoesiHarnessError::SnapshotResourceMismatch {
            resource: "dram_qos",
        }),
    }
}

fn validate_moesi_fabric_snapshot(
    current: &Option<Arc<Mutex<FabricModel>>>,
    snapshot: Option<&[FabricLaneSnapshot]>,
) -> Result<(), MoesiHarnessError> {
    match (current, snapshot) {
        (Some(_), Some(snapshot)) => {
            let mut fabric = FabricModel::new();
            fabric
                .restore_lane_snapshots(snapshot.iter().cloned())
                .map_err(MoesiHarnessError::Fabric)?;
            Ok(())
        }
        (None, None) => Ok(()),
        _ => Err(MoesiHarnessError::SnapshotResourceMismatch { resource: "fabric" }),
    }
}

fn restore_moesi_fabric_in_place(
    current: &Option<Arc<Mutex<FabricModel>>>,
    snapshot: Option<&[FabricLaneSnapshot]>,
) -> Result<(), MoesiHarnessError> {
    match (current, snapshot) {
        (Some(fabric), Some(snapshot)) => fabric
            .lock()
            .expect("fabric lock")
            .restore_lane_snapshots(snapshot.iter().cloned())
            .map_err(MoesiHarnessError::Fabric),
        (None, None) => Ok(()),
        _ => Err(MoesiHarnessError::SnapshotResourceMismatch { resource: "fabric" }),
    }
}
