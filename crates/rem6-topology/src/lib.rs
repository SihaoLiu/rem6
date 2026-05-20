use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_fabric::{FabricError, FabricLinkId, FabricPath, FabricPathHop};
use rem6_kernel::{ClockDomain, PartitionId, Tick};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentId(String);

impl ComponentId {
    pub fn new(value: impl Into<String>) -> Result<Self, TopologyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TopologyError::EmptyIdentifier);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ComponentKind(String);

impl ComponentKind {
    pub fn new(value: impl Into<String>) -> Result<Self, TopologyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TopologyError::EmptyIdentifier);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PortName(String);

impl PortName {
    pub fn new(value: impl Into<String>) -> Result<Self, TopologyError> {
        let value = value.into();
        if value.is_empty() {
            return Err(TopologyError::EmptyIdentifier);
        }

        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PortDirection {
    Initiator,
    Target,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Endpoint {
    component: ComponentId,
    port: PortName,
}

impl Endpoint {
    pub fn new(component: ComponentId, port: PortName) -> Self {
        Self { component, port }
    }

    pub fn component(&self) -> &ComponentId {
        &self.component
    }

    pub fn port(&self) -> &PortName {
        &self.port
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TopologyError {
    EmptyIdentifier,
    DuplicateComponent {
        component: ComponentId,
    },
    DuplicatePort {
        component: ComponentId,
        port: PortName,
    },
    UnknownComponent {
        component: ComponentId,
    },
    UnknownPort {
        component: ComponentId,
        port: PortName,
    },
    PartitionOutOfRange {
        component: ComponentId,
        partition: PartitionId,
        partitions: u32,
    },
    ZeroConnectionLatency,
    InvalidConnectionDirection {
        from: Endpoint,
        from_direction: PortDirection,
        to: Endpoint,
        to_direction: PortDirection,
    },
    Fabric(FabricError),
}

impl fmt::Display for TopologyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyIdentifier => write!(formatter, "identifier must not be empty"),
            Self::DuplicateComponent { component } => {
                write!(formatter, "component {} is already declared", component.as_str())
            }
            Self::DuplicatePort { component, port } => write!(
                formatter,
                "port {} is already declared on component {}",
                port.as_str(),
                component.as_str()
            ),
            Self::UnknownComponent { component } => {
                write!(formatter, "component {} is not declared", component.as_str())
            }
            Self::UnknownPort { component, port } => write!(
                formatter,
                "port {} is not declared on component {}",
                port.as_str(),
                component.as_str()
            ),
            Self::PartitionOutOfRange {
                component,
                partition,
                partitions,
            } => write!(
                formatter,
                "component {} uses partition {}; topology has {partitions} partitions",
                component.as_str(),
                partition.index()
            ),
            Self::ZeroConnectionLatency => {
                write!(formatter, "topology connections require positive latency")
            }
            Self::InvalidConnectionDirection {
                from,
                from_direction,
                to,
                to_direction,
            } => write!(
                formatter,
                "connection {}.{} ({from_direction:?}) to {}.{} ({to_direction:?}) must be initiator to target",
                from.component().as_str(),
                from.port().as_str(),
                to.component().as_str(),
                to.port().as_str()
            ),
            Self::Fabric(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TopologyError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Fabric(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ComponentSpec {
    id: ComponentId,
    kind: ComponentKind,
    partition: PartitionId,
    clock_domain: ClockDomain,
    ports: Vec<PortSpec>,
}

impl ComponentSpec {
    pub fn new(
        id: ComponentId,
        kind: ComponentKind,
        partition: PartitionId,
        clock_domain: ClockDomain,
    ) -> Self {
        Self {
            id,
            kind,
            partition,
            clock_domain,
            ports: Vec::new(),
        }
    }

    pub fn id(&self) -> &ComponentId {
        &self.id
    }

    pub fn kind(&self) -> &ComponentKind {
        &self.kind
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn clock_domain(&self) -> ClockDomain {
        self.clock_domain
    }

    pub fn ports(&self) -> &[PortSpec] {
        &self.ports
    }

    pub fn port_direction(&self, port: &PortName) -> Option<PortDirection> {
        self.ports
            .iter()
            .find(|spec| spec.name() == port)
            .map(PortSpec::direction)
    }

    pub fn add_port(
        mut self,
        name: PortName,
        direction: PortDirection,
    ) -> Result<Self, TopologyError> {
        if self.ports.iter().any(|port| port.name == name) {
            return Err(TopologyError::DuplicatePort {
                component: self.id,
                port: name,
            });
        }

        self.ports.push(PortSpec { name, direction });
        Ok(self)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PortSpec {
    name: PortName,
    direction: PortDirection,
}

impl PortSpec {
    pub fn name(&self) -> &PortName {
        &self.name
    }

    pub fn direction(&self) -> PortDirection {
        self.direction
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConnectionSpec {
    from: Endpoint,
    to: Endpoint,
    request_latency: Tick,
    response_latency: Tick,
    request_fabric_path: Option<FabricPath>,
    response_fabric_path: Option<FabricPath>,
}

impl ConnectionSpec {
    pub fn from(&self) -> &Endpoint {
        &self.from
    }

    pub fn to(&self) -> &Endpoint {
        &self.to
    }

    pub fn latency(&self) -> Tick {
        self.request_latency
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
pub struct TopologyPathHop {
    from: Endpoint,
    to: Endpoint,
    request_latency: Tick,
    response_latency: Tick,
    request_fabric_path: Option<FabricPath>,
    response_fabric_path: Option<FabricPath>,
}

impl TopologyPathHop {
    fn from_connection(connection: &ConnectionSpec) -> Self {
        Self {
            from: connection.from().clone(),
            to: connection.to().clone(),
            request_latency: connection.request_latency(),
            response_latency: connection.response_latency(),
            request_fabric_path: connection.request_fabric_path().cloned(),
            response_fabric_path: connection.response_fabric_path().cloned(),
        }
    }

    pub fn from(&self) -> &Endpoint {
        &self.from
    }

    pub fn to(&self) -> &Endpoint {
        &self.to
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
pub struct TopologyPath {
    source: ComponentId,
    target: ComponentId,
    hops: Vec<TopologyPathHop>,
    request_latency: Tick,
    response_latency: Tick,
}

impl TopologyPath {
    fn new(
        source: ComponentId,
        target: ComponentId,
        hops: Vec<TopologyPathHop>,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Self {
        Self {
            source,
            target,
            hops,
            request_latency,
            response_latency,
        }
    }

    pub fn source(&self) -> &ComponentId {
        &self.source
    }

    pub fn target(&self) -> &ComponentId {
        &self.target
    }

    pub fn hops(&self) -> &[TopologyPathHop] {
        &self.hops
    }

    pub fn request_latency(&self) -> Tick {
        self.request_latency
    }

    pub fn response_latency(&self) -> Tick {
        self.response_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PathSearchState {
    cost: Tick,
    request_latency: Tick,
    response_latency: Tick,
    hops: Vec<TopologyPathHop>,
}

impl PathSearchState {
    fn root() -> Self {
        Self {
            cost: 0,
            request_latency: 0,
            response_latency: 0,
            hops: Vec::new(),
        }
    }

    fn extend(&self, connection: &ConnectionSpec) -> Option<Self> {
        let request_latency = self
            .request_latency
            .checked_add(connection.request_latency())?;
        let response_latency = self
            .response_latency
            .checked_add(connection.response_latency())?;
        let cost = request_latency.checked_add(response_latency)?;
        let mut hops = self.hops.clone();
        hops.push(TopologyPathHop::from_connection(connection));
        Some(Self {
            cost,
            request_latency,
            response_latency,
            hops,
        })
    }

    fn is_better_than(&self, other: &Self) -> bool {
        (
            self.cost,
            self.request_latency,
            self.response_latency,
            self.hops.len(),
        ) < (
            other.cost,
            other.request_latency,
            other.response_latency,
            other.hops.len(),
        )
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TopologyBuilder {
    partition_count: u32,
    components: Vec<ComponentSpec>,
    connections: Vec<ConnectionSpec>,
}

impl TopologyBuilder {
    pub fn new(partition_count: u32) -> Self {
        Self {
            partition_count,
            components: Vec::new(),
            connections: Vec::new(),
        }
    }

    pub fn add_component(mut self, component: ComponentSpec) -> Result<Self, TopologyError> {
        if self
            .components
            .iter()
            .any(|existing| existing.id() == component.id())
        {
            return Err(TopologyError::DuplicateComponent {
                component: component.id,
            });
        }

        if component.partition().index() >= self.partition_count {
            return Err(TopologyError::PartitionOutOfRange {
                component: component.id,
                partition: component.partition,
                partitions: self.partition_count,
            });
        }

        self.components.push(component);
        Ok(self)
    }

    pub fn connect(
        mut self,
        from: Endpoint,
        to: Endpoint,
        latency: Tick,
    ) -> Result<Self, TopologyError> {
        self.validate_connection(&from, &to, latency, latency)?;
        self.connections.push(ConnectionSpec {
            from,
            to,
            request_latency: latency,
            response_latency: latency,
            request_fabric_path: None,
            response_fabric_path: None,
        });
        Ok(self)
    }

    pub fn connect_with_latencies(
        mut self,
        from: Endpoint,
        to: Endpoint,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Result<Self, TopologyError> {
        self.validate_connection(&from, &to, request_latency, response_latency)?;
        self.connections.push(ConnectionSpec {
            from,
            to,
            request_latency,
            response_latency,
            request_fabric_path: None,
            response_fabric_path: None,
        });
        Ok(self)
    }

    pub fn connect_with_fabric_latencies(
        mut self,
        from: Endpoint,
        to: Endpoint,
        request_latency: Tick,
        response_latency: Tick,
        fabric_link: FabricLinkId,
        bandwidth_bytes_per_tick: u64,
    ) -> Result<Self, TopologyError> {
        self.validate_connection(&from, &to, request_latency, response_latency)?;
        let request_fabric_path = FabricPath::new([FabricPathHop::new(
            fabric_link.clone(),
            request_latency,
            bandwidth_bytes_per_tick,
        )
        .map_err(TopologyError::Fabric)?])
        .map_err(TopologyError::Fabric)?;
        let response_fabric_path = FabricPath::new([FabricPathHop::new(
            fabric_link,
            response_latency,
            bandwidth_bytes_per_tick,
        )
        .map_err(TopologyError::Fabric)?])
        .map_err(TopologyError::Fabric)?;
        self.connections.push(ConnectionSpec {
            from,
            to,
            request_latency,
            response_latency,
            request_fabric_path: Some(request_fabric_path),
            response_fabric_path: Some(response_fabric_path),
        });
        Ok(self)
    }

    pub fn build(self) -> Result<Topology, TopologyError> {
        let mut components_by_partition = vec![Vec::new(); self.partition_count as usize];
        for component in &self.components {
            components_by_partition[component.partition().index() as usize]
                .push(component.id().clone());
        }

        Ok(Topology {
            partition_count: self.partition_count,
            components: self.components,
            connections: self.connections,
            components_by_partition,
        })
    }

    fn validate_connection(
        &self,
        from: &Endpoint,
        to: &Endpoint,
        request_latency: Tick,
        response_latency: Tick,
    ) -> Result<(), TopologyError> {
        if request_latency == 0 || response_latency == 0 {
            return Err(TopologyError::ZeroConnectionLatency);
        }

        let from_direction = self.endpoint_direction(from)?;
        let to_direction = self.endpoint_direction(to)?;
        if from_direction != PortDirection::Initiator || to_direction != PortDirection::Target {
            return Err(TopologyError::InvalidConnectionDirection {
                from: from.clone(),
                from_direction,
                to: to.clone(),
                to_direction,
            });
        }

        Ok(())
    }

    fn endpoint_direction(&self, endpoint: &Endpoint) -> Result<PortDirection, TopologyError> {
        let component = self
            .components
            .iter()
            .find(|component| component.id() == endpoint.component())
            .ok_or_else(|| TopologyError::UnknownComponent {
                component: endpoint.component().clone(),
            })?;

        component
            .port_direction(endpoint.port())
            .ok_or_else(|| TopologyError::UnknownPort {
                component: endpoint.component().clone(),
                port: endpoint.port().clone(),
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Topology {
    partition_count: u32,
    components: Vec<ComponentSpec>,
    connections: Vec<ConnectionSpec>,
    components_by_partition: Vec<Vec<ComponentId>>,
}

impl Topology {
    pub fn partition_count(&self) -> u32 {
        self.partition_count
    }

    pub fn component_count(&self) -> usize {
        self.components.len()
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    pub fn component(&self, id: &ComponentId) -> Option<&ComponentSpec> {
        self.components
            .iter()
            .find(|component| component.id() == id)
    }

    pub fn components(&self) -> &[ComponentSpec] {
        &self.components
    }

    pub fn connections(&self) -> &[ConnectionSpec] {
        &self.connections
    }

    pub fn components_in_partition(&self, partition: PartitionId) -> &[ComponentId] {
        self.components_by_partition
            .get(partition.index() as usize)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub fn connection_between(&self, from: &Endpoint, to: &Endpoint) -> Option<&ConnectionSpec> {
        self.connections
            .iter()
            .find(|connection| connection.from() == from && connection.to() == to)
    }

    pub fn find_component_path(
        &self,
        source: &ComponentId,
        target: &ComponentId,
    ) -> Option<TopologyPath> {
        self.component(source)?;
        self.component(target)?;

        if source == target {
            return Some(TopologyPath::new(
                source.clone(),
                target.clone(),
                Vec::new(),
                0,
                0,
            ));
        }

        let mut best = BTreeMap::from([(source.clone(), PathSearchState::root())]);
        let mut visited = BTreeSet::new();

        while let Some(component) = next_search_component(&best, &visited) {
            if component == *target {
                let state = best.remove(&component)?;
                return Some(TopologyPath::new(
                    source.clone(),
                    target.clone(),
                    state.hops,
                    state.request_latency,
                    state.response_latency,
                ));
            }

            visited.insert(component.clone());
            let state = best.get(&component).cloned()?;
            for connection in self
                .connections
                .iter()
                .filter(|connection| connection.from().component() == &component)
            {
                let next = connection.to().component().clone();
                if visited.contains(&next) {
                    continue;
                }

                let Some(candidate) = state.extend(connection) else {
                    continue;
                };
                if best
                    .get(&next)
                    .is_none_or(|existing| candidate.is_better_than(existing))
                {
                    best.insert(next, candidate);
                }
            }
        }

        None
    }

    pub fn find_endpoint_path(&self, from: &Endpoint, to: &Endpoint) -> Option<TopologyPath> {
        self.component(from.component())?
            .port_direction(from.port())?;
        self.component(to.component())?.port_direction(to.port())?;

        if from == to {
            return Some(TopologyPath::new(
                from.component().clone(),
                to.component().clone(),
                Vec::new(),
                0,
                0,
            ));
        }

        let mut best = BTreeMap::new();
        for connection in self
            .connections
            .iter()
            .filter(|connection| connection.from() == from)
        {
            if connection.to().component() == to.component() && connection.to() != to {
                continue;
            }

            let state = PathSearchState::root().extend(connection)?;
            let next = connection.to().component().clone();
            if best
                .get(&next)
                .is_none_or(|existing| state.is_better_than(existing))
            {
                best.insert(next, state);
            }
        }

        let mut visited = BTreeSet::new();
        while let Some(component) = next_search_component(&best, &visited) {
            if component == *to.component() {
                let state = best.remove(&component)?;
                return Some(TopologyPath::new(
                    from.component().clone(),
                    to.component().clone(),
                    state.hops,
                    state.request_latency,
                    state.response_latency,
                ));
            }

            visited.insert(component.clone());
            let state = best.get(&component).cloned()?;
            for connection in self
                .connections
                .iter()
                .filter(|connection| connection.from().component() == &component)
            {
                let next = connection.to().component().clone();
                if visited.contains(&next) {
                    continue;
                }
                if connection.to().component() == to.component() && connection.to() != to {
                    continue;
                }

                let Some(candidate) = state.extend(connection) else {
                    continue;
                };
                if best
                    .get(&next)
                    .is_none_or(|existing| candidate.is_better_than(existing))
                {
                    best.insert(next, candidate);
                }
            }
        }

        None
    }
}

fn next_search_component(
    best: &BTreeMap<ComponentId, PathSearchState>,
    visited: &BTreeSet<ComponentId>,
) -> Option<ComponentId> {
    best.iter()
        .filter(|(component, _)| !visited.contains(*component))
        .min_by(|(left_component, left), (right_component, right)| {
            (
                left.cost,
                left.request_latency,
                left.response_latency,
                left.hops.len(),
                *left_component,
            )
                .cmp(&(
                    right.cost,
                    right.request_latency,
                    right.response_latency,
                    right.hops.len(),
                    *right_component,
                ))
        })
        .map(|(component, _)| component.clone())
}
