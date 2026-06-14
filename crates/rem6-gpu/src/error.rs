use std::error::Error;
use std::fmt;

use rem6_kernel::{PartitionId, SchedulerError, Tick};
use rem6_memory::{MemoryError, MemoryRequestId};
use rem6_topology::{Endpoint, TopologyError};
use rem6_transport::{TopologyRouteError, TransportError};

use crate::{GpuDeviceId, GpuDmaId, GpuKernelId};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum GpuError {
    ZeroComputeUnits {
        device: GpuDeviceId,
    },
    ZeroWaveSlots {
        device: GpuDeviceId,
    },
    ZeroWorkgroups {
        kernel: GpuKernelId,
    },
    ZeroWorkgroupLatency {
        kernel: GpuKernelId,
    },
    DmaReadRequiresData {
        transfer: GpuDmaId,
        request: MemoryRequestId,
    },
    SnapshotSlotCountMismatch {
        device: GpuDeviceId,
        expected: usize,
        actual: usize,
    },
    SnapshotQueuedIsaProgramOutOfRange {
        device: GpuDeviceId,
        slot_index: usize,
        queue_index: usize,
    },
    SnapshotQueuedIsaProgramDuplicate {
        device: GpuDeviceId,
        slot_index: usize,
        queue_index: usize,
    },
    SnapshotQueuedIsaProgramMissing {
        device: GpuDeviceId,
        slot_index: usize,
        queue_index: usize,
    },
    CommandTargetPartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
    MemorySourcePartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
    TickOverflow {
        now: Tick,
        delay: Tick,
    },
    Scheduler(SchedulerError),
    Memory(MemoryError),
    Topology(TopologyError),
    TopologyRoute(TopologyRouteError),
    Transport(TransportError),
}

impl fmt::Display for GpuError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroComputeUnits { device } => {
                write!(
                    formatter,
                    "GPU device {} needs at least one compute unit",
                    device.get()
                )
            }
            Self::ZeroWaveSlots { device } => write!(
                formatter,
                "GPU device {} needs at least one wave slot per compute unit",
                device.get()
            ),
            Self::ZeroWorkgroups { kernel } => write!(
                formatter,
                "GPU kernel {} needs at least one workgroup",
                kernel.get()
            ),
            Self::ZeroWorkgroupLatency { kernel } => write!(
                formatter,
                "GPU kernel {} needs positive workgroup latency",
                kernel.get()
            ),
            Self::DmaReadRequiresData { transfer, request } => write!(
                formatter,
                "GPU DMA transfer {} read request {} from agent {} must return data",
                transfer.get(),
                request.sequence(),
                request.agent().get(),
            ),
            Self::SnapshotSlotCountMismatch {
                device,
                expected,
                actual,
            } => write!(
                formatter,
                "GPU device {} snapshot has {actual} slots but expected {expected}",
                device.get()
            ),
            Self::SnapshotQueuedIsaProgramOutOfRange {
                device,
                slot_index,
                queue_index,
            } => write!(
                formatter,
                "GPU device {} snapshot queued ISA program references missing slot {slot_index} queue {queue_index}",
                device.get()
            ),
            Self::SnapshotQueuedIsaProgramDuplicate {
                device,
                slot_index,
                queue_index,
            } => write!(
                formatter,
                "GPU device {} snapshot has duplicate queued ISA program for slot {slot_index} queue {queue_index}",
                device.get()
            ),
            Self::SnapshotQueuedIsaProgramMissing {
                device,
                slot_index,
                queue_index,
            } => write!(
                formatter,
                "GPU device {} snapshot is missing queued ISA program for slot {slot_index} queue {queue_index}",
                device.get()
            ),
            Self::CommandTargetPartitionMismatch {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "command endpoint {}.{} is on partition {} but GPU partition is {}",
                endpoint.component().as_str(),
                endpoint.port().as_str(),
                actual.index(),
                expected.index()
            ),
            Self::MemorySourcePartitionMismatch {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "memory endpoint {}.{} is on partition {} but GPU partition is {}",
                endpoint.component().as_str(),
                endpoint.port().as_str(),
                actual.index(),
                expected.index()
            ),
            Self::TickOverflow { now, delay } => {
                write!(formatter, "tick {now} overflows when adding delay {delay}")
            }
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::TopologyRoute(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for GpuError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Topology(error) => Some(error),
            Self::TopologyRoute(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}
