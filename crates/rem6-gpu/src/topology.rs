use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, Tick};
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_transport::TopologyRouteError;

use crate::{GpuComputeConfig, GpuDevice, GpuError, GpuKernelLaunch};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GpuTopologyConfig {
    compute: GpuComputeConfig,
    command_source: Endpoint,
    command_target: Endpoint,
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
        }
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
}

impl GpuTopologyDevice {
    pub fn from_topology(topology: &Topology, config: GpuTopologyConfig) -> Result<Self, GpuError> {
        let command_path = build_command_path(
            topology,
            config.command_source(),
            config.command_target(),
            config.compute().partition(),
        )?;

        Ok(Self {
            gpu: GpuDevice::new(config.compute().clone()),
            command_path,
        })
    }

    pub const fn gpu(&self) -> &GpuDevice {
        &self.gpu
    }

    pub const fn command_path(&self) -> &GpuCommandPath {
        &self.command_path
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
