use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, CacheLineLayout};
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_transport::{MemoryTransport, TopologyRouteError, TransportEndpointId};

use crate::{
    CpuCore, CpuDataConfig, CpuError, CpuFetchConfig, CpuResetState, RiscvCluster,
    RiscvClusterError, RiscvCore,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCoreTopologyConfig {
    reset: CpuResetState,
    fetch_source: Endpoint,
    fetch_target: Endpoint,
    fetch_line_layout: CacheLineLayout,
    fetch_width: AccessSize,
    data: Option<RiscvCoreTopologyDataConfig>,
}

impl RiscvCoreTopologyConfig {
    pub fn new(
        reset: CpuResetState,
        fetch_source: Endpoint,
        fetch_target: Endpoint,
        fetch_line_layout: CacheLineLayout,
        fetch_width: AccessSize,
    ) -> Self {
        Self {
            reset,
            fetch_source,
            fetch_target,
            fetch_line_layout,
            fetch_width,
            data: None,
        }
    }

    pub fn with_data(
        mut self,
        source: Endpoint,
        target: Endpoint,
        line_layout: CacheLineLayout,
    ) -> Self {
        self.data = Some(RiscvCoreTopologyDataConfig {
            source,
            target,
            line_layout,
        });
        self
    }

    pub const fn reset(&self) -> &CpuResetState {
        &self.reset
    }

    pub const fn fetch_source(&self) -> &Endpoint {
        &self.fetch_source
    }

    pub const fn fetch_target(&self) -> &Endpoint {
        &self.fetch_target
    }

    pub const fn fetch_line_layout(&self) -> CacheLineLayout {
        self.fetch_line_layout
    }

    pub const fn fetch_width(&self) -> AccessSize {
        self.fetch_width
    }

    pub const fn data(&self) -> Option<&RiscvCoreTopologyDataConfig> {
        self.data.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCoreTopologyDataConfig {
    source: Endpoint,
    target: Endpoint,
    line_layout: CacheLineLayout,
}

impl RiscvCoreTopologyDataConfig {
    pub const fn source(&self) -> &Endpoint {
        &self.source
    }

    pub const fn target(&self) -> &Endpoint {
        &self.target
    }

    pub const fn line_layout(&self) -> CacheLineLayout {
        self.line_layout
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvClusterTopologyConfig {
    cores: Vec<RiscvCoreTopologyConfig>,
}

impl RiscvClusterTopologyConfig {
    pub fn new<I>(cores: I) -> Self
    where
        I: IntoIterator<Item = RiscvCoreTopologyConfig>,
    {
        Self {
            cores: cores.into_iter().collect(),
        }
    }

    pub fn cores(&self) -> &[RiscvCoreTopologyConfig] {
        &self.cores
    }

    fn into_cores(self) -> Vec<RiscvCoreTopologyConfig> {
        self.cores
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CpuTopologyError {
    SourcePartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
    Cluster(RiscvClusterError),
    Cpu(CpuError),
    Topology(TopologyError),
    TopologyRoute(TopologyRouteError),
}

impl fmt::Display for CpuTopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourcePartitionMismatch {
                endpoint,
                expected,
                actual,
            } => write!(
                formatter,
                "endpoint {}.{} is on partition {} but CPU reset partition is {}",
                endpoint.component().as_str(),
                endpoint.port().as_str(),
                actual.index(),
                expected.index()
            ),
            Self::Cluster(error) => write!(formatter, "{error}"),
            Self::Cpu(error) => write!(formatter, "{error}"),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::TopologyRoute(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CpuTopologyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cluster(error) => Some(error),
            Self::Cpu(error) => Some(error),
            Self::Topology(error) => Some(error),
            Self::TopologyRoute(error) => Some(error),
            _ => None,
        }
    }
}

impl RiscvCore {
    pub fn from_topology(
        topology: &Topology,
        transport: &mut MemoryTransport,
        config: RiscvCoreTopologyConfig,
    ) -> Result<Self, CpuTopologyError> {
        validate_source_partition(topology, config.fetch_source(), config.reset().partition())?;
        if let Some(data) = config.data() {
            validate_source_partition(topology, data.source(), config.reset().partition())?;
        }

        let fetch_route = transport
            .add_topology_route(
                topology,
                config.fetch_source().clone(),
                config.fetch_target().clone(),
            )
            .map_err(CpuTopologyError::TopologyRoute)?;
        let core = CpuCore::new(
            config.reset().clone(),
            CpuFetchConfig::new(
                TransportEndpointId::from_topology_endpoint(config.fetch_source())
                    .map_err(CpuTopologyError::TopologyRoute)?,
                fetch_route,
                config.fetch_line_layout(),
                config.fetch_width(),
            ),
        )
        .map_err(CpuTopologyError::Cpu)?;

        let Some(data) = config.data() else {
            return Ok(Self::new(core));
        };
        let data_route = transport
            .add_topology_route(topology, data.source().clone(), data.target().clone())
            .map_err(CpuTopologyError::TopologyRoute)?;
        Ok(Self::with_data(
            core,
            CpuDataConfig::new(
                TransportEndpointId::from_topology_endpoint(data.source())
                    .map_err(CpuTopologyError::TopologyRoute)?,
                data_route,
                data.line_layout(),
            ),
        ))
    }
}

impl RiscvCluster {
    pub fn from_topology(
        topology: &Topology,
        transport: &mut MemoryTransport,
        config: RiscvClusterTopologyConfig,
    ) -> Result<Self, CpuTopologyError> {
        validate_cluster_config(config.cores())?;
        let mut cores = Vec::with_capacity(config.cores().len());
        for core_config in config.into_cores() {
            cores.push(RiscvCore::from_topology(topology, transport, core_config)?);
        }
        Self::new(cores).map_err(CpuTopologyError::Cluster)
    }
}

fn validate_cluster_config(configs: &[RiscvCoreTopologyConfig]) -> Result<(), CpuTopologyError> {
    let mut by_cpu = BTreeMap::new();
    let mut by_agent = BTreeMap::new();
    let mut by_fetch_endpoint = BTreeMap::new();
    let mut by_data_endpoint = BTreeMap::new();

    for config in configs {
        let cpu = config.reset().cpu();
        if by_cpu.insert(cpu, cpu).is_some() {
            return Err(CpuTopologyError::Cluster(RiscvClusterError::DuplicateCpu {
                cpu,
            }));
        }

        let agent = config.reset().agent();
        if let Some(existing) = by_agent.insert(agent, cpu) {
            return Err(CpuTopologyError::Cluster(
                RiscvClusterError::DuplicateAgent {
                    agent,
                    existing,
                    duplicate: cpu,
                },
            ));
        }

        let fetch_endpoint = topology_transport_endpoint(config.fetch_source())?;
        if let Some(existing) = by_fetch_endpoint.insert(fetch_endpoint.clone(), cpu) {
            return Err(CpuTopologyError::Cluster(
                RiscvClusterError::DuplicateFetchEndpoint {
                    endpoint: fetch_endpoint,
                    existing,
                    duplicate: cpu,
                },
            ));
        }

        if let Some(data) = config.data() {
            let data_endpoint = topology_transport_endpoint(data.source())?;
            if let Some(existing) = by_data_endpoint.insert(data_endpoint.clone(), cpu) {
                return Err(CpuTopologyError::Cluster(
                    RiscvClusterError::DuplicateDataEndpoint {
                        endpoint: data_endpoint,
                        existing,
                        duplicate: cpu,
                    },
                ));
            }
        }
    }
    Ok(())
}

fn validate_source_partition(
    topology: &Topology,
    endpoint: &Endpoint,
    expected: PartitionId,
) -> Result<(), CpuTopologyError> {
    let component = topology.component(endpoint.component()).ok_or_else(|| {
        CpuTopologyError::Topology(TopologyError::UnknownComponent {
            component: endpoint.component().clone(),
        })
    })?;
    component.port_direction(endpoint.port()).ok_or_else(|| {
        CpuTopologyError::Topology(TopologyError::UnknownPort {
            component: endpoint.component().clone(),
            port: endpoint.port().clone(),
        })
    })?;

    let actual = component.partition();
    if actual != expected {
        return Err(CpuTopologyError::SourcePartitionMismatch {
            endpoint: endpoint.clone(),
            expected,
            actual,
        });
    }
    Ok(())
}

fn topology_transport_endpoint(
    endpoint: &Endpoint,
) -> Result<TransportEndpointId, CpuTopologyError> {
    TransportEndpointId::from_topology_endpoint(endpoint).map_err(CpuTopologyError::TopologyRoute)
}
