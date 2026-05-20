use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::{BootError, BootImage};
use rem6_cpu::{CpuId, CpuTopologyError, RiscvCluster, RiscvClusterTopologyConfig};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryController, DramMemoryError, DramTiming,
};
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
    memory: Option<RiscvTopologyMemoryBackend>,
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

#[derive(Clone, Debug)]
enum RiscvTopologyMemoryBackend {
    Store(Arc<Mutex<PartitionedMemoryStore>>),
    Dram(Arc<Mutex<DramMemoryController>>),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyDramConfig {
    targets: Vec<RiscvTopologyDramTargetConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvTopologyDramTargetConfig {
    memory: RiscvTopologyMemoryConfig,
    geometry: DramGeometry,
    timing: DramTiming,
}

impl RiscvTopologyDramTargetConfig {
    fn new(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Self {
        Self {
            memory: RiscvTopologyMemoryConfig::new(target, line_layout),
            geometry,
            timing,
        }
    }

    const fn target(&self) -> MemoryTargetId {
        self.memory.target()
    }

    const fn line_layout(&self) -> CacheLineLayout {
        self.memory.line_layout()
    }

    const fn geometry(&self) -> DramGeometry {
        self.geometry
    }

    const fn timing(&self) -> DramTiming {
        self.timing
    }

    fn regions(&self) -> &[RiscvTopologyMemoryRegion] {
        self.memory.regions()
    }
}

impl RiscvTopologyDramConfig {
    pub fn new(
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Self {
        Self {
            targets: vec![RiscvTopologyDramTargetConfig::new(
                target,
                line_layout,
                geometry,
                timing,
            )],
        }
    }

    pub fn add_region(mut self, start: Address, size: AccessSize) -> Self {
        self.targets[0].memory = self.targets[0].memory.clone().add_region(start, size);
        self
    }

    pub fn add_target(
        mut self,
        target: MemoryTargetId,
        line_layout: CacheLineLayout,
        geometry: DramGeometry,
        timing: DramTiming,
    ) -> Result<Self, MemoryError> {
        if self.targets.iter().any(|config| config.target() == target) {
            return Err(MemoryError::DuplicateMemoryTarget { target });
        }

        self.targets.push(RiscvTopologyDramTargetConfig::new(
            target,
            line_layout,
            geometry,
            timing,
        ));
        Ok(self)
    }

    pub fn add_region_for_target(
        mut self,
        target: MemoryTargetId,
        start: Address,
        size: AccessSize,
    ) -> Result<Self, MemoryError> {
        let Some(config) = self
            .targets
            .iter_mut()
            .find(|config| config.target() == target)
        else {
            return Err(MemoryError::UnknownMemoryTarget { target });
        };
        config.memory = config.memory.clone().add_region(start, size);
        Ok(self)
    }

    pub fn target(&self) -> MemoryTargetId {
        self.targets[0].target()
    }

    pub fn line_layout(&self) -> CacheLineLayout {
        self.targets[0].line_layout()
    }

    pub fn geometry(&self) -> DramGeometry {
        self.targets[0].geometry()
    }

    pub fn timing(&self) -> DramTiming {
        self.targets[0].timing()
    }

    pub fn regions(&self) -> &[RiscvTopologyMemoryRegion] {
        self.targets[0].regions()
    }

    fn build_staging_store(&self) -> Result<PartitionedMemoryStore, MemoryError> {
        let mut store = PartitionedMemoryStore::new();
        for target in &self.targets {
            store.add_partition(target.target(), target.line_layout())?;
        }
        for target in &self.targets {
            for region in target.regions() {
                store.map_region(target.target(), region.start(), region.size())?;
            }
        }
        Ok(store)
    }

    fn build_controller(&self) -> Result<DramMemoryController, DramMemoryError> {
        let mut controller = DramMemoryController::new();
        for target in &self.targets {
            controller.add_target(DramControllerConfig::new(
                target.target(),
                target.line_layout(),
                target.geometry(),
                target.timing(),
            ))?;
        }
        for target in &self.targets {
            for region in target.regions() {
                controller.map_region(target.target(), region.start(), region.size())?;
            }
        }
        Ok(controller)
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
        self.memory = Some(RiscvTopologyMemoryBackend::Store(Arc::new(Mutex::new(
            memory,
        ))));
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
        self.memory = Some(RiscvTopologyMemoryBackend::Store(Arc::new(Mutex::new(
            memory,
        ))));
        Ok(self)
    }

    pub fn with_boot_image_dram_memory(
        mut self,
        config: RiscvTopologyDramConfig,
        image: &BootImage,
    ) -> Result<Self, RiscvTopologySystemError> {
        let mut staging = config
            .build_staging_store()
            .map_err(RiscvTopologySystemError::Memory)?;
        image
            .load_into_partitioned_store_by_address(&mut staging)
            .map_err(RiscvTopologySystemError::Boot)?;

        let mut controller = config
            .build_controller()
            .map_err(RiscvTopologySystemError::Dram)?;
        for partition in staging.snapshot().partitions() {
            for line in partition.lines() {
                controller
                    .insert_line(partition.target(), line.line(), line.data().to_vec())
                    .map_err(RiscvTopologySystemError::Dram)?;
            }
        }

        self.memory = Some(RiscvTopologyMemoryBackend::Dram(Arc::new(Mutex::new(
            controller,
        ))));
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
        match self.memory.as_ref()? {
            RiscvTopologyMemoryBackend::Store(memory) => Some(memory),
            RiscvTopologyMemoryBackend::Dram(_) => None,
        }
    }

    pub fn dram_memory_controller(&self) -> Option<&Arc<Mutex<DramMemoryController>>> {
        match self.memory.as_ref()? {
            RiscvTopologyMemoryBackend::Store(_) => None,
            RiscvTopologyMemoryBackend::Dram(memory) => Some(memory),
        }
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

        let fetch_memory = memory.clone();
        let fetch_error = Arc::clone(&memory_error);
        let fetch_responder = move |_cpu| {
            let memory = fetch_memory.clone();
            let memory_error = Arc::clone(&fetch_error);
            move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                topology_memory_response(&memory, &memory_error, &delivery)
            }
        };

        let data_memory = memory.clone();
        let data_error = Arc::clone(&memory_error);
        let data_responder = move |_cpu| {
            let memory = data_memory.clone();
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
                    return Err(memory_error);
                }
                return Err(RiscvTopologySystemError::System(error));
            }
        };
        if let Some(memory_error) = take_memory_error(&memory_error) {
            return Err(memory_error);
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
    Dram(DramMemoryError),
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
            Self::Dram(error) => write!(formatter, "{error}"),
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
            Self::Dram(error) => Some(error),
            Self::Boot(error) => Some(error),
            Self::System(error) => Some(error),
        }
    }
}

fn topology_memory_response(
    memory: &RiscvTopologyMemoryBackend,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    match memory {
        RiscvTopologyMemoryBackend::Store(memory) => match memory
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
                record_memory_error(memory_error, RiscvTopologySystemError::Memory(error));
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            }
        },
        RiscvTopologyMemoryBackend::Dram(memory) => match memory
            .lock()
            .expect("topology DRAM memory lock")
            .accept(delivery.tick(), delivery.request())
        {
            Ok(outcome) => {
                let Some(response) = outcome.response().cloned() else {
                    return TargetOutcome::NoResponse;
                };
                let delay = outcome
                    .ready_cycle()
                    .checked_sub(delivery.tick())
                    .expect("DRAM response is not ready before request arrival");
                if delay == 0 {
                    TargetOutcome::Respond(response)
                } else {
                    TargetOutcome::RespondAfter { delay, response }
                }
            }
            Err(error) => {
                record_memory_error(memory_error, RiscvTopologySystemError::Dram(error));
                TargetOutcome::Respond(MemoryResponse::retry(delivery.request()))
            }
        },
    }
}

fn record_memory_error(
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    error: RiscvTopologySystemError,
) {
    let mut guard = memory_error.lock().expect("topology memory error lock");
    if guard.is_none() {
        *guard = Some(error);
    }
}

fn take_memory_error(
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
) -> Option<RiscvTopologySystemError> {
    memory_error
        .lock()
        .expect("topology memory error lock")
        .take()
}
