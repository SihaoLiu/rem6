mod heterogeneous_run;
mod host_checkpoint;

pub use heterogeneous_run::{RiscvTopologyHeterogeneousRunSummary, RiscvTopologyHeterogeneousWork};

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use rem6_accelerator::{
    AcceleratorCommand, AcceleratorDmaCopy, AcceleratorDmaIssueRecord, AcceleratorDmaWriteRollback,
    AcceleratorEngineId, AcceleratorEngineSnapshot, AcceleratorError, AcceleratorTopologyConfig,
    AcceleratorTopologyDevice,
};
use rem6_boot::{BootError, BootImage};
use rem6_checkpoint::CheckpointComponentId;
use rem6_cpu::{CpuId, CpuTopologyError, RiscvCluster, RiscvClusterTopologyConfig};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryController, DramMemoryError, DramTiming,
    ExternalMemoryProfile,
};
use rem6_fabric::FabricModel;
use rem6_gpu::{
    GpuDeviceId, GpuDeviceSnapshot, GpuDmaCopy, GpuDmaIssueRecord, GpuDmaWriteRollback, GpuError,
    GpuKernelLaunch, GpuTopologyConfig, GpuTopologyDevice,
};
use rem6_kernel::{
    ParallelRunProfile, ParallelSchedulerContext, PartitionEventId, PartitionId,
    PartitionedScheduler, RecordedConservativeRunSummary, SchedulerError, Tick,
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
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
    TransportError,
};
use rem6_uart::UartId;

use crate::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvTrapEventPort, SystemError, SystemHostController, SystemHostEventPort,
};

pub struct RiscvTopologySystem {
    topology: Topology,
    scheduler: Arc<Mutex<PartitionedScheduler>>,
    transport: MemoryTransport,
    cluster: RiscvCluster,
    accelerators: BTreeMap<AcceleratorEngineId, AcceleratorTopologyDevice>,
    gpus: BTreeMap<GpuDeviceId, GpuTopologyDevice>,
    platform: Option<Platform>,
    memory: Option<RiscvTopologyMemoryBackend>,
    host: Option<RiscvTopologyHost>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvTopologyDmaCopy {
    Accelerator {
        engine: AcceleratorEngineId,
        copy: AcceleratorDmaCopy,
    },
    Gpu {
        device: GpuDeviceId,
        copy: GpuDmaCopy,
    },
}

impl RiscvTopologyDmaCopy {
    pub const fn accelerator(engine: AcceleratorEngineId, copy: AcceleratorDmaCopy) -> Self {
        Self::Accelerator { engine, copy }
    }

    pub const fn gpu(device: GpuDeviceId, copy: GpuDmaCopy) -> Self {
        Self::Gpu { device, copy }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyDmaStageRunSummary {
    events: Vec<PartitionEventId>,
    scheduler_run: RecordedConservativeRunSummary,
    trace_event_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
}

impl RiscvTopologyDmaStageRunSummary {
    pub fn new(
        events: Vec<PartitionEventId>,
        scheduler_run: RecordedConservativeRunSummary,
        trace_event_count: usize,
        pending_dma_write_count: usize,
        dma_completion_count: usize,
    ) -> Self {
        Self {
            events,
            scheduler_run,
            trace_event_count,
            pending_dma_write_count,
            dma_completion_count,
        }
    }

    pub fn events(&self) -> &[PartitionEventId] {
        &self.events
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }

    pub const fn scheduler_run(&self) -> &RecordedConservativeRunSummary {
        &self.scheduler_run
    }

    pub fn profile(&self) -> ParallelRunProfile {
        self.scheduler_run.profile()
    }

    pub fn epoch_count(&self) -> usize {
        self.scheduler_run.epoch_count()
    }

    pub fn empty_epoch_count(&self) -> usize {
        self.scheduler_run.empty_epoch_count()
    }

    pub fn dispatch_count(&self) -> usize {
        self.scheduler_run.dispatch_count()
    }

    pub fn batch_count(&self) -> usize {
        self.scheduler_run.batch_count()
    }

    pub fn total_parallel_workers(&self) -> usize {
        self.scheduler_run.total_parallel_workers()
    }

    pub fn max_parallel_workers(&self) -> usize {
        self.scheduler_run.max_parallel_workers()
    }

    pub fn has_parallel_work(&self) -> bool {
        self.scheduler_run.has_parallel_work()
    }

    pub fn final_tick(&self) -> Tick {
        self.scheduler_run.summary().final_tick()
    }

    pub const fn trace_event_count(&self) -> usize {
        self.trace_event_count
    }

    pub const fn pending_dma_write_count(&self) -> usize {
        self.pending_dma_write_count
    }

    pub const fn dma_completion_count(&self) -> usize {
        self.dma_completion_count
    }

    pub const fn device_activity_count(&self) -> usize {
        self.trace_event_count + self.pending_dma_write_count + self.dma_completion_count
    }

    pub const fn has_dma_activity(&self) -> bool {
        self.device_activity_count() != 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyDmaRunSummary {
    read: RiscvTopologyDmaStageRunSummary,
    write: RiscvTopologyDmaStageRunSummary,
}

impl RiscvTopologyDmaRunSummary {
    pub const fn new(
        read: RiscvTopologyDmaStageRunSummary,
        write: RiscvTopologyDmaStageRunSummary,
    ) -> Self {
        Self { read, write }
    }

    pub const fn read(&self) -> &RiscvTopologyDmaStageRunSummary {
        &self.read
    }

    pub const fn write(&self) -> &RiscvTopologyDmaStageRunSummary {
        &self.write
    }

    pub fn profile(&self) -> ParallelRunProfile {
        self.read.profile().merge(self.write.profile())
    }

    pub fn event_count(&self) -> usize {
        self.read.event_count() + self.write.event_count()
    }

    pub const fn trace_event_count(&self) -> usize {
        self.read.trace_event_count() + self.write.trace_event_count()
    }

    pub const fn pending_dma_write_count(&self) -> usize {
        self.write.pending_dma_write_count()
    }

    pub const fn dma_completion_count(&self) -> usize {
        self.read.dma_completion_count() + self.write.dma_completion_count()
    }

    pub const fn device_activity_count(&self) -> usize {
        self.read.device_activity_count() + self.write.device_activity_count()
    }

    pub const fn has_dma_activity(&self) -> bool {
        self.device_activity_count() != 0
    }

    pub fn has_parallel_work(&self) -> bool {
        self.read.has_parallel_work() || self.write.has_parallel_work()
    }

    pub fn final_tick(&self) -> Tick {
        self.write.final_tick()
    }
}

#[derive(Clone, Debug)]
struct RiscvTopologyDmaDeviceSnapshots {
    accelerators: BTreeMap<AcceleratorEngineId, AcceleratorEngineSnapshot>,
    gpus: BTreeMap<GpuDeviceId, GpuDeviceSnapshot>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct RiscvTopologyDmaActivityCounts {
    trace_event_count: usize,
    pending_dma_write_count: usize,
    dma_completion_count: usize,
}

enum RiscvTopologyDmaIssueRecord {
    Accelerator(AcceleratorDmaIssueRecord),
    Gpu(GpuDmaIssueRecord),
}

impl RiscvTopologyDmaIssueRecord {
    fn record(self) {
        match self {
            Self::Accelerator(record) => record.record(),
            Self::Gpu(record) => record.record(),
        }
    }
}

enum RiscvTopologyDmaWriteRollback {
    Accelerator(AcceleratorDmaWriteRollback),
    Gpu(GpuDmaWriteRollback),
}

impl RiscvTopologyDmaWriteRollback {
    fn restore(self) {
        match self {
            Self::Accelerator(rollback) => rollback.restore(),
            Self::Gpu(rollback) => rollback.restore(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvTopologyHostConfig {
    host_partition: PartitionId,
    host_latency: Tick,
    source: GuestSourceId,
    scheduler_checkpoint_component: CheckpointComponentId,
    fabric_checkpoint_component: CheckpointComponentId,
}

impl RiscvTopologyHostConfig {
    pub fn new(host_partition: PartitionId, host_latency: Tick, source: GuestSourceId) -> Self {
        Self {
            host_partition,
            host_latency,
            source,
            scheduler_checkpoint_component: default_scheduler_checkpoint_component(),
            fabric_checkpoint_component: default_fabric_checkpoint_component(),
        }
    }

    pub fn with_scheduler_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.scheduler_checkpoint_component = component;
        self
    }

    pub fn with_fabric_checkpoint_component(mut self, component: CheckpointComponentId) -> Self {
        self.fabric_checkpoint_component = component;
        self
    }

    pub const fn host_partition(&self) -> PartitionId {
        self.host_partition
    }

    pub const fn host_latency(&self) -> Tick {
        self.host_latency
    }

    pub const fn source(&self) -> GuestSourceId {
        self.source
    }

    pub fn scheduler_checkpoint_component(&self) -> &CheckpointComponentId {
        &self.scheduler_checkpoint_component
    }

    pub fn fabric_checkpoint_component(&self) -> &CheckpointComponentId {
        &self.fabric_checkpoint_component
    }
}

#[derive(Clone, Debug)]
struct RiscvTopologyHost {
    controller: Arc<Mutex<SystemHostController>>,
    driver: RiscvSystemRunDriver,
    scheduler_checkpoint_component: CheckpointComponentId,
    fabric_checkpoint_component: CheckpointComponentId,
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

fn default_accelerator_checkpoint_component(engine: AcceleratorEngineId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("accelerator{}", engine.get()))
        .expect("formatted accelerator checkpoint component is nonempty")
}

fn default_gpu_checkpoint_component(device: GpuDeviceId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("gpu{}", device.get()))
        .expect("formatted GPU checkpoint component is nonempty")
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

fn default_scheduler_checkpoint_component() -> CheckpointComponentId {
    CheckpointComponentId::new("scheduler0")
        .expect("static scheduler checkpoint component is nonempty")
}

fn default_fabric_checkpoint_component() -> CheckpointComponentId {
    CheckpointComponentId::new("fabric0").expect("static fabric checkpoint component is nonempty")
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
        Self::with_parallel_worker_limit(topology, cluster_config, min_remote_delay, usize::MAX)
    }

    pub fn with_parallel_worker_limit(
        topology: Topology,
        cluster_config: RiscvClusterTopologyConfig,
        min_remote_delay: Tick,
        max_parallel_workers: usize,
    ) -> Result<Self, RiscvTopologySystemError> {
        let scheduler = PartitionedScheduler::with_parallel_worker_limit(
            topology.partition_count(),
            min_remote_delay,
            max_parallel_workers,
        )
        .map_err(RiscvTopologySystemError::Scheduler)?;
        let scheduler = Arc::new(Mutex::new(scheduler));
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
        self.attach_heterogeneous_checkpoint_to_host()?;
        Ok(self)
    }

    pub fn with_gpu(mut self, config: GpuTopologyConfig) -> Result<Self, RiscvTopologySystemError> {
        let device_id = config.compute().device();
        if self.gpus.contains_key(&device_id) {
            return Err(RiscvTopologySystemError::DuplicateGpu { device: device_id });
        }

        let device = GpuTopologyDevice::from_topology(&self.topology, &mut self.transport, config)
            .map_err(RiscvTopologySystemError::Gpu)?;
        self.gpus.insert(device_id, device);
        self.attach_heterogeneous_checkpoint_to_host()?;
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
            scheduler_checkpoint_component: config.scheduler_checkpoint_component().clone(),
            fabric_checkpoint_component: config.fabric_checkpoint_component().clone(),
        });
        self.attach_fabric_checkpoint_to_host()?;
        self.attach_scheduler_checkpoint_to_host()?;
        self.attach_riscv_checkpoint_to_host()?;
        self.attach_heterogeneous_checkpoint_to_host()?;
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

    pub fn scheduler(&self) -> MutexGuard<'_, PartitionedScheduler> {
        self.lock_scheduler()
    }

    pub fn scheduler_mut(&self) -> MutexGuard<'_, PartitionedScheduler> {
        self.lock_scheduler()
    }

    pub fn scheduler_handle(&self) -> Arc<Mutex<PartitionedScheduler>> {
        Arc::clone(&self.scheduler)
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

    fn lock_scheduler(&self) -> MutexGuard<'_, PartitionedScheduler> {
        self.scheduler.lock().expect("topology scheduler lock")
    }

    pub fn execution_parts_mut(
        &self,
    ) -> (
        &RiscvCluster,
        MutexGuard<'_, PartitionedScheduler>,
        &MemoryTransport,
    ) {
        (&self.cluster, self.lock_scheduler(), &self.transport)
    }

    pub fn execution_parts_with_mmio_mut(
        &self,
    ) -> Option<(
        &RiscvCluster,
        MutexGuard<'_, PartitionedScheduler>,
        &MemoryTransport,
        &MmioBus,
    )> {
        let platform = self.platform.as_ref()?;
        Some((
            &self.cluster,
            self.lock_scheduler(),
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
        self.run_accelerator_dma_copy_parallel_recorded(engine, copy, trace)
            .map(|_| ())
    }

    pub fn run_accelerator_dma_copy_parallel_recorded(
        &mut self,
        engine: AcceleratorEngineId,
        copy: AcceleratorDmaCopy,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaRunSummary, RiscvTopologySystemError> {
        self.run_dma_copies_parallel_recorded(
            [RiscvTopologyDmaCopy::accelerator(engine, copy)],
            trace,
        )
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
        let mut scheduler = self.lock_scheduler();
        device
            .submit_command(&mut scheduler, command)
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
        let mut scheduler = self.lock_scheduler();
        gpu.submit_kernel(&mut scheduler, launch)
            .map_err(RiscvTopologySystemError::Gpu)
    }

    pub fn run_gpu_dma_copy_parallel(
        &mut self,
        device: GpuDeviceId,
        copy: GpuDmaCopy,
        trace: MemoryTrace,
    ) -> Result<(), RiscvTopologySystemError> {
        self.run_gpu_dma_copy_parallel_recorded(device, copy, trace)
            .map(|_| ())
    }

    pub fn run_gpu_dma_copy_parallel_recorded(
        &mut self,
        device: GpuDeviceId,
        copy: GpuDmaCopy,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaRunSummary, RiscvTopologySystemError> {
        self.run_dma_copies_parallel_recorded([RiscvTopologyDmaCopy::gpu(device, copy)], trace)
    }

    pub fn run_dma_copy_reads_parallel<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<Vec<PartitionEventId>, RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        if copies.is_empty() {
            return Ok(Vec::new());
        }

        self.run_dma_copy_reads_parallel_recorded(copies, trace)
            .map(|summary| summary.events().to_vec())
    }

    pub fn run_dma_copy_reads_parallel_recorded<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaStageRunSummary, RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        if copies.is_empty() {
            return Ok(self.empty_dma_stage_run_summary());
        }

        let memory = self
            .memory
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingMemoryStore)?
            .clone();
        let memory_error = Arc::new(Mutex::new(None));
        let before = self.dma_device_snapshots(&copies)?;
        let mut scheduler = self.lock_scheduler();
        let issued_at = scheduler.now();
        let mut issue_records = Vec::new();
        let mut transactions = Vec::<ParallelMemoryTransaction>::new();

        for copy in copies {
            match copy {
                RiscvTopologyDmaCopy::Accelerator { engine, copy } => {
                    let accelerator = self
                        .accelerators
                        .get(&engine)
                        .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine })?
                        .engine()
                        .clone();
                    let read_memory = memory.clone();
                    let read_error = Arc::clone(&memory_error);
                    let prepared = accelerator.prepare_dma_copy_read(
                        issued_at,
                        copy,
                        trace.clone(),
                        move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                            topology_memory_response(&read_memory, &read_error, &delivery)
                        },
                    );
                    let (issue, transaction) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Accelerator(issue));
                    transactions.push(transaction);
                }
                RiscvTopologyDmaCopy::Gpu { device, copy } => {
                    let gpu = self
                        .gpus
                        .get(&device)
                        .ok_or(RiscvTopologySystemError::UnknownGpu { device })?
                        .gpu()
                        .clone();
                    let read_memory = memory.clone();
                    let read_error = Arc::clone(&memory_error);
                    let prepared = gpu.prepare_dma_copy_read(
                        issued_at,
                        copy,
                        trace.clone(),
                        move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                            topology_memory_response(&read_memory, &read_error, &delivery)
                        },
                    );
                    let (issue, transaction) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Gpu(issue));
                    transactions.push(transaction);
                }
            }
        }

        let events = self
            .transport
            .submit_parallel_batch(&mut scheduler, transactions)
            .map_err(RiscvTopologySystemError::Transport)?;
        for issue in issue_records {
            issue.record();
        }
        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(RiscvTopologySystemError::Scheduler)?;
        drop(scheduler);
        if let Some(error) = take_memory_error(&memory_error) {
            return Err(error);
        }
        let activity = self.dma_activity_since(&before)?;

        Ok(RiscvTopologyDmaStageRunSummary::new(
            events,
            scheduler_run,
            activity.trace_event_count,
            activity.pending_dma_write_count,
            activity.dma_completion_count,
        ))
    }

    pub fn run_dma_copies_parallel<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<(), RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        if copies.is_empty() {
            return Ok(());
        }

        self.run_dma_copies_parallel_recorded(copies, trace)
            .map(|_| ())
    }

    pub fn run_dma_copies_parallel_recorded<I>(
        &mut self,
        copies: I,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaRunSummary, RiscvTopologySystemError>
    where
        I: IntoIterator<Item = RiscvTopologyDmaCopy>,
    {
        let copies: Vec<_> = copies.into_iter().collect();
        let read = self.run_dma_copy_reads_parallel_recorded(copies.clone(), trace.clone())?;
        let write = self.run_dma_copy_writes_parallel_recorded(copies, trace)?;
        Ok(RiscvTopologyDmaRunSummary::new(read, write))
    }

    fn run_dma_copy_writes_parallel_recorded(
        &mut self,
        copies: Vec<RiscvTopologyDmaCopy>,
        trace: MemoryTrace,
    ) -> Result<RiscvTopologyDmaStageRunSummary, RiscvTopologySystemError> {
        if copies.is_empty() {
            return Ok(self.empty_dma_stage_run_summary());
        }

        let memory = self
            .memory
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingMemoryStore)?
            .clone();
        let memory_error = Arc::new(Mutex::new(None));
        let before = self.dma_device_snapshots(&copies)?;
        let mut scheduler = self.lock_scheduler();
        let issued_at = scheduler.now();
        let mut issue_records = Vec::new();
        let mut rollbacks = Vec::new();
        let mut transactions = Vec::<ParallelMemoryTransaction>::new();

        for copy in copies {
            match copy {
                RiscvTopologyDmaCopy::Accelerator { engine, .. } => {
                    let accelerator = self
                        .accelerators
                        .get(&engine)
                        .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine })?
                        .engine()
                        .clone();
                    let write_memory = memory.clone();
                    let write_error = Arc::clone(&memory_error);
                    let Some(prepared) = accelerator
                        .prepare_next_dma_write(
                            issued_at,
                            trace.clone(),
                            move |delivery, _context| {
                                topology_memory_response(&write_memory, &write_error, &delivery)
                            },
                        )
                        .map_err(RiscvTopologySystemError::Accelerator)?
                    else {
                        return Err(RiscvTopologySystemError::AcceleratorDmaWriteNotReady {
                            engine,
                        });
                    };
                    let (issue, transaction, rollback) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Accelerator(issue));
                    rollbacks.push(RiscvTopologyDmaWriteRollback::Accelerator(rollback));
                    transactions.push(transaction);
                }
                RiscvTopologyDmaCopy::Gpu { device, .. } => {
                    let gpu = self
                        .gpus
                        .get(&device)
                        .ok_or(RiscvTopologySystemError::UnknownGpu { device })?
                        .gpu()
                        .clone();
                    let write_memory = memory.clone();
                    let write_error = Arc::clone(&memory_error);
                    let Some(prepared) = gpu
                        .prepare_next_dma_write(
                            issued_at,
                            trace.clone(),
                            move |delivery, _context| {
                                topology_memory_response(&write_memory, &write_error, &delivery)
                            },
                        )
                        .map_err(RiscvTopologySystemError::Gpu)?
                    else {
                        return Err(RiscvTopologySystemError::GpuDmaWriteNotReady { device });
                    };
                    let (issue, transaction, rollback) = prepared.into_parts();
                    issue_records.push(RiscvTopologyDmaIssueRecord::Gpu(issue));
                    rollbacks.push(RiscvTopologyDmaWriteRollback::Gpu(rollback));
                    transactions.push(transaction);
                }
            }
        }

        let events = match self
            .transport
            .submit_parallel_batch(&mut scheduler, transactions)
        {
            Ok(events) => events,
            Err(error) => {
                for rollback in rollbacks {
                    rollback.restore();
                }
                return Err(RiscvTopologySystemError::Transport(error));
            }
        };
        for issue in issue_records {
            issue.record();
        }
        let scheduler_run = scheduler
            .run_until_idle_parallel_recorded()
            .map_err(RiscvTopologySystemError::Scheduler)?;
        drop(scheduler);
        if let Some(error) = take_memory_error(&memory_error) {
            return Err(error);
        }
        let activity = self.dma_activity_since(&before)?;

        Ok(RiscvTopologyDmaStageRunSummary::new(
            events,
            scheduler_run,
            activity.trace_event_count,
            activity.pending_dma_write_count,
            activity.dma_completion_count,
        ))
    }

    fn dma_device_snapshots(
        &self,
        copies: &[RiscvTopologyDmaCopy],
    ) -> Result<RiscvTopologyDmaDeviceSnapshots, RiscvTopologySystemError> {
        let mut accelerators = BTreeMap::new();
        let mut gpus = BTreeMap::new();

        for copy in copies {
            match copy {
                RiscvTopologyDmaCopy::Accelerator { engine, .. } => {
                    if !accelerators.contains_key(engine) {
                        let snapshot = self
                            .accelerators
                            .get(engine)
                            .ok_or(RiscvTopologySystemError::UnknownAccelerator {
                                engine: *engine,
                            })?
                            .engine()
                            .snapshot();
                        accelerators.insert(*engine, snapshot);
                    }
                }
                RiscvTopologyDmaCopy::Gpu { device, .. } => {
                    if !gpus.contains_key(device) {
                        let snapshot = self
                            .gpus
                            .get(device)
                            .ok_or(RiscvTopologySystemError::UnknownGpu { device: *device })?
                            .gpu()
                            .snapshot();
                        gpus.insert(*device, snapshot);
                    }
                }
            }
        }

        Ok(RiscvTopologyDmaDeviceSnapshots { accelerators, gpus })
    }

    fn empty_dma_stage_run_summary(&self) -> RiscvTopologyDmaStageRunSummary {
        let final_tick = self.scheduler().now();
        RiscvTopologyDmaStageRunSummary::new(
            Vec::new(),
            RecordedConservativeRunSummary::empty(final_tick),
            0,
            0,
            0,
        )
    }

    fn dma_activity_since(
        &self,
        before: &RiscvTopologyDmaDeviceSnapshots,
    ) -> Result<RiscvTopologyDmaActivityCounts, RiscvTopologySystemError> {
        let mut activity = RiscvTopologyDmaActivityCounts::default();

        for (engine, before) in &before.accelerators {
            let after = self
                .accelerators
                .get(engine)
                .ok_or(RiscvTopologySystemError::UnknownAccelerator { engine: *engine })?
                .engine()
                .snapshot();
            activity.trace_event_count += after.trace().len().saturating_sub(before.trace().len());
            activity.pending_dma_write_count += after.pending_dma_writes().len();
            activity.dma_completion_count += after
                .dma_completions()
                .len()
                .saturating_sub(before.dma_completions().len());
        }

        for (device, before) in &before.gpus {
            let after = self
                .gpus
                .get(device)
                .ok_or(RiscvTopologySystemError::UnknownGpu { device: *device })?
                .gpu()
                .snapshot();
            activity.trace_event_count += after.trace().len().saturating_sub(before.trace().len());
            activity.pending_dma_write_count += after.pending_dma_writes().len();
            activity.dma_completion_count += after
                .dma_completions()
                .len()
                .saturating_sub(before.dma_completions().len());
        }

        Ok(activity)
    }

    pub fn drive_until_host_stop_parallel<E>(
        &self,
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

        let mut scheduler = self.lock_scheduler();
        let result = if let Some(platform) = self.platform.as_ref() {
            driver.drive_until_host_stop_parallel_with_mmio(
                &self.cluster,
                &mut scheduler,
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
                &mut scheduler,
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
        &self,
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
    DuplicateAccelerator { engine: AcceleratorEngineId },
    UnknownAccelerator { engine: AcceleratorEngineId },
    AcceleratorDmaWriteNotReady { engine: AcceleratorEngineId },
    DuplicateGpu { device: GpuDeviceId },
    UnknownGpu { device: GpuDeviceId },
    GpuDmaWriteNotReady { device: GpuDeviceId },
    PlatformPartitionMismatch { topology: u32, platform: u32 },
    HostPartitionOutOfRange { host: PartitionId, partitions: u32 },
    MissingMemoryStore,
    MissingHostController,
    Accelerator(AcceleratorError),
    Gpu(GpuError),
    Transport(TransportError),
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
            Self::GpuDmaWriteNotReady { device } => {
                write!(
                    formatter,
                    "GPU device {} has no completed DMA read to write",
                    device.get()
                )
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
            Self::Transport(error) => write!(formatter, "{error}"),
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
            Self::GpuDmaWriteNotReady { .. } => None,
            Self::PlatformPartitionMismatch { .. } => None,
            Self::HostPartitionOutOfRange { .. } => None,
            Self::MissingMemoryStore => None,
            Self::MissingHostController => None,
            Self::Accelerator(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::Transport(error) => Some(error),
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
