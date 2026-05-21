use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_accelerator::{
    AcceleratorCommand, AcceleratorDmaCopy, AcceleratorEngineId, AcceleratorError,
    AcceleratorTopologyConfig, AcceleratorTopologyDevice,
};
use rem6_boot::{BootError, BootImage};
use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, CpuTopologyError, RiscvCluster, RiscvClusterTopologyConfig};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryController, DramMemoryError, DramTiming,
    ExternalMemoryProfile,
};
use rem6_fabric::FabricModel;
use rem6_gpu::{GpuDeviceId, GpuError, GpuKernelLaunch, GpuTopologyConfig, GpuTopologyDevice};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError,
    Tick,
};
use rem6_memory::{
    AccessSize, Address, CacheLineLayout, MemoryError, MemoryResponse, MemoryTargetId,
    PartitionedMemoryStore,
};
use rem6_mmio::MmioBus;
use rem6_platform::Platform;
use rem6_stats::StatsRegistry;
use rem6_timer::TimerId;
use rem6_topology::Topology;
use rem6_transport::{MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome};
use rem6_uart::UartId;

use crate::{
    DramMemoryCheckpointBank, DramMemoryCheckpointPort, GuestEventId, GuestSourceId,
    HostEventPolicy, InterruptControllerCheckpointBank, InterruptControllerCheckpointPort,
    MemoryStoreCheckpointBank, MemoryStoreCheckpointPort, RiscvCoreCheckpointBank,
    RiscvCoreCheckpointPort, RiscvSystemRun, RiscvSystemRunDriver, RiscvTrapEventPort, SystemError,
    SystemHostController, SystemHostEventPort, TimerCheckpointBank, TimerCheckpointPort,
    UartCheckpointBank, UartCheckpointPort,
};

pub struct RiscvTopologySystem {
    topology: Topology,
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    cluster: RiscvCluster,
    accelerators: BTreeMap<AcceleratorEngineId, AcceleratorTopologyDevice>,
    gpus: BTreeMap<GpuDeviceId, GpuTopologyDevice>,
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

fn default_memory_checkpoint_component(target: MemoryTargetId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("memory{}", target.get()))
        .expect("formatted memory checkpoint component is nonempty")
}

fn default_dram_checkpoint_component(target: MemoryTargetId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("dram{}", target.get()))
        .expect("formatted DRAM checkpoint component is nonempty")
}

fn default_riscv_checkpoint_component(cpu: CpuId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("cpu{}", cpu.get()))
        .expect("formatted CPU checkpoint component is nonempty")
}

fn default_timer_checkpoint_component(timer: TimerId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("timer{}", timer.get()))
        .expect("formatted timer checkpoint component is nonempty")
}

fn default_uart_checkpoint_component(uart: UartId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("uart{}", uart.get()))
        .expect("formatted UART checkpoint component is nonempty")
}

fn default_interrupt_checkpoint_component() -> CheckpointComponentId {
    CheckpointComponentId::new("interrupt0")
        .expect("static interrupt checkpoint component is nonempty")
}

#[derive(Clone, Debug)]
enum RiscvTopologyMemoryBackend {
    Store {
        component: CheckpointComponentId,
        memory: Arc<Mutex<PartitionedMemoryStore>>,
    },
    Dram {
        component: CheckpointComponentId,
        memory: Arc<Mutex<DramMemoryController>>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyMemoryConfig {
    target: MemoryTargetId,
    line_layout: CacheLineLayout,
    regions: Vec<RiscvTopologyMemoryRegion>,
    checkpoint_component: CheckpointComponentId,
}

impl RiscvTopologyMemoryConfig {
    pub fn new(target: MemoryTargetId, line_layout: CacheLineLayout) -> Self {
        Self {
            target,
            line_layout,
            regions: Vec::new(),
            checkpoint_component: default_memory_checkpoint_component(target),
        }
    }

    pub fn add_region(mut self, start: Address, size: AccessSize) -> Self {
        self.regions
            .push(RiscvTopologyMemoryRegion::new(start, size));
        self
    }

    pub fn with_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.checkpoint_component = component;
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

    pub fn checkpoint_component(&self) -> &CheckpointComponentId {
        &self.checkpoint_component
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
    checkpoint_component: CheckpointComponentId,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvTopologyDramTargetConfig {
    memory: RiscvTopologyMemoryConfig,
    geometry: DramGeometry,
    timing: DramTiming,
    profile: Option<ExternalMemoryProfile>,
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
            profile: None,
        }
    }

    fn from_profile(profile: ExternalMemoryProfile) -> Self {
        Self {
            memory: RiscvTopologyMemoryConfig::new(profile.target(), profile.line_layout()),
            geometry: profile.geometry(),
            timing: profile.timing(),
            profile: Some(profile),
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

    const fn profile(&self) -> Option<ExternalMemoryProfile> {
        self.profile
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
            checkpoint_component: default_dram_checkpoint_component(target),
        }
    }

    pub fn from_profile(profile: ExternalMemoryProfile) -> Self {
        Self {
            targets: vec![RiscvTopologyDramTargetConfig::from_profile(profile)],
            checkpoint_component: default_dram_checkpoint_component(profile.target()),
        }
    }

    pub fn add_region(mut self, start: Address, size: AccessSize) -> Self {
        self.targets[0].memory = self.targets[0].memory.clone().add_region(start, size);
        self
    }

    pub fn with_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.checkpoint_component = component;
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

    pub fn add_profile_target(
        mut self,
        profile: ExternalMemoryProfile,
    ) -> Result<Self, MemoryError> {
        if self
            .targets
            .iter()
            .any(|config| config.target() == profile.target())
        {
            return Err(MemoryError::DuplicateMemoryTarget {
                target: profile.target(),
            });
        }

        self.targets
            .push(RiscvTopologyDramTargetConfig::from_profile(profile));
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

    pub fn checkpoint_component(&self) -> &CheckpointComponentId {
        &self.checkpoint_component
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
            if let Some(profile) = target.profile() {
                controller.add_profile(profile)?;
            } else {
                controller.add_target(DramControllerConfig::new(
                    target.target(),
                    target.line_layout(),
                    target.geometry(),
                    target.timing(),
                ))?;
            }
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
        let mut transport = MemoryTransport::with_fabric(FabricModel::new());
        let cluster = RiscvCluster::from_topology(&topology, &mut transport, cluster_config)
            .map_err(RiscvTopologySystemError::CpuTopology)?;

        Ok(Self {
            topology,
            scheduler,
            transport,
            cluster,
            accelerators: BTreeMap::new(),
            gpus: BTreeMap::new(),
            platform: None,
            memory: None,
            host: None,
        })
    }

    pub fn with_accelerator(
        mut self,
        config: AcceleratorTopologyConfig,
    ) -> Result<Self, RiscvTopologySystemError> {
        let engine = config.engine().id();
        if self.accelerators.contains_key(&engine) {
            return Err(RiscvTopologySystemError::DuplicateAccelerator { engine });
        }

        let device =
            AcceleratorTopologyDevice::from_topology(&self.topology, &mut self.transport, config)
                .map_err(RiscvTopologySystemError::Accelerator)?;
        self.accelerators.insert(engine, device);
        Ok(self)
    }

    pub fn with_gpu(mut self, config: GpuTopologyConfig) -> Result<Self, RiscvTopologySystemError> {
        let device_id = config.compute().device();
        if self.gpus.contains_key(&device_id) {
            return Err(RiscvTopologySystemError::DuplicateGpu { device: device_id });
        }

        let device = GpuTopologyDevice::from_topology(&self.topology, config)
            .map_err(RiscvTopologySystemError::Gpu)?;
        self.gpus.insert(device_id, device);
        Ok(self)
    }

    pub fn with_platform(mut self, platform: Platform) -> Result<Self, RiscvTopologySystemError> {
        if platform.partition_count() != self.topology.partition_count() {
            return Err(RiscvTopologySystemError::PlatformPartitionMismatch {
                topology: self.topology.partition_count(),
                platform: platform.partition_count(),
            });
        }

        self.platform = Some(platform);
        self.attach_platform_checkpoint_to_host()?;
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
        self.attach_riscv_checkpoint_to_host()?;
        self.attach_memory_checkpoint_to_host()?;
        self.attach_platform_checkpoint_to_host()?;
        Ok(self)
    }

    pub fn with_memory_store(
        mut self,
        memory: PartitionedMemoryStore,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.memory = Some(RiscvTopologyMemoryBackend::Store {
            component: default_memory_checkpoint_component(MemoryTargetId::new(0)),
            memory: Arc::new(Mutex::new(memory)),
        });
        self.attach_memory_checkpoint_to_host()?;
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
        self.memory = Some(RiscvTopologyMemoryBackend::Store {
            component: config.checkpoint_component().clone(),
            memory: Arc::new(Mutex::new(memory)),
        });
        self.attach_memory_checkpoint_to_host()?;
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

        self.memory = Some(RiscvTopologyMemoryBackend::Dram {
            component: config.checkpoint_component().clone(),
            memory: Arc::new(Mutex::new(controller)),
        });
        self.attach_memory_checkpoint_to_host()?;
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

    pub fn accelerator(&self, engine: AcceleratorEngineId) -> Option<&AcceleratorTopologyDevice> {
        self.accelerators.get(&engine)
    }

    pub fn accelerators(
        &self,
    ) -> impl Iterator<Item = (AcceleratorEngineId, &AcceleratorTopologyDevice)> {
        self.accelerators
            .iter()
            .map(|(engine, device)| (*engine, device))
    }

    pub fn gpu(&self, device: GpuDeviceId) -> Option<&GpuTopologyDevice> {
        self.gpus.get(&device)
    }

    pub fn gpus(&self) -> impl Iterator<Item = (GpuDeviceId, &GpuTopologyDevice)> {
        self.gpus.iter().map(|(device, gpu)| (*device, gpu))
    }

    pub const fn platform(&self) -> Option<&Platform> {
        self.platform.as_ref()
    }

    pub fn platform_bus(&self) -> Option<&MmioBus> {
        self.platform.as_ref().map(Platform::mmio_bus)
    }

    pub fn memory_store(&self) -> Option<&Arc<Mutex<PartitionedMemoryStore>>> {
        match self.memory.as_ref()? {
            RiscvTopologyMemoryBackend::Store { memory, .. } => Some(memory),
            RiscvTopologyMemoryBackend::Dram { .. } => None,
        }
    }

    pub fn dram_memory_controller(&self) -> Option<&Arc<Mutex<DramMemoryController>>> {
        match self.memory.as_ref()? {
            RiscvTopologyMemoryBackend::Store { .. } => None,
            RiscvTopologyMemoryBackend::Dram { memory, .. } => Some(memory),
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

    pub fn run_accelerator_dma_copy_parallel(
        &mut self,
        engine: AcceleratorEngineId,
        copy: AcceleratorDmaCopy,
        trace: MemoryTrace,
    ) -> Result<(), RiscvTopologySystemError> {
        let accelerator = self
            .accelerators
            .get(&engine)
            .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine })?
            .engine()
            .clone();
        let memory = self
            .memory
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingMemoryStore)?
            .clone();
        let memory_error = Arc::new(Mutex::new(None));

        let read_memory = memory.clone();
        let read_error = Arc::clone(&memory_error);
        accelerator
            .submit_dma_copy_read(
                &mut self.scheduler,
                &self.transport,
                copy,
                trace.clone(),
                move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                    topology_memory_response(&read_memory, &read_error, &delivery)
                },
            )
            .map_err(RiscvTopologySystemError::Accelerator)?;
        self.scheduler
            .run_until_idle_parallel()
            .map_err(RiscvTopologySystemError::Scheduler)?;
        if let Some(error) = take_memory_error(&memory_error) {
            return Err(error);
        }

        let write_memory = memory.clone();
        let write_error = Arc::clone(&memory_error);
        let Some(_event) = accelerator
            .issue_next_dma_write(
                &mut self.scheduler,
                &self.transport,
                trace,
                move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                    topology_memory_response(&write_memory, &write_error, &delivery)
                },
            )
            .map_err(RiscvTopologySystemError::Accelerator)?
        else {
            return Err(RiscvTopologySystemError::AcceleratorDmaWriteNotReady { engine });
        };
        self.scheduler
            .run_until_idle_parallel()
            .map_err(RiscvTopologySystemError::Scheduler)?;
        if let Some(error) = take_memory_error(&memory_error) {
            return Err(error);
        }

        Ok(())
    }

    pub fn submit_accelerator_command_parallel(
        &mut self,
        engine: AcceleratorEngineId,
        command: AcceleratorCommand,
    ) -> Result<PartitionEventId, RiscvTopologySystemError> {
        let device = self
            .accelerators
            .get(&engine)
            .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine })?;
        device
            .submit_command(&mut self.scheduler, command)
            .map_err(RiscvTopologySystemError::Accelerator)
    }

    pub fn submit_gpu_kernel_parallel(
        &mut self,
        device: GpuDeviceId,
        launch: GpuKernelLaunch,
    ) -> Result<PartitionEventId, RiscvTopologySystemError> {
        let gpu = self
            .gpus
            .get(&device)
            .ok_or(RiscvTopologySystemError::UnknownGpu { device })?;
        gpu.submit_kernel(&mut self.scheduler, launch)
            .map_err(RiscvTopologySystemError::Gpu)
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

    fn attach_memory_checkpoint_to_host(&mut self) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let Some(memory) = self.memory.as_ref() else {
            return Ok(());
        };
        let mut host = host
            .controller
            .lock()
            .expect("topology host controller lock");
        match memory {
            RiscvTopologyMemoryBackend::Store { component, memory } => {
                let bank = MemoryStoreCheckpointBank::new([MemoryStoreCheckpointPort::new(
                    component.clone(),
                    Arc::clone(memory),
                )])
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
                host.executor_mut()
                    .attach_memory_checkpoint_bank(bank)
                    .map_err(SystemError::Checkpoint)
                    .map_err(RiscvTopologySystemError::System)?;
            }
            RiscvTopologyMemoryBackend::Dram { component, memory } => {
                let bank = DramMemoryCheckpointBank::new([DramMemoryCheckpointPort::new(
                    component.clone(),
                    Arc::clone(memory),
                )])
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
                host.executor_mut()
                    .attach_dram_memory_checkpoint_bank(bank)
                    .map_err(SystemError::Checkpoint)
                    .map_err(RiscvTopologySystemError::System)?;
            }
        }
        Ok(())
    }

    fn attach_riscv_checkpoint_to_host(&mut self) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let ports = self.cluster.core_ids().into_iter().map(|cpu| {
            RiscvCoreCheckpointPort::new(
                default_riscv_checkpoint_component(cpu),
                self.cluster
                    .core(cpu)
                    .expect("cluster core ids resolve to cores"),
            )
        });
        let bank = RiscvCoreCheckpointBank::new(ports)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_riscv_checkpoint_bank(bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        Ok(())
    }

    fn attach_platform_checkpoint_to_host(&mut self) -> Result<(), RiscvTopologySystemError> {
        let Some(host) = self.host.as_ref() else {
            return Ok(());
        };
        let Some(platform) = self.platform.as_ref() else {
            return Ok(());
        };
        let interrupt_bank =
            InterruptControllerCheckpointBank::new([InterruptControllerCheckpointPort::new(
                default_interrupt_checkpoint_component(),
                platform.interrupt_controller(),
            )])
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_interrupt_controller_checkpoint_bank(interrupt_bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;

        let timer_bank = TimerCheckpointBank::new(platform.timers().map(|(timer, device)| {
            TimerCheckpointPort::new(default_timer_checkpoint_component(timer), device.clone())
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if timer_bank.component_count() != 0 {
            host.controller
                .lock()
                .expect("topology host controller lock")
                .executor_mut()
                .attach_timer_checkpoint_bank(timer_bank)
                .map_err(SystemError::Checkpoint)
                .map_err(RiscvTopologySystemError::System)?;
        }

        let bank = UartCheckpointBank::new(platform.uarts().map(|(uart, device)| {
            UartCheckpointPort::new(default_uart_checkpoint_component(uart), device.clone())
        }))
        .map_err(SystemError::Checkpoint)
        .map_err(RiscvTopologySystemError::System)?;
        if bank.component_count() == 0 {
            return Ok(());
        }
        host.controller
            .lock()
            .expect("topology host controller lock")
            .executor_mut()
            .attach_uart_checkpoint_bank(bank)
            .map_err(SystemError::Checkpoint)
            .map_err(RiscvTopologySystemError::System)?;
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvTopologySystemError {
    Scheduler(SchedulerError),
    CpuTopology(CpuTopologyError),
    DuplicateAccelerator { engine: AcceleratorEngineId },
    UnknownAccelerator { engine: AcceleratorEngineId },
    AcceleratorDmaWriteNotReady { engine: AcceleratorEngineId },
    DuplicateGpu { device: GpuDeviceId },
    UnknownGpu { device: GpuDeviceId },
    PlatformPartitionMismatch { topology: u32, platform: u32 },
    HostPartitionOutOfRange { host: PartitionId, partitions: u32 },
    MissingMemoryStore,
    MissingHostController,
    Accelerator(AcceleratorError),
    Gpu(GpuError),
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
            Self::DuplicateAccelerator { engine } => {
                write!(formatter, "accelerator engine {} is already attached", engine.get())
            }
            Self::UnknownAccelerator { engine } => {
                write!(formatter, "accelerator engine {} is not attached", engine.get())
            }
            Self::AcceleratorDmaWriteNotReady { engine } => write!(
                formatter,
                "accelerator engine {} has no completed DMA read to write",
                engine.get()
            ),
            Self::DuplicateGpu { device } => {
                write!(formatter, "GPU device {} is already attached", device.get())
            }
            Self::UnknownGpu { device } => {
                write!(formatter, "GPU device {} is not attached", device.get())
            }
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
            Self::Accelerator(error) => write!(formatter, "{error}"),
            Self::Gpu(error) => write!(formatter, "{error}"),
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
            Self::DuplicateAccelerator { .. } => None,
            Self::UnknownAccelerator { .. } => None,
            Self::AcceleratorDmaWriteNotReady { .. } => None,
            Self::DuplicateGpu { .. } => None,
            Self::UnknownGpu { .. } => None,
            Self::PlatformPartitionMismatch { .. } => None,
            Self::HostPartitionOutOfRange { .. } => None,
            Self::MissingMemoryStore => None,
            Self::MissingHostController => None,
            Self::Accelerator(error) => Some(error),
            Self::Gpu(error) => Some(error),
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
        RiscvTopologyMemoryBackend::Store { memory, .. } => match memory
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
        RiscvTopologyMemoryBackend::Dram { memory, .. } => match memory
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
