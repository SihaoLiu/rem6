mod boot_handoff;
mod cache_attach;
mod coherence_data;
mod data_cache_history;
mod dma_ops;
mod dma_run;
mod heterogeneous_run;
mod host_checkpoint;
mod instruction_cache;
mod net_checkpoint;
mod pci_checkpoint;
mod sinic_pci_device;
mod storage_checkpoint;
mod virtio_checkpoint;

pub use boot_handoff::{
    RiscvDtbHandoffReport, RiscvLinuxBootHandoffConfig, RiscvLinuxBootHandoffReport,
    RiscvLinuxInitrdImage,
};
pub use dma_run::{
    RiscvTopologyDmaCopy, RiscvTopologyDmaDeviceActivity, RiscvTopologyDmaRunSummary,
    RiscvTopologyDmaStageRunSummary,
};
pub use heterogeneous_run::{
    RiscvTopologyAcceleratorComputeActivity, RiscvTopologyGpuComputeActivity,
    RiscvTopologyHeterogeneousRunSummary, RiscvTopologyHeterogeneousWork,
};
pub use sinic_pci_device::{RiscvTopologySinicPciDeviceConfig, RiscvTopologyWorkloadSinicPciError};

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};

use rem6_accelerator::{
    AcceleratorCommand, AcceleratorEngineId, AcceleratorError, AcceleratorTopologyConfig,
    AcceleratorTopologyDevice,
};
use rem6_boot::{BootError, BootImage, BootLoadReport};
use rem6_checkpoint::CheckpointComponentId;
use rem6_coherence::{ChiHarnessError, HarnessError, MesiHarnessError, MoesiHarnessError};
use rem6_cpu::{CpuId, CpuTopologyError, RiscvCluster, RiscvClusterTopologyConfig};
use rem6_dram::{
    DramControllerConfig, DramGeometry, DramMemoryActivityMarker, DramMemoryActivityProfile,
    DramMemoryController, DramMemoryError, DramMemoryWaitForMarker, DramTargetActivity, DramTiming,
    ExternalMemoryProfile,
};
use rem6_fabric::FabricModel;
use rem6_gpu::{GpuDeviceId, GpuError, GpuKernelLaunch, GpuTopologyConfig, GpuTopologyDevice};
use rem6_isa_riscv::Register;
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, SchedulerError,
    Tick, WaitForGraph,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, CacheLineLayout, MemoryError, MemoryRequestId,
    MemoryResponse, MemoryTargetId, PartitionedMemoryStore,
};
use rem6_mmio::MmioBus;
use rem6_net::SinicError;
use rem6_pci::{PciBarIndex, PciError, PciFunctionAddress};
use rem6_platform::{Platform, PlatformError, PlatformRiscvDeviceTreeConfig};
use rem6_stats::StatsRegistry;
use rem6_timer::{ClintId, TimerId};
use rem6_topology::Topology;
use rem6_transport::{
    MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome, TransportError,
};
use rem6_uart::UartId;

use crate::{
    GuestEventId, GuestSourceId, HostEventPolicy, RiscvSystemRun, RiscvSystemRunDriver,
    RiscvTrapEventPort, SystemError, SystemHostController, SystemHostEventPort,
};

use coherence_data::{
    chi_data_cache_run_records_since, merge_chi_data_cache_activity,
    merge_mesi_data_cache_activity, merge_moesi_data_cache_activity, merge_msi_data_cache_activity,
    mesi_data_cache_run_records_since, moesi_data_cache_run_records_since,
    msi_bank_data_cache_run_records_since, msi_data_cache_run_records_since,
    topology_data_cache_response, RiscvTopologyCachedDataCaches, RiscvTopologyChiDataCache,
    RiscvTopologyMesiDataCache, RiscvTopologyMoesiDataCache, RiscvTopologyMsiBankDataCache,
    RiscvTopologyMsiDataCache,
};
use instruction_cache::{
    merge_msi_instruction_cache_activity, topology_msi_instruction_cache_response,
    RiscvTopologyMsiInstructionCache,
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
    storage_checkpoint_ports: BTreeMap<CheckpointComponentId, crate::StorageImageCheckpointPort>,
    ide_checkpoint_ports: BTreeMap<CheckpointComponentId, crate::IdeControllerCheckpointPort>,
    sinic_register_checkpoint_ports:
        BTreeMap<CheckpointComponentId, crate::SinicRegisterCheckpointPort>,
    sinic_fifo_checkpoint_ports: BTreeMap<CheckpointComponentId, crate::SinicFifoCheckpointPort>,
    pci_host_checkpoint_ports: BTreeMap<CheckpointComponentId, crate::PciHostCheckpointPort>,
    pci_legacy_interrupt_router_checkpoint_ports:
        BTreeMap<CheckpointComponentId, crate::PciLegacyInterruptRouterCheckpointPort>,
    virtio_split_queue_checkpoint_ports:
        BTreeMap<CheckpointComponentId, crate::VirtioSplitQueueCheckpointPort>,
    virtio_pci_common_checkpoint_ports:
        BTreeMap<CheckpointComponentId, crate::VirtioPciCommonCheckpointPort>,
    virtio_pci_notify_checkpoint_ports:
        BTreeMap<CheckpointComponentId, crate::VirtioPciNotifyCheckpointPort>,
    virtio_pci_isr_checkpoint_ports:
        BTreeMap<CheckpointComponentId, crate::VirtioPciIsrCheckpointPort>,
    virtio_pci_device_config_checkpoint_ports:
        BTreeMap<CheckpointComponentId, crate::VirtioPciDeviceConfigCheckpointPort>,
    msi_instruction_cache: Option<RiscvTopologyMsiInstructionCache>,
    msi_bank_data_cache: Option<RiscvTopologyMsiBankDataCache>,
    msi_data_cache: Option<RiscvTopologyMsiDataCache>,
    mesi_data_cache: Option<RiscvTopologyMesiDataCache>,
    moesi_data_cache: Option<RiscvTopologyMoesiDataCache>,
    chi_data_cache: Option<RiscvTopologyChiDataCache>,
    host: Option<RiscvTopologyHost>,
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

fn default_clint_checkpoint_component(clint: ClintId) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("clint{}", clint.get()))
        .expect("formatted CLINT checkpoint component is nonempty")
}

fn default_plic_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("plic.{:x}", base.get()))
        .expect("formatted PLIC checkpoint component is nonempty")
}

fn default_rtc_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("rtc.{:x}", base.get()))
        .expect("formatted RTC checkpoint component is nonempty")
}

fn default_readfile_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("readfile.{:x}", base.get()))
        .expect("formatted readfile checkpoint component is nonempty")
}

fn default_pl031_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("pl031.{:x}", base.get()))
        .expect("formatted PL031 checkpoint component is nonempty")
}

fn default_sp804_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("sp804.{:x}", base.get()))
        .expect("formatted SP804 checkpoint component is nonempty")
}

fn default_sp805_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("sp805.{:x}", base.get()))
        .expect("formatted SP805 checkpoint component is nonempty")
}

fn default_cpu_local_timer_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("cpu_local_timer.{:x}", base.get()))
        .expect("formatted CPU local timer checkpoint component is nonempty")
}

fn default_pl011_uart_checkpoint_component(base: Address) -> CheckpointComponentId {
    CheckpointComponentId::new(format!("pl011.{:x}", base.get()))
        .expect("formatted PL011 checkpoint component is nonempty")
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
            storage_checkpoint_ports: BTreeMap::new(),
            ide_checkpoint_ports: BTreeMap::new(),
            sinic_register_checkpoint_ports: BTreeMap::new(),
            sinic_fifo_checkpoint_ports: BTreeMap::new(),
            pci_host_checkpoint_ports: BTreeMap::new(),
            pci_legacy_interrupt_router_checkpoint_ports: BTreeMap::new(),
            virtio_split_queue_checkpoint_ports: BTreeMap::new(),
            virtio_pci_common_checkpoint_ports: BTreeMap::new(),
            virtio_pci_notify_checkpoint_ports: BTreeMap::new(),
            virtio_pci_isr_checkpoint_ports: BTreeMap::new(),
            virtio_pci_device_config_checkpoint_ports: BTreeMap::new(),
            msi_instruction_cache: None,
            msi_bank_data_cache: None,
            msi_data_cache: None,
            mesi_data_cache: None,
            moesi_data_cache: None,
            chi_data_cache: None,
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
        self.attach_storage_checkpoint_to_host()?;
        self.attach_sinic_checkpoint_to_host()?;
        self.attach_pci_checkpoint_to_host()?;
        self.attach_virtio_pci_checkpoint_to_host()?;
        self.attach_platform_checkpoint_to_host()?;
        Ok(self)
    }

    pub fn with_memory_store(
        mut self,
        memory: PartitionedMemoryStore,
    ) -> Result<Self, RiscvTopologySystemError> {
        self.attach_store_line_layouts_to_cores(&memory)?;
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
        self.attach_store_line_layouts_to_cores(&memory)?;
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

        self.attach_dram_line_layouts_to_cores(&config)?;
        self.memory = Some(RiscvTopologyMemoryBackend::Dram {
            component: config.checkpoint_component().clone(),
            memory: Arc::new(Mutex::new(controller)),
        });
        self.attach_memory_checkpoint_to_host()?;
        Ok(self)
    }

    fn attach_store_line_layouts_to_cores(
        &self,
        memory: &PartitionedMemoryStore,
    ) -> Result<(), RiscvTopologySystemError> {
        for (target, range) in memory.regions() {
            let layout = memory
                .partition_layout(*target)
                .map_err(RiscvTopologySystemError::Memory)?;
            self.attach_line_layout_range_to_cores(range.base_range(), layout);
        }
        Ok(())
    }

    fn attach_dram_line_layouts_to_cores(
        &self,
        config: &RiscvTopologyDramConfig,
    ) -> Result<(), RiscvTopologySystemError> {
        for target in &config.targets {
            for region in target.regions() {
                let range = AddressRange::new(region.start(), region.size())
                    .map_err(RiscvTopologySystemError::Memory)?;
                self.attach_line_layout_range_to_cores(range, target.line_layout());
            }
        }
        Ok(())
    }

    fn attach_line_layout_range_to_cores(&self, range: AddressRange, line_layout: CacheLineLayout) {
        for cpu in self.cluster.core_ids() {
            self.cluster
                .core(cpu)
                .expect("cluster core id")
                .add_memory_line_layout_range(range, line_layout);
        }
    }

    pub fn install_riscv_device_tree_handoff(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
        dtb_addr: Address,
    ) -> Result<RiscvDtbHandoffReport, RiscvTopologySystemError> {
        let (dtb_len, image) = self.build_riscv_device_tree_image(config, dtb_addr)?;
        self.install_riscv_device_tree_image_handoff(dtb_addr, dtb_len, &image)
    }

    pub fn install_riscv_linux_boot_handoff(
        &self,
        config: &RiscvLinuxBootHandoffConfig,
    ) -> Result<RiscvLinuxBootHandoffReport, RiscvTopologySystemError> {
        let device_tree = if let Some(initrd) = config.initrd() {
            let size =
                AccessSize::new(initrd.len() as u64).map_err(RiscvTopologySystemError::Memory)?;
            config
                .device_tree()
                .clone()
                .with_initrd(initrd.start(), size)
                .map_err(RiscvTopologySystemError::Platform)?
        } else {
            config.device_tree().clone()
        };
        let (dtb_len, dtb_image) =
            self.build_riscv_device_tree_image(&device_tree, config.dtb_addr())?;

        let initrd_load_report = if let Some(initrd) = config.initrd() {
            let image = BootImage::new(initrd.start())
                .add_segment(initrd.start(), initrd.data().to_vec())
                .map_err(RiscvTopologySystemError::Boot)?;
            Some(self.load_boot_image_by_address(&image)?)
        } else {
            None
        };
        let dtb =
            self.install_riscv_device_tree_image_handoff(config.dtb_addr(), dtb_len, &dtb_image)?;

        Ok(RiscvLinuxBootHandoffReport::new(dtb, initrd_load_report))
    }

    fn build_riscv_device_tree_image(
        &self,
        config: &PlatformRiscvDeviceTreeConfig,
        dtb_addr: Address,
    ) -> Result<(usize, BootImage), RiscvTopologySystemError> {
        let platform = self
            .platform
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingPlatform)?;
        let dtb = platform
            .riscv_device_tree(config)
            .map_err(RiscvTopologySystemError::Platform)?
            .to_dtb();
        let dtb_len = dtb.len();
        let image = BootImage::new(dtb_addr)
            .add_segment(dtb_addr, dtb)
            .map_err(RiscvTopologySystemError::Boot)?;
        Ok((dtb_len, image))
    }

    fn install_riscv_device_tree_image_handoff(
        &self,
        dtb_addr: Address,
        dtb_len: usize,
        image: &BootImage,
    ) -> Result<RiscvDtbHandoffReport, RiscvTopologySystemError> {
        let load_report = self.load_boot_image_by_address(image)?;

        let a0 = Register::new(10).expect("RISC-V A0 register index is valid");
        let a1 = Register::new(11).expect("RISC-V A1 register index is valid");
        for cpu in self.cluster.core_ids() {
            let core = self.cluster.core(cpu).expect("cluster core id is valid");
            core.write_register(a0, u64::from(cpu.get()));
            core.write_register(a1, dtb_addr.get());
        }

        Ok(RiscvDtbHandoffReport::new(dtb_addr, dtb_len, load_report))
    }

    fn load_boot_image_by_address(
        &self,
        image: &BootImage,
    ) -> Result<BootLoadReport, RiscvTopologySystemError> {
        match self
            .memory
            .as_ref()
            .ok_or(RiscvTopologySystemError::MissingMemoryStore)?
        {
            RiscvTopologyMemoryBackend::Store { memory, .. } => image
                .load_into_partitioned_store_by_address(
                    &mut memory.lock().expect("topology memory store lock"),
                )
                .map_err(RiscvTopologySystemError::Boot),
            RiscvTopologyMemoryBackend::Dram { memory, .. } => {
                load_boot_image_into_dram_controller_by_address(
                    &mut memory.lock().expect("topology DRAM memory lock"),
                    image,
                )
            }
        }
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

    pub fn dram_activity_profile(&self) -> Option<DramMemoryActivityProfile> {
        self.dram_memory_controller().map(|controller| {
            controller
                .lock()
                .expect("DRAM memory lock")
                .activity_profile()
        })
    }

    pub fn dram_target_activity(&self, target: MemoryTargetId) -> Option<DramTargetActivity> {
        self.dram_memory_controller().and_then(|controller| {
            controller
                .lock()
                .expect("DRAM memory lock")
                .target_activity(target)
        })
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
        let fabric_activity_start = self.transport.mark_fabric_activity();
        let fabric_wait_for_start = self.transport.mark_fabric_wait_for();
        let dram_activity_start = mark_dram_activity(&memory);
        let dram_wait_for_start = mark_dram_wait_for(&memory);
        let msi_instruction_cache = self.msi_instruction_cache.clone();
        let msi_instruction_run_start = msi_instruction_cache
            .as_ref()
            .map(RiscvTopologyMsiInstructionCache::mark_runs);
        let msi_bank_data_cache = self.msi_bank_data_cache.clone();
        let msi_bank_data_run_start = msi_bank_data_cache
            .as_ref()
            .map(RiscvTopologyMsiBankDataCache::mark_runs);
        let msi_data_cache = self.msi_data_cache.clone();
        let msi_data_run_start = msi_data_cache
            .as_ref()
            .map(RiscvTopologyMsiDataCache::mark_runs);
        let mesi_data_cache = self.mesi_data_cache.clone();
        let mesi_data_run_start = mesi_data_cache
            .as_ref()
            .map(RiscvTopologyMesiDataCache::mark_runs);
        let moesi_data_cache = self.moesi_data_cache.clone();
        let moesi_data_run_start = moesi_data_cache
            .as_ref()
            .map(RiscvTopologyMoesiDataCache::mark_runs);
        let chi_data_cache = self.chi_data_cache.clone();
        let chi_data_run_start = chi_data_cache
            .as_ref()
            .map(RiscvTopologyChiDataCache::mark_runs);

        let fetch_memory = memory.clone();
        let fetch_error = Arc::clone(&memory_error);
        let fetch_msi_instruction_cache = msi_instruction_cache.clone();
        let fetch_cluster = self.cluster.clone();
        let fetch_responder = move |_cpu| {
            let memory = fetch_memory.clone();
            let memory_error = Arc::clone(&fetch_error);
            let msi_instruction_cache = fetch_msi_instruction_cache.clone();
            let cluster = fetch_cluster.clone();
            move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                topology_msi_instruction_cache_response(
                    msi_instruction_cache.as_ref(),
                    &cluster,
                    &memory_error,
                    &delivery,
                )
                .unwrap_or_else(|| topology_memory_response(&memory, &memory_error, &delivery))
            }
        };

        let data_memory = memory.clone();
        let data_error = Arc::clone(&memory_error);
        let data_msi_bank_cache = msi_bank_data_cache.clone();
        let data_msi_cache = msi_data_cache.clone();
        let data_mesi_cache = mesi_data_cache.clone();
        let data_moesi_cache = moesi_data_cache.clone();
        let data_chi_cache = chi_data_cache.clone();
        let data_cluster = self.cluster.clone();
        let data_responder = move |_cpu| {
            let memory = data_memory.clone();
            let memory_error = Arc::clone(&data_error);
            let msi_bank_data_cache = data_msi_bank_cache.clone();
            let msi_data_cache = data_msi_cache.clone();
            let mesi_data_cache = data_mesi_cache.clone();
            let moesi_data_cache = data_moesi_cache.clone();
            let chi_data_cache = data_chi_cache.clone();
            let cluster = data_cluster.clone();
            move |delivery, _context: &mut ParallelSchedulerContext<'_>| {
                topology_cached_memory_response(
                    &memory,
                    &memory_error,
                    RiscvTopologyCachedDataCaches {
                        msi_bank: msi_bank_data_cache.as_ref(),
                        msi: msi_data_cache.as_ref(),
                        mesi: mesi_data_cache.as_ref(),
                        moesi: moesi_data_cache.as_ref(),
                        chi: chi_data_cache.as_ref(),
                        cluster: &cluster,
                    },
                    &delivery,
                )
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
        drop(scheduler);

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
        let fabric_activity = fabric_activity_start
            .and_then(|marker| self.transport.fabric_lane_activities_since(marker))
            .unwrap_or_default();
        let fabric_wait_for = fabric_wait_for_start
            .and_then(|marker| self.transport.fabric_wait_for_graph_since(marker))
            .unwrap_or_default();
        let dram_activity = dram_activities_since(&memory, dram_activity_start, run.final_tick());
        let dram_wait_for = dram_wait_for_since(&memory, dram_wait_for_start);
        let (fabric_activity, dram_activity) = merge_msi_instruction_cache_activity(
            fabric_activity,
            dram_activity,
            msi_instruction_cache.as_ref(),
            msi_instruction_run_start,
        );
        let (fabric_activity, dram_activity) = merge_msi_data_cache_activity(
            fabric_activity,
            dram_activity,
            msi_data_cache.as_ref(),
            msi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_mesi_data_cache_activity(
            fabric_activity,
            dram_activity,
            mesi_data_cache.as_ref(),
            mesi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_moesi_data_cache_activity(
            fabric_activity,
            dram_activity,
            moesi_data_cache.as_ref(),
            moesi_data_run_start,
        );
        let (fabric_activity, dram_activity) = merge_chi_data_cache_activity(
            fabric_activity,
            dram_activity,
            chi_data_cache.as_ref(),
            chi_data_run_start,
        );
        let mut data_cache_run_records = msi_bank_data_cache_run_records_since(
            msi_bank_data_cache.as_ref(),
            msi_bank_data_run_start,
        );
        data_cache_run_records.extend(msi_data_cache_run_records_since(
            msi_data_cache.as_ref(),
            msi_data_run_start,
        ));
        data_cache_run_records.extend(mesi_data_cache_run_records_since(
            mesi_data_cache.as_ref(),
            mesi_data_run_start,
        ));
        data_cache_run_records.extend(moesi_data_cache_run_records_since(
            moesi_data_cache.as_ref(),
            moesi_data_run_start,
        ));
        data_cache_run_records.extend(chi_data_cache_run_records_since(
            chi_data_cache.as_ref(),
            chi_data_run_start,
        ));

        Ok(run
            .with_fabric_activity(fabric_activity)
            .with_fabric_wait_for(fabric_wait_for)
            .with_dram_activity(dram_activity)
            .with_dram_wait_for(dram_wait_for)
            .with_data_cache_run_records(data_cache_run_records))
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
    DuplicateAccelerator {
        engine: AcceleratorEngineId,
    },
    UnknownAccelerator {
        engine: AcceleratorEngineId,
    },
    AcceleratorDmaWriteNotReady {
        engine: AcceleratorEngineId,
    },
    DuplicateGpu {
        device: GpuDeviceId,
    },
    UnknownGpu {
        device: GpuDeviceId,
    },
    GpuDmaWriteNotReady {
        device: GpuDeviceId,
    },
    PlatformPartitionMismatch {
        topology: u32,
        platform: u32,
    },
    DuplicateSinicPciCheckpointComponent {
        component: CheckpointComponentId,
    },
    SinicPciBarAddressBelowHostBase {
        address: Address,
        host_base: Address,
    },
    SinicPciBarAddressMisaligned {
        address: Address,
        alignment_bytes: u64,
    },
    SinicPciBarAddressTooWide {
        address: Address,
    },
    MissingSinicPciBarRange {
        function: PciFunctionAddress,
        bar: PciBarIndex,
    },
    WorkloadSinicPci(RiscvTopologyWorkloadSinicPciError),
    HostPartitionOutOfRange {
        host: PartitionId,
        partitions: u32,
    },
    MissingPlatform,
    MissingMemoryStore,
    MissingHostController,
    MissingMsiInstructionResponse {
        request: MemoryRequestId,
    },
    MissingMsiDataResponse {
        request: MemoryRequestId,
    },
    MissingMesiDataResponse {
        request: MemoryRequestId,
    },
    MissingMoesiDataResponse {
        request: MemoryRequestId,
    },
    MissingChiDataResponse {
        request: MemoryRequestId,
    },
    Accelerator(AcceleratorError),
    Gpu(GpuError),
    MsiInstructionCache(HarnessError),
    MsiDataCache(HarnessError),
    MesiDataCache(MesiHarnessError),
    MoesiDataCache(MoesiHarnessError),
    ChiDataCache(ChiHarnessError),
    Transport(TransportError),
    Memory(MemoryError),
    Dram(DramMemoryError),
    Platform(PlatformError),
    Pci(PciError),
    Sinic(SinicError),
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
            Self::DuplicateSinicPciCheckpointComponent { component } => write!(
                formatter,
                "SINIC PCI checkpoint component {} is already attached",
                component.as_str()
            ),
            Self::SinicPciBarAddressBelowHostBase { address, host_base } => write!(
                formatter,
                "SINIC PCI BAR address {:#x} is below host base {:#x}",
                address.get(),
                host_base.get()
            ),
            Self::SinicPciBarAddressMisaligned {
                address,
                alignment_bytes,
            } => write!(
                formatter,
                "SINIC PCI BAR address {:#x} is not aligned to {alignment_bytes} bytes",
                address.get()
            ),
            Self::SinicPciBarAddressTooWide { address } => write!(
                formatter,
                "SINIC PCI BAR address {:#x} does not fit a 32-bit memory BAR",
                address.get()
            ),
            Self::MissingSinicPciBarRange { function, bar } => write!(
                formatter,
                "SINIC PCI endpoint {:?} BAR {} is not active in the host bridge",
                function,
                bar.get()
            ),
            Self::WorkloadSinicPci(error) => write!(formatter, "{error}"),
            Self::HostPartitionOutOfRange { host, partitions } => write!(
                formatter,
                "host partition {} is outside topology partition count {partitions}",
                host.index()
            ),
            Self::MissingPlatform => write!(formatter, "topology system has no platform"),
            Self::MissingMemoryStore => write!(formatter, "topology system has no memory store"),
            Self::MissingHostController => {
                write!(formatter, "topology system has no host controller")
            }
            Self::MissingMsiInstructionResponse { request } => write!(
                formatter,
                "MSI instruction cache produced no response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingMsiDataResponse { request } => write!(
                formatter,
                "MSI data cache produced no response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingMesiDataResponse { request } => write!(
                formatter,
                "MESI data cache produced no response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingMoesiDataResponse { request } => write!(
                formatter,
                "MOESI data cache produced no response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::MissingChiDataResponse { request } => write!(
                formatter,
                "CHI data cache produced no response for request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::Accelerator(error) => write!(formatter, "{error}"),
            Self::Gpu(error) => write!(formatter, "{error}"),
            Self::MsiInstructionCache(error) => write!(formatter, "{error}"),
            Self::MsiDataCache(error) => write!(formatter, "{error}"),
            Self::MesiDataCache(error) => write!(formatter, "{error}"),
            Self::MoesiDataCache(error) => write!(formatter, "{error}"),
            Self::ChiDataCache(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Dram(error) => write!(formatter, "{error}"),
            Self::Platform(error) => write!(formatter, "{error}"),
            Self::Pci(error) => write!(formatter, "{error}"),
            Self::Sinic(error) => write!(formatter, "{error}"),
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
            Self::DuplicateSinicPciCheckpointComponent { .. } => None,
            Self::SinicPciBarAddressBelowHostBase { .. } => None,
            Self::SinicPciBarAddressMisaligned { .. } => None,
            Self::SinicPciBarAddressTooWide { .. } => None,
            Self::MissingSinicPciBarRange { .. } => None,
            Self::WorkloadSinicPci(error) => Some(error),
            Self::HostPartitionOutOfRange { .. } => None,
            Self::MissingPlatform => None,
            Self::MissingMemoryStore => None,
            Self::MissingHostController => None,
            Self::MissingMsiInstructionResponse { .. } => None,
            Self::MissingMsiDataResponse { .. } => None,
            Self::MissingMesiDataResponse { .. } => None,
            Self::MissingMoesiDataResponse { .. } => None,
            Self::MissingChiDataResponse { .. } => None,
            Self::Accelerator(error) => Some(error),
            Self::Gpu(error) => Some(error),
            Self::MsiInstructionCache(error) => Some(error),
            Self::MsiDataCache(error) => Some(error),
            Self::MesiDataCache(error) => Some(error),
            Self::MoesiDataCache(error) => Some(error),
            Self::ChiDataCache(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Dram(error) => Some(error),
            Self::Platform(error) => Some(error),
            Self::Pci(error) => Some(error),
            Self::Sinic(error) => Some(error),
            Self::Boot(error) => Some(error),
            Self::System(error) => Some(error),
        }
    }
}

fn load_boot_image_into_dram_controller_by_address(
    controller: &mut DramMemoryController,
    image: &BootImage,
) -> Result<BootLoadReport, RiscvTopologySystemError> {
    let mut staging = PartitionedMemoryStore::from_snapshot(controller.snapshot().store())
        .map_err(RiscvTopologySystemError::Memory)?;
    let report = image
        .load_into_partitioned_store_by_address(&mut staging)
        .map_err(RiscvTopologySystemError::Boot)?;

    for partition in staging.snapshot().partitions() {
        for line in partition.lines() {
            controller
                .insert_line(partition.target(), line.line(), line.data().to_vec())
                .map_err(RiscvTopologySystemError::Dram)?;
        }
    }

    Ok(report)
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

fn topology_cached_memory_response(
    memory: &RiscvTopologyMemoryBackend,
    memory_error: &Arc<Mutex<Option<RiscvTopologySystemError>>>,
    data_caches: RiscvTopologyCachedDataCaches<'_>,
    delivery: &RequestDelivery,
) -> TargetOutcome {
    topology_data_cache_response(data_caches, memory_error, delivery)
        .unwrap_or_else(|| topology_memory_response(memory, memory_error, delivery))
}

fn mark_dram_activity(memory: &RiscvTopologyMemoryBackend) -> Option<DramMemoryActivityMarker> {
    match memory {
        RiscvTopologyMemoryBackend::Store { .. } => None,
        RiscvTopologyMemoryBackend::Dram { memory, .. } => Some(
            memory
                .lock()
                .expect("topology DRAM memory lock")
                .mark_activity(),
        ),
    }
}

fn mark_dram_wait_for(memory: &RiscvTopologyMemoryBackend) -> Option<DramMemoryWaitForMarker> {
    match memory {
        RiscvTopologyMemoryBackend::Store { .. } => None,
        RiscvTopologyMemoryBackend::Dram { memory, .. } => Some(
            memory
                .lock()
                .expect("topology DRAM memory lock")
                .mark_wait_for(),
        ),
    }
}

fn dram_activities_since(
    memory: &RiscvTopologyMemoryBackend,
    marker: Option<DramMemoryActivityMarker>,
    end_tick: Option<Tick>,
) -> Vec<DramTargetActivity> {
    let Some(marker) = marker else {
        return Vec::new();
    };
    match memory {
        RiscvTopologyMemoryBackend::Store { .. } => Vec::new(),
        RiscvTopologyMemoryBackend::Dram { memory, .. } => {
            let memory = memory.lock().expect("topology DRAM memory lock");
            if let Some(end_tick) = end_tick {
                memory.target_activities_since_until(&marker, end_tick)
            } else {
                memory.target_activities_since(&marker)
            }
        }
    }
}

fn dram_wait_for_since(
    memory: &RiscvTopologyMemoryBackend,
    marker: Option<DramMemoryWaitForMarker>,
) -> WaitForGraph {
    let Some(marker) = marker else {
        return WaitForGraph::new();
    };
    match memory {
        RiscvTopologyMemoryBackend::Store { .. } => WaitForGraph::new(),
        RiscvTopologyMemoryBackend::Dram { memory, .. } => memory
            .lock()
            .expect("topology DRAM memory lock")
            .wait_for_graph_since(&marker),
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
