use rem6_kernel::{PartitionEventId, PartitionId, PartitionedScheduler, Tick};
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_transport::{MemoryRouteId, MemoryTransport, TopologyRouteError};

use crate::{AcceleratorCommand, AcceleratorEngine, AcceleratorEngineConfig, AcceleratorError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorTopologyConfig {
    engine: AcceleratorEngineConfig,
    dma_source: Endpoint,
    dma_target: Endpoint,
    command_submission: Option<AcceleratorCommandSubmissionConfig>,
}

impl AcceleratorTopologyConfig {
    pub const fn new(
        engine: AcceleratorEngineConfig,
        dma_source: Endpoint,
        dma_target: Endpoint,
    ) -> Self {
        Self {
            engine,
            dma_source,
            dma_target,
            command_submission: None,
        }
    }

    pub fn with_command_submission(mut self, source: Endpoint, target: Endpoint) -> Self {
        self.command_submission = Some(AcceleratorCommandSubmissionConfig::new(source, target));
        self
    }

    pub const fn engine(&self) -> &AcceleratorEngineConfig {
        &self.engine
    }

    pub const fn dma_source(&self) -> &Endpoint {
        &self.dma_source
    }

    pub const fn dma_target(&self) -> &Endpoint {
        &self.dma_target
    }

    pub const fn command_submission(&self) -> Option<&AcceleratorCommandSubmissionConfig> {
        self.command_submission.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorCommandSubmissionConfig {
    source: Endpoint,
    target: Endpoint,
}

impl AcceleratorCommandSubmissionConfig {
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
pub struct AcceleratorCommandPath {
    source: Endpoint,
    target: Endpoint,
    source_partition: PartitionId,
    target_partition: PartitionId,
    latency: Tick,
}

impl AcceleratorCommandPath {
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
pub struct AcceleratorTopologyDevice {
    engine: AcceleratorEngine,
    dma_route: MemoryRouteId,
    command_path: Option<AcceleratorCommandPath>,
}

impl AcceleratorTopologyDevice {
    pub fn from_topology(
        topology: &Topology,
        transport: &mut MemoryTransport,
        config: AcceleratorTopologyConfig,
    ) -> Result<Self, AcceleratorError> {
        validate_source_partition(topology, config.dma_source(), config.engine().partition())?;
        let command_path = config
            .command_submission()
            .map(|command| build_command_path(topology, command, config.engine().partition()))
            .transpose()?;
        let dma_route = transport
            .add_topology_route(
                topology,
                config.dma_source().clone(),
                config.dma_target().clone(),
            )
            .map_err(AcceleratorError::TopologyRoute)?;

        Ok(Self {
            engine: AcceleratorEngine::new(config.engine().clone()),
            dma_route,
            command_path,
        })
    }

    pub const fn engine(&self) -> &AcceleratorEngine {
        &self.engine
    }

    pub const fn dma_route(&self) -> MemoryRouteId {
        self.dma_route
    }

    pub const fn command_path(&self) -> Option<&AcceleratorCommandPath> {
        self.command_path.as_ref()
    }

    pub fn submit_command(
        &self,
        scheduler: &mut PartitionedScheduler,
        command: AcceleratorCommand,
    ) -> Result<PartitionEventId, AcceleratorError> {
        let path = self
            .command_path()
            .ok_or(AcceleratorError::MissingCommandSubmission {
                engine: self.engine.id(),
            })?;
        self.engine.submit_from_partition(
            scheduler,
            path.source_partition(),
            path.latency(),
            command,
        )
    }
}

fn build_command_path(
    topology: &Topology,
    command: &AcceleratorCommandSubmissionConfig,
    expected_target: PartitionId,
) -> Result<AcceleratorCommandPath, AcceleratorError> {
    let source_partition = endpoint_partition(topology, command.source())?;
    let target_partition = endpoint_partition(topology, command.target())?;
    if target_partition != expected_target {
        return Err(AcceleratorError::CommandTargetPartitionMismatch {
            endpoint: command.target().clone(),
            expected: expected_target,
            actual: target_partition,
        });
    }

    let path = topology
        .find_endpoint_path(command.source(), command.target())
        .ok_or_else(|| {
            AcceleratorError::TopologyRoute(TopologyRouteError::MissingTopologyConnection {
                from: command.source().clone(),
                to: command.target().clone(),
            })
        })?;

    Ok(AcceleratorCommandPath::new(
        command.source().clone(),
        command.target().clone(),
        source_partition,
        target_partition,
        path.request_latency(),
    ))
}

fn validate_source_partition(
    topology: &Topology,
    endpoint: &Endpoint,
    expected: PartitionId,
) -> Result<(), AcceleratorError> {
    let actual = endpoint_partition(topology, endpoint)?;
    if actual != expected {
        return Err(AcceleratorError::SourcePartitionMismatch {
            endpoint: endpoint.clone(),
            expected,
            actual,
        });
    }

    Ok(())
}

fn endpoint_partition(
    topology: &Topology,
    endpoint: &Endpoint,
) -> Result<PartitionId, AcceleratorError> {
    let component = topology.component(endpoint.component()).ok_or_else(|| {
        AcceleratorError::Topology(TopologyError::UnknownComponent {
            component: endpoint.component().clone(),
        })
    })?;
    component.port_direction(endpoint.port()).ok_or_else(|| {
        AcceleratorError::Topology(TopologyError::UnknownPort {
            component: endpoint.component().clone(),
            port: endpoint.port().clone(),
        })
    })?;
    Ok(component.partition())
}

impl From<TopologyRouteError> for AcceleratorError {
    fn from(error: TopologyRouteError) -> Self {
        Self::TopologyRoute(error)
    }
}
