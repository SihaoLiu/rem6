use std::collections::BTreeMap;

use rem6_cache::{
    MesiCacheController, MesiCacheControllerError, MesiCacheControllerResultKind,
    MesiCacheControllerSnapshot,
};
use rem6_directory::{
    MesiDirectory, MesiDirectoryDataSource, MesiDirectoryDecision, MesiDirectoryGrant,
    MesiDirectoryLineState,
};
use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryRequest, MemoryResponse};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::TargetOutcome;

use crate::{HarnessError, LineBackingStore, SubmitKind};

use super::{
    fill_event, map_mesi_cache_error, mesi_response_record, MesiCpuResponseRecord,
    MesiHarnessError, MesiSubmitResult,
};

pub struct MesiDirectoryLineHarness {
    line: MesiLineId,
    directory: MesiDirectory,
    caches: BTreeMap<AgentId, MesiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<MesiCpuResponseRecord>,
    directory_decisions: Vec<MesiDirectoryDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiDirectoryLineHarnessSnapshot {
    line: MesiLineId,
    directory: MesiDirectoryLineState,
    caches: BTreeMap<AgentId, MesiCacheControllerSnapshot>,
    backing: LineBackingStore,
    cpu_responses: Vec<MesiCpuResponseRecord>,
    directory_decisions: Vec<MesiDirectoryDecision>,
}

impl MesiDirectoryLineHarnessSnapshot {
    pub fn new(
        line: MesiLineId,
        directory: MesiDirectoryLineState,
        caches: BTreeMap<AgentId, MesiCacheControllerSnapshot>,
        backing: LineBackingStore,
        cpu_responses: Vec<MesiCpuResponseRecord>,
        directory_decisions: Vec<MesiDirectoryDecision>,
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

    pub const fn line(&self) -> MesiLineId {
        self.line
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

    pub fn cpu_responses(&self) -> &[MesiCpuResponseRecord] {
        &self.cpu_responses
    }

    pub fn directory_decisions(&self) -> &[MesiDirectoryDecision] {
        &self.directory_decisions
    }
}

impl MesiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        agents: I,
    ) -> Result<Self, MesiHarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(MesiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(agent, MesiCacheController::new(agent, layout, line_address))
                .is_some()
            {
                return Err(MesiHarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            line: MesiLineId::new(line_address),
            directory: MesiDirectory::new(),
            caches,
            backing,
            cpu_responses: Vec::new(),
            directory_decisions: Vec::new(),
        })
    }

    pub fn submit_cpu_request(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<MesiSubmitResult, MesiHarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(map_mesi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
            return Ok(MesiSubmitResult::new(
                SubmitKind::ImmediateHit,
                cache_result,
            ));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(MesiHarnessError::Cache(
                MesiCacheControllerError::NoPendingMiss,
            ))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(MesiHarnessError::Directory)?;
        let fill_event = fill_event(&decision)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response, fill_event)
            .map_err(map_mesi_cache_error)?;
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

        Ok(
            MesiSubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
                .with_directory_decision(decision),
        )
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<MesiState, MesiHarnessError> {
        Ok(self.cache(agent)?.state())
    }

    pub fn directory_state(&self) -> MesiDirectoryLineState {
        self.directory.line_state(self.line)
    }

    pub fn cpu_responses(&self) -> Vec<MesiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[MesiDirectoryDecision] {
        &self.directory_decisions
    }

    pub fn cache_data(&self, agent: AgentId) -> Result<Option<Vec<u8>>, MesiHarnessError> {
        Ok(self.cache(agent)?.cached_data().map(<[u8]>::to_vec))
    }

    pub const fn line(&self) -> MesiLineId {
        self.line
    }

    pub fn snapshot(&self) -> MesiDirectoryLineHarnessSnapshot {
        MesiDirectoryLineHarnessSnapshot::new(
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

    pub fn restore(
        &mut self,
        snapshot: &MesiDirectoryLineHarnessSnapshot,
    ) -> Result<(), MesiHarnessError> {
        self.validate_snapshot_identity(snapshot)?;
        let mut directory = self.directory.clone();
        directory
            .restore_line_state(snapshot.directory())
            .map_err(MesiHarnessError::Directory)?;
        let mut caches = self.caches.clone();
        for (agent, cache_snapshot) in snapshot.caches() {
            caches
                .get_mut(agent)
                .ok_or(MesiHarnessError::UnknownCache { agent: *agent })?
                .restore(cache_snapshot)
                .map_err(map_mesi_cache_error)?;
        }

        self.directory = directory;
        self.caches = caches;
        self.backing = snapshot.backing.clone();
        self.cpu_responses = snapshot.cpu_responses.clone();
        self.directory_decisions = snapshot.directory_decisions.clone();
        Ok(())
    }

    fn directory_response(
        &mut self,
        request: &MemoryRequest,
        decision: &MesiDirectoryDecision,
    ) -> Result<MemoryResponse, MesiHarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(MesiHarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        if decision
            .snoops()
            .iter()
            .any(|snoop| snoop.event() == MesiEvent::SnoopRead)
        {
            if let Some(data) = &source_data {
                self.backing
                    .replace_data(data.clone())
                    .map_err(MesiHarnessError::Backing)?;
            }
        }

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(snoop.event())
                .map_err(map_mesi_cache_error)?;
        }

        match grant.data_source() {
            MesiDirectoryDataSource::BackingMemory => self
                .backing
                .respond(request)
                .map_err(MesiHarnessError::Backing),
            MesiDirectoryDataSource::OwnedCache(_) => {
                MemoryResponse::completed(request, source_data).map_err(MesiHarnessError::Memory)
            }
            MesiDirectoryDataSource::NoData => {
                MemoryResponse::completed(request, None).map_err(MesiHarnessError::Memory)
            }
        }
    }

    fn source_data(&self, grant: MesiDirectoryGrant) -> Result<Option<Vec<u8>>, MesiHarnessError> {
        match grant.data_source() {
            MesiDirectoryDataSource::BackingMemory | MesiDirectoryDataSource::NoData => Ok(None),
            MesiDirectoryDataSource::OwnedCache(agent) => {
                let data = self.cache(agent)?.cached_data().ok_or(
                    MesiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    },
                )?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn cache(&self, agent: AgentId) -> Result<&MesiCacheController, MesiHarnessError> {
        self.caches
            .get(&agent)
            .ok_or(MesiHarnessError::UnknownCache { agent })
    }

    fn cache_mut(&mut self, agent: AgentId) -> Result<&mut MesiCacheController, MesiHarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(MesiHarnessError::UnknownCache { agent })
    }

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: MesiCacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(mesi_response_record(tick, cache_result, response));
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &MesiDirectoryLineHarnessSnapshot,
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
