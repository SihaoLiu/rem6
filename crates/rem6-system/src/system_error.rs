use std::error::Error;
use std::fmt;

use rem6_checkpoint::CheckpointError;
use rem6_cpu::RiscvClusterError;
use rem6_kernel::SchedulerError;
use rem6_stats::StatsError;

use crate::{
    AcceleratorCheckpointError, ClintCheckpointError, CpuLocalTimerCheckpointError,
    DramMemoryCheckpointError, ExecutionModeCheckpointError, FabricCheckpointError,
    GpuCheckpointError, GuestFdCheckpointError, GuestFutexCheckpointError,
    GuestWaitCheckpointError, InterruptControllerCheckpointError, MemoryStoreCheckpointError,
    MsiBankCheckpointError, PciHostCheckpointError, PciLegacyInterruptRouterCheckpointError,
    Pl011UartCheckpointError, Pl031CheckpointError, PlicCheckpointError, ReadfileCheckpointError,
    RiscvCoreCheckpointError, RtcCheckpointError, SchedulerCheckpointError,
    SinicFifoCheckpointError, SinicRegisterCheckpointError, Sp804CheckpointError,
    Sp805CheckpointError, StorageCheckpointError, TimerCheckpointError, UartCheckpointError,
    VirtioPciCommonCheckpointError, VirtioPciDeviceConfigCheckpointError,
    VirtioPciIsrCheckpointError, VirtioPciNotifyCheckpointError, VirtioSplitQueueCheckpointError,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SystemError {
    ZeroHostLatency,
    Scheduler(SchedulerError),
    RiscvCluster(RiscvClusterError),
    Stats(StatsError),
    Checkpoint(CheckpointError),
    MissingCheckpointManifest { label: String },
    ExecutionModeCheckpoint(ExecutionModeCheckpointError),
    AcceleratorCheckpoint(AcceleratorCheckpointError),
    CpuLocalTimerCheckpoint(CpuLocalTimerCheckpointError),
    MsiBankCheckpoint(MsiBankCheckpointError),
    FabricCheckpoint(FabricCheckpointError),
    GpuCheckpoint(GpuCheckpointError),
    GuestFdCheckpoint(GuestFdCheckpointError),
    GuestFutexCheckpoint(GuestFutexCheckpointError),
    GuestWaitCheckpoint(GuestWaitCheckpointError),
    PciHostCheckpoint(PciHostCheckpointError),
    PciLegacyInterruptRouterCheckpoint(PciLegacyInterruptRouterCheckpointError),
    Pl031Checkpoint(Pl031CheckpointError),
    ReadfileCheckpoint(ReadfileCheckpointError),
    Sp804Checkpoint(Sp804CheckpointError),
    Sp805Checkpoint(Sp805CheckpointError),
    RiscvCheckpoint(RiscvCoreCheckpointError),
    RtcCheckpoint(RtcCheckpointError),
    SchedulerCheckpoint(SchedulerCheckpointError),
    MemoryCheckpoint(MemoryStoreCheckpointError),
    StorageCheckpoint(StorageCheckpointError),
    SinicRegisterCheckpoint(SinicRegisterCheckpointError),
    SinicFifoCheckpoint(SinicFifoCheckpointError),
    DramMemoryCheckpoint(DramMemoryCheckpointError),
    InterruptControllerCheckpoint(InterruptControllerCheckpointError),
    ClintCheckpoint(ClintCheckpointError),
    TimerCheckpoint(TimerCheckpointError),
    UartCheckpoint(UartCheckpointError),
    Pl011UartCheckpoint(Pl011UartCheckpointError),
    PlicCheckpoint(PlicCheckpointError),
    VirtioPciCommonCheckpoint(VirtioPciCommonCheckpointError),
    VirtioPciDeviceConfigCheckpoint(VirtioPciDeviceConfigCheckpointError),
    VirtioPciIsrCheckpoint(VirtioPciIsrCheckpointError),
    VirtioPciNotifyCheckpoint(VirtioPciNotifyCheckpointError),
    VirtioCheckpoint(VirtioSplitQueueCheckpointError),
}

impl fmt::Display for SystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroHostLatency => {
                write!(formatter, "guest event channel latency must be positive")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::RiscvCluster(error) => write!(formatter, "{error}"),
            Self::Stats(error) => write!(formatter, "{error}"),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::MissingCheckpointManifest { label } => {
                write!(formatter, "checkpoint manifest {label} is not available")
            }
            Self::ExecutionModeCheckpoint(error) => write!(formatter, "{error}"),
            Self::AcceleratorCheckpoint(error) => write!(formatter, "{error}"),
            Self::CpuLocalTimerCheckpoint(error) => write!(formatter, "{error}"),
            Self::MsiBankCheckpoint(error) => write!(formatter, "{error}"),
            Self::FabricCheckpoint(error) => write!(formatter, "{error}"),
            Self::GpuCheckpoint(error) => write!(formatter, "{error}"),
            Self::GuestFdCheckpoint(error) => write!(formatter, "{error}"),
            Self::GuestFutexCheckpoint(error) => write!(formatter, "{error}"),
            Self::GuestWaitCheckpoint(error) => write!(formatter, "{error}"),
            Self::PciHostCheckpoint(error) => write!(formatter, "{error}"),
            Self::PciLegacyInterruptRouterCheckpoint(error) => write!(formatter, "{error}"),
            Self::Pl031Checkpoint(error) => write!(formatter, "{error}"),
            Self::ReadfileCheckpoint(error) => write!(formatter, "{error}"),
            Self::Sp804Checkpoint(error) => write!(formatter, "{error}"),
            Self::Sp805Checkpoint(error) => write!(formatter, "{error}"),
            Self::RiscvCheckpoint(error) => write!(formatter, "{error}"),
            Self::RtcCheckpoint(error) => write!(formatter, "{error}"),
            Self::SchedulerCheckpoint(error) => write!(formatter, "{error}"),
            Self::MemoryCheckpoint(error) => write!(formatter, "{error}"),
            Self::StorageCheckpoint(error) => write!(formatter, "{error}"),
            Self::SinicRegisterCheckpoint(error) => write!(formatter, "{error}"),
            Self::SinicFifoCheckpoint(error) => write!(formatter, "{error}"),
            Self::DramMemoryCheckpoint(error) => write!(formatter, "{error}"),
            Self::InterruptControllerCheckpoint(error) => write!(formatter, "{error}"),
            Self::ClintCheckpoint(error) => write!(formatter, "{error}"),
            Self::TimerCheckpoint(error) => write!(formatter, "{error}"),
            Self::UartCheckpoint(error) => write!(formatter, "{error}"),
            Self::Pl011UartCheckpoint(error) => write!(formatter, "{error}"),
            Self::PlicCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciCommonCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciDeviceConfigCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciIsrCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioPciNotifyCheckpoint(error) => write!(formatter, "{error}"),
            Self::VirtioCheckpoint(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for SystemError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            Self::RiscvCluster(error) => Some(error),
            Self::Stats(error) => Some(error),
            Self::Checkpoint(error) => Some(error),
            Self::MissingCheckpointManifest { .. } => None,
            Self::ExecutionModeCheckpoint(error) => Some(error),
            Self::AcceleratorCheckpoint(error) => Some(error),
            Self::CpuLocalTimerCheckpoint(error) => Some(error),
            Self::MsiBankCheckpoint(error) => Some(error),
            Self::FabricCheckpoint(error) => Some(error),
            Self::GpuCheckpoint(error) => Some(error),
            Self::GuestFdCheckpoint(error) => Some(error),
            Self::GuestFutexCheckpoint(error) => Some(error),
            Self::GuestWaitCheckpoint(error) => Some(error),
            Self::PciHostCheckpoint(error) => Some(error),
            Self::PciLegacyInterruptRouterCheckpoint(error) => Some(error),
            Self::Pl031Checkpoint(error) => Some(error),
            Self::ReadfileCheckpoint(error) => Some(error),
            Self::Sp804Checkpoint(error) => Some(error),
            Self::Sp805Checkpoint(error) => Some(error),
            Self::RiscvCheckpoint(error) => Some(error),
            Self::RtcCheckpoint(error) => Some(error),
            Self::SchedulerCheckpoint(error) => Some(error),
            Self::MemoryCheckpoint(error) => Some(error),
            Self::StorageCheckpoint(error) => Some(error),
            Self::SinicRegisterCheckpoint(error) => Some(error),
            Self::SinicFifoCheckpoint(error) => Some(error),
            Self::DramMemoryCheckpoint(error) => Some(error),
            Self::InterruptControllerCheckpoint(error) => Some(error),
            Self::ClintCheckpoint(error) => Some(error),
            Self::TimerCheckpoint(error) => Some(error),
            Self::UartCheckpoint(error) => Some(error),
            Self::Pl011UartCheckpoint(error) => Some(error),
            Self::PlicCheckpoint(error) => Some(error),
            Self::VirtioPciCommonCheckpoint(error) => Some(error),
            Self::VirtioPciDeviceConfigCheckpoint(error) => Some(error),
            Self::VirtioPciIsrCheckpoint(error) => Some(error),
            Self::VirtioPciNotifyCheckpoint(error) => Some(error),
            Self::VirtioCheckpoint(error) => Some(error),
            Self::ZeroHostLatency => None,
        }
    }
}
