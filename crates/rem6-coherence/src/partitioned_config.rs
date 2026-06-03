use rem6_directory::DirectoryDecision;
use rem6_dram::{
    DramMemoryController, DramMemoryError, DramMemoryOutcome, DramQosSchedulingPolicy,
};
use rem6_fabric::{
    FabricPath, QosPriorityPolicy, QosQueueArbiter, QosRequestorId, VirtualNetworkId,
};
use rem6_kernel::PartitionId;
use rem6_memory::{AgentId, MemoryRequest, MemoryRequestId, MemoryTargetId};
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
    qos: Option<PartitionedDramQosState>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedDramQosState {
    priority_policy: QosPriorityPolicy,
    arbiter: QosQueueArbiter,
    scheduling_policy: DramQosSchedulingPolicy,
    next_order: u64,
}

impl PartitionedDramQosState {
    pub fn new(
        priority_policy: QosPriorityPolicy,
        arbiter: QosQueueArbiter,
        scheduling_policy: DramQosSchedulingPolicy,
    ) -> Self {
        Self {
            priority_policy,
            arbiter,
            scheduling_policy,
            next_order: 0,
        }
    }

    pub fn accept(
        &mut self,
        controller: &mut DramMemoryController,
        arrival_cycle: u64,
        request: &MemoryRequest,
    ) -> Result<DramMemoryOutcome, DramMemoryError> {
        let mut priority_policy = self.priority_policy.clone();
        let priority = priority_policy
            .priority_for(
                QosRequestorId::new(request.id().agent().get()),
                request.size().bytes(),
            )
            .map_err(|source| DramMemoryError::Qos { source })?;
        let order = self.next_order;
        let mut arbiter = self.arbiter.clone();
        let outcome = controller.accept_qos_with_policy(
            arrival_cycle,
            request,
            priority,
            order,
            &mut arbiter,
            self.scheduling_policy,
        )?;
        self.priority_policy = priority_policy;
        self.arbiter = arbiter;
        self.next_order = self
            .next_order
            .checked_add(1)
            .expect("partitioned DRAM QoS order does not overflow");
        Ok(outcome)
    }
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
            qos: None,
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

    pub fn with_qos(mut self, qos: PartitionedDramQosState) -> Self {
        self.qos = Some(qos);
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

    pub const fn qos(&self) -> Option<&PartitionedDramQosState> {
        self.qos.as_ref()
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

#[cfg(test)]
mod tests {
    use super::*;
    use rem6_dram::{DramControllerConfig, DramGeometry, DramQosTurnaroundPolicy, DramTiming};
    use rem6_fabric::{QosFixedPriorityPolicy, QosPriority, QosQueuePolicyKind};
    use rem6_memory::{AccessSize, Address, CacheLineLayout};

    fn layout() -> CacheLineLayout {
        CacheLineLayout::new(64).unwrap()
    }

    fn geometry() -> DramGeometry {
        DramGeometry::new(4, 256, 64).unwrap()
    }

    fn timing() -> DramTiming {
        DramTiming::new(3, 5, 7, 2, 4).unwrap()
    }

    fn request_id(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(6), sequence)
    }

    fn line_data() -> Vec<u8> {
        vec![0xaa; layout().bytes() as usize]
    }

    fn controller_with_line() -> DramMemoryController {
        let target = MemoryTargetId::new(1);
        let mut controller = DramMemoryController::new();
        controller
            .add_target(DramControllerConfig::new(
                target,
                layout(),
                geometry(),
                timing(),
            ))
            .unwrap();
        controller
            .map_region(
                target,
                Address::new(0x0000),
                AccessSize::new(0x4000).unwrap(),
            )
            .unwrap();
        controller
            .insert_line(target, Address::new(0x1000), line_data())
            .unwrap();
        controller
    }

    #[test]
    fn partitioned_dram_qos_state_does_not_advance_on_rejected_access() {
        let mut controller = controller_with_line();
        let mut state = PartitionedDramQosState::new(
            QosPriorityPolicy::fixed_priority(
                QosFixedPriorityPolicy::new(8, QosPriority::new(3)).unwrap(),
            ),
            QosQueueArbiter::new(QosQueuePolicyKind::LeastRecentlyGranted),
            DramQosSchedulingPolicy::new().with_turnaround(DramQosTurnaroundPolicy::RequestOrder),
        );
        let before = state.clone();
        let request =
            MemoryRequest::clean_evict(request_id(1), Address::new(0x1000), layout()).unwrap();

        let error = state.accept(&mut controller, 0, &request).unwrap_err();

        assert!(matches!(error, DramMemoryError::Dram { .. }));
        assert_eq!(state, before);
    }
}
