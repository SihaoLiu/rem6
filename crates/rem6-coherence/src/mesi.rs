use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_cache::{MesiCacheController, MesiCacheControllerError, MesiCacheControllerResultKind};
use rem6_directory::{
    MesiDirectory, MesiDirectoryDataSource, MesiDirectoryDecision, MesiDirectoryError,
    MesiDirectoryGrant, MesiDirectoryLineState,
};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::TargetOutcome;

use crate::{HarnessError, LineBackingStore, SubmitKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MesiHarnessError {
    LineBusy { state: MesiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: MesiLineId },
    UnexpectedGrantState { state: MesiState },
    Cache(MesiCacheControllerError),
    Directory(MesiDirectoryError),
    Memory(MemoryError),
    Backing(HarnessError),
}

impl fmt::Display for MesiHarnessError {
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

impl Error for MesiHarnessError {
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
pub struct MesiSubmitResult {
    kind: SubmitKind,
    cache_result: MesiCacheControllerResultKind,
    directory_decision: Option<MesiDirectoryDecision>,
}

impl MesiSubmitResult {
    fn new(kind: SubmitKind, cache_result: MesiCacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
        }
    }

    fn with_directory_decision(mut self, decision: MesiDirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> MesiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&MesiDirectoryDecision> {
        self.directory_decision.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiCpuResponseRecord {
    tick: u64,
    cache_result: MesiCacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl MesiCpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: MesiCacheControllerResultKind,
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

    pub const fn cache_result(&self) -> MesiCacheControllerResultKind {
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

pub struct MesiDirectoryLineHarness {
    line: MesiLineId,
    directory: MesiDirectory,
    caches: BTreeMap<AgentId, MesiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<MesiCpuResponseRecord>,
    directory_decisions: Vec<MesiDirectoryDecision>,
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
}

fn fill_event(decision: &MesiDirectoryDecision) -> Result<MesiEvent, MesiHarnessError> {
    let state = decision
        .grant()
        .ok_or(MesiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?
        .state();
    match state {
        MesiState::Shared => Ok(MesiEvent::DataShared),
        MesiState::Exclusive => Ok(MesiEvent::DataExclusive),
        MesiState::Modified => Ok(MesiEvent::DataModified),
        state => Err(MesiHarnessError::UnexpectedGrantState { state }),
    }
}

fn mesi_response_record(
    tick: u64,
    cache_result: MesiCacheControllerResultKind,
    response: &MemoryResponse,
) -> MesiCpuResponseRecord {
    MesiCpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_mesi_cache_error(error: MesiCacheControllerError) -> MesiHarnessError {
    match error {
        MesiCacheControllerError::LineBusy { state } => MesiHarnessError::LineBusy { state },
        error => MesiHarnessError::Cache(error),
    }
}
