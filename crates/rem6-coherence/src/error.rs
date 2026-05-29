use std::error::Error;
use std::fmt;

use rem6_cache::{CacheControllerError, MsiCacheBankError};
use rem6_directory::DirectoryError;
use rem6_dram::DramMemoryError;
use rem6_fabric::FabricError;
use rem6_kernel::SchedulerError;
use rem6_memory::{Address, AgentId, MemoryError, MemoryRequestId};
use rem6_protocol_msi::{MsiLineId, MsiState};
use rem6_topology::{Endpoint, TopologyError};
use rem6_transport::TransportError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HarnessError {
    LineBusy {
        state: MsiState,
    },
    UnknownCache {
        agent: AgentId,
    },
    DuplicateCache {
        agent: AgentId,
    },
    MissingTopologyConnection {
        from: Endpoint,
        to: Endpoint,
    },
    MissingBackingMemory {
        line: Address,
    },
    WrongLine {
        expected: Address,
        actual: Address,
    },
    LineDataSizeMismatch {
        expected: u64,
        actual: u64,
    },
    MissingDirectoryGrant {
        request: MemoryRequestId,
    },
    GrantDataUnavailable {
        agent: AgentId,
        line: MsiLineId,
    },
    ParallelLineConflict {
        line: Address,
        first: MemoryRequestId,
        second: MemoryRequestId,
    },
    Cache(CacheControllerError),
    CacheBank(MsiCacheBankError),
    Directory(DirectoryError),
    Dram(DramMemoryError),
    Fabric(FabricError),
    Memory(MemoryError),
    Scheduler(SchedulerError),
    SnapshotResourceMismatch {
        resource: &'static str,
    },
    Topology(TopologyError),
    Transport(TransportError),
}

impl fmt::Display for HarnessError {
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
            Self::MissingTopologyConnection { from, to } => write!(
                formatter,
                "topology connection {}.{} to {}.{} is not declared",
                from.component().as_str(),
                from.port().as_str(),
                to.component().as_str(),
                to.port().as_str()
            ),
            Self::MissingBackingMemory { line } => {
                write!(formatter, "line {:#x} has no backing memory", line.get())
            }
            Self::WrongLine { expected, actual } => write!(
                formatter,
                "request for line {:#x} reached backing line {:#x}",
                actual.get(),
                expected.get()
            ),
            Self::LineDataSizeMismatch { expected, actual } => write!(
                formatter,
                "line data has {actual} bytes but line expects {expected}"
            ),
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
            Self::ParallelLineConflict {
                line,
                first,
                second,
            } => write!(
                formatter,
                "parallel MSI bank cycle has conflicting requests {} from agent {} and {} from agent {} for line {:#x}",
                first.sequence(),
                first.agent().get(),
                second.sequence(),
                second.agent().get(),
                line.get()
            ),
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::CacheBank(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Dram(error) => write!(formatter, "{error}"),
            Self::Fabric(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::SnapshotResourceMismatch { resource } => write!(
                formatter,
                "snapshot resource {resource} does not match harness configuration"
            ),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for HarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::CacheBank(error) => Some(error),
            Self::Directory(error) => Some(error),
            Self::Dram(error) => Some(error),
            Self::Fabric(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Topology(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}
