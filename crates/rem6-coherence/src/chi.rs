use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_cache::{
    ChiCacheController, ChiCacheControllerError, ChiCacheControllerResultKind,
    ChiCacheControllerSnapshot,
};
use rem6_directory::{
    ChiDirectory, ChiDirectoryDataSource, ChiDirectoryDecision, ChiDirectoryError,
    ChiDirectoryGrant, ChiDirectoryLineState,
};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_protocol_chi::{ChiEvent, ChiLineId, ChiState};
use rem6_transport::TargetOutcome;

use crate::{HarnessError, LineBackingStore, SubmitKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChiHarnessError {
    LineBusy { state: ChiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: ChiLineId },
    UnexpectedGrantState { state: ChiState },
    Cache(ChiCacheControllerError),
    Directory(ChiDirectoryError),
    Memory(MemoryError),
    Backing(HarnessError),
}

impl fmt::Display for ChiHarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineBusy { state } => write!(formatter, "cache line is busy in {state:?}"),
            Self::UnknownCache { agent } => {
                write!(formatter, "cache agent {} is not registered", agent.get())
            }
            Self::DuplicateCache { agent } => {
                write!(
                    formatter,
                    "cache agent {} is already registered",
                    agent.get()
                )
            }
            Self::MissingDirectoryGrant { request } => write!(
                formatter,
                "directory did not grant request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::GrantDataUnavailable { agent, line } => write!(
                formatter,
                "agent {} has no data for CHI line {:#x}",
                agent.get(),
                line.address().get()
            ),
            Self::UnexpectedGrantState { state } => {
                write!(formatter, "directory granted invalid CHI state {state:?}")
            }
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Backing(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ChiHarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Directory(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Backing(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiSubmitResult {
    kind: SubmitKind,
    cache_result: ChiCacheControllerResultKind,
    directory_decision: Option<ChiDirectoryDecision>,
}

impl ChiSubmitResult {
    fn new(kind: SubmitKind, cache_result: ChiCacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
        }
    }

    fn with_directory_decision(mut self, decision: ChiDirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> ChiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&ChiDirectoryDecision> {
        self.directory_decision.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiCpuResponseRecord {
    tick: u64,
    cache_result: ChiCacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl ChiCpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: ChiCacheControllerResultKind,
        request: MemoryRequestId,
        status: ResponseStatus,
        data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            tick,
            cache_result,
            request,
            status,
            data,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn cache_result(&self) -> ChiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

pub struct ChiDirectoryLineHarness {
    line: ChiLineId,
    directory: ChiDirectory,
    caches: BTreeMap<AgentId, ChiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<ChiCpuResponseRecord>,
    directory_decisions: Vec<ChiDirectoryDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiDirectoryLineHarnessSnapshot {
    line: ChiLineId,
    directory: ChiDirectoryLineState,
    caches: BTreeMap<AgentId, ChiCacheControllerSnapshot>,
    backing: LineBackingStore,
    cpu_responses: Vec<ChiCpuResponseRecord>,
    directory_decisions: Vec<ChiDirectoryDecision>,
}

impl ChiDirectoryLineHarnessSnapshot {
    pub fn new(
        line: ChiLineId,
        directory: ChiDirectoryLineState,
        caches: BTreeMap<AgentId, ChiCacheControllerSnapshot>,
        backing: LineBackingStore,
        cpu_responses: Vec<ChiCpuResponseRecord>,
        directory_decisions: Vec<ChiDirectoryDecision>,
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

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn directory(&self) -> &ChiDirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, ChiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> &LineBackingStore {
        &self.backing
    }

    pub fn cpu_responses(&self) -> &[ChiCpuResponseRecord] {
        &self.cpu_responses
    }

    pub fn directory_decisions(&self) -> &[ChiDirectoryDecision] {
        &self.directory_decisions
    }
}

impl ChiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        agents: I,
    ) -> Result<Self, ChiHarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(ChiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(agent, ChiCacheController::new(agent, layout, line_address))
                .is_some()
            {
                return Err(ChiHarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            line: ChiLineId::new(line_address),
            directory: ChiDirectory::new(),
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
    ) -> Result<ChiSubmitResult, ChiHarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(map_chi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
            return Ok(ChiSubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(ChiHarnessError::Cache(
                ChiCacheControllerError::NoPendingMiss,
            ))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(ChiHarnessError::Directory)?;
        let fill_event = chi_fill_event(&decision)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response, fill_event)
            .map_err(map_chi_cache_error)?;
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

        Ok(
            ChiSubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
                .with_directory_decision(decision),
        )
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<ChiState, ChiHarnessError> {
        Ok(self.cache(agent)?.state())
    }

    pub fn directory_state(&self) -> ChiDirectoryLineState {
        self.directory.line_state(self.line)
    }

    pub fn cpu_responses(&self) -> Vec<ChiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[ChiDirectoryDecision] {
        &self.directory_decisions
    }

    pub fn cache_data(&self, agent: AgentId) -> Result<Option<Vec<u8>>, ChiHarnessError> {
        Ok(self.cache(agent)?.cached_data().map(<[u8]>::to_vec))
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub fn snapshot(&self) -> ChiDirectoryLineHarnessSnapshot {
        ChiDirectoryLineHarnessSnapshot::new(
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
        snapshot: &ChiDirectoryLineHarnessSnapshot,
    ) -> Result<(), ChiHarnessError> {
        self.validate_snapshot_identity(snapshot)?;
        let mut directory = self.directory.clone();
        directory
            .restore_line_state(snapshot.directory())
            .map_err(ChiHarnessError::Directory)?;
        let mut caches = self.caches.clone();
        for (agent, cache_snapshot) in snapshot.caches() {
            caches
                .get_mut(agent)
                .ok_or(ChiHarnessError::UnknownCache { agent: *agent })?
                .restore(cache_snapshot)
                .map_err(map_chi_cache_error)?;
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
        decision: &ChiDirectoryDecision,
    ) -> Result<MemoryResponse, ChiHarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(ChiHarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        if decision_downgrades_unique_owner(decision) {
            if let Some(data) = &source_data {
                self.backing
                    .replace_data(data.clone())
                    .map_err(ChiHarnessError::Backing)?;
            }
        }

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(snoop.event())
                .map_err(map_chi_cache_error)?;
        }

        match grant.data_source() {
            ChiDirectoryDataSource::BackingMemory => self
                .backing
                .respond(request)
                .map_err(ChiHarnessError::Backing),
            ChiDirectoryDataSource::OwnerCache(_) if request.returns_data() => {
                MemoryResponse::completed(request, source_data).map_err(ChiHarnessError::Memory)
            }
            ChiDirectoryDataSource::OwnerCache(_) | ChiDirectoryDataSource::NoData => {
                MemoryResponse::completed(request, None).map_err(ChiHarnessError::Memory)
            }
        }
    }

    fn source_data(&self, grant: ChiDirectoryGrant) -> Result<Option<Vec<u8>>, ChiHarnessError> {
        match grant.data_source() {
            ChiDirectoryDataSource::BackingMemory | ChiDirectoryDataSource::NoData => Ok(None),
            ChiDirectoryDataSource::OwnerCache(agent) => {
                let data = self.cache(agent)?.cached_data().ok_or(
                    ChiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    },
                )?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn cache(&self, agent: AgentId) -> Result<&ChiCacheController, ChiHarnessError> {
        self.caches
            .get(&agent)
            .ok_or(ChiHarnessError::UnknownCache { agent })
    }

    fn cache_mut(&mut self, agent: AgentId) -> Result<&mut ChiCacheController, ChiHarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(ChiHarnessError::UnknownCache { agent })
    }

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: ChiCacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(chi_response_record(tick, cache_result, response));
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &ChiDirectoryLineHarnessSnapshot,
    ) -> Result<(), ChiHarnessError> {
        if self.line != snapshot.line() {
            return Err(ChiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            }));
        }
        for agent in self.caches.keys() {
            if !snapshot.caches().contains_key(agent) {
                return Err(ChiHarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.caches().keys() {
            if !self.caches.contains_key(agent) {
                return Err(ChiHarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

fn chi_fill_event(decision: &ChiDirectoryDecision) -> Result<ChiEvent, ChiHarnessError> {
    let state = decision
        .grant()
        .ok_or(ChiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?
        .state();
    match state {
        ChiState::SharedClean => Ok(ChiEvent::CompDataSharedClean),
        ChiState::SharedDirty => Ok(ChiEvent::CompDataSharedDirty),
        ChiState::UniqueClean => Ok(ChiEvent::CompDataUniqueClean),
        ChiState::UniqueDirty => Ok(ChiEvent::CompDataUniqueDirty),
        state => Err(ChiHarnessError::UnexpectedGrantState { state }),
    }
}

fn decision_downgrades_unique_owner(decision: &ChiDirectoryDecision) -> bool {
    decision.before().unique_owner().is_some()
        && decision
            .snoops()
            .iter()
            .any(|snoop| snoop.event() == ChiEvent::SnoopShared)
}

fn chi_response_record(
    tick: u64,
    cache_result: ChiCacheControllerResultKind,
    response: &MemoryResponse,
) -> ChiCpuResponseRecord {
    ChiCpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_chi_cache_error(error: ChiCacheControllerError) -> ChiHarnessError {
    match error {
        ChiCacheControllerError::LineBusy { state } => ChiHarnessError::LineBusy { state },
        error => ChiHarnessError::Cache(error),
    }
}
