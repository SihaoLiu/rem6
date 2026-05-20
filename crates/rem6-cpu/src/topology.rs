use std::error::Error;
use std::fmt;

use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, CacheLineLayout};
use rem6_topology::{Endpoint, Topology, TopologyError};
use rem6_transport::{MemoryTransport, TopologyRouteError, TransportEndpointId};

use crate::{CpuCore, CpuDataConfig, CpuError, CpuFetchConfig, CpuResetState, RiscvCore};

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
pub enum CpuTopologyError {
    SourcePartitionMismatch {
        endpoint: Endpoint,
        expected: PartitionId,
        actual: PartitionId,
    },
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
            Self::Cpu(error) => write!(formatter, "{error}"),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::TopologyRoute(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for CpuTopologyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
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
