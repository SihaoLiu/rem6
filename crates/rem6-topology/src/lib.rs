use std::error::Error;
use std::fmt;

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
        }
    }
}

impl Error for TopologyError {}

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
    latency: Tick,
}

impl ConnectionSpec {
    pub fn from(&self) -> &Endpoint {
        &self.from
    }

    pub fn to(&self) -> &Endpoint {
        &self.to
    }

    pub fn latency(&self) -> Tick {
        self.latency
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
        if latency == 0 {
            return Err(TopologyError::ZeroConnectionLatency);
        }

        let from_direction = self.endpoint_direction(&from)?;
        let to_direction = self.endpoint_direction(&to)?;
        if from_direction != PortDirection::Initiator || to_direction != PortDirection::Target {
            return Err(TopologyError::InvalidConnectionDirection {
                from,
                from_direction,
                to,
                to_direction,
            });
        }

        self.connections.push(ConnectionSpec { from, to, latency });
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
}
