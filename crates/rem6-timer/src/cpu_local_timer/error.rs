use std::error::Error;
use std::fmt;

use rem6_interrupt::InterruptError;
use rem6_kernel::{PartitionId, Tick};
use rem6_mmio::{MmioError, MmioRequestId};

use super::MAX_PRESCALAR_SHIFT_ENTRY;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuLocalTimerError {
    InvalidClockTick { clock_tick: Tick },
    InvalidCpuCount { cpu_count: usize },
    CpuPartitionCountMismatch { cpus: usize, partitions: usize },
    DuplicateCpuPartition { partition: PartitionId },
    UnknownCpu { index: usize },
    UnknownCpuPartition { partition: PartitionId },
    UnknownRegister { offset: u64 },
    WriteOnlyRegister { offset: u64 },
    InvalidPrescalar { prescalar: u32 },
    TimeWentBack { tick: Tick, last_updated_tick: Tick },
    DeadlineOverflow,
    GenerationOverflow,
    Interrupt(InterruptError),
    Scheduler(rem6_kernel::SchedulerError),
}

impl fmt::Display for CpuLocalTimerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidClockTick { clock_tick } => {
                write!(
                    formatter,
                    "CPU local timer clock tick must be positive, got {clock_tick}"
                )
            }
            Self::InvalidCpuCount { cpu_count } => {
                write!(
                    formatter,
                    "CPU local timer CPU count must be positive, got {cpu_count}"
                )
            }
            Self::CpuPartitionCountMismatch { cpus, partitions } => write!(
                formatter,
                "CPU local timer has {cpus} CPUs but {partitions} partition mappings"
            ),
            Self::DuplicateCpuPartition { partition } => write!(
                formatter,
                "duplicate CPU local timer partition {}",
                partition.index()
            ),
            Self::UnknownCpu { index } => {
                write!(formatter, "unknown CPU local timer CPU index {index}")
            }
            Self::UnknownCpuPartition { partition } => write!(
                formatter,
                "unknown CPU local timer partition {}",
                partition.index()
            ),
            Self::UnknownRegister { offset } => write!(
                formatter,
                "unknown CPU local timer register offset {offset:#x}"
            ),
            Self::WriteOnlyRegister { offset } => write!(
                formatter,
                "CPU local timer register offset {offset:#x} is write-only"
            ),
            Self::InvalidPrescalar { prescalar } => write!(
                formatter,
                "CPU local timer prescalar shift entry must be at most {MAX_PRESCALAR_SHIFT_ENTRY}, got {prescalar}"
            ),
            Self::TimeWentBack {
                tick,
                last_updated_tick,
            } => write!(
                formatter,
                "CPU local timer tick {tick} is earlier than last updated tick {last_updated_tick}"
            ),
            Self::DeadlineOverflow => write!(formatter, "CPU local timer deadline overflowed"),
            Self::GenerationOverflow => write!(formatter, "CPU local timer generation overflowed"),
            Self::Interrupt(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CpuLocalTimerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Interrupt(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}

pub(super) fn mmio_error(request: MmioRequestId, error: CpuLocalTimerError) -> MmioError {
    MmioError::DeviceError {
        request,
        message: error.to_string(),
    }
}
