use std::collections::BTreeMap;

use rem6_cache::MsiCacheControllerSnapshot;
use rem6_directory::{DirectoryDecision, DirectoryLineState};
use rem6_memory::AgentId;
use rem6_protocol_msi::MsiLineId;

use crate::{
    map_cache_error, CpuResponseRecord, DirectoryLineHarness, HarnessError, LineBackingStore,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryLineHarnessSnapshot {
    line: MsiLineId,
    directory: DirectoryLineState,
    caches: BTreeMap<AgentId, MsiCacheControllerSnapshot>,
    backing: LineBackingStore,
    cpu_responses: Vec<CpuResponseRecord>,
    directory_decisions: Vec<DirectoryDecision>,
}

impl DirectoryLineHarnessSnapshot {
    pub fn new(
        line: MsiLineId,
        directory: DirectoryLineState,
        caches: BTreeMap<AgentId, MsiCacheControllerSnapshot>,
        backing: LineBackingStore,
        cpu_responses: Vec<CpuResponseRecord>,
        directory_decisions: Vec<DirectoryDecision>,
    ) -> Self {
        Self {
            line,
            directory,
            caches,
            backing,
            cpu_responses,
            directory_decisions,
        }
    }

    pub const fn line(&self) -> MsiLineId {
        self.line
    }

    pub const fn directory(&self) -> &DirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, MsiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> &LineBackingStore {
        &self.backing
    }

    pub fn cpu_responses(&self) -> &[CpuResponseRecord] {
        &self.cpu_responses
    }

    pub fn directory_decisions(&self) -> &[DirectoryDecision] {
        &self.directory_decisions
    }
}

impl DirectoryLineHarness {
    pub fn snapshot(&self) -> DirectoryLineHarnessSnapshot {
        DirectoryLineHarnessSnapshot::new(
            self.line,
            self.directory.line_state(self.line),
            self.caches
                .iter()
                .map(|(agent, cache)| (*agent, cache.snapshot()))
                .collect(),
            self.backing.clone(),
            self.cpu_responses.clone(),
            self.directory_decisions.clone(),
        )
    }

    pub fn restore(&mut self, snapshot: &DirectoryLineHarnessSnapshot) -> Result<(), HarnessError> {
        self.validate_snapshot_identity(snapshot)?;
        let mut caches = self.caches.clone();
        for (agent, cache_snapshot) in snapshot.caches() {
            caches
                .get_mut(agent)
                .ok_or(HarnessError::UnknownCache { agent: *agent })?
                .restore(cache_snapshot)
                .map_err(map_cache_error)?;
        }

        self.directory.restore_line_state(snapshot.directory());
        self.caches = caches;
        self.backing = snapshot.backing.clone();
        self.cpu_responses = snapshot.cpu_responses.clone();
        self.directory_decisions = snapshot.directory_decisions.clone();
        Ok(())
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &DirectoryLineHarnessSnapshot,
    ) -> Result<(), HarnessError> {
        if self.line != snapshot.line() {
            return Err(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            });
        }
        if snapshot.backing().line_address() != self.line.address() {
            return Err(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.backing().line_address(),
            });
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
