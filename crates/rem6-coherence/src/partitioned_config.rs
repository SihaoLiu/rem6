use rem6_directory::DirectoryDecision;
use rem6_dram::DramMemoryController;
use rem6_fabric::{FabricPath, VirtualNetworkId};
use rem6_kernel::PartitionId;
use rem6_memory::{AgentId, MemoryRequestId, MemoryTargetId};
use rem6_transport::TransportEndpointId;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedRouteHopConfig {
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_fabric_path: Option<FabricPath>,
    response_fabric_path: Option<FabricPath>,
}

impl PartitionedRouteHopConfig {
    pub fn new(
        partition: PartitionId,
        endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
    ) -> Self {
        Self {
            partition,
            endpoint,
            request_latency,
            response_latency,
            request_fabric_path: None,
            response_fabric_path: None,
        }
    }

    pub fn with_request_fabric_path(mut self, path: FabricPath) -> Self {
        self.request_fabric_path = Some(path);
        self
    }

    pub fn with_response_fabric_path(mut self, path: FabricPath) -> Self {
        self.response_fabric_path = Some(path);
        self
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn request_latency(&self) -> u64 {
        self.request_latency
    }

    pub const fn response_latency(&self) -> u64 {
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
pub struct PartitionedCacheAgentConfig {
    agent: AgentId,
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    route_hops: Vec<PartitionedRouteHopConfig>,
}

impl PartitionedCacheAgentConfig {
    pub fn new(
        agent: AgentId,
        partition: PartitionId,
        endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
    ) -> Self {
        Self {
            agent,
            partition,
            endpoint,
            request_latency,
            response_latency,
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
            route_hops: Vec::new(),
        }
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

    pub fn with_route_hops<I>(mut self, route_hops: I) -> Self
    where
        I: IntoIterator<Item = PartitionedRouteHopConfig>,
    {
        self.route_hops = route_hops.into_iter().collect();
        self
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn request_latency(&self) -> u64 {
        self.request_latency
    }

    pub const fn response_latency(&self) -> u64 {
        self.response_latency
    }

    pub const fn request_virtual_network(&self) -> VirtualNetworkId {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> VirtualNetworkId {
        self.response_virtual_network
    }

    pub fn route_hops(&self) -> &[PartitionedRouteHopConfig] {
        &self.route_hops
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMemoryConfig {
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    route_hops: Vec<PartitionedRouteHopConfig>,
}

impl PartitionedMemoryConfig {
    pub fn new(
        partition: PartitionId,
        endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
    ) -> Self {
        Self {
            partition,
            endpoint,
            request_latency,
            response_latency,
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
            route_hops: Vec::new(),
        }
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

    pub fn with_route_hops<I>(mut self, route_hops: I) -> Self
    where
        I: IntoIterator<Item = PartitionedRouteHopConfig>,
    {
        self.route_hops = route_hops.into_iter().collect();
        self
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn request_latency(&self) -> u64 {
        self.request_latency
    }

    pub const fn response_latency(&self) -> u64 {
        self.response_latency
    }

    pub const fn request_virtual_network(&self) -> VirtualNetworkId {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> VirtualNetworkId {
        self.response_virtual_network
    }

    pub fn route_hops(&self) -> &[PartitionedRouteHopConfig] {
        &self.route_hops
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedDramMemoryConfig {
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    controller: DramMemoryController,
    route_hops: Vec<PartitionedRouteHopConfig>,
}

impl PartitionedDramMemoryConfig {
    pub fn new(
        partition: PartitionId,
        endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
        controller: DramMemoryController,
    ) -> Self {
        Self {
            partition,
            endpoint,
            request_latency,
            response_latency,
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
            controller,
            route_hops: Vec::new(),
        }
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

    pub fn with_route_hops<I>(mut self, route_hops: I) -> Self
    where
        I: IntoIterator<Item = PartitionedRouteHopConfig>,
    {
        self.route_hops = route_hops.into_iter().collect();
        self
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn request_latency(&self) -> u64 {
        self.request_latency
    }

    pub const fn response_latency(&self) -> u64 {
        self.response_latency
    }

    pub const fn request_virtual_network(&self) -> VirtualNetworkId {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> VirtualNetworkId {
        self.response_virtual_network
    }

    pub fn route_hops(&self) -> &[PartitionedRouteHopConfig] {
        &self.route_hops
    }

    pub(crate) fn into_controller(self) -> DramMemoryController {
        self.controller
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryDecisionRecord {
    tick: u64,
    requester: AgentId,
    decision: DirectoryDecision,
}

impl DirectoryDecisionRecord {
    pub const fn new(tick: u64, requester: AgentId, decision: DirectoryDecision) -> Self {
        Self {
            tick,
            requester,
            decision,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn requester(&self) -> AgentId {
        self.requester
    }

    pub const fn decision(&self) -> &DirectoryDecision {
        &self.decision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryAccessRecord {
    arrival_tick: u64,
    target: MemoryTargetId,
    request: MemoryRequestId,
    bank: u32,
    row: u64,
    row_hit: bool,
    ready_cycle: u64,
}

impl DramMemoryAccessRecord {
    pub const fn new(
        arrival_tick: u64,
        target: MemoryTargetId,
        request: MemoryRequestId,
        bank: u32,
        row: u64,
        row_hit: bool,
        ready_cycle: u64,
    ) -> Self {
        Self {
            arrival_tick,
            target,
            request,
            bank,
            row,
            row_hit,
            ready_cycle,
        }
    }

    pub const fn arrival_tick(&self) -> u64 {
        self.arrival_tick
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn bank(&self) -> u32 {
        self.bank
    }

    pub const fn row(&self) -> u64 {
        self.row
    }

    pub const fn row_hit(&self) -> bool {
        self.row_hit
    }

    pub const fn ready_cycle(&self) -> u64 {
        self.ready_cycle
    }
}
