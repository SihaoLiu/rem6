use rem6_dram::ExternalMemoryProfile;
use rem6_kernel::Tick;
use rem6_memory::{Address, AddressRange};

use crate::{
    WorkloadAcceleratorCommand, WorkloadAcceleratorDevice, WorkloadAcceleratorDmaCopy,
    WorkloadDataCacheProtocol, WorkloadError, WorkloadGpuDevice, WorkloadGpuDmaCopy,
    WorkloadGpuKernelLaunch, WorkloadQosPolicy,
};

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
pub struct WorkloadRouteFabric {
    link: String,
    bandwidth_bytes_per_tick: u64,
    request_virtual_network: u16,
    response_virtual_network: u16,
    credit_depth: Option<u32>,
}

impl WorkloadRouteFabric {
    pub fn new(
        link: impl Into<String>,
        bandwidth_bytes_per_tick: u64,
    ) -> Result<Self, WorkloadError> {
        let link = link.into();
        if link.is_empty() {
            return Err(WorkloadError::EmptyFabricLink);
        }
        if bandwidth_bytes_per_tick == 0 {
            return Err(WorkloadError::ZeroFabricBandwidth { link });
        }

        Ok(Self {
            link,
            bandwidth_bytes_per_tick,
            request_virtual_network: 0,
            response_virtual_network: 0,
            credit_depth: None,
        })
    }

    pub const fn with_virtual_networks(
        mut self,
        request_virtual_network: u16,
        response_virtual_network: u16,
    ) -> Self {
        self.request_virtual_network = request_virtual_network;
        self.response_virtual_network = response_virtual_network;
        self
    }

    pub fn with_credit_depth(mut self, credit_depth: u32) -> Result<Self, WorkloadError> {
        if credit_depth == 0 {
            return Err(WorkloadError::ZeroFabricCreditDepth {
                link: self.link.clone(),
            });
        }

        self.credit_depth = Some(credit_depth);
        Ok(self)
    }

    pub fn link(&self) -> &str {
        &self.link
    }

    pub const fn bandwidth_bytes_per_tick(&self) -> u64 {
        self.bandwidth_bytes_per_tick
    }

    pub const fn request_virtual_network(&self) -> u16 {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> u16 {
        self.response_virtual_network
    }

    pub const fn credit_depth(&self) -> Option<u32> {
        self.credit_depth
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadRouteHop {
    endpoint: String,
    partition: u32,
    request_latency: Tick,
    response_latency: Tick,
    fabric: Option<WorkloadRouteFabric>,
}

impl WorkloadRouteHop {
    pub fn new(
        endpoint: impl Into<String>,
        partition: u32,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Result<Self, WorkloadError> {
        let endpoint = endpoint.into();
        if endpoint.is_empty() {
            return Err(WorkloadError::EmptyEndpoint);
        }
        if request_latency == 0 {
            return Err(WorkloadError::ZeroRouteHopLatency {
                endpoint,
                latency: WorkloadRouteLatency::Request,
            });
        }
        if response_latency == 0 {
            return Err(WorkloadError::ZeroRouteHopLatency {
                endpoint,
                latency: WorkloadRouteLatency::Response,
            });
        }

        Ok(Self {
            endpoint,
            partition,
            request_latency,
            response_latency,
            fabric: None,
        })
    }

    pub fn with_fabric(mut self, fabric: WorkloadRouteFabric) -> Self {
        self.fabric = Some(fabric);
        self
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    pub const fn partition(&self) -> u32 {
        self.partition
    }

    pub const fn request_latency(&self) -> Tick {
        self.request_latency
    }

    pub const fn response_latency(&self) -> Tick {
        self.response_latency
    }

    pub fn fabric(&self) -> Option<&WorkloadRouteFabric> {
        self.fabric.as_ref()
    }
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
    hops: Vec<WorkloadRouteHop>,
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
            target_endpoint: target_endpoint.clone(),
            target_partition,
            request_latency,
            response_latency,
            hops: vec![WorkloadRouteHop {
                endpoint: target_endpoint,
                partition: target_partition,
                request_latency,
                response_latency,
                fabric: None,
            }],
        })
    }

    pub fn new_path<I>(
        id: WorkloadRouteId,
        source_endpoint: impl Into<String>,
        source_partition: u32,
        hops: I,
    ) -> Result<Self, WorkloadError>
    where
        I: IntoIterator<Item = WorkloadRouteHop>,
    {
        let source_endpoint = source_endpoint.into();
        if source_endpoint.is_empty() {
            return Err(WorkloadError::EmptyEndpoint);
        }

        let hops: Vec<_> = hops.into_iter().collect();
        let Some(last) = hops.last() else {
            return Err(WorkloadError::EmptyMemoryRoutePath { route: id });
        };
        let target_endpoint = last.endpoint().to_string();
        let target_partition = last.partition();
        let request_latency = hops.iter().map(WorkloadRouteHop::request_latency).sum();
        let response_latency = hops.iter().map(WorkloadRouteHop::response_latency).sum();

        Ok(Self {
            id,
            source_endpoint,
            source_partition,
            target_endpoint,
            target_partition,
            request_latency,
            response_latency,
            hops,
        })
    }

    pub fn with_fabric(mut self, fabric: WorkloadRouteFabric) -> Self {
        if let Some(hop) = self.hops.first_mut() {
            hop.fabric = Some(fabric);
        }
        self
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

    pub fn hops(&self) -> &[WorkloadRouteHop] {
        &self.hops
    }

    pub fn fabric(&self) -> Option<&WorkloadRouteFabric> {
        self.hops.first().and_then(WorkloadRouteHop::fabric)
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
    backing_route: WorkloadRouteId,
}

impl WorkloadRiscvDataCache {
    pub fn new(
        protocol: WorkloadDataCacheProtocol,
        memory_target: u32,
        line_address: Address,
        directory_partition: u32,
        directory_endpoint: impl Into<String>,
        backing_route: WorkloadRouteId,
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
            backing_route,
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

    pub fn backing_route(&self) -> &WorkloadRouteId {
        &self.backing_route
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
    gpu_dma_copies: Vec<WorkloadGpuDmaCopy>,
    accelerator_devices: Vec<WorkloadAcceleratorDevice>,
    accelerator_commands: Vec<WorkloadAcceleratorCommand>,
    accelerator_dma_copies: Vec<WorkloadAcceleratorDmaCopy>,
    qos_policy: Option<WorkloadQosPolicy>,
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
            gpu_dma_copies: Vec::new(),
            accelerator_devices: Vec::new(),
            accelerator_commands: Vec::new(),
            accelerator_dma_copies: Vec::new(),
            qos_policy: None,
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
        for hop in route.hops() {
            self.validate_partition(hop.partition())?;
        }
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
        let fetch_route = self
            .memory_routes
            .iter()
            .find(|route| route.id() == core.fetch_route())
            .ok_or_else(|| WorkloadError::MissingCoreFetchRoute {
                cpu: core.cpu(),
                route: core.fetch_route().clone(),
            })?;
        if fetch_route.source_partition() != core.partition() {
            return Err(WorkloadError::CoreFetchRouteSourceMismatch {
                cpu: core.cpu(),
                route: core.fetch_route().clone(),
                expected: core.partition(),
                actual: fetch_route.source_partition(),
            });
        }
        if fetch_route.source_endpoint() != core.fetch_endpoint() {
            return Err(WorkloadError::CoreFetchRouteEndpointMismatch {
                cpu: core.cpu(),
                route: core.fetch_route().clone(),
                expected: core.fetch_endpoint().to_string(),
                actual: fetch_route.source_endpoint().to_string(),
            });
        }
        if let Some(route) = core.data_route() {
            let data_route = self
                .memory_routes
                .iter()
                .find(|existing| existing.id() == route)
                .ok_or_else(|| WorkloadError::MissingCoreDataRoute {
                    cpu: core.cpu(),
                    route: route.clone(),
                })?;
            if data_route.source_partition() != core.partition() {
                return Err(WorkloadError::CoreDataRouteSourceMismatch {
                    cpu: core.cpu(),
                    route: route.clone(),
                    expected: core.partition(),
                    actual: data_route.source_partition(),
                });
            }
            let data_endpoint = core
                .data_endpoint()
                .expect("data route implies a data endpoint");
            if data_route.source_endpoint() != data_endpoint {
                return Err(WorkloadError::CoreDataRouteEndpointMismatch {
                    cpu: core.cpu(),
                    route: route.clone(),
                    expected: data_endpoint.to_string(),
                    actual: data_route.source_endpoint().to_string(),
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
        if route.target_endpoint() != device.command_endpoint() {
            return Err(WorkloadError::GpuCommandRouteEndpointMismatch {
                device: device.device(),
                route: device.command_route().clone(),
                expected: device.command_endpoint().to_string(),
                actual: route.target_endpoint().to_string(),
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

    pub fn add_gpu_dma_copy(mut self, copy: WorkloadGpuDmaCopy) -> Result<Self, WorkloadError> {
        let device = self
            .gpu_devices
            .iter()
            .find(|device| device.device() == copy.device())
            .ok_or_else(|| WorkloadError::MissingGpuDevice {
                device: copy.device(),
            })?;
        let route = self
            .memory_routes
            .iter()
            .find(|route| route.id() == copy.route())
            .ok_or_else(|| WorkloadError::MissingGpuDmaRoute {
                device: copy.device(),
                route: copy.route().clone(),
            })?;
        if route.source_partition() != device.partition() {
            return Err(WorkloadError::GpuDmaRouteSourceMismatch {
                device: copy.device(),
                route: copy.route().clone(),
                expected: device.partition(),
                actual: route.source_partition(),
            });
        }
        if route.source_endpoint() != device.dma_endpoint() {
            return Err(WorkloadError::GpuDmaRouteEndpointMismatch {
                device: copy.device(),
                route: copy.route().clone(),
                expected: device.dma_endpoint().to_string(),
                actual: route.source_endpoint().to_string(),
            });
        }

        self.gpu_dma_copies.push(copy);
        self.gpu_dma_copies
            .sort_by_key(|copy| (copy.device(), copy.transfer()));
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
        if route.target_endpoint() != device.command_endpoint() {
            return Err(WorkloadError::AcceleratorCommandRouteEndpointMismatch {
                engine: device.engine(),
                route: device.command_route().clone(),
                expected: device.command_endpoint().to_string(),
                actual: route.target_endpoint().to_string(),
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

    pub fn add_accelerator_dma_copy(
        mut self,
        copy: WorkloadAcceleratorDmaCopy,
    ) -> Result<Self, WorkloadError> {
        let device = self
            .accelerator_devices
            .iter()
            .find(|device| device.engine() == copy.engine())
            .ok_or_else(|| WorkloadError::MissingAcceleratorDevice {
                engine: copy.engine(),
            })?;
        let route = self
            .memory_routes
            .iter()
            .find(|route| route.id() == copy.route())
            .ok_or_else(|| WorkloadError::MissingAcceleratorDmaRoute {
                engine: copy.engine(),
                route: copy.route().clone(),
            })?;
        if route.source_partition() != device.partition() {
            return Err(WorkloadError::AcceleratorDmaRouteSourceMismatch {
                engine: copy.engine(),
                route: copy.route().clone(),
                expected: device.partition(),
                actual: route.source_partition(),
            });
        }
        if route.source_endpoint() != device.dma_endpoint() {
            return Err(WorkloadError::AcceleratorDmaRouteEndpointMismatch {
                engine: copy.engine(),
                route: copy.route().clone(),
                expected: device.dma_endpoint().to_string(),
                actual: route.source_endpoint().to_string(),
            });
        }

        self.accelerator_dma_copies.push(copy);
        self.accelerator_dma_copies
            .sort_by_key(|copy| (copy.engine(), copy.transfer()));
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
        let route = self
            .memory_routes
            .iter()
            .find(|route| route.id() == cache.backing_route())
            .ok_or_else(|| WorkloadError::MissingDataCacheBackingRoute {
                route: cache.backing_route().clone(),
            })?;
        if route.source_partition() != cache.directory_partition() {
            return Err(WorkloadError::DataCacheBackingRouteSourceMismatch {
                route: cache.backing_route().clone(),
                expected: cache.directory_partition(),
                actual: route.source_partition(),
            });
        }
        if route.source_endpoint() != cache.directory_endpoint() {
            return Err(WorkloadError::DataCacheBackingRouteEndpointMismatch {
                route: cache.backing_route().clone(),
                expected: cache.directory_endpoint().to_string(),
                actual: route.source_endpoint().to_string(),
            });
        }

        self.riscv_data_cache = Some(cache);
        Ok(self)
    }

    pub fn with_qos_policy(mut self, policy: WorkloadQosPolicy) -> Self {
        self.qos_policy = Some(policy);
        self
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

    pub fn gpu_dma_copies(&self) -> &[WorkloadGpuDmaCopy] {
        &self.gpu_dma_copies
    }

    pub fn accelerator_devices(&self) -> &[WorkloadAcceleratorDevice] {
        &self.accelerator_devices
    }

    pub fn accelerator_commands(&self) -> &[WorkloadAcceleratorCommand] {
        &self.accelerator_commands
    }

    pub fn accelerator_dma_copies(&self) -> &[WorkloadAcceleratorDmaCopy] {
        &self.accelerator_dma_copies
    }

    pub fn qos_policy(&self) -> Option<&WorkloadQosPolicy> {
        self.qos_policy.as_ref()
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
