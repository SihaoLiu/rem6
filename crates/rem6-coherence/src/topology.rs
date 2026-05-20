use rem6_dram::DramMemoryController;
use rem6_memory::{Address, AgentId, CacheLineLayout};
use rem6_topology::{ComponentId, ComponentSpec, Endpoint, PortName, Topology, TopologyError};
use rem6_transport::TransportEndpointId;

use crate::{
    HarnessError, PartitionedCacheAgentConfig, PartitionedDirectoryLineHarness,
    PartitionedDramMemoryConfig,
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
        let memory_latency = topology_route_latency(
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
            memory_latency.request,
            memory_latency.response,
            memory.into_controller(),
        );

        let mut agents = Vec::with_capacity(caches.len());
        for cache in caches {
            let cache_component = topology_component(topology, cache.component())?;
            let cache_latency = topology_route_latency(
                topology,
                Endpoint::new(cache.component().clone(), cache.port().clone()),
                Endpoint::new(
                    directory.component().clone(),
                    directory.cache_port().clone(),
                ),
            )?;
            agents.push(PartitionedCacheAgentConfig::new(
                cache.agent(),
                cache_component.partition(),
                transport_endpoint(cache.component())?,
                cache_latency.request,
                cache_latency.response,
            ));
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct TopologyRouteLatency {
    request: u64,
    response: u64,
}

fn topology_route_latency(
    topology: &Topology,
    from: Endpoint,
    to: Endpoint,
) -> Result<TopologyRouteLatency, HarnessError> {
    topology
        .connection_between(&from, &to)
        .map(|connection| TopologyRouteLatency {
            request: connection.request_latency(),
            response: connection.response_latency(),
        })
        .ok_or(HarnessError::MissingTopologyConnection { from, to })
}

fn transport_endpoint(component: &ComponentId) -> Result<TransportEndpointId, HarnessError> {
    TransportEndpointId::new(component.as_str()).map_err(HarnessError::Transport)
}
