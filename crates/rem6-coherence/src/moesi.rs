use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_cache::{MoesiCacheController, MoesiCacheControllerError, MoesiCacheControllerResultKind};
use rem6_directory::{
    MoesiDirectory, MoesiDirectoryDataSource, MoesiDirectoryDecision, MoesiDirectoryError,
    MoesiDirectoryGrant, MoesiDirectoryLineState,
};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_protocol_moesi::{MoesiEvent, MoesiLineId, MoesiState};
use rem6_transport::TargetOutcome;

use crate::{HarnessError, LineBackingStore, SubmitKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoesiHarnessError {
    LineBusy { state: MoesiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: MoesiLineId },
    UnexpectedGrantState { state: MoesiState },
    Cache(MoesiCacheControllerError),
    Directory(MoesiDirectoryError),
    Memory(MemoryError),
    Backing(HarnessError),
}

impl fmt::Display for MoesiHarnessError {
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
                "agent {} has no data for line {:#x}",
                agent.get(),
                line.address().get()
            ),
            Self::UnexpectedGrantState { state } => {
                write!(formatter, "directory granted transient state {state:?}")
            }
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Backing(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for MoesiHarnessError {
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
pub struct MoesiSubmitResult {
    kind: SubmitKind,
    cache_result: MoesiCacheControllerResultKind,
    directory_decision: Option<MoesiDirectoryDecision>,
}

impl MoesiSubmitResult {
    fn new(kind: SubmitKind, cache_result: MoesiCacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
        }
    }

    fn with_directory_decision(mut self, decision: MoesiDirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> MoesiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&MoesiDirectoryDecision> {
        self.directory_decision.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiCpuResponseRecord {
    tick: u64,
    cache_result: MoesiCacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl MoesiCpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: MoesiCacheControllerResultKind,
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

    pub const fn cache_result(&self) -> MoesiCacheControllerResultKind {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiDirectoryDecisionRecord {
    tick: u64,
    requester: AgentId,
    decision: MoesiDirectoryDecision,
}

impl MoesiDirectoryDecisionRecord {
    pub const fn new(tick: u64, requester: AgentId, decision: MoesiDirectoryDecision) -> Self {
        Self {
            tick,
            requester,
            decision,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn requester(&self) -> AgentId {
        self.requester
    }

    pub const fn decision(&self) -> &MoesiDirectoryDecision {
        &self.decision
    }
}

pub struct MoesiDirectoryLineHarness {
    line: MoesiLineId,
    directory: MoesiDirectory,
    caches: BTreeMap<AgentId, MoesiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<MoesiCpuResponseRecord>,
    directory_decisions: Vec<MoesiDirectoryDecision>,
}

impl MoesiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        agents: I,
    ) -> Result<Self, MoesiHarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(
                    agent,
                    MoesiCacheController::new(agent, layout, line_address),
                )
                .is_some()
            {
                return Err(MoesiHarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            line: MoesiLineId::new(line_address),
            directory: MoesiDirectory::new(),
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
    ) -> Result<MoesiSubmitResult, MoesiHarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(map_moesi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
            return Ok(MoesiSubmitResult::new(
                SubmitKind::ImmediateHit,
                cache_result,
            ));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(MoesiHarnessError::Cache(
                MoesiCacheControllerError::NoPendingMiss,
            ))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(MoesiHarnessError::Directory)?;
        let fill_event = moesi_fill_event(&decision)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response, fill_event)
            .map_err(map_moesi_cache_error)?;
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

        Ok(
            MoesiSubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
                .with_directory_decision(decision),
        )
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<MoesiState, MoesiHarnessError> {
        Ok(self.cache(agent)?.state())
    }

    pub fn directory_state(&self) -> MoesiDirectoryLineState {
        self.directory.line_state(self.line)
    }

    pub fn cpu_responses(&self) -> Vec<MoesiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[MoesiDirectoryDecision] {
        &self.directory_decisions
    }

    pub fn cache_data(&self, agent: AgentId) -> Result<Option<Vec<u8>>, MoesiHarnessError> {
        Ok(self.cache(agent)?.cached_data().map(<[u8]>::to_vec))
    }

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    fn directory_response(
        &mut self,
        request: &MemoryRequest,
        decision: &MoesiDirectoryDecision,
    ) -> Result<MemoryResponse, MoesiHarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(MoesiHarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(snoop.event())
                .map_err(map_moesi_cache_error)?;
        }

        match grant.data_source() {
            MoesiDirectoryDataSource::BackingMemory => self
                .backing
                .respond(request)
                .map_err(MoesiHarnessError::Backing),
            MoesiDirectoryDataSource::OwnerCache(_) if request.returns_data() => {
                MemoryResponse::completed(request, source_data).map_err(MoesiHarnessError::Memory)
            }
            MoesiDirectoryDataSource::OwnerCache(_) => {
                MemoryResponse::completed(request, None).map_err(MoesiHarnessError::Memory)
            }
            MoesiDirectoryDataSource::NoData => {
                MemoryResponse::completed(request, None).map_err(MoesiHarnessError::Memory)
            }
        }
    }

    fn source_data(
        &self,
        grant: MoesiDirectoryGrant,
    ) -> Result<Option<Vec<u8>>, MoesiHarnessError> {
        match grant.data_source() {
            MoesiDirectoryDataSource::BackingMemory | MoesiDirectoryDataSource::NoData => Ok(None),
            MoesiDirectoryDataSource::OwnerCache(agent) => {
                let data = self.cache(agent)?.cached_data().ok_or(
                    MoesiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    },
                )?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn cache(&self, agent: AgentId) -> Result<&MoesiCacheController, MoesiHarnessError> {
        self.caches
            .get(&agent)
            .ok_or(MoesiHarnessError::UnknownCache { agent })
    }

    fn cache_mut(
        &mut self,
        agent: AgentId,
    ) -> Result<&mut MoesiCacheController, MoesiHarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(MoesiHarnessError::UnknownCache { agent })
    }

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: MoesiCacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(moesi_response_record(tick, cache_result, response));
    }
}

fn moesi_fill_event(decision: &MoesiDirectoryDecision) -> Result<MoesiEvent, MoesiHarnessError> {
    let state = decision
        .grant()
        .ok_or(MoesiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?
        .state();
    match state {
        MoesiState::Shared => Ok(MoesiEvent::DataShared),
        MoesiState::Exclusive => Ok(MoesiEvent::DataExclusive),
        MoesiState::Modified => Ok(MoesiEvent::DataModified),
        state => Err(MoesiHarnessError::UnexpectedGrantState { state }),
    }
}

fn moesi_response_record(
    tick: u64,
    cache_result: MoesiCacheControllerResultKind,
    response: &MemoryResponse,
) -> MoesiCpuResponseRecord {
    MoesiCpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_moesi_cache_error(error: MoesiCacheControllerError) -> MoesiHarnessError {
    match error {
        MoesiCacheControllerError::LineBusy { state } => MoesiHarnessError::LineBusy { state },
        error => MoesiHarnessError::Cache(error),
    }
}
