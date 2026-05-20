use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_cpu::{CpuId, CpuTopologyError, RiscvCluster, RiscvClusterTopologyConfig};
use rem6_kernel::{ParallelSchedulerContext, PartitionedScheduler, SchedulerError, Tick};
use rem6_memory::{MemoryError, MemoryResponse, PartitionedMemoryStore};
use rem6_mmio::MmioBus;
use rem6_platform::Platform;
use rem6_topology::Topology;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::{GuestEventId, RiscvSystemRun, RiscvSystemRunDriver, SystemError};

pub struct RiscvTopologySystem {
    topology: Topology,
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    cluster: RiscvCluster,
    platform: Option<Platform>,
    memory: Option<Arc<Mutex<PartitionedMemoryStore>>>,
}

impl RiscvTopologySystem {
    pub fn with_min_remote_delay(
        topology: Topology,
        cluster_config: RiscvClusterTopologyConfig,
        min_remote_delay: Tick,
    ) -> Result<Self, RiscvTopologySystemError> {
        let scheduler = PartitionedScheduler::with_min_remote_delay(
            topology.partition_count(),
            min_remote_delay,
        )
        .map_err(RiscvTopologySystemError::Scheduler)?;
        let mut transport = MemoryTransport::new();
        let cluster = RiscvCluster::from_topology(&topology, &mut transport, cluster_config)
            .map_err(RiscvTopologySystemError::CpuTopology)?;

        Ok(Self {
            topology,
            scheduler,
            transport,
            cluster,
            platform: None,
            memory: None,
        })
    }

    pub fn with_platform(mut self, platform: Platform) -> Result<Self, RiscvTopologySystemError> {
        if platform.partition_count() != self.topology.partition_count() {
            return Err(RiscvTopologySystemError::PlatformPartitionMismatch {
                topology: self.topology.partition_count(),
                platform: platform.partition_count(),
            });
        }

        self.platform = Some(platform);
        Ok(self)
    }

    pub fn with_memory_store(
        mut self,
        memory: PartitionedMemoryStore,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.memory = Some(Arc::new(Mutex::new(memory)));
        Ok(self)
    }

    pub const fn topology(&self) -> &Topology {
        &self.topology
    }

    pub const fn scheduler(&self) -> &PartitionedScheduler {
        &self.scheduler
    }

    pub fn scheduler_mut(&mut self) -> &mut PartitionedScheduler {
        &mut self.scheduler
    }

    pub const fn transport(&self) -> &MemoryTransport {
        &self.transport
    }

    pub const fn cluster(&self) -> &RiscvCluster {
        &self.cluster
    }

    pub const fn platform(&self) -> Option<&Platform> {
        self.platform.as_ref()
    }

    pub fn platform_bus(&self) -> Option<&MmioBus> {
        self.platform.as_ref().map(Platform::mmio_bus)
    }

    pub fn memory_store(&self) -> Option<&Arc<Mutex<PartitionedMemoryStore>>> {
        self.memory.as_ref()
    }

    pub fn execution_parts_mut(
        &mut self,
    ) -> (&RiscvCluster, &mut PartitionedScheduler, &MemoryTransport) {
        (&self.cluster, &mut self.scheduler, &self.transport)
    }

    pub fn execution_parts_with_mmio_mut(
        &mut self,
    ) -> Option<(
        &RiscvCluster,
        &mut PartitionedScheduler,
        &MemoryTransport,
        &MmioBus,
    )> {
        let platform = self.platform.as_ref()?;
        Some((
            &self.cluster,
            &mut self.scheduler,
            &self.transport,
            platform.mmio_bus(),
        ))
    }

    pub fn drive_until_host_stop_parallel<E>(
        &mut self,
        driver: &RiscvSystemRunDriver,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        max_turns: usize,
        event_for: E,
    ) -> Result<RiscvSystemRun, RiscvTopologySystemError>
    where
        E: FnMut(CpuId) -> GuestEventId,
    {
        let memory = self
            .memory
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingMemoryStore)?
            .clone();
        let memory_error = Arc::new(Mutex::new(None));

        let fetch_memory = Arc::clone(&memory);
        let fetch_error = Arc::clone(&memory_error);
        let fetch_responder = move |_cpu| {
            let memory = Arc::clone(&fetch_memory);
            let memory_error = Arc::clone(&fetch_error);
            move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                topology_memory_response(&memory, &memory_error, &delivery)
            }
        };

        let data_memory = Arc::clone(&memory);
        let data_error = Arc::clone(&memory_error);
        let data_responder = move |_cpu| {
            let memory = Arc::clone(&data_memory);
            let memory_error = Arc::clone(&data_error);
            move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                topology_memory_response(&memory, &memory_error, &delivery)
            }
        };

        let result = if let Some(platform) = self.platform.as_ref() {
            driver.drive_until_host_stop_parallel_with_mmio(
                &self.cluster,
                &mut self.scheduler,
                &self.transport,
                platform.mmio_bus(),
                fetch_trace,
                data_trace,
                fetch_responder,
                data_responder,
                max_turns,
                event_for,
            )
        } else {
            driver.drive_until_host_stop_parallel(
                &self.cluster,
                &mut self.scheduler,
                &self.transport,
                fetch_trace,
                data_trace,
                fetch_responder,
                data_responder,
                max_turns,
                event_for,
            )
        };

        let run = match result {
            Ok(run) => run,
            Err(error) => {
                if let Some(memory_error) = take_memory_error(&memory_error) {
                    return Err(RiscvTopologySystemError::Memory(memory_error));
                }
                return Err(RiscvTopologySystemError::System(error));
            }
        };
        if let Some(memory_error) = take_memory_error(&memory_error) {
            return Err(RiscvTopologySystemError::Memory(memory_error));
        }

        Ok(run)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvTopologySystemError {
    Scheduler(SchedulerError),
    CpuTopology(CpuTopologyError),
    PlatformPartitionMismatch { topology: u32, platform: u32 },
    MissingMemoryStore,
    Memory(MemoryError),
    System(SystemError),
}

impl fmt::Display for RiscvTopologySystemError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::CpuTopology(error) => write!(formatter, "{error}"),
            Self::PlatformPartitionMismatch { topology, platform } => write!(
                formatter,
                "platform partition count {platform} does not match topology partition count {topology}"
            ),
            Self::MissingMemoryStore => write!(formatter, "topology system has no memory store"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::System(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for RiscvTopologySystemError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
            Self::CpuTopology(error) => Some(error),
            Self::PlatformPartitionMismatch { .. } => None,
            Self::MissingMemoryStore => None,
            Self::Memory(error) => Some(error),
            Self::System(error) => Some(error),
        }
    }
}

fn topology_memory_response(
    memory: &Arc<Mutex<PartitionedMemoryStore>>,
    memory_error: &Arc<Mutex<Option<MemoryError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match memory
        .lock()
        .expect("topology memory store lock")
        .respond(delivery.request())
    {
        Ok(outcome) => outcome
            .response()
            .cloned()
            .map(TargetOutcome::Respond)
            .unwrap_or(TargetOutcome::NoResponse),
        Err(error) => {
            *memory_error.lock().expect("topology memory error lock") = Some(error);
            TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
        }
    }
}

fn take_memory_error(memory_error: &Arc<Mutex<Option<MemoryError>>>) -> Option<MemoryError> {
    memory_error
        .lock()
        .expect("topology memory error lock")
        .take()
}
