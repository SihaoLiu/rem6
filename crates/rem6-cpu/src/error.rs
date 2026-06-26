use std::error::Error;
use std::fmt;

use rem6_isa_riscv::{RiscvError, RiscvPmaError, RiscvPmpError};
use rem6_kernel::{PartitionId, SchedulerError};
use rem6_memory::{AccessSize, Address, AgentId, MemoryError, MemoryRequestId, TranslationFault};
use rem6_mmio::MmioError;
use rem6_transport::{MemoryRouteId, TransportEndpointId, TransportError};

use crate::{
    BiModeBranchPredictorError, BranchPredictorError, CpuId, CpuTranslationFrontendError,
    GShareBranchPredictorError, InOrderPipelineError, MultiperspectivePerceptronError,
    TageScLBranchPredictorError, TournamentBranchPredictorError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuError {
    FetchCrossesLine {
        pc: Address,
        size: AccessSize,
        line_size: u64,
    },
    RouteEndpointMismatch {
        route: MemoryRouteId,
        expected: TransportEndpointId,
        actual: TransportEndpointId,
    },
    RoutePartitionMismatch {
        route: MemoryRouteId,
        expected: PartitionId,
        actual: PartitionId,
    },
    Memory(MemoryError),
    Transport(TransportError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuClusterError {
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
    UnknownCpu {
        cpu: CpuId,
    },
    Cpu(CpuError),
}

impl fmt::Display for CpuClusterError {
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
            Self::UnknownCpu { cpu } => write!(formatter, "CPU {} is not registered", cpu.get()),
            Self::Cpu(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CpuClusterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            _ => None,
        }
    }
}

impl fmt::Display for CpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FetchCrossesLine {
                pc,
                size,
                line_size,
            } => write!(
                formatter,
                "instruction fetch at {:#x} for {} bytes crosses a {line_size}-byte line",
                pc.get(),
                size.bytes()
            ),
            Self::RouteEndpointMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU fetch route {} starts at endpoint {} but CPU endpoint is {}",
                route.get(),
                actual.as_str(),
                expected.as_str()
            ),
            Self::RoutePartitionMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU fetch route {} starts on partition {} but CPU partition is {}",
                route.get(),
                actual.index(),
                expected.index()
            ),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Memory(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCpuError {
    MissingDataConfig {
        fetch: MemoryRequestId,
    },
    MissingDataTranslationConfig {
        fetch: MemoryRequestId,
    },
    DataTranslationPageMapRequired {
        fetch: MemoryRequestId,
    },
    MissingFetchData {
        request: MemoryRequestId,
    },
    InvalidFetchWidth {
        request: MemoryRequestId,
        bytes: u64,
    },
    MissingBranchSpeculationInstruction {
        sequence: u64,
    },
    PcMismatch {
        fetch: Address,
        architectural: Address,
    },
    DataAccessCrossesLine {
        address: Address,
        size: AccessSize,
        line_size: u64,
    },
    DataRouteEndpointMismatch {
        route: MemoryRouteId,
        expected: TransportEndpointId,
        actual: TransportEndpointId,
    },
    DataRoutePartitionMismatch {
        route: MemoryRouteId,
        expected: PartitionId,
        actual: PartitionId,
    },
    MmioRoutePartitionMismatch {
        expected: PartitionId,
        actual: PartitionId,
    },
    UnsupportedMmioAtomic {
        request: MemoryRequestId,
        address: Address,
    },
    DataTranslation(CpuTranslationFrontendError),
    DataTranslationFault {
        fetch: MemoryRequestId,
        fault: TranslationFault,
    },
    FetchPmpAccess {
        pc: Address,
        error: RiscvPmpError,
    },
    FetchPmaAccess {
        pc: Address,
        error: RiscvPmaError,
    },
    DataPmpAccess {
        fetch: MemoryRequestId,
        error: RiscvPmpError,
    },
    DataPmaAccess {
        fetch: MemoryRequestId,
        error: RiscvPmaError,
    },
    Cpu(CpuError),
    BranchPredictor(BranchPredictorError),
    GShareBranchPredictor(GShareBranchPredictorError),
    BiModeBranchPredictor(BiModeBranchPredictorError),
    TournamentBranchPredictor(TournamentBranchPredictorError),
    TageScLBranchPredictor(TageScLBranchPredictorError),
    MultiperspectivePerceptron(MultiperspectivePerceptronError),
    InOrderPipeline(InOrderPipelineError),
    Isa(RiscvError),
    Memory(MemoryError),
    Mmio(MmioError),
    Scheduler(SchedulerError),
    Transport(TransportError),
}

impl fmt::Display for RiscvCpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingDataConfig { fetch } => write!(
                formatter,
                "fetch response {} from agent {} needs a data route for memory access",
                fetch.sequence(),
                fetch.agent().get()
            ),
            Self::MissingDataTranslationConfig { fetch } => write!(
                formatter,
                "fetch response {} from agent {} needs a data translation frontend",
                fetch.sequence(),
                fetch.agent().get()
            ),
            Self::DataTranslationPageMapRequired { fetch } => write!(
                formatter,
                "fetch response {} from agent {} needs a data translation page map",
                fetch.sequence(),
                fetch.agent().get()
            ),
            Self::MissingFetchData { request } => write!(
                formatter,
                "fetch response {} from agent {} has no instruction bytes",
                request.sequence(),
                request.agent().get()
            ),
            Self::InvalidFetchWidth { request, bytes } => write!(
                formatter,
                "fetch response {} from agent {} has {bytes} bytes instead of 4",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingBranchSpeculationInstruction { sequence } => write!(
                formatter,
                "branch speculation sequence {sequence} has no decodable completed instruction"
            ),
            Self::PcMismatch {
                fetch,
                architectural,
            } => write!(
                formatter,
                "fetch pc {:#x} does not match architectural pc {:#x}",
                fetch.get(),
                architectural.get()
            ),
            Self::DataAccessCrossesLine {
                address,
                size,
                line_size,
            } => write!(
                formatter,
                "data access at {:#x} for {} bytes crosses a {line_size}-byte line",
                address.get(),
                size.bytes()
            ),
            Self::DataRouteEndpointMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU data route {} starts at endpoint {} but CPU data endpoint is {}",
                route.get(),
                actual.as_str(),
                expected.as_str()
            ),
            Self::DataRoutePartitionMismatch {
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "CPU data route {} starts on partition {} but CPU partition is {}",
                route.get(),
                actual.index(),
                expected.index()
            ),
            Self::MmioRoutePartitionMismatch { expected, actual } => write!(
                formatter,
                "MMIO data route starts on partition {} but CPU partition is {}",
                actual.index(),
                expected.index()
            ),
            Self::UnsupportedMmioAtomic { request, address } => write!(
                formatter,
                "MMIO data request {} from agent {} at {:#x} cannot provide atomic old-value response data",
                request.sequence(),
                request.agent().get(),
                address.get()
            ),
            Self::DataTranslation(error) => write!(formatter, "{error}"),
            Self::DataTranslationFault { fetch, fault } => write!(
                formatter,
                "data translation for fetch response {} from agent {} faulted at {:#x}",
                fetch.sequence(),
                fetch.agent().get(),
                fault.virtual_address().get()
            ),
            Self::FetchPmpAccess { pc, error } => write!(
                formatter,
                "instruction fetch PMP check at {:#x} failed: {error}",
                pc.get()
            ),
            Self::FetchPmaAccess { pc, error } => write!(
                formatter,
                "instruction fetch PMA check at {:#x} failed: {error}",
                pc.get()
            ),
            Self::DataPmpAccess { fetch, error } => write!(
                formatter,
                "data PMP check for fetch response {} from agent {} failed: {error}",
                fetch.sequence(),
                fetch.agent().get()
            ),
            Self::DataPmaAccess { fetch, error } => write!(
                formatter,
                "data PMA check for fetch response {} from agent {} failed: {error}",
                fetch.sequence(),
                fetch.agent().get()
            ),
            Self::Cpu(error) => write!(formatter, "{error}"),
            Self::BranchPredictor(error) => write!(formatter, "{error}"),
            Self::GShareBranchPredictor(error) => write!(formatter, "{error}"),
            Self::BiModeBranchPredictor(error) => write!(formatter, "{error}"),
            Self::TournamentBranchPredictor(error) => write!(formatter, "{error}"),
            Self::TageScLBranchPredictor(error) => write!(formatter, "{error}"),
            Self::MultiperspectivePerceptron(error) => write!(formatter, "{error}"),
            Self::InOrderPipeline(error) => write!(formatter, "{error}"),
            Self::Isa(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Mmio(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvCpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cpu(error) => Some(error),
            Self::BranchPredictor(error) => Some(error),
            Self::GShareBranchPredictor(error) => Some(error),
            Self::BiModeBranchPredictor(error) => Some(error),
            Self::TournamentBranchPredictor(error) => Some(error),
            Self::TageScLBranchPredictor(error) => Some(error),
            Self::MultiperspectivePerceptron(error) => Some(error),
            Self::InOrderPipeline(error) => Some(error),
            Self::DataTranslation(error) => Some(error),
            Self::FetchPmpAccess { error, .. } => Some(error),
            Self::FetchPmaAccess { error, .. } => Some(error),
            Self::DataPmpAccess { error, .. } => Some(error),
            Self::DataPmaAccess { error, .. } => Some(error),
            Self::Isa(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Mmio(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}
