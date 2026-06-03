use std::error::Error;
use std::fmt;

use rem6_fabric::{
    FabricError, FabricPath, QosError, QosPriority, QosRequestorId, VirtualNetworkId,
};
use rem6_kernel::{PartitionId, SchedulerError, Tick};
use rem6_topology::{Endpoint, Topology, TopologyError, TopologyPath};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct TransportEndpointId(String);

impl TransportEndpointId {
    pub fn new(value: impl Into<String>) -> Result<Self, TransportError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TransportError::EmptyEndpoint);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_topology_endpoint(endpoint: &Endpoint) -> Result<Self, TopologyRouteError> {
        Self::new(format!(
            "{}.{}",
            endpoint.component().as_str(),
            endpoint.port().as_str()
        ))
        .map_err(TopologyRouteError::Transport)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MemoryRouteId(u64);

impl MemoryRouteId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TransportLatency {
    Request,
    Response,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportQosClass {
    requestor: QosRequestorId,
    priority: QosPriority,
}

impl TransportQosClass {
    pub const fn new(requestor: QosRequestorId, priority: QosPriority) -> Self {
        Self {
            requestor,
            priority,
        }
    }

    pub const fn from_raw(requestor: u32, priority: u8) -> Self {
        Self {
            requestor: QosRequestorId::new(requestor),
            priority: QosPriority::new(priority),
        }
    }

    pub const fn requestor(self) -> QosRequestorId {
        self.requestor
    }

    pub const fn priority(self) -> QosPriority {
        self.priority
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportError {
    EmptyEndpoint,
    EmptyRoutePath,
    ZeroRouteLatency {
        latency: TransportLatency,
    },
    DuplicateRoute {
        source: TransportEndpointId,
        target: TransportEndpointId,
    },
    UnknownRoute {
        route: MemoryRouteId,
    },
    MissingFabricModel {
        route: MemoryRouteId,
    },
    Qos {
        source: QosError,
    },
    Fabric(FabricError),
    Scheduler(SchedulerError),
}

impl fmt::Display for TransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyEndpoint => write!(formatter, "transport endpoint must not be empty"),
            Self::EmptyRoutePath => write!(formatter, "memory route path must contain a hop"),
            Self::ZeroRouteLatency { latency } => {
                write!(formatter, "{latency:?} route latency must be positive")
            }
            Self::DuplicateRoute { source, target } => write!(
                formatter,
                "route from {} to {} is already declared",
                source.as_str(),
                target.as_str()
            ),
            Self::UnknownRoute { route } => {
                write!(formatter, "route {} is not declared", route.get())
            }
            Self::MissingFabricModel { route } => {
                write!(formatter, "route {} needs a fabric model", route.get())
            }
            Self::Qos { source } => write!(formatter, "{source}"),
            Self::Fabric(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Qos { source } => Some(source),
            Self::Fabric(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TopologyRouteError {
    MissingTopologyConnection { from: Endpoint, to: Endpoint },
    Topology(TopologyError),
    Transport(TransportError),
}

impl fmt::Display for TopologyRouteError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingTopologyConnection { from, to } => write!(
                formatter,
                "topology connection {}.{} to {}.{} is not declared",
                from.component().as_str(),
                from.port().as_str(),
                to.component().as_str(),
                to.port().as_str()
            ),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TopologyRouteError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Topology(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRouteHop {
    endpoint: TransportEndpointId,
    partition: PartitionId,
    request_latency: Tick,
    response_latency: Tick,
    request_fabric_path: Option<FabricPath>,
    response_fabric_path: Option<FabricPath>,
}

impl MemoryRouteHop {
    pub fn new(
        endpoint: TransportEndpointId,
        partition: PartitionId,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Result<Self, TransportError> {
        if request_latency == 0 {
            return Err(TransportError::ZeroRouteLatency {
                latency: TransportLatency::Request,
            });
        }
        if response_latency == 0 {
            return Err(TransportError::ZeroRouteLatency {
                latency: TransportLatency::Response,
            });
        }

        Ok(Self {
            endpoint,
            partition,
            request_latency,
            response_latency,
            request_fabric_path: None,
            response_fabric_path: None,
        })
    }

    pub fn with_request_fabric_path(mut self, path: FabricPath) -> Self {
        self.request_fabric_path = Some(path);
        self
    }

    pub fn with_response_fabric_path(mut self, path: FabricPath) -> Self {
        self.response_fabric_path = Some(path);
        self
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn request_latency(&self) -> Tick {
        self.request_latency
    }

    pub fn response_latency(&self) -> Tick {
        self.response_latency
    }

    pub fn request_fabric_path(&self) -> Option<&FabricPath> {
        self.request_fabric_path.as_ref()
    }

    pub fn response_fabric_path(&self) -> Option<&FabricPath> {
        self.response_fabric_path.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryRoute {
    source: TransportEndpointId,
    source_partition: PartitionId,
    target: TransportEndpointId,
    target_partition: PartitionId,
    request_latency: Tick,
    response_latency: Tick,
    hops: Vec<MemoryRouteHop>,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
}

impl MemoryRoute {
    pub fn new(
        source: TransportEndpointId,
        source_partition: PartitionId,
        target: TransportEndpointId,
        target_partition: PartitionId,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Result<Self, TransportError> {
        let hop = MemoryRouteHop::new(
            target.clone(),
            target_partition,
            request_latency,
            response_latency,
        )?;

        Ok(Self {
            source,
            source_partition,
            target,
            target_partition,
            request_latency,
            response_latency,
            hops: vec![hop],
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
        })
    }

    pub fn new_path<I>(
        source: TransportEndpointId,
        source_partition: PartitionId,
        hops: I,
    ) -> Result<Self, TransportError>
    where
        I: IntoIterator<Item = MemoryRouteHop>,
    {
        let hops: Vec<_> = hops.into_iter().collect();
        let Some(last) = hops.last() else {
            return Err(TransportError::EmptyRoutePath);
        };
        let request_latency = hops.iter().map(MemoryRouteHop::request_latency).sum();
        let response_latency = hops.iter().map(MemoryRouteHop::response_latency).sum();

        Ok(Self {
            source,
            source_partition,
            target: last.endpoint().clone(),
            target_partition: last.partition(),
            request_latency,
            response_latency,
            hops,
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
        })
    }

    pub fn from_topology(
        topology: &Topology,
        from: Endpoint,
        to: Endpoint,
    ) -> Result<Self, TopologyRouteError> {
        let source_partition = topology_endpoint_partition(topology, &from)?;
        topology_endpoint_partition(topology, &to)?;
        let path = topology.find_endpoint_path(&from, &to).ok_or_else(|| {
            TopologyRouteError::MissingTopologyConnection {
                from: from.clone(),
                to: to.clone(),
            }
        })?;
        let hops = path
            .hops()
            .iter()
            .map(|hop| {
                let partition = topology_endpoint_partition(topology, hop.to())?;
                let mut route_hop = MemoryRouteHop::new(
                    TransportEndpointId::from_topology_endpoint(hop.to())?,
                    partition,
                    hop.request_latency(),
                    hop.response_latency(),
                )
                .map_err(TopologyRouteError::Transport)?;
                if let Some(path) = hop.request_fabric_path() {
                    route_hop = route_hop.with_request_fabric_path(path.clone());
                }
                if let Some(path) = hop.response_fabric_path() {
                    route_hop = route_hop.with_response_fabric_path(path.clone());
                }
                Ok(route_hop)
            })
            .collect::<Result<Vec<_>, TopologyRouteError>>()?;

        Ok(Self::new_path(
            TransportEndpointId::from_topology_endpoint(&from)?,
            source_partition,
            hops,
        )
        .map_err(TopologyRouteError::Transport)?
        .with_virtual_networks(
            topology_request_virtual_network(&path),
            topology_response_virtual_network(&path),
        ))
    }

    pub fn with_virtual_networks(
        mut self,
        request_virtual_network: VirtualNetworkId,
        response_virtual_network: VirtualNetworkId,
    ) -> Self {
        self.request_virtual_network = request_virtual_network;
        self.response_virtual_network = response_virtual_network;
        self
    }

    pub fn source(&self) -> &TransportEndpointId {
        &self.source
    }

    pub fn target(&self) -> &TransportEndpointId {
        &self.target
    }

    pub fn source_partition(&self) -> PartitionId {
        self.source_partition
    }

    pub fn target_partition(&self) -> PartitionId {
        self.target_partition
    }

    pub fn request_latency(&self) -> Tick {
        self.request_latency
    }

    pub fn response_latency(&self) -> Tick {
        self.response_latency
    }

    pub fn hops(&self) -> &[MemoryRouteHop] {
        &self.hops
    }

    pub fn request_virtual_network(&self) -> VirtualNetworkId {
        self.request_virtual_network
    }

    pub fn response_virtual_network(&self) -> VirtualNetworkId {
        self.response_virtual_network
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct StoredRoute {
    pub(crate) id: MemoryRouteId,
    pub(crate) route: MemoryRoute,
}

fn topology_endpoint_partition(
    topology: &Topology,
    endpoint: &Endpoint,
) -> Result<PartitionId, TopologyRouteError> {
    let component = topology.component(endpoint.component()).ok_or_else(|| {
        TopologyRouteError::Topology(TopologyError::UnknownComponent {
            component: endpoint.component().clone(),
        })
    })?;
    component.port_direction(endpoint.port()).ok_or_else(|| {
        TopologyRouteError::Topology(TopologyError::UnknownPort {
            component: endpoint.component().clone(),
            port: endpoint.port().clone(),
        })
    })?;
    Ok(component.partition())
}

fn topology_request_virtual_network(path: &TopologyPath) -> VirtualNetworkId {
    path.hops()
        .iter()
        .find(|hop| hop.request_fabric_path().is_some())
        .map_or(VirtualNetworkId::new(0), |hop| {
            hop.request_virtual_network()
        })
}

fn topology_response_virtual_network(path: &TopologyPath) -> VirtualNetworkId {
    path.hops()
        .iter()
        .find(|hop| hop.response_fabric_path().is_some())
        .map_or(VirtualNetworkId::new(0), |hop| {
            hop.response_virtual_network()
        })
}
