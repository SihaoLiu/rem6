use std::error::Error;
use std::fmt;

use rem6_cache::{MesiCacheControllerError, MesiCacheControllerResultKind};
use rem6_directory::{MesiDirectoryDataSource, MesiDirectoryDecision, MesiDirectoryError};
use rem6_dram::DramMemoryError;
use rem6_kernel::SchedulerError;
use rem6_memory::{AgentId, MemoryError, MemoryRequestId, MemoryResponse, ResponseStatus};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::TransportError;

use crate::{HarnessError, SubmitKind};

mod partitioned;
mod serial;

pub use partitioned::PartitionedMesiDirectoryLineHarness;
pub use serial::MesiDirectoryLineHarness;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MesiHarnessError {
    LineBusy { state: MesiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: MesiLineId },
    UnexpectedGrantState { state: MesiState },
    UnsupportedRouteHops { agent: AgentId },
    UnsupportedMemoryRouteHops,
    Cache(MesiCacheControllerError),
    Directory(MesiDirectoryError),
    Memory(MemoryError),
    Dram(DramMemoryError),
    Scheduler(SchedulerError),
    Transport(TransportError),
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
            Self::UnsupportedRouteHops { agent } => write!(
                formatter,
                "cache agent {} uses route hops that are not supported by this harness",
                agent.get()
            ),
            Self::UnsupportedMemoryRouteHops => {
                write!(
                    formatter,
                    "memory route hops are not supported by this harness"
                )
            }
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Dram(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
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
            Self::Dram(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiDirectoryDecisionRecord {
    tick: u64,
    requester: AgentId,
    decision: MesiDirectoryDecision,
}

impl MesiDirectoryDecisionRecord {
    pub const fn new(tick: u64, requester: AgentId, decision: MesiDirectoryDecision) -> Self {
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

    pub const fn decision(&self) -> &MesiDirectoryDecision {
        &self.decision
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

fn decision_uses_mesi_backing_memory(decision: &MesiDirectoryDecision) -> bool {
    decision
        .grant()
        .is_some_and(|grant| grant.data_source() == MesiDirectoryDataSource::BackingMemory)
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
