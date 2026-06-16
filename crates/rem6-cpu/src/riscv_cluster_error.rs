use std::error::Error;
use std::fmt;

use rem6_kernel::SchedulerError;
use rem6_memory::AgentId;
use rem6_transport::TransportEndpointId;

use crate::{CpuId, RiscvCpuError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvClusterError {
    DuplicateCpu {
        cpu: CpuId,
    },
    DuplicateAgent {
        agent: AgentId,
        existing: CpuId,
        duplicate: CpuId,
    },
    DuplicateFetchEndpoint {
        endpoint: TransportEndpointId,
        existing: CpuId,
        duplicate: CpuId,
    },
    DuplicateDataEndpoint {
        endpoint: TransportEndpointId,
        existing: CpuId,
        duplicate: CpuId,
    },
    UnknownCpu {
        cpu: CpuId,
    },
    Core {
        cpu: CpuId,
        error: RiscvCpuError,
    },
    Scheduler(SchedulerError),
    TurnLimitExceeded {
        limit: usize,
        completed: usize,
    },
}

impl fmt::Display for RiscvClusterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateCpu { cpu } => {
                write!(formatter, "CPU {} is already registered", cpu.get())
            }
            Self::DuplicateAgent {
                agent,
                existing,
                duplicate,
            } => write!(
                formatter,
                "agent {} is assigned to CPU {} and CPU {}",
                agent.get(),
                existing.get(),
                duplicate.get()
            ),
            Self::DuplicateFetchEndpoint {
                endpoint,
                existing,
                duplicate,
            } => write!(
                formatter,
                "fetch endpoint {} is assigned to CPU {} and CPU {}",
                endpoint.as_str(),
                existing.get(),
                duplicate.get()
            ),
            Self::DuplicateDataEndpoint {
                endpoint,
                existing,
                duplicate,
            } => write!(
                formatter,
                "data endpoint {} is assigned to CPU {} and CPU {}",
                endpoint.as_str(),
                existing.get(),
                duplicate.get()
            ),
            Self::UnknownCpu { cpu } => write!(formatter, "CPU {} is not registered", cpu.get()),
            Self::Core { cpu, error } => {
                write!(formatter, "CPU {} action failed: {error}", cpu.get())
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::TurnLimitExceeded { limit, completed } => write!(
                formatter,
                "RISC-V cluster run reached turn limit {limit} after {completed} completed turns"
            ),
        }
    }
}

impl Error for RiscvClusterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Core { error, .. } => Some(error),
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}
