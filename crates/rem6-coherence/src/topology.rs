use rem6_dram::DramMemoryController;
use rem6_fabric::VirtualNetworkId;
use rem6_memory::{Address, AgentId, CacheLineLayout};
use rem6_topology::{ComponentId, ComponentSpec, Endpoint, PortName, Topology, TopologyError};
use rem6_transport::TransportEndpointId;

use crate::{
    HarnessError, MesiHarnessError, PartitionedCacheAgentConfig, PartitionedDirectoryLineHarness,
    PartitionedDramMemoryConfig, PartitionedMesiDirectoryLineHarness, PartitionedRouteHopConfig,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyCacheAgentConfig {
    agent: AgentId,
    component: ComponentId,
    port: PortName,
}

impl TopologyCacheAgentConfig {
    pub fn new(agent: AgentId, component: ComponentId, port: PortName) -> Self {
        Self {
            agent,
            component,
            port,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn component(&self) -> &ComponentId {
        &self.component
    }

    pub const fn port(&self) -> &PortName {
        &self.port
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyDirectoryConfig {
    component: ComponentId,
    cache_port: PortName,
    memory_port: PortName,
}

impl TopologyDirectoryConfig {
    pub fn new(component: ComponentId, cache_port: PortName, memory_port: PortName) -> Self {
        Self {
            component,
            cache_port,
            memory_port,
        }
    }

    pub const fn component(&self) -> &ComponentId {
        &self.component
    }

    pub const fn cache_port(&self) -> &PortName {
        &self.cache_port
    }

    pub const fn memory_port(&self) -> &PortName {
        &self.memory_port
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyDramMemoryConfig {
    component: ComponentId,
    port: PortName,
    controller: DramMemoryController,
}

impl TopologyDramMemoryConfig {
    pub fn new(component: ComponentId, port: PortName, controller: DramMemoryController) -> Self {
        Self {
            component,
            port,
            controller,
        }
    }

    pub const fn component(&self) -> &ComponentId {
        &self.component
    }

    pub const fn port(&self) -> &PortName {
        &self.port
    }

    fn into_controller(self) -> DramMemoryController {
        self.controller
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyDirectoryHarnessConfig {
    layout: CacheLineLayout,
    line_address: Address,
    directory: TopologyDirectoryConfig,
    memory: TopologyDramMemoryConfig,
    caches: Vec<TopologyCacheAgentConfig>,
}

impl TopologyDirectoryHarnessConfig {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        directory: TopologyDirectoryConfig,
        memory: TopologyDramMemoryConfig,
        caches: I,
    ) -> Self
    where
        I: IntoIterator<Item = TopologyCacheAgentConfig>,
    {
        Self {
            layout,
            line_address,
            directory,
            memory,
            caches: caches.into_iter().collect(),
        }
    }

    pub const fn layout(&self) -> CacheLineLayout {
        self.layout
    }

    pub const fn line_address(&self) -> Address {
        self.line_address
    }

    pub const fn directory(&self) -> &TopologyDirectoryConfig {
        &self.directory
    }

    pub const fn memory(&self) -> &TopologyDramMemoryConfig {
        &self.memory
    }

    pub fn caches(&self) -> &[TopologyCacheAgentConfig] {
        &self.caches
    }

    pub fn set_directory(&mut self, directory: TopologyDirectoryConfig) {
        self.directory = directory;
    }
}

impl PartitionedDirectoryLineHarness {
    pub fn new_with_topology(
        topology: &Topology,
        config: TopologyDirectoryHarnessConfig,
    ) -> Result<Self, HarnessError> {
        let TopologyDirectoryHarnessConfig {
            layout,
            line_address,
            directory,
            memory,
            caches,
        } = config;
        let directory_component = topology_component(topology, directory.component())?;
        let directory_endpoint = transport_endpoint(directory.component())?;
        let memory_component = topology_component(topology, memory.component())?;
        let memory_path = topology_route_path(
            topology,
            Endpoint::new(
                directory.component().clone(),
                directory.memory_port().clone(),
            ),
            Endpoint::new(memory.component().clone(), memory.port().clone()),
        )?;
        let memory = PartitionedDramMemoryConfig::new(
            memory_component.partition(),
            transport_endpoint(memory.component())?,
            memory_path.request,
            memory_path.response,
            memory.into_controller(),
        )
        .with_virtual_networks(
            memory_path.request_virtual_network,
            memory_path.response_virtual_network,
        )
        .with_route_hops(memory_path.route_hops);

        let mut agents = Vec::with_capacity(caches.len());
        for cache in caches {
            let cache_component = topology_component(topology, cache.component())?;
            let cache_path = topology_route_path(
                topology,
                Endpoint::new(cache.component().clone(), cache.port().clone()),
                Endpoint::new(
                    directory.component().clone(),
                    directory.cache_port().clone(),
                ),
            )?;
            agents.push(
                PartitionedCacheAgentConfig::new(
                    cache.agent(),
                    cache_component.partition(),
                    transport_endpoint(cache.component())?,
                    cache_path.request,
                    cache_path.response,
                )
                .with_virtual_networks(
                    cache_path.request_virtual_network,
                    cache_path.response_virtual_network,
                )
                .with_route_hops(cache_path.route_hops),
            );
        }

        Self::new_with_dram_memory(
            layout,
            line_address,
            directory_component.partition(),
            directory_endpoint,
            memory,
            agents,
        )
    }
}

impl PartitionedMesiDirectoryLineHarness {
    pub fn new_with_topology(
        topology: &Topology,
        config: TopologyDirectoryHarnessConfig,
    ) -> Result<Self, MesiHarnessError> {
        let TopologyDirectoryHarnessConfig {
            layout,
            line_address,
            directory,
            memory,
            caches,
        } = config;
        let directory_component =
            topology_component(topology, directory.component()).map_err(map_mesi_topology_error)?;
        let directory_endpoint = transport_endpoint_mesi(directory.component())?;
        let memory_component =
            topology_component(topology, memory.component()).map_err(map_mesi_topology_error)?;
        let memory_path = topology_route_path(
            topology,
            Endpoint::new(
                directory.component().clone(),
                directory.memory_port().clone(),
            ),
            Endpoint::new(memory.component().clone(), memory.port().clone()),
        )
        .map_err(map_mesi_topology_error)?;
        let memory = PartitionedDramMemoryConfig::new(
            memory_component.partition(),
            transport_endpoint_mesi(memory.component())?,
            memory_path.request,
            memory_path.response,
            memory.into_controller(),
        )
        .with_virtual_networks(
            memory_path.request_virtual_network,
            memory_path.response_virtual_network,
        )
        .with_route_hops(memory_path.route_hops);

        let mut agents = Vec::with_capacity(caches.len());
        for cache in caches {
            let cache_component =
                topology_component(topology, cache.component()).map_err(map_mesi_topology_error)?;
            let cache_path = topology_route_path(
                topology,
                Endpoint::new(cache.component().clone(), cache.port().clone()),
                Endpoint::new(
                    directory.component().clone(),
                    directory.cache_port().clone(),
                ),
            )
            .map_err(map_mesi_topology_error)?;
            agents.push(
                PartitionedCacheAgentConfig::new(
                    cache.agent(),
                    cache_component.partition(),
                    transport_endpoint_mesi(cache.component())?,
                    cache_path.request,
                    cache_path.response,
                )
                .with_virtual_networks(
                    cache_path.request_virtual_network,
                    cache_path.response_virtual_network,
                )
                .with_route_hops(cache_path.route_hops),
            );
        }

        Self::new_with_dram_memory(
            layout,
            line_address,
            directory_component.partition(),
            directory_endpoint,
            memory,
            agents,
        )
    }
}

fn topology_component<'a>(
    topology: &'a Topology,
    component: &ComponentId,
) -> Result<&'a ComponentSpec, HarnessError> {
    topology.component(component).ok_or_else(|| {
        HarnessError::Topology(TopologyError::UnknownComponent {
            component: component.clone(),
        })
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TopologyRoutePath {
    request: u64,
    response: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    route_hops: Vec<PartitionedRouteHopConfig>,
}

fn topology_route_path(
    topology: &Topology,
    from: Endpoint,
    to: Endpoint,
) -> Result<TopologyRoutePath, HarnessError> {
    let path = topology
        .find_endpoint_path(&from, &to)
        .ok_or(HarnessError::MissingTopologyConnection { from, to })?;
    let route_hops = path
        .hops()
        .iter()
        .map(|hop| {
            let component = topology_component(topology, hop.to().component())?;
            let mut route_hop = PartitionedRouteHopConfig::new(
                component.partition(),
                transport_endpoint(hop.to().component())?,
                hop.request_latency(),
                hop.response_latency(),
            );
            if let Some(path) = hop.request_fabric_path() {
                route_hop = route_hop.with_request_fabric_path(path.clone());
            }
            if let Some(path) = hop.response_fabric_path() {
                route_hop = route_hop.with_response_fabric_path(path.clone());
            }
            Ok(route_hop)
        })
        .collect::<Result<Vec<_>, HarnessError>>()?;

    Ok(TopologyRoutePath {
        request: path.request_latency(),
        response: path.response_latency(),
        request_virtual_network: request_virtual_network(&path),
        response_virtual_network: response_virtual_network(&path),
        route_hops,
    })
}

fn request_virtual_network(path: &rem6_topology::TopologyPath) -> VirtualNetworkId {
    path.hops()
        .iter()
        .find(|hop| hop.request_fabric_path().is_some())
        .map_or(VirtualNetworkId::new(0), |hop| {
            hop.request_virtual_network()
        })
}

fn response_virtual_network(path: &rem6_topology::TopologyPath) -> VirtualNetworkId {
    path.hops()
        .iter()
        .find(|hop| hop.response_fabric_path().is_some())
        .map_or(VirtualNetworkId::new(0), |hop| {
            hop.response_virtual_network()
        })
}

fn transport_endpoint(component: &ComponentId) -> Result<TransportEndpointId, HarnessError> {
    TransportEndpointId::new(component.as_str()).map_err(HarnessError::Transport)
}

fn transport_endpoint_mesi(
    component: &ComponentId,
) -> Result<TransportEndpointId, MesiHarnessError> {
    TransportEndpointId::new(component.as_str()).map_err(MesiHarnessError::Transport)
}

fn map_mesi_topology_error(error: HarnessError) -> MesiHarnessError {
    match error {
        HarnessError::MissingTopologyConnection { from, to } => {
            MesiHarnessError::MissingTopologyConnection { from, to }
        }
        HarnessError::Topology(error) => MesiHarnessError::Topology(error),
        HarnessError::Transport(error) => MesiHarnessError::Transport(error),
        error => MesiHarnessError::Backing(error),
    }
}
