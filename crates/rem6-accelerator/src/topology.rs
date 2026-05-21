use rem6_kernel::PartitionId;
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_transport::{MemoryRouteId, MemoryTransport, TopologyRouteError};

use crate::{AcceleratorEngine, AcceleratorEngineConfig, AcceleratorError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AcceleratorTopologyConfig {
    engine: AcceleratorEngineConfig,
    dma_source: Endpoint,
    dma_target: Endpoint,
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
        }
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
}

#[derive(Clone, Debug)]
pub struct AcceleratorTopologyDevice {
    engine: AcceleratorEngine,
    dma_route: MemoryRouteId,
}

impl AcceleratorTopologyDevice {
    pub fn from_topology(
        topology: &Topology,
        transport: &mut MemoryTransport,
        config: AcceleratorTopologyConfig,
    ) -> Result<Self, AcceleratorError> {
        validate_source_partition(topology, config.dma_source(), config.engine().partition())?;
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
        })
    }

    pub const fn engine(&self) -> &AcceleratorEngine {
        &self.engine
    }

    pub const fn dma_route(&self) -> MemoryRouteId {
        self.dma_route
    }
}

fn validate_source_partition(
    topology: &Topology,
    endpoint: &Endpoint,
    expected: PartitionId,
) -> Result<(), AcceleratorError> {
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

    let actual = component.partition();
    if actual != expected {
        return Err(AcceleratorError::SourcePartitionMismatch {
            endpoint: endpoint.clone(),
            expected,
            actual,
        });
    }

    Ok(())
}

impl From<TopologyRouteError> for AcceleratorError {
    fn from(error: TopologyRouteError) -> Self {
        Self::TopologyRoute(error)
    }
}
