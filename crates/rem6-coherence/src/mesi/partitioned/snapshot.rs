use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::MesiCacheControllerSnapshot;
use rem6_directory::MesiDirectoryLineState;
use rem6_dram::{DramMemoryController, DramMemorySnapshot};
use rem6_kernel::SchedulerSnapshot;
use rem6_memory::AgentId;
use rem6_protocol_mesi::MesiLineId;
use rem6_transport::{MemoryTrace, MemoryTraceEvent};

use crate::{
    DramMemoryAccessRecord, HarnessError, LineBackingStore, ParallelCoherenceRunHistory,
    ParallelCoherenceRunSummary,
};

use super::super::{
    map_mesi_cache_error, MesiCpuResponseRecord, MesiDirectoryDecisionRecord, MesiHarnessError,
};
use super::PartitionedMesiDirectoryLineHarness;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMesiDirectoryLineHarnessSnapshot {
    line: MesiLineId,
    scheduler: SchedulerSnapshot,
    directory: MesiDirectoryLineState,
    caches: BTreeMap<AgentId, MesiCacheControllerSnapshot>,
    backing: LineBackingStore,
    dram_memory: Option<DramMemorySnapshot>,
    trace: Vec<MemoryTraceEvent>,
    cpu_responses: Vec<MesiCpuResponseRecord>,
    directory_decisions: Vec<MesiDirectoryDecisionRecord>,
    dram_accesses: Vec<DramMemoryAccessRecord>,
    parallel_runs: Vec<ParallelCoherenceRunSummary>,
}

impl PartitionedMesiDirectoryLineHarnessSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line: MesiLineId,
        scheduler: SchedulerSnapshot,
        directory: MesiDirectoryLineState,
        caches: BTreeMap<AgentId, MesiCacheControllerSnapshot>,
        backing: LineBackingStore,
        dram_memory: Option<DramMemorySnapshot>,
        trace: Vec<MemoryTraceEvent>,
        cpu_responses: Vec<MesiCpuResponseRecord>,
        directory_decisions: Vec<MesiDirectoryDecisionRecord>,
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
            trace,
            cpu_responses,
            directory_decisions,
            dram_accesses,
            parallel_runs,
        }
    }

    pub const fn line(&self) -> MesiLineId {
        self.line
    }

    pub const fn scheduler(&self) -> &SchedulerSnapshot {
        &self.scheduler
    }

    pub const fn directory(&self) -> &MesiDirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, MesiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> &LineBackingStore {
        &self.backing
    }

    pub const fn dram_memory(&self) -> Option<&DramMemorySnapshot> {
        self.dram_memory.as_ref()
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.clone()
    }

    pub fn cpu_responses(&self) -> Vec<MesiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> Vec<MesiDirectoryDecisionRecord> {
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

impl PartitionedMesiDirectoryLineHarness {
    pub fn quiescent_snapshot(
        &self,
    ) -> Result<PartitionedMesiDirectoryLineHarnessSnapshot, MesiHarnessError> {
        let scheduler = self
            .scheduler
            .quiescent_snapshot()
            .map_err(MesiHarnessError::Scheduler)?;
        Ok(PartitionedMesiDirectoryLineHarnessSnapshot::new(
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
        snapshot: &PartitionedMesiDirectoryLineHarnessSnapshot,
    ) -> Result<(), MesiHarnessError> {
        self.validate_quiescent_snapshot_identity(snapshot)?;
        self.scheduler
            .quiescent_snapshot()
            .map_err(MesiHarnessError::Scheduler)?;

        let mut directory = self.directory.lock().expect("directory lock").clone();
        directory
            .restore_line_state(snapshot.directory())
            .map_err(MesiHarnessError::Directory)?;

        let mut caches = BTreeMap::new();
        for (agent, current) in &self.caches {
            let cache_snapshot = snapshot
                .caches()
                .get(agent)
                .ok_or(MesiHarnessError::UnknownCache { agent: *agent })?;
            let mut cache = current.lock().expect("cache lock").clone();
            cache
                .restore(cache_snapshot)
                .map_err(map_mesi_cache_error)?;
            caches.insert(*agent, Arc::new(Mutex::new(cache)));
        }

        let backing = Arc::new(Mutex::new(snapshot.backing.clone()));
        let dram_memory = restored_mesi_dram_memory(&self.dram_memory, snapshot.dram_memory())?;

        self.scheduler
            .restore_quiescent(snapshot.scheduler())
            .map_err(MesiHarnessError::Scheduler)?;
        self.directory = Arc::new(Mutex::new(directory));
        self.caches = caches;
        self.backing = backing;
        self.dram_memory = dram_memory;
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
        snapshot: &PartitionedMesiDirectoryLineHarnessSnapshot,
    ) -> Result<(), MesiHarnessError> {
        if self.line != snapshot.line() {
            return Err(MesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            }));
        }
        if snapshot.backing().line_address() != self.line.address() {
            return Err(MesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.backing().line_address(),
            }));
        }
        if snapshot.directory().line() != self.line {
            return Err(MesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.directory().line().address(),
            }));
        }
        for agent in self.caches.keys() {
            if !snapshot.caches().contains_key(agent) {
                return Err(MesiHarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.caches().keys() {
            if !self.caches.contains_key(agent) {
                return Err(MesiHarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

fn restored_mesi_dram_memory(
    current: &Option<Arc<Mutex<DramMemoryController>>>,
    snapshot: Option<&DramMemorySnapshot>,
) -> Result<Option<Arc<Mutex<DramMemoryController>>>, MesiHarnessError> {
    match (current, snapshot) {
        (Some(_), Some(snapshot)) => {
            let controller =
                DramMemoryController::from_snapshot(snapshot).map_err(MesiHarnessError::Dram)?;
            Ok(Some(Arc::new(Mutex::new(controller))))
        }
        (None, None) => Ok(None),
        _ => Err(MesiHarnessError::SnapshotResourceMismatch { resource: "dram" }),
    }
}
