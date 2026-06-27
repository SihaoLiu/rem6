use std::error::Error;
use std::fmt;

use rem6_accelerator::{AcceleratorEngineId, AcceleratorError};
use rem6_boot::BootError;
use rem6_coherence::{ChiHarnessError, HarnessError, MesiHarnessError, MoesiHarnessError};
use rem6_cpu::CpuError;
use rem6_dram::DramMemoryError;
use rem6_gpu::{GpuDeviceId, GpuError};
use rem6_kernel::SchedulerError;
use rem6_memory::{MemoryError, MemoryRequestId, TranslationError};
use rem6_transport::TransportError;
use rem6_workload::{WorkloadError, WorkloadRouteId};

use crate::{
    RiscvDataCacheControllerErrorRecord, SystemError, TrafficTraceReplayControllerParallelErrors,
    TrafficTraceReplayControllerParallelSubmitError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvWorkloadReplayError {
    MissingTopology,
    MissingMemoryTarget,
    MissingDataCacheAgent,
    MissingDataCacheLine,
    MissingDataCacheResponse {
        request: MemoryRequestId,
    },
    GpuDmaRequestSequenceOverflow {
        transfer: u64,
    },
    MissingGpuDmaWrite {
        device: GpuDeviceId,
    },
    AcceleratorDmaRequestSequenceOverflow {
        transfer: u64,
    },
    MissingAcceleratorDmaWrite {
        engine: AcceleratorEngineId,
    },
    MissingRoute {
        route: WorkloadRouteId,
    },
    MissingFinalTick,
    Workload(WorkloadError),
    Boot(BootError),
    Dram(DramMemoryError),
    DramModel(rem6_dram::DramError),
    Memory(MemoryError),
    Translation(TranslationError),
    DataTranslationPageSizeMismatch {
        cpu: u32,
        expected: u64,
        actual: u64,
    },
    Gpu(GpuError),
    Accelerator(AcceleratorError),
    MsiDataCache(HarnessError),
    MesiDataCache(MesiHarnessError),
    MoesiDataCache(MoesiHarnessError),
    ChiDataCache(ChiHarnessError),
    DataCacheController {
        record: Box<RiscvDataCacheControllerErrorRecord>,
    },
    Cpu(CpuError),
    RiscvCluster(rem6_cpu::RiscvClusterError),
    Scheduler(SchedulerError),
    Transport(TransportError),
    TrafficTraceReplay(TrafficTraceReplayControllerParallelSubmitError),
    TrafficTraceReplayCallback {
        route: WorkloadRouteId,
        errors: TrafficTraceReplayControllerParallelErrors,
    },
    HostActionErrors {
        errors: Vec<SystemError>,
    },
    System(SystemError),
}

impl fmt::Display for RiscvWorkloadReplayError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTopology => write!(formatter, "workload replay plan is missing topology"),
            Self::MissingMemoryTarget => {
                write!(formatter, "workload replay topology has no memory target")
            }
            Self::MissingDataCacheAgent => {
                write!(
                    formatter,
                    "workload replay data cache has no RISC-V data agents"
                )
            }
            Self::MissingDataCacheLine => {
                write!(formatter, "workload replay data cache has no line data")
            }
            Self::MissingDataCacheResponse { request } => write!(
                formatter,
                "workload replay data cache did not record response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::GpuDmaRequestSequenceOverflow { transfer } => write!(
                formatter,
                "workload replay GPU DMA transfer {transfer} cannot be mapped to request sequences"
            ),
            Self::MissingGpuDmaWrite { device } => write!(
                formatter,
                "workload replay GPU device {} has no pending DMA write",
                device.get()
            ),
            Self::AcceleratorDmaRequestSequenceOverflow { transfer } => write!(
                formatter,
                "workload replay accelerator DMA transfer {transfer} cannot be mapped to request sequences"
            ),
            Self::MissingAcceleratorDmaWrite { engine } => write!(
                formatter,
                "workload replay accelerator engine {} has no pending DMA write",
                engine.get()
            ),
            Self::MissingRoute { route } => {
                write!(
                    formatter,
                    "workload replay route {} is not declared",
                    route.as_str()
                )
            }
            Self::MissingFinalTick => write!(formatter, "RISC-V run did not report a final tick"),
            Self::Workload(error) => write!(formatter, "{error}"),
            Self::Boot(error) => write!(formatter, "{error}"),
            Self::Dram(error) => write!(formatter, "{error}"),
            Self::DramModel(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Translation(error) => write!(formatter, "{error}"),
            Self::DataTranslationPageSizeMismatch {
                cpu,
                expected,
                actual,
            } => write!(
                formatter,
                "workload RISC-V core {cpu} data translation page size {actual} does not match expected {expected}"
            ),
            Self::Gpu(error) => write!(formatter, "{error}"),
            Self::Accelerator(error) => write!(formatter, "{error}"),
            Self::MsiDataCache(error) => write!(formatter, "{error}"),
            Self::MesiDataCache(error) => write!(formatter, "{error}"),
            Self::MoesiDataCache(error) => write!(formatter, "{error}"),
            Self::ChiDataCache(error) => write!(formatter, "{error}"),
            Self::DataCacheController { record } => write!(formatter, "{record}"),
            Self::Cpu(error) => write!(formatter, "{error}"),
            Self::RiscvCluster(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::TrafficTraceReplay(error) => write!(formatter, "{error}"),
            Self::TrafficTraceReplayCallback { route, errors } => write!(
                formatter,
                "workload traffic trace replay on route {} recorded {} target callback errors and {} control callback errors",
                route.as_str(),
                errors.target().len(),
                errors.control().len(),
            ),
            Self::HostActionErrors { errors } => match errors.as_slice() {
                [error] => write!(formatter, "host action failed: {error}"),
                errors => write!(
                    formatter,
                    "host actions recorded {} errors; first error: {}",
                    errors.len(),
                    errors
                        .first()
                        .map(ToString::to_string)
                        .unwrap_or_else(|| "none".to_string())
                ),
            },
            Self::System(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvWorkloadReplayError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Workload(error) => Some(error),
            Self::Boot(error) => Some(error),
            Self::Dram(error) => Some(error),
            Self::DramModel(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Translation(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Accelerator(error) => Some(error),
            Self::MsiDataCache(error) => Some(error),
            Self::MesiDataCache(error) => Some(error),
            Self::MoesiDataCache(error) => Some(error),
            Self::ChiDataCache(error) => Some(error),
            Self::DataCacheController { record } => Some(record.error()),
            Self::Cpu(error) => Some(error),
            Self::RiscvCluster(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::TrafficTraceReplay(error) => Some(error),
            Self::System(error) => Some(error),
            Self::HostActionErrors { errors } => {
                errors.first().map(|error| error as &(dyn Error + 'static))
            }
            Self::MissingTopology
            | Self::MissingMemoryTarget
            | Self::MissingDataCacheAgent
            | Self::MissingDataCacheLine
            | Self::MissingDataCacheResponse { .. }
            | Self::GpuDmaRequestSequenceOverflow { .. }
            | Self::MissingGpuDmaWrite { .. }
            | Self::AcceleratorDmaRequestSequenceOverflow { .. }
            | Self::MissingAcceleratorDmaWrite { .. }
            | Self::MissingRoute { .. }
            | Self::MissingFinalTick
            | Self::DataTranslationPageSizeMismatch { .. }
            | Self::TrafficTraceReplayCallback { .. } => None,
        }
    }
}
