use std::collections::BTreeMap;

use rem6_cache::{
    CacheControllerError, CacheControllerResultKind, MsiCacheBank, MsiCacheBankSnapshot,
};
use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryGrant, DirectoryLineState, MsiDirectory,
};
use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryRequest, MemoryResponse};
use rem6_protocol_msi::{MsiLineId, MsiState};
use rem6_transport::TargetOutcome;

use crate::{response_record, CpuResponseRecord, HarnessError, SubmitKind, SubmitResult};

pub struct MsiBankDirectoryHarness {
    layout: CacheLineLayout,
    directory: MsiDirectory,
    caches: BTreeMap<AgentId, MsiCacheBank>,
    backing: BTreeMap<Address, Vec<u8>>,
    cpu_responses: Vec<CpuResponseRecord>,
    directory_decisions: Vec<DirectoryDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankBackingLineSnapshot {
    line_address: Address,
    data: Vec<u8>,
}

impl MsiBankBackingLineSnapshot {
    pub fn new(line_address: Address, data: Vec<u8>) -> Self {
        Self { line_address, data }
    }

    pub const fn line_address(&self) -> Address {
        self.line_address
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankDirectoryHarnessSnapshot {
    layout: CacheLineLayout,
    cache_snapshots: BTreeMap<AgentId, MsiCacheBankSnapshot>,
    directory_states: Vec<DirectoryLineState>,
    backing_lines: Vec<MsiBankBackingLineSnapshot>,
    cpu_responses: Vec<CpuResponseRecord>,
    directory_decisions: Vec<DirectoryDecision>,
}

impl MsiBankDirectoryHarnessSnapshot {
    pub fn new(
        layout: CacheLineLayout,
        cache_snapshots: BTreeMap<AgentId, MsiCacheBankSnapshot>,
        directory_states: Vec<DirectoryLineState>,
        backing_lines: Vec<MsiBankBackingLineSnapshot>,
        cpu_responses: Vec<CpuResponseRecord>,
        directory_decisions: Vec<DirectoryDecision>,
    ) -> Self {
        Self {
            layout,
            cache_snapshots,
            directory_states,
            backing_lines,
            cpu_responses,
            directory_decisions,
        }
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub fn cache_snapshots(&self) -> &BTreeMap<AgentId, MsiCacheBankSnapshot> {
        &self.cache_snapshots
    }

    pub fn cache_count(&self) -> usize {
        self.cache_snapshots.len()
    }

    pub fn cache_agents(&self) -> Vec<AgentId> {
        self.cache_snapshots.keys().copied().collect()
    }

    pub fn cache_snapshot(&self, agent: AgentId) -> Option<&MsiCacheBankSnapshot> {
        self.cache_snapshots.get(&agent)
    }

    pub fn directory_states(&self) -> &[DirectoryLineState] {
        &self.directory_states
    }

    pub fn directory_line_count(&self) -> usize {
        self.directory_states.len()
    }

    pub fn directory_line_addresses(&self) -> Vec<Address> {
        self.directory_states
            .iter()
            .map(|state| state.line().address())
            .collect()
    }

    pub fn backing_lines(&self) -> &[MsiBankBackingLineSnapshot] {
        &self.backing_lines
    }

    pub fn backing_line_count(&self) -> usize {
        self.backing_lines.len()
    }

    pub fn backing_line(&self, address: Address) -> Option<&[u8]> {
        let line_address = self.layout.line_address(address);
        self.backing_lines
            .iter()
            .find(|line| line.line_address() == line_address)
            .map(MsiBankBackingLineSnapshot::data)
    }

    pub fn cpu_responses(&self) -> &[CpuResponseRecord] {
        &self.cpu_responses
    }

    pub fn directory_decisions(&self) -> &[DirectoryDecision] {
        &self.directory_decisions
    }
}

impl MsiBankDirectoryHarness {
    pub fn new<I>(layout: CacheLineLayout, agents: I) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(agent, MsiCacheBank::new(agent, layout))
                .is_some()
            {
                return Err(HarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            layout,
            directory: MsiDirectory::new(),
            caches,
            backing: BTreeMap::new(),
            cpu_responses: Vec::new(),
            directory_decisions: Vec::new(),
        })
    }

    pub fn insert_backing_line(
        &mut self,
        line_address: Address,
        data: Vec<u8>,
    ) -> Result<(), HarnessError> {
        let line_address = self.layout.line_address(line_address);
        if data.len() as u64 != self.layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.backing.insert(line_address, data);
        Ok(())
    }

    pub fn cache_count(&self) -> usize {
        self.caches.len()
    }

    pub fn cache_agents(&self) -> Vec<AgentId> {
        self.caches.keys().copied().collect()
    }

    pub fn backing_line_addresses(&self) -> Vec<Address> {
        self.backing.keys().copied().collect()
    }

    pub fn submit_cpu_request(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(HarnessError::CacheBank)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
            return Ok(SubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(HarnessError::Cache(CacheControllerError::NoPendingMiss))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(HarnessError::Directory)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response)
            .map_err(HarnessError::CacheBank)?;
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
            .with_directory_decision(decision))
    }

    pub fn cache_state(
        &self,
        agent: AgentId,
        address: Address,
    ) -> Result<Option<MsiState>, HarnessError> {
        Ok(self.cache(agent)?.state(address))
    }

    pub fn cache_line_addresses(&self, agent: AgentId) -> Result<Vec<Address>, HarnessError> {
        Ok(self.cache(agent)?.line_addresses())
    }

    pub fn cache_data(
        &self,
        agent: AgentId,
        address: Address,
    ) -> Result<Option<Vec<u8>>, HarnessError> {
        Ok(self.cache(agent)?.cached_data(address).map(<[u8]>::to_vec))
    }

    pub fn directory_state(&self, address: Address) -> DirectoryLineState {
        self.directory
            .line_state(MsiLineId::new(self.layout.line_address(address)))
    }

    pub fn directory_line_addresses(&self) -> Vec<Address> {
        self.directory.line_addresses()
    }

    pub fn backing_line(&self, address: Address) -> Option<&[u8]> {
        self.backing
            .get(&self.layout.line_address(address))
            .map(Vec::as_slice)
    }

    pub fn cpu_responses(&self) -> Vec<CpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[DirectoryDecision] {
        &self.directory_decisions
    }

    pub fn snapshot(&self) -> MsiBankDirectoryHarnessSnapshot {
        MsiBankDirectoryHarnessSnapshot::new(
            self.layout,
            self.caches
                .iter()
                .map(|(agent, cache)| (*agent, cache.snapshot()))
                .collect(),
            self.directory.line_states(),
            self.backing
                .iter()
                .map(|(line_address, data)| {
                    MsiBankBackingLineSnapshot::new(*line_address, data.clone())
                })
                .collect(),
            self.cpu_responses.clone(),
            self.directory_decisions.clone(),
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &MsiBankDirectoryHarnessSnapshot,
    ) -> Result<(), HarnessError> {
        self.validate_snapshot_identity(snapshot)?;

        let mut caches = self.caches.clone();
        for (agent, cache_snapshot) in snapshot.cache_snapshots() {
            caches
                .get_mut(agent)
                .ok_or(HarnessError::UnknownCache { agent: *agent })?
                .restore(cache_snapshot)
                .map_err(HarnessError::CacheBank)?;
        }

        let mut directory = MsiDirectory::new();
        for state in snapshot.directory_states() {
            let expected = self.layout.line_address(state.line().address());
            if expected != state.line().address() {
                return Err(HarnessError::WrongLine {
                    expected,
                    actual: state.line().address(),
                });
            }
        }
        directory.restore_line_states(snapshot.directory_states());

        let mut backing = BTreeMap::new();
        for line in snapshot.backing_lines() {
            let expected = self.layout.line_address(line.line_address());
            if expected != line.line_address() {
                return Err(HarnessError::WrongLine {
                    expected,
                    actual: line.line_address(),
                });
            }
            if line.data().len() as u64 != self.layout.bytes() {
                return Err(HarnessError::LineDataSizeMismatch {
                    expected: self.layout.bytes(),
                    actual: line.data().len() as u64,
                });
            }
            if backing
                .insert(line.line_address(), line.data().to_vec())
                .is_some()
            {
                return Err(HarnessError::SnapshotResourceMismatch {
                    resource: "msi bank backing line",
                });
            }
        }

        self.directory = directory;
        self.caches = caches;
        self.backing = backing;
        self.cpu_responses = snapshot.cpu_responses.clone();
        self.directory_decisions = snapshot.directory_decisions.clone();
        Ok(())
    }

    fn directory_response(
        &mut self,
        request: &MemoryRequest,
        decision: &DirectoryDecision,
    ) -> Result<MemoryResponse, HarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(HarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(grant.line().address(), snoop.event())
                .map_err(HarnessError::CacheBank)?;
        }

        match grant.data_source() {
            DirectoryDataSource::BackingMemory => {
                let data = self.backing_data(grant.line())?;
                response_from_line(request, Some(data))
            }
            DirectoryDataSource::ModifiedOwner(_) => response_from_line(request, source_data),
            DirectoryDataSource::NoData => response_from_line(request, None),
        }
    }

    fn source_data(&self, grant: DirectoryGrant) -> Result<Option<Vec<u8>>, HarnessError> {
        match grant.data_source() {
            DirectoryDataSource::BackingMemory | DirectoryDataSource::NoData => Ok(None),
            DirectoryDataSource::ModifiedOwner(agent) => {
                let data = self
                    .cache(agent)?
                    .cached_data(grant.line().address())
                    .ok_or(HarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    })?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn backing_data(&self, line: MsiLineId) -> Result<Vec<u8>, HarnessError> {
        self.backing
            .get(&line.address())
            .cloned()
            .ok_or(HarnessError::MissingBackingMemory {
                line: line.address(),
            })
    }

    fn cache(&self, agent: AgentId) -> Result<&MsiCacheBank, HarnessError> {
        self.caches
            .get(&agent)
            .ok_or(HarnessError::UnknownCache { agent })
    }

    fn cache_mut(&mut self, agent: AgentId) -> Result<&mut MsiCacheBank, HarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(HarnessError::UnknownCache { agent })
    }

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: CacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(response_record(tick, cache_result, response));
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &MsiBankDirectoryHarnessSnapshot,
    ) -> Result<(), HarnessError> {
        if self.layout != snapshot.layout() {
            return Err(HarnessError::SnapshotResourceMismatch {
                resource: "msi bank directory harness layout",
            });
        }

        for agent in self.caches.keys() {
            if !snapshot.cache_snapshots().contains_key(agent) {
                return Err(HarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.cache_snapshots().keys() {
            if !self.caches.contains_key(agent) {
                return Err(HarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

fn response_from_line(
    request: &MemoryRequest,
    line_data: Option<Vec<u8>>,
) -> Result<MemoryResponse, HarnessError> {
    if !request.returns_data() {
        return MemoryResponse::completed(request, None).map_err(HarnessError::Memory);
    }

    let data = line_data.ok_or(HarnessError::MissingBackingMemory {
        line: request.line_address(),
    })?;
    MemoryResponse::completed(request, Some(data)).map_err(HarnessError::Memory)
}
