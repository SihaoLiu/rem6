use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::{BootError, BootImage};
use rem6_cpu::{CpuId, CpuTopologyError, RiscvCluster, RiscvClusterTopologyConfig};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionId, PartitionedScheduler, SchedulerError, Tick,
};
use rem6_memory::{
    AccessSize, Address, CacheLineLayout, MemoryError, MemoryResponse, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_mmio::MmioBus;
use rem6_platform::Platform;
use rem6_stats::StatsRegistry;
use rem6_topology::Topology;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};

use crate::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvTrapEventPort, SystemError, SystemHostController, SystemHostEventPort,
};

pub struct RiscvTopologySystem {
    topology: Topology,
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    cluster: RiscvCluster,
    platform: Option<Platform>,
    memory: Option<Arc<Mutex<PartitionedMemoryStore>>>,
    host: Option<RiscvTopologyHost>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvTopologyHostConfig {
    host_partition: PartitionId,
    host_latency: Tick,
    source: GuestSourceId,
}

impl RiscvTopologyHostConfig {
    pub const fn new(
        host_partition: PartitionId,
        host_latency: Tick,
        source: GuestSourceId,
    ) -> Self {
        Self {
            host_partition,
            host_latency,
            source,
        }
    }

    pub const fn host_partition(self) -> PartitionId {
        self.host_partition
    }

    pub const fn host_latency(self) -> Tick {
        self.host_latency
    }

    pub const fn source(self) -> GuestSourceId {
        self.source
    }
}

#[derive(Clone, Debug)]
struct RiscvTopologyHost {
    controller: Arc<Mutex<SystemHostController>>,
    driver: RiscvSystemRunDriver,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyMemoryConfig {
    target: MemoryTargetId,
    line_layout: CacheLineLayout,
    regions: Vec<RiscvTopologyMemoryRegion>,
}

impl RiscvTopologyMemoryConfig {
    pub fn new(target: MemoryTargetId, line_layout: CacheLineLayout) -> Self {
        Self {
            target,
            line_layout,
            regions: Vec::new(),
        }
    }

    pub fn add_region(mut self, start: Address, size: AccessSize) -> Self {
        self.regions
            .push(RiscvTopologyMemoryRegion::new(start, size));
        self
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }

    pub fn regions(&self) -> &[RiscvTopologyMemoryRegion] {
        &self.regions
    }

    fn build_store(&self) -> Result<PartitionedMemoryStore, MemoryError> {
        let mut store = PartitionedMemoryStore::new();
        store.add_partition(self.target, self.line_layout)?;
        for region in &self.regions {
            store.map_region(self.target, region.start(), region.size())?;
        }
        Ok(store)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RiscvTopologyMemoryRegion {
    start: Address,
    size: AccessSize,
}

impl RiscvTopologyMemoryRegion {
    pub const fn new(start: Address, size: AccessSize) -> Self {
        Self { start, size }
    }

    pub const fn start(self) -> Address {
        self.start
    }

    pub const fn size(self) -> AccessSize {
        self.size
    }
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
            host: None,
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

    pub fn with_host_controller(
        mut self,
        config: RiscvTopologyHostConfig,
        stats: StatsRegistry,
    ) -> Result<Self, RiscvTopologySystemError> {
        if config.host_partition().index() >= self.topology.partition_count() {
            return Err(RiscvTopologySystemError::HostPartitionOutOfRange {
                host: config.host_partition(),
                partitions: self.topology.partition_count(),
            });
        }

        let controller = Arc::new(Mutex::new(SystemHostController::new(
            HostEventPolicy,
            stats,
        )));
        let host_port = SystemHostEventPort::with_controller(
            config.host_partition(),
            config.host_latency(),
            Arc::clone(&controller),
        )
        .map_err(RiscvTopologySystemError::System)?;
        let trap_port = RiscvTrapEventPort::new(host_port, config.source());
        self.host = Some(RiscvTopologyHost {
            controller,
            driver: RiscvSystemRunDriver::new(trap_port),
        });
        Ok(self)
    }

    pub fn with_memory_store(
        mut self,
        memory: PartitionedMemoryStore,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.memory = Some(Arc::new(Mutex::new(memory)));
        Ok(self)
    }

    pub fn with_boot_image_memory(
        mut self,
        config: RiscvTopologyMemoryConfig,
        image: &BootImage,
    ) -> Result<Self, RiscvTopologySystemError> {
        let mut memory = config
            .build_store()
            .map_err(RiscvTopologySystemError::Memory)?;
        image
            .load_into_partitioned_store(&mut memory, config.target())
            .map_err(RiscvTopologySystemError::Boot)?;
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

    pub fn host_controller(&self) -> Option<Arc<Mutex<SystemHostController>>> {
        self.host.as_ref().map(|host| Arc::clone(&host.controller))
    }

    pub fn host_driver(&self) -> Option<&RiscvSystemRunDriver> {
        self.host.as_ref().map(|host| &host.driver)
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

    pub fn drive_attached_until_host_stop_parallel<E>(
        &mut self,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        max_turns: usize,
        event_for: E,
    ) -> Result<RiscvSystemRun, RiscvTopologySystemError>
    where
        E: FnMut(CpuId) -> GuestEventId,
    {
        let driver = self
            .host
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingHostController)?
            .driver
            .clone();
        self.drive_until_host_stop_parallel(&driver, fetch_trace, data_trace, max_turns, event_for)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvTopologySystemError {
    Scheduler(SchedulerError),
    CpuTopology(CpuTopologyError),
    PlatformPartitionMismatch { topology: u32, platform: u32 },
    HostPartitionOutOfRange { host: PartitionId, partitions: u32 },
    MissingMemoryStore,
    MissingHostController,
    Memory(MemoryError),
    Boot(BootError),
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
            Self::HostPartitionOutOfRange { host, partitions } => write!(
                formatter,
                "host partition {} is outside topology partition count {partitions}",
                host.index()
            ),
            Self::MissingMemoryStore => write!(formatter, "topology system has no memory store"),
            Self::MissingHostController => {
                write!(formatter, "topology system has no host controller")
            }
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Boot(error) => write!(formatter, "{error}"),
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
            Self::HostPartitionOutOfRange { .. } => None,
            Self::MissingMemoryStore => None,
            Self::MissingHostController => None,
            Self::Memory(error) => Some(error),
            Self::Boot(error) => Some(error),
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
