use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_boot::{BootError, BootImage};
use rem6_dram::ExternalMemoryProfile;
use rem6_kernel::Tick;
use rem6_memory::{Address, AddressRange};
use rem6_stats::StatSnapshot;

mod heterogeneous;
mod identity;
mod result;

pub use heterogeneous::{
    WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind, WorkloadAcceleratorDevice,
    WorkloadGpuDevice, WorkloadGpuKernelLaunch,
};
use identity::manifest_identity;
pub use result::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadParallelExecutionSummary,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadId(String);

impl WorkloadId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkloadError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkloadError::EmptyWorkloadId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadResourceId(String);

impl WorkloadResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkloadError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkloadError::EmptyResourceId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceKind {
    Kernel,
    DiskImage,
    Firmware,
    DeviceTree,
    Input,
    Output,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResource {
    id: WorkloadResourceId,
    kind: WorkloadResourceKind,
    digest: String,
    locator: String,
}

impl WorkloadResource {
    pub fn new(
        id: WorkloadResourceId,
        kind: WorkloadResourceKind,
        digest: impl Into<String>,
        locator: impl Into<String>,
    ) -> Result<Self, WorkloadError> {
        let digest = digest.into();
        if digest.is_empty() {
            return Err(WorkloadError::EmptyResourceDigest {
                resource: id.clone(),
            });
        }

        let locator = locator.into();
        if locator.is_empty() {
            return Err(WorkloadError::EmptyResourceLocator {
                resource: id.clone(),
            });
        }

        Ok(Self {
            id,
            kind,
            digest,
            locator,
        })
    }

    pub fn id(&self) -> &WorkloadResourceId {
        &self.id
    }

    pub const fn kind(&self) -> WorkloadResourceKind {
        self.kind
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn locator(&self) -> &str {
        &self.locator
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadRouteId(String);

impl WorkloadRouteId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkloadError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkloadError::EmptyRouteId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadHostPlacement {
    partition: u32,
    latency: Tick,
    source: u32,
}

impl WorkloadHostPlacement {
    pub const fn new(partition: u32, latency: Tick, source: u32) -> Result<Self, WorkloadError> {
        if latency == 0 {
            return Err(WorkloadError::ZeroHostLatency);
        }

        Ok(Self {
            partition,
            latency,
            source,
        })
    }

    pub const fn partition(self) -> u32 {
        self.partition
    }

    pub const fn latency(self) -> Tick {
        self.latency
    }

    pub const fn source(self) -> u32 {
        self.source
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadMemoryTarget {
    target: u32,
    line_bytes: u64,
    range: AddressRange,
    external_memory_profile: Option<ExternalMemoryProfile>,
}

impl WorkloadMemoryTarget {
    pub const fn new(
        target: u32,
        line_bytes: u64,
        range: AddressRange,
    ) -> Result<Self, WorkloadError> {
        if line_bytes == 0 {
            return Err(WorkloadError::ZeroLineBytes { target });
        }

        Ok(Self {
            target,
            line_bytes,
            range,
            external_memory_profile: None,
        })
    }

    pub fn with_external_memory_profile(
        mut self,
        profile: ExternalMemoryProfile,
    ) -> Result<Self, WorkloadError> {
        if profile.target().get() != self.target {
            return Err(WorkloadError::MemoryProfileTargetMismatch {
                target: self.target,
                profile_target: profile.target().get(),
            });
        }
        if profile.line_layout().bytes() != self.line_bytes {
            return Err(WorkloadError::MemoryProfileLineSizeMismatch {
                target: self.target,
                line_bytes: self.line_bytes,
                profile_line_bytes: profile.line_layout().bytes(),
            });
        }
        if profile.geometry().line_size() != profile.line_layout().bytes() {
            return Err(WorkloadError::MemoryProfileGeometryLineSizeMismatch {
                target: self.target,
                layout_line_bytes: profile.line_layout().bytes(),
                geometry_line_bytes: profile.geometry().line_size(),
            });
        }

        self.external_memory_profile = Some(profile);
        Ok(self)
    }

    pub const fn target(self) -> u32 {
        self.target
    }

    pub const fn line_bytes(self) -> u64 {
        self.line_bytes
    }

    pub const fn range(self) -> AddressRange {
        self.range
    }

    pub fn external_memory_profile(&self) -> Option<&ExternalMemoryProfile> {
        self.external_memory_profile.as_ref()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkloadRouteLatency {
    Request,
    Response,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadMemoryRoute {
    id: WorkloadRouteId,
    source_endpoint: String,
    source_partition: u32,
    target_endpoint: String,
    target_partition: u32,
    request_latency: Tick,
    response_latency: Tick,
}

impl WorkloadMemoryRoute {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: WorkloadRouteId,
        source_endpoint: impl Into<String>,
        source_partition: u32,
        target_endpoint: impl Into<String>,
        target_partition: u32,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Result<Self, WorkloadError> {
        let source_endpoint = source_endpoint.into();
        if source_endpoint.is_empty() {
            return Err(WorkloadError::EmptyEndpoint);
        }

        let target_endpoint = target_endpoint.into();
        if target_endpoint.is_empty() {
            return Err(WorkloadError::EmptyEndpoint);
        }

        if request_latency == 0 {
            return Err(WorkloadError::ZeroRouteLatency {
                route: id.clone(),
                latency: WorkloadRouteLatency::Request,
            });
        }

        if response_latency == 0 {
            return Err(WorkloadError::ZeroRouteLatency {
                route: id.clone(),
                latency: WorkloadRouteLatency::Response,
            });
        }

        Ok(Self {
            id,
            source_endpoint,
            source_partition,
            target_endpoint,
            target_partition,
            request_latency,
            response_latency,
        })
    }

    pub fn id(&self) -> &WorkloadRouteId {
        &self.id
    }

    pub fn source_endpoint(&self) -> &str {
        &self.source_endpoint
    }

    pub const fn source_partition(&self) -> u32 {
        self.source_partition
    }

    pub fn target_endpoint(&self) -> &str {
        &self.target_endpoint
    }

    pub const fn target_partition(&self) -> u32 {
        self.target_partition
    }

    pub const fn request_latency(&self) -> Tick {
        self.request_latency
    }

    pub const fn response_latency(&self) -> Tick {
        self.response_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadRiscvCore {
    cpu: u32,
    partition: u32,
    agent: u32,
    entry: Address,
    fetch_endpoint: String,
    fetch_route: WorkloadRouteId,
    data_endpoint: Option<String>,
    data_route: Option<WorkloadRouteId>,
}

impl WorkloadRiscvCore {
    pub fn new(
        cpu: u32,
        partition: u32,
        agent: u32,
        entry: Address,
        fetch_endpoint: impl Into<String>,
        fetch_route: WorkloadRouteId,
    ) -> Result<Self, WorkloadError> {
        let fetch_endpoint = fetch_endpoint.into();
        if fetch_endpoint.is_empty() {
            return Err(WorkloadError::EmptyEndpoint);
        }

        Ok(Self {
            cpu,
            partition,
            agent,
            entry,
            fetch_endpoint,
            fetch_route,
            data_endpoint: None,
            data_route: None,
        })
    }

    pub fn with_data(
        mut self,
        data_endpoint: impl Into<String>,
        data_route: WorkloadRouteId,
    ) -> Result<Self, WorkloadError> {
        let data_endpoint = data_endpoint.into();
        if data_endpoint.is_empty() {
            return Err(WorkloadError::EmptyEndpoint);
        }

        self.data_endpoint = Some(data_endpoint);
        self.data_route = Some(data_route);
        Ok(self)
    }

    pub const fn cpu(&self) -> u32 {
        self.cpu
    }

    pub const fn partition(&self) -> u32 {
        self.partition
    }

    pub const fn agent(&self) -> u32 {
        self.agent
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }

    pub fn fetch_endpoint(&self) -> &str {
        &self.fetch_endpoint
    }

    pub fn fetch_route(&self) -> &WorkloadRouteId {
        &self.fetch_route
    }

    pub fn data_endpoint(&self) -> Option<&str> {
        self.data_endpoint.as_deref()
    }

    pub fn data_route(&self) -> Option<&WorkloadRouteId> {
        self.data_route.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadRiscvDataCache {
    protocol: WorkloadDataCacheProtocol,
    memory_target: u32,
    line_addresses: Vec<Address>,
    directory_partition: u32,
    directory_endpoint: String,
}

impl WorkloadRiscvDataCache {
    pub fn new(
        protocol: WorkloadDataCacheProtocol,
        memory_target: u32,
        line_address: Address,
        directory_partition: u32,
        directory_endpoint: impl Into<String>,
    ) -> Result<Self, WorkloadError> {
        let directory_endpoint = directory_endpoint.into();
        if directory_endpoint.is_empty() {
            return Err(WorkloadError::EmptyEndpoint);
        }

        Ok(Self {
            protocol,
            memory_target,
            line_addresses: vec![line_address],
            directory_partition,
            directory_endpoint,
        })
    }

    pub fn with_line_address(mut self, line_address: Address) -> Self {
        if !self.line_addresses.contains(&line_address) {
            self.line_addresses.push(line_address);
            self.line_addresses.sort_by_key(|address| address.get());
        }
        self
    }

    pub const fn protocol(&self) -> WorkloadDataCacheProtocol {
        self.protocol
    }

    pub const fn memory_target(&self) -> u32 {
        self.memory_target
    }

    pub fn line_address(&self) -> Address {
        self.line_addresses[0]
    }

    pub fn line_addresses(&self) -> &[Address] {
        &self.line_addresses
    }

    pub const fn directory_partition(&self) -> u32 {
        self.directory_partition
    }

    pub fn directory_endpoint(&self) -> &str {
        &self.directory_endpoint
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadTopology {
    partition_count: u32,
    min_remote_delay: Tick,
    parallel_worker_limit: usize,
    host: WorkloadHostPlacement,
    memory_targets: Vec<WorkloadMemoryTarget>,
    memory_routes: Vec<WorkloadMemoryRoute>,
    riscv_cores: Vec<WorkloadRiscvCore>,
    riscv_data_cache: Option<WorkloadRiscvDataCache>,
    gpu_devices: Vec<WorkloadGpuDevice>,
    gpu_kernel_launches: Vec<WorkloadGpuKernelLaunch>,
    accelerator_devices: Vec<WorkloadAcceleratorDevice>,
    accelerator_commands: Vec<WorkloadAcceleratorCommand>,
}

impl WorkloadTopology {
    pub const fn new(
        partition_count: u32,
        min_remote_delay: Tick,
        parallel_worker_limit: usize,
        host: WorkloadHostPlacement,
    ) -> Result<Self, WorkloadError> {
        if partition_count == 0 {
            return Err(WorkloadError::ZeroTopologyPartitions);
        }
        if min_remote_delay == 0 {
            return Err(WorkloadError::ZeroMinRemoteDelay);
        }
        if parallel_worker_limit == 0 {
            return Err(WorkloadError::ZeroParallelWorkerLimit);
        }
        if host.partition() >= partition_count {
            return Err(WorkloadError::PartitionOutOfRange {
                partition: host.partition(),
                partition_count,
            });
        }

        Ok(Self {
            partition_count,
            min_remote_delay,
            parallel_worker_limit,
            host,
            memory_targets: Vec::new(),
            memory_routes: Vec::new(),
            riscv_cores: Vec::new(),
            riscv_data_cache: None,
            gpu_devices: Vec::new(),
            gpu_kernel_launches: Vec::new(),
            accelerator_devices: Vec::new(),
            accelerator_commands: Vec::new(),
        })
    }

    pub fn add_memory_target(
        mut self,
        target: WorkloadMemoryTarget,
    ) -> Result<Self, WorkloadError> {
        if self
            .memory_targets
            .iter()
            .any(|existing| existing.target() == target.target())
        {
            return Err(WorkloadError::DuplicateMemoryTarget {
                target: target.target(),
            });
        }

        self.memory_targets.push(target);
        self.memory_targets
            .sort_by_key(|target| (target.target(), target.range().start()));
        Ok(self)
    }

    pub fn add_memory_route(mut self, route: WorkloadMemoryRoute) -> Result<Self, WorkloadError> {
        self.validate_partition(route.source_partition())?;
        self.validate_partition(route.target_partition())?;
        if self
            .memory_routes
            .iter()
            .any(|existing| existing.id() == route.id())
        {
            return Err(WorkloadError::DuplicateRoute {
                route: route.id().clone(),
            });
        }

        self.memory_routes.push(route);
        self.memory_routes
            .sort_by(|left, right| left.id().cmp(right.id()));
        Ok(self)
    }

    pub fn add_riscv_core(mut self, core: WorkloadRiscvCore) -> Result<Self, WorkloadError> {
        self.validate_partition(core.partition())?;
        if self
            .riscv_cores
            .iter()
            .any(|existing| existing.cpu() == core.cpu())
        {
            return Err(WorkloadError::DuplicateRiscvCore { cpu: core.cpu() });
        }
        if !self
            .memory_routes
            .iter()
            .any(|route| route.id() == core.fetch_route())
        {
            return Err(WorkloadError::MissingCoreFetchRoute {
                cpu: core.cpu(),
                route: core.fetch_route().clone(),
            });
        }
        if let Some(route) = core.data_route() {
            if !self
                .memory_routes
                .iter()
                .any(|existing| existing.id() == route)
            {
                return Err(WorkloadError::MissingCoreDataRoute {
                    cpu: core.cpu(),
                    route: route.clone(),
                });
            }
        }

        self.riscv_cores.push(core);
        self.riscv_cores.sort_by_key(WorkloadRiscvCore::cpu);
        Ok(self)
    }

    pub fn add_gpu_device(mut self, device: WorkloadGpuDevice) -> Result<Self, WorkloadError> {
        self.validate_partition(device.partition())?;
        if self
            .gpu_devices
            .iter()
            .any(|existing| existing.device() == device.device())
        {
            return Err(WorkloadError::DuplicateGpuDevice {
                device: device.device(),
            });
        }
        let route = self
            .memory_routes
            .iter()
            .find(|route| route.id() == device.command_route())
            .ok_or_else(|| WorkloadError::MissingGpuCommandRoute {
                device: device.device(),
                route: device.command_route().clone(),
            })?;
        if route.target_partition() != device.partition() {
            return Err(WorkloadError::GpuCommandRouteTargetMismatch {
                device: device.device(),
                route: device.command_route().clone(),
                expected: device.partition(),
                actual: route.target_partition(),
            });
        }

        self.gpu_devices.push(device);
        self.gpu_devices.sort_by_key(WorkloadGpuDevice::device);
        Ok(self)
    }

    pub fn add_gpu_kernel_launch(
        mut self,
        launch: WorkloadGpuKernelLaunch,
    ) -> Result<Self, WorkloadError> {
        if !self
            .gpu_devices
            .iter()
            .any(|device| device.device() == launch.device())
        {
            return Err(WorkloadError::MissingGpuDevice {
                device: launch.device(),
            });
        }

        self.gpu_kernel_launches.push(launch);
        self.gpu_kernel_launches
            .sort_by_key(|launch| (launch.device(), launch.kernel()));
        Ok(self)
    }

    pub fn add_accelerator_device(
        mut self,
        device: WorkloadAcceleratorDevice,
    ) -> Result<Self, WorkloadError> {
        self.validate_partition(device.partition())?;
        if self
            .accelerator_devices
            .iter()
            .any(|existing| existing.engine() == device.engine())
        {
            return Err(WorkloadError::DuplicateAcceleratorDevice {
                engine: device.engine(),
            });
        }
        let route = self
            .memory_routes
            .iter()
            .find(|route| route.id() == device.command_route())
            .ok_or_else(|| WorkloadError::MissingAcceleratorCommandRoute {
                engine: device.engine(),
                route: device.command_route().clone(),
            })?;
        if route.target_partition() != device.partition() {
            return Err(WorkloadError::AcceleratorCommandRouteTargetMismatch {
                engine: device.engine(),
                route: device.command_route().clone(),
                expected: device.partition(),
                actual: route.target_partition(),
            });
        }

        self.accelerator_devices.push(device);
        self.accelerator_devices
            .sort_by_key(WorkloadAcceleratorDevice::engine);
        Ok(self)
    }

    pub fn add_accelerator_command(
        mut self,
        command: WorkloadAcceleratorCommand,
    ) -> Result<Self, WorkloadError> {
        if !self
            .accelerator_devices
            .iter()
            .any(|device| device.engine() == command.engine())
        {
            return Err(WorkloadError::MissingAcceleratorDevice {
                engine: command.engine(),
            });
        }

        self.accelerator_commands.push(command);
        self.accelerator_commands
            .sort_by_key(|command| (command.engine(), command.command()));
        Ok(self)
    }

    pub fn with_riscv_data_cache(
        mut self,
        cache: WorkloadRiscvDataCache,
    ) -> Result<Self, WorkloadError> {
        self.validate_partition(cache.directory_partition())?;
        if !self
            .memory_targets
            .iter()
            .any(|target| target.target() == cache.memory_target())
        {
            return Err(WorkloadError::MissingMemoryTarget {
                target: cache.memory_target(),
            });
        }

        self.riscv_data_cache = Some(cache);
        Ok(self)
    }

    pub const fn partition_count(&self) -> u32 {
        self.partition_count
    }

    pub const fn min_remote_delay(&self) -> Tick {
        self.min_remote_delay
    }

    pub const fn parallel_worker_limit(&self) -> usize {
        self.parallel_worker_limit
    }

    pub const fn host(&self) -> WorkloadHostPlacement {
        self.host
    }

    pub fn memory_targets(&self) -> &[WorkloadMemoryTarget] {
        &self.memory_targets
    }

    pub fn external_memory_profile(&self, target: u32) -> Option<&ExternalMemoryProfile> {
        self.memory_targets
            .iter()
            .find(|memory_target| memory_target.target() == target)
            .and_then(WorkloadMemoryTarget::external_memory_profile)
    }

    pub fn memory_routes(&self) -> &[WorkloadMemoryRoute] {
        &self.memory_routes
    }

    pub fn riscv_cores(&self) -> &[WorkloadRiscvCore] {
        &self.riscv_cores
    }

    pub fn riscv_data_cache(&self) -> Option<&WorkloadRiscvDataCache> {
        self.riscv_data_cache.as_ref()
    }

    pub fn gpu_devices(&self) -> &[WorkloadGpuDevice] {
        &self.gpu_devices
    }

    pub fn gpu_kernel_launches(&self) -> &[WorkloadGpuKernelLaunch] {
        &self.gpu_kernel_launches
    }

    pub fn accelerator_devices(&self) -> &[WorkloadAcceleratorDevice] {
        &self.accelerator_devices
    }

    pub fn accelerator_commands(&self) -> &[WorkloadAcceleratorCommand] {
        &self.accelerator_commands
    }

    fn validate_partition(&self, partition: u32) -> Result<(), WorkloadError> {
        if partition >= self.partition_count {
            return Err(WorkloadError::PartitionOutOfRange {
                partition,
                partition_count: self.partition_count,
            });
        }

        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadBootImage {
    entry: Address,
    segments: Vec<WorkloadBootSegment>,
}

impl WorkloadBootImage {
    pub fn from_boot_image(image: &BootImage) -> Self {
        Self {
            entry: image.entry(),
            segments: image
                .segments()
                .iter()
                .map(|segment| WorkloadBootSegment::new(segment.range(), segment.data().to_vec()))
                .collect(),
        }
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }

    pub fn segments(&self) -> &[WorkloadBootSegment] {
        &self.segments
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        let mut image = BootImage::new(self.entry);
        for segment in &self.segments {
            image = image
                .add_segment(segment.range().start(), segment.data().to_vec())
                .map_err(WorkloadError::Boot)?;
        }
        Ok(image)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadBootSegment {
    range: AddressRange,
    data: Vec<u8>,
}

impl WorkloadBootSegment {
    pub const fn new(range: AddressRange, data: Vec<u8>) -> Self {
        Self { range, data }
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostEventIntent {
    RoiBegin { label: String },
    RoiEnd { label: String },
    StatsReset { label: String },
    StatsDump { label: String },
    Checkpoint { label: String },
    Stop { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadHostEvent {
    tick: Tick,
    intent: HostEventIntent,
}

impl WorkloadHostEvent {
    pub const fn new(tick: Tick, intent: HostEventIntent) -> Self {
        Self { tick, intent }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn intent(&self) -> &HostEventIntent {
        &self.intent
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointLineage {
    CreatedByWorkload {
        label: String,
    },
    RestoredFrom {
        label: String,
        manifest_identity: String,
    },
}

impl CheckpointLineage {
    pub fn label(&self) -> &str {
        match self {
            Self::CreatedByWorkload { label } | Self::RestoredFrom { label, .. } => label,
        }
    }

    pub fn manifest_identity(&self) -> Option<&str> {
        match self {
            Self::CreatedByWorkload { .. } => None,
            Self::RestoredFrom {
                manifest_identity, ..
            } => Some(manifest_identity),
        }
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadManifestIdentity(String);

impl WorkloadManifestIdentity {
    fn new(hash: u64) -> Self {
        Self(format!("wl-{hash:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadManifest {
    id: WorkloadId,
    boot: WorkloadBootImage,
    topology: Option<WorkloadTopology>,
    resources: Vec<WorkloadResource>,
    required_resources: Vec<WorkloadResourceId>,
    host_events: Vec<WorkloadHostEvent>,
    checkpoint_lineage: Option<CheckpointLineage>,
    identity: WorkloadManifestIdentity,
}

impl WorkloadManifest {
    pub fn builder(id: WorkloadId, boot: BootImage) -> WorkloadManifestBuilder {
        WorkloadManifestBuilder::new(id, WorkloadBootImage::from_boot_image(&boot))
    }

    pub fn id(&self) -> &WorkloadId {
        &self.id
    }

    pub const fn boot(&self) -> &WorkloadBootImage {
        &self.boot
    }

    pub fn topology(&self) -> Option<&WorkloadTopology> {
        self.topology.as_ref()
    }

    pub fn resources(&self) -> &[WorkloadResource] {
        &self.resources
    }

    pub fn resource(&self, id: &WorkloadResourceId) -> Option<&WorkloadResource> {
        self.resources.iter().find(|resource| resource.id() == id)
    }

    pub fn required_resources(&self) -> &[WorkloadResourceId] {
        &self.required_resources
    }

    pub fn required_resource_details(&self) -> Result<Vec<WorkloadResource>, WorkloadError> {
        self.required_resources
            .iter()
            .map(|id| {
                self.resource(id)
                    .cloned()
                    .ok_or_else(|| WorkloadError::MissingRequiredResource {
                        resource: id.clone(),
                    })
            })
            .collect()
    }

    pub fn host_events(&self) -> &[WorkloadHostEvent] {
        &self.host_events
    }

    pub fn checkpoint_lineage(&self) -> Option<&CheckpointLineage> {
        self.checkpoint_lineage.as_ref()
    }

    pub fn identity(&self) -> WorkloadManifestIdentity {
        self.identity.clone()
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        self.boot.to_boot_image()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadManifestBuilder {
    id: WorkloadId,
    boot: WorkloadBootImage,
    topology: Option<WorkloadTopology>,
    resources: BTreeMap<WorkloadResourceId, WorkloadResource>,
    required_resources: BTreeSet<WorkloadResourceId>,
    host_events: Vec<WorkloadHostEvent>,
    checkpoint_lineage: Option<CheckpointLineage>,
}

impl WorkloadManifestBuilder {
    fn new(id: WorkloadId, boot: WorkloadBootImage) -> Self {
        Self {
            id,
            boot,
            topology: None,
            resources: BTreeMap::new(),
            required_resources: BTreeSet::new(),
            host_events: Vec::new(),
            checkpoint_lineage: None,
        }
    }

    pub fn add_resource(mut self, resource: WorkloadResource) -> Result<Self, WorkloadError> {
        let id = resource.id().clone();
        if self.resources.contains_key(&id) {
            return Err(WorkloadError::DuplicateResource { resource: id });
        }
        self.resources.insert(id, resource);
        Ok(self)
    }

    pub fn add_required_resource(mut self, resource: WorkloadResourceId) -> Self {
        self.required_resources.insert(resource);
        self
    }

    pub fn add_host_event(mut self, event: WorkloadHostEvent) -> Self {
        self.host_events.push(event);
        self
    }

    pub fn with_topology(mut self, topology: WorkloadTopology) -> Self {
        self.topology = Some(topology);
        self
    }

    pub fn with_checkpoint_lineage(mut self, lineage: CheckpointLineage) -> Self {
        self.checkpoint_lineage = Some(lineage);
        self
    }

    pub fn build(mut self) -> Result<WorkloadManifest, WorkloadError> {
        for resource in &self.required_resources {
            if !self.resources.contains_key(resource) {
                return Err(WorkloadError::MissingRequiredResource {
                    resource: resource.clone(),
                });
            }
        }

        self.host_events.sort_by_key(host_event_sort_key);
        let resources = self.resources.into_values().collect::<Vec<_>>();
        let required_resources = self.required_resources.into_iter().collect::<Vec<_>>();
        let identity = manifest_identity(
            &self.id,
            &self.boot,
            self.topology.as_ref(),
            &resources,
            &required_resources,
            &self.host_events,
            self.checkpoint_lineage.as_ref(),
        );

        Ok(WorkloadManifest {
            id: self.id,
            boot: self.boot,
            topology: self.topology,
            resources,
            required_resources,
            host_events: self.host_events,
            checkpoint_lineage: self.checkpoint_lineage,
            identity,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadReplayPlan {
    manifest_identity: WorkloadManifestIdentity,
    boot: WorkloadBootImage,
    topology: Option<WorkloadTopology>,
    required_resources: Vec<WorkloadResource>,
    host_events: Vec<WorkloadHostEvent>,
    planned_checkpoint_labels: Vec<String>,
    planned_stop_reason: Option<String>,
    checkpoint_lineage: Option<CheckpointLineage>,
}

impl WorkloadReplayPlan {
    pub fn from_manifest(manifest: &WorkloadManifest) -> Result<Self, WorkloadError> {
        let host_events = manifest.host_events().to_vec();
        Ok(Self {
            manifest_identity: manifest.identity(),
            boot: manifest.boot().clone(),
            topology: manifest.topology().cloned(),
            required_resources: manifest.required_resource_details()?,
            planned_checkpoint_labels: planned_checkpoint_labels(&host_events),
            planned_stop_reason: planned_stop_reason(&host_events),
            host_events,
            checkpoint_lineage: manifest.checkpoint_lineage().cloned(),
        })
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn boot(&self) -> &WorkloadBootImage {
        &self.boot
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        self.boot.to_boot_image()
    }

    pub fn topology(&self) -> Option<&WorkloadTopology> {
        self.topology.as_ref()
    }

    pub fn required_resources(&self) -> &[WorkloadResource] {
        &self.required_resources
    }

    pub fn host_events(&self) -> &[WorkloadHostEvent] {
        &self.host_events
    }

    pub fn planned_checkpoint_labels(&self) -> &[String] {
        &self.planned_checkpoint_labels
    }

    pub fn planned_stop_reason(&self) -> Option<&str> {
        self.planned_stop_reason.as_deref()
    }

    pub fn checkpoint_lineage(&self) -> Option<&CheckpointLineage> {
        self.checkpoint_lineage.as_ref()
    }

    pub fn verify_result(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        if result.manifest_identity != self.manifest_identity {
            return Err(WorkloadError::ManifestIdentityMismatch {
                expected: self.manifest_identity.clone(),
                actual: result.manifest_identity.clone(),
            });
        }

        result.verify_stats_timing()?;
        self.verify_all_planned_events_reached(result.final_tick())?;
        self.verify_checkpoint_labels(result)?;
        self.verify_stop_reason(result)?;
        Ok(())
    }

    fn verify_all_planned_events_reached(&self, final_tick: Tick) -> Result<(), WorkloadError> {
        if let Some(event) = self
            .host_events
            .iter()
            .find(|event| event.tick() > final_tick)
        {
            return Err(WorkloadError::PlannedHostEventAfterFinalTick {
                event_tick: event.tick(),
                final_tick,
            });
        }

        Ok(())
    }

    fn verify_checkpoint_labels(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        for label in result.checkpoint_labels() {
            if !self
                .planned_checkpoint_labels
                .iter()
                .any(|planned| planned == label)
            {
                return Err(WorkloadError::UnexpectedCheckpointLabel {
                    label: label.clone(),
                });
            }
        }

        for label in &self.planned_checkpoint_labels {
            if !result
                .checkpoint_labels()
                .iter()
                .any(|actual| actual == label)
            {
                return Err(WorkloadError::MissingCheckpointLabel {
                    label: label.clone(),
                });
            }
        }

        Ok(())
    }

    fn verify_stop_reason(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        let Some(expected) = &self.planned_stop_reason else {
            return match result.stop_reason() {
                Some(actual) => Err(WorkloadError::UnexpectedStopReason {
                    actual: actual.to_string(),
                }),
                None => Ok(()),
            };
        };

        if result.stop_reason() == Some(expected.as_str()) {
            return Ok(());
        }

        Err(WorkloadError::StopReasonMismatch {
            expected: expected.clone(),
            actual: result.stop_reason().map(str::to_string),
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResult {
    manifest_identity: WorkloadManifestIdentity,
    final_tick: Tick,
    stop_reason: Option<String>,
    stats_snapshot: Option<StatSnapshot>,
    parallel_execution_summary: Option<WorkloadParallelExecutionSummary>,
    checkpoint_labels: Vec<String>,
}

impl WorkloadResult {
    pub const fn new(manifest_identity: WorkloadManifestIdentity, final_tick: Tick) -> Self {
        Self {
            manifest_identity,
            final_tick,
            stop_reason: None,
            stats_snapshot: None,
            parallel_execution_summary: None,
            checkpoint_labels: Vec::new(),
        }
    }

    pub fn with_stop_reason(mut self, reason: impl Into<String>) -> Self {
        self.stop_reason = Some(reason.into());
        self
    }

    pub fn with_stats_snapshot(mut self, snapshot: StatSnapshot) -> Self {
        self.stats_snapshot = Some(snapshot);
        self
    }

    pub fn with_parallel_execution_summary(
        mut self,
        summary: WorkloadParallelExecutionSummary,
    ) -> Self {
        self.parallel_execution_summary = Some(summary);
        self
    }

    pub fn with_checkpoint_label(mut self, label: impl Into<String>) -> Self {
        self.checkpoint_labels.push(label.into());
        self
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn final_tick(&self) -> Tick {
        self.final_tick
    }

    pub fn stop_reason(&self) -> Option<&str> {
        self.stop_reason.as_deref()
    }

    pub const fn stats_snapshot(&self) -> Option<&StatSnapshot> {
        self.stats_snapshot.as_ref()
    }

    pub const fn parallel_execution_summary(&self) -> Option<&WorkloadParallelExecutionSummary> {
        self.parallel_execution_summary.as_ref()
    }

    pub fn checkpoint_labels(&self) -> &[String] {
        &self.checkpoint_labels
    }

    pub fn verify_manifest(&self, manifest: &WorkloadManifest) -> Result<(), WorkloadError> {
        let expected = manifest.identity();
        if self.manifest_identity != expected {
            return Err(WorkloadError::ManifestIdentityMismatch {
                expected,
                actual: self.manifest_identity.clone(),
            });
        }

        self.verify_stats_timing()
    }

    fn verify_stats_timing(&self) -> Result<(), WorkloadError> {
        let Some(snapshot) = &self.stats_snapshot else {
            return Ok(());
        };

        if snapshot.tick() <= self.final_tick {
            return Ok(());
        }

        Err(WorkloadError::StatsAfterFinalTick {
            stats_tick: snapshot.tick(),
            final_tick: self.final_tick,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadError {
    Boot(BootError),
    EmptyWorkloadId,
    EmptyResourceId,
    EmptyRouteId,
    EmptyEndpoint,
    EmptyResourceDigest {
        resource: WorkloadResourceId,
    },
    EmptyResourceLocator {
        resource: WorkloadResourceId,
    },
    DuplicateResource {
        resource: WorkloadResourceId,
    },
    MissingRequiredResource {
        resource: WorkloadResourceId,
    },
    ZeroHostLatency,
    ZeroLineBytes {
        target: u32,
    },
    MemoryProfileTargetMismatch {
        target: u32,
        profile_target: u32,
    },
    MemoryProfileLineSizeMismatch {
        target: u32,
        line_bytes: u64,
        profile_line_bytes: u64,
    },
    MemoryProfileGeometryLineSizeMismatch {
        target: u32,
        layout_line_bytes: u64,
        geometry_line_bytes: u64,
    },
    ZeroRouteLatency {
        route: WorkloadRouteId,
        latency: WorkloadRouteLatency,
    },
    ZeroTopologyPartitions,
    ZeroMinRemoteDelay,
    ZeroParallelWorkerLimit,
    PartitionOutOfRange {
        partition: u32,
        partition_count: u32,
    },
    DuplicateMemoryTarget {
        target: u32,
    },
    MissingMemoryTarget {
        target: u32,
    },
    DuplicateRoute {
        route: WorkloadRouteId,
    },
    DuplicateRiscvCore {
        cpu: u32,
    },
    MissingCoreFetchRoute {
        cpu: u32,
        route: WorkloadRouteId,
    },
    MissingCoreDataRoute {
        cpu: u32,
        route: WorkloadRouteId,
    },
    ZeroGpuComputeUnits {
        device: u32,
    },
    ZeroGpuWaveSlots {
        device: u32,
    },
    DuplicateGpuDevice {
        device: u32,
    },
    MissingGpuCommandRoute {
        device: u32,
        route: WorkloadRouteId,
    },
    GpuCommandRouteTargetMismatch {
        device: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    MissingGpuDevice {
        device: u32,
    },
    ZeroGpuKernelWorkgroups {
        device: u32,
        kernel: u64,
    },
    ZeroGpuKernelLatency {
        device: u32,
        kernel: u64,
    },
    ZeroAcceleratorLanes {
        engine: u32,
    },
    DuplicateAcceleratorDevice {
        engine: u32,
    },
    MissingAcceleratorCommandRoute {
        engine: u32,
        route: WorkloadRouteId,
    },
    AcceleratorCommandRouteTargetMismatch {
        engine: u32,
        route: WorkloadRouteId,
        expected: u32,
        actual: u32,
    },
    MissingAcceleratorDevice {
        engine: u32,
    },
    ZeroAcceleratorExecutionLatency {
        engine: u32,
        command: u64,
    },
    ZeroAcceleratorGpuWorkgroups {
        engine: u32,
        command: u64,
    },
    ZeroAcceleratorNpuTiles {
        engine: u32,
        command: u64,
    },
    ZeroAcceleratorDmaBytes {
        engine: u32,
        command: u64,
    },
    ManifestIdentityMismatch {
        expected: WorkloadManifestIdentity,
        actual: WorkloadManifestIdentity,
    },
    StatsAfterFinalTick {
        stats_tick: Tick,
        final_tick: Tick,
    },
    PlannedHostEventAfterFinalTick {
        event_tick: Tick,
        final_tick: Tick,
    },
    MissingCheckpointLabel {
        label: String,
    },
    UnexpectedCheckpointLabel {
        label: String,
    },
    StopReasonMismatch {
        expected: String,
        actual: Option<String>,
    },
    UnexpectedStopReason {
        actual: String,
    },
}

impl fmt::Display for WorkloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Boot(error) => write!(formatter, "{error}"),
            Self::EmptyWorkloadId => write!(formatter, "workload id must not be empty"),
            Self::EmptyResourceId => write!(formatter, "resource id must not be empty"),
            Self::EmptyRouteId => write!(formatter, "route id must not be empty"),
            Self::EmptyEndpoint => write!(formatter, "endpoint id must not be empty"),
            Self::EmptyResourceDigest { resource } => write!(
                formatter,
                "resource {} must include a digest",
                resource.as_str()
            ),
            Self::EmptyResourceLocator { resource } => write!(
                formatter,
                "resource {} must include a locator",
                resource.as_str()
            ),
            Self::DuplicateResource { resource } => {
                write!(
                    formatter,
                    "resource {} is already defined",
                    resource.as_str()
                )
            }
            Self::MissingRequiredResource { resource } => write!(
                formatter,
                "required resource {} is not defined",
                resource.as_str()
            ),
            Self::ZeroHostLatency => write!(formatter, "host latency must be positive"),
            Self::ZeroLineBytes { target } => {
                write!(
                    formatter,
                    "memory target {target} line bytes must be positive"
                )
            }
            Self::MemoryProfileTargetMismatch {
                target,
                profile_target,
            } => write!(
                formatter,
                "memory target {target} cannot use external memory profile for target {profile_target}"
            ),
            Self::MemoryProfileLineSizeMismatch {
                target,
                line_bytes,
                profile_line_bytes,
            } => write!(
                formatter,
                "memory target {target} has {line_bytes}-byte lines but external memory profile uses {profile_line_bytes}"
            ),
            Self::MemoryProfileGeometryLineSizeMismatch {
                target,
                layout_line_bytes,
                geometry_line_bytes,
            } => write!(
                formatter,
                "memory target {target} external memory profile has {layout_line_bytes}-byte layout lines but {geometry_line_bytes}-byte DRAM geometry lines"
            ),
            Self::ZeroRouteLatency { route, latency } => write!(
                formatter,
                "route {} {latency:?} latency must be positive",
                route.as_str()
            ),
            Self::ZeroTopologyPartitions => {
                write!(formatter, "topology partition count must be positive")
            }
            Self::ZeroMinRemoteDelay => write!(formatter, "minimum remote delay must be positive"),
            Self::ZeroParallelWorkerLimit => {
                write!(formatter, "parallel worker limit must be positive")
            }
            Self::PartitionOutOfRange {
                partition,
                partition_count,
            } => write!(
                formatter,
                "partition {partition} is outside topology partition count {partition_count}"
            ),
            Self::DuplicateMemoryTarget { target } => {
                write!(formatter, "memory target {target} is already defined")
            }
            Self::MissingMemoryTarget { target } => {
                write!(formatter, "memory target {target} is not defined")
            }
            Self::DuplicateRoute { route } => {
                write!(formatter, "route {} is already defined", route.as_str())
            }
            Self::DuplicateRiscvCore { cpu } => {
                write!(formatter, "RISC-V core {cpu} is already defined")
            }
            Self::MissingCoreFetchRoute { cpu, route } => write!(
                formatter,
                "RISC-V core {cpu} fetch route {} is not defined",
                route.as_str()
            ),
            Self::MissingCoreDataRoute { cpu, route } => write!(
                formatter,
                "RISC-V core {cpu} data route {} is not defined",
                route.as_str()
            ),
            Self::ZeroGpuComputeUnits { device } => {
                write!(formatter, "GPU device {device} needs at least one compute unit")
            }
            Self::ZeroGpuWaveSlots { device } => write!(
                formatter,
                "GPU device {device} needs at least one wave slot per compute unit"
            ),
            Self::DuplicateGpuDevice { device } => {
                write!(formatter, "GPU device {device} is already defined")
            }
            Self::MissingGpuCommandRoute { device, route } => write!(
                formatter,
                "GPU device {device} command route {} is not defined",
                route.as_str()
            ),
            Self::GpuCommandRouteTargetMismatch {
                device,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "GPU device {device} command route {} targets partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::MissingGpuDevice { device } => {
                write!(formatter, "GPU device {device} is not defined")
            }
            Self::ZeroGpuKernelWorkgroups { device, kernel } => write!(
                formatter,
                "GPU kernel {kernel} on device {device} needs at least one workgroup"
            ),
            Self::ZeroGpuKernelLatency { device, kernel } => write!(
                formatter,
                "GPU kernel {kernel} on device {device} needs positive workgroup latency"
            ),
            Self::ZeroAcceleratorLanes { engine } => {
                write!(
                    formatter,
                    "accelerator engine {engine} needs at least one lane"
                )
            }
            Self::DuplicateAcceleratorDevice { engine } => {
                write!(formatter, "accelerator engine {engine} is already defined")
            }
            Self::MissingAcceleratorCommandRoute { engine, route } => write!(
                formatter,
                "accelerator engine {engine} command route {} is not defined",
                route.as_str()
            ),
            Self::AcceleratorCommandRouteTargetMismatch {
                engine,
                route,
                expected,
                actual,
            } => write!(
                formatter,
                "accelerator engine {engine} command route {} targets partition {actual}, expected {expected}",
                route.as_str()
            ),
            Self::MissingAcceleratorDevice { engine } => {
                write!(formatter, "accelerator engine {engine} is not defined")
            }
            Self::ZeroAcceleratorExecutionLatency { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs positive execution latency"
            ),
            Self::ZeroAcceleratorGpuWorkgroups { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs at least one GPU workgroup"
            ),
            Self::ZeroAcceleratorNpuTiles { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs at least one NPU tile"
            ),
            Self::ZeroAcceleratorDmaBytes { engine, command } => write!(
                formatter,
                "accelerator command {command} on engine {engine} needs at least one DMA byte"
            ),
            Self::ManifestIdentityMismatch { expected, actual } => write!(
                formatter,
                "workload result belongs to manifest {}, expected {}",
                actual.as_str(),
                expected.as_str()
            ),
            Self::StatsAfterFinalTick {
                stats_tick,
                final_tick,
            } => write!(
                formatter,
                "stats snapshot tick {stats_tick} is after final tick {final_tick}"
            ),
            Self::PlannedHostEventAfterFinalTick {
                event_tick,
                final_tick,
            } => write!(
                formatter,
                "planned host event at tick {event_tick} is after final tick {final_tick}"
            ),
            Self::MissingCheckpointLabel { label } => {
                write!(
                    formatter,
                    "planned checkpoint label {label} was not recorded"
                )
            }
            Self::UnexpectedCheckpointLabel { label } => {
                write!(formatter, "checkpoint label {label} was not planned")
            }
            Self::StopReasonMismatch { expected, actual } => match actual {
                Some(actual) => write!(
                    formatter,
                    "stop reason {actual} does not match planned reason {expected}"
                ),
                None => write!(formatter, "missing planned stop reason {expected}"),
            },
            Self::UnexpectedStopReason { actual } => {
                write!(formatter, "stop reason {actual} was not planned")
            }
        }
    }
}

impl Error for WorkloadError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Boot(error) => Some(error),
            _ => None,
        }
    }
}

fn host_event_sort_key(event: &WorkloadHostEvent) -> (Tick, u8, String) {
    let (rank, label) = match event.intent() {
        HostEventIntent::RoiBegin { label } => (0, label.as_str()),
        HostEventIntent::RoiEnd { label } => (1, label.as_str()),
        HostEventIntent::StatsReset { label } => (2, label.as_str()),
        HostEventIntent::StatsDump { label } => (3, label.as_str()),
        HostEventIntent::Checkpoint { label } => (4, label.as_str()),
        HostEventIntent::Stop { reason } => (5, reason.as_str()),
    };
    (event.tick(), rank, label.to_string())
}

fn planned_checkpoint_labels(events: &[WorkloadHostEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event.intent() {
            HostEventIntent::Checkpoint { label } => Some(label.clone()),
            _ => None,
        })
        .collect()
}

fn planned_stop_reason(events: &[WorkloadHostEvent]) -> Option<String> {
    events.iter().find_map(|event| match event.intent() {
        HostEventIntent::Stop { reason } => Some(reason.clone()),
        _ => None,
    })
}
