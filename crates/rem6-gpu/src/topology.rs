use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, Tick};
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_transport::{MemoryRouteId, MemoryTransport, TopologyRouteError};

use crate::{GpuComputeConfig, GpuDevice, GpuError, GpuKernelLaunch};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuTopologyConfig {
    compute: GpuComputeConfig,
    command_source: Endpoint,
    command_target: Endpoint,
    memory: Option<GpuMemoryConfig>,
}

impl GpuTopologyConfig {
    pub fn new(
        compute: GpuComputeConfig,
        command_source: Endpoint,
        command_target: Endpoint,
    ) -> Self {
        Self {
            compute,
            command_source,
            command_target,
            memory: None,
        }
    }

    pub fn with_memory(mut self, source: Endpoint, target: Endpoint) -> Self {
        self.memory = Some(GpuMemoryConfig::new(source, target));
        self
    }

    pub const fn compute(&self) -> &GpuComputeConfig {
        &self.compute
    }

    pub const fn command_source(&self) -> &Endpoint {
        &self.command_source
    }

    pub const fn command_target(&self) -> &Endpoint {
        &self.command_target
    }

    pub const fn memory(&self) -> Option<&GpuMemoryConfig> {
        self.memory.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuMemoryConfig {
    source: Endpoint,
    target: Endpoint,
}

impl GpuMemoryConfig {
    pub fn new(source: Endpoint, target: Endpoint) -> Self {
        Self { source, target }
    }

    pub const fn source(&self) -> &Endpoint {
        &self.source
    }

    pub const fn target(&self) -> &Endpoint {
        &self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuCommandPath {
    source: Endpoint,
    target: Endpoint,
    source_partition: PartitionId,
    target_partition: PartitionId,
    latency: Tick,
}

impl GpuCommandPath {
    fn new(
        source: Endpoint,
        target: Endpoint,
        source_partition: PartitionId,
        target_partition: PartitionId,
        latency: Tick,
    ) -> Self {
        Self {
            source,
            target,
            source_partition,
            target_partition,
            latency,
        }
    }

    pub const fn source(&self) -> &Endpoint {
        &self.source
    }

    pub const fn target(&self) -> &Endpoint {
        &self.target
    }

    pub const fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub const fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub const fn latency(&self) -> Tick {
        self.latency
    }
}

#[derive(Clone, Debug)]
pub struct GpuTopologyDevice {
    gpu: GpuDevice,
    command_path: GpuCommandPath,
    memory_route: Option<MemoryRouteId>,
}

impl GpuTopologyDevice {
    pub fn from_topology(
        topology: &Topology,
        transport: &mut MemoryTransport,
        config: GpuTopologyConfig,
    ) -> Result<Self, GpuError> {
        let command_path = build_command_path(
            topology,
            config.command_source(),
            config.command_target(),
            config.compute().partition(),
        )?;
        let memory_route = config
            .memory()
            .map(|memory| {
                validate_source_partition(topology, memory.source(), config.compute().partition())?;
                transport
                    .add_topology_route(topology, memory.source().clone(), memory.target().clone())
                    .map_err(GpuError::TopologyRoute)
            })
            .transpose()?;

        Ok(Self {
            gpu: GpuDevice::new(config.compute().clone()),
            command_path,
            memory_route,
        })
    }

    pub const fn gpu(&self) -> &GpuDevice {
        &self.gpu
    }

    pub const fn command_path(&self) -> &GpuCommandPath {
        &self.command_path
    }

    pub const fn memory_route(&self) -> Option<MemoryRouteId> {
        self.memory_route
    }

    pub fn submit_kernel(
        &self,
        scheduler: &mut PartitionedScheduler,
        launch: GpuKernelLaunch,
    ) -> Result<PartitionEventId, GpuError> {
        self.gpu.submit_kernel_from_partition(
            scheduler,
            self.command_path.source_partition(),
            self.command_path.latency(),
            launch,
        )
    }
}

fn build_command_path(
    topology: &Topology,
    source: &Endpoint,
    target: &Endpoint,
    expected_target: PartitionId,
) -> Result<GpuCommandPath, GpuError> {
    let source_partition = endpoint_partition(topology, source)?;
    let target_partition = endpoint_partition(topology, target)?;
    if target_partition != expected_target {
        return Err(GpuError::CommandTargetPartitionMismatch {
            endpoint: target.clone(),
            expected: expected_target,
            actual: target_partition,
        });
    }

    let path = topology.find_endpoint_path(source, target).ok_or_else(|| {
        GpuError::TopologyRoute(TopologyRouteError::MissingTopologyConnection {
            from: source.clone(),
            to: target.clone(),
        })
    })?;

    Ok(GpuCommandPath::new(
        source.clone(),
        target.clone(),
        source_partition,
        target_partition,
        path.request_latency(),
    ))
}

fn validate_source_partition(
    topology: &Topology,
    endpoint: &Endpoint,
    expected: PartitionId,
) -> Result<(), GpuError> {
    let actual = endpoint_partition(topology, endpoint)?;
    if actual != expected {
        return Err(GpuError::MemorySourcePartitionMismatch {
            endpoint: endpoint.clone(),
            expected,
            actual,
        });
    }

    Ok(())
}

fn endpoint_partition(topology: &Topology, endpoint: &Endpoint) -> Result<PartitionId, GpuError> {
    let component = topology.component(endpoint.component()).ok_or_else(|| {
        GpuError::Topology(TopologyError::UnknownComponent {
            component: endpoint.component().clone(),
        })
    })?;
    component.port_direction(endpoint.port()).ok_or_else(|| {
        GpuError::Topology(TopologyError::UnknownPort {
            component: endpoint.component().clone(),
            port: endpoint.port().clone(),
        })
    })?;
    Ok(component.partition())
}
