use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, SchedulerError, Tick};
use rem6_memory::{MemoryError, MemoryRequestId};
use rem6_topology::{Endpoint, TopologyError};
use rem6_transport::{TopologyRouteError, TransportError};

use crate::{AcceleratorCommandId, AcceleratorEngineId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AcceleratorError {
    ZeroLanes {
        engine: AcceleratorEngineId,
    },
    ZeroExecutionLatency {
        command: AcceleratorCommandId,
    },
    DmaReadRequiresData {
        command: AcceleratorCommandId,
        request: MemoryRequestId,
    },
    SnapshotLaneCountMismatch {
        engine: AcceleratorEngineId,
        expected: usize,
        actual: usize,
    },
    MissingCommandSubmission {
        engine: AcceleratorEngineId,
    },
    SourcePartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
    CommandTargetPartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
    TickOverflow {
        now: Tick,
        delay: Tick,
    },
    Memory(MemoryError),
    Scheduler(SchedulerError),
    Topology(TopologyError),
    TopologyRoute(TopologyRouteError),
    Transport(TransportError),
}

impl fmt::Display for AcceleratorError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroLanes { engine } => {
                write!(
                    formatter,
                    "accelerator engine {} needs at least one lane",
                    engine.get()
                )
            }
            Self::ZeroExecutionLatency { command } => write!(
                formatter,
                "accelerator command {} needs positive execution latency",
                command.get()
            ),
            Self::DmaReadRequiresData { command, request } => write!(
                formatter,
                "accelerator command {} DMA read request {} from agent {} must return data",
                command.get(),
                request.sequence(),
                request.agent().get(),
            ),
            Self::SnapshotLaneCountMismatch {
                engine,
                expected,
                actual,
            } => write!(
                formatter,
                "accelerator engine {} snapshot has {actual} lanes but expected {expected}",
                engine.get()
            ),
            Self::MissingCommandSubmission { engine } => write!(
                formatter,
                "accelerator engine {} has no topology command submission path",
                engine.get()
            ),
            Self::SourcePartitionMismatch {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "endpoint {}.{} is on partition {} but accelerator partition is {}",
                endpoint.component().as_str(),
                endpoint.port().as_str(),
                actual.index(),
                expected.index(),
            ),
            Self::CommandTargetPartitionMismatch {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "command endpoint {}.{} is on partition {} but accelerator partition is {}",
                endpoint.component().as_str(),
                endpoint.port().as_str(),
                actual.index(),
                expected.index(),
            ),
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::TopologyRoute(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for AcceleratorError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Topology(error) => Some(error),
            Self::TopologyRoute(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}
