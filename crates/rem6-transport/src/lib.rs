use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_fabric::{
    FabricError, FabricModel, FabricPacket, FabricPacketId, FabricPath, VirtualNetworkId,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler,
    SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{MemoryRequest, MemoryRequestId, MemoryResponse, ResponseStatus};
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
    LatencyBelowLookahead {
        route: MemoryRouteId,
        latency: TransportLatency,
        delay: Tick,
        minimum: Tick,
    },
    MissingFabricModel {
        route: MemoryRouteId,
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
            Self::LatencyBelowLookahead {
                route,
                latency,
                delay,
                minimum,
            } => write!(
                formatter,
                "route {} {latency:?} latency {delay} is below scheduler lookahead {minimum}",
                route.get()
            ),
            Self::MissingFabricModel { route } => {
                write!(formatter, "route {} needs a fabric model", route.get())
            }
            Self::Fabric(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
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
struct StoredRoute {
    id: MemoryRouteId,
    route: MemoryRoute,
}

pub struct MemoryTransport {
    next_route_id: u64,
    routes: Vec<StoredRoute>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: None,
        }
    }

    pub fn with_fabric(fabric: FabricModel) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(Arc::new(Mutex::new(fabric))),
        }
    }

    pub fn with_shared_fabric(fabric: Arc<Mutex<FabricModel>>) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(fabric),
        }
    }

    pub fn add_route(&mut self, route: MemoryRoute) -> Result<MemoryRouteId, TransportError> {
        if self.routes.iter().any(|stored| {
            stored.route.source() == route.source() && stored.route.target() == route.target()
        }) {
            return Err(TransportError::DuplicateRoute {
                source: route.source,
                target: route.target,
            });
        }

        let id = MemoryRouteId::new(self.next_route_id);
        self.next_route_id += 1;
        self.routes.push(StoredRoute { id, route });
        Ok(id)
    }

    pub fn add_topology_route(
        &mut self,
        topology: &Topology,
        from: Endpoint,
        to: Endpoint,
    ) -> Result<MemoryRouteId, TopologyRouteError> {
        let route = MemoryRoute::from_topology(topology, from, to)?;
        self.add_route(route).map_err(TopologyRouteError::Transport)
    }

    pub fn route(&self, id: MemoryRouteId) -> Option<&MemoryRoute> {
        self.routes
            .iter()
            .find(|stored| stored.id == id)
            .map(|stored| &stored.route)
    }

    pub fn route_count(&self) -> usize {
        self.routes.len()
    }

    pub fn submit<F, G>(
        &self,
        scheduler: &mut PartitionedScheduler,
        route_id: MemoryRouteId,
        request: MemoryRequest,
        trace: MemoryTrace,
        responder: F,
        response_sink: G,
    ) -> Result<PartitionEventId, TransportError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let route = self
            .route(route_id)
            .cloned()
            .ok_or(TransportError::UnknownRoute { route: route_id })?;
        self.validate_scheduler_route(scheduler, route_id, &route)?;

        let source_partition = route.source_partition();
        let start_tick = scheduler.now();
        let fabric = self.fabric.clone();
        scheduler
            .schedule_at(source_partition, start_tick, move |context| {
                let request_id = request.id();
                trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    route.source().clone(),
                    MemoryTraceKind::RequestSent,
                    request_id,
                ));

                Self::schedule_request_hop(
                    context,
                    route_id,
                    route,
                    0,
                    request,
                    trace,
                    fabric,
                    responder,
                    response_sink,
                );
            })
            .map_err(TransportError::Scheduler)
    }

    pub fn submit_parallel<F, G>(
        &self,
        scheduler: &mut PartitionedScheduler,
        route_id: MemoryRouteId,
        request: MemoryRequest,
        trace: MemoryTrace,
        responder: F,
        response_sink: G,
    ) -> Result<PartitionEventId, TransportError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let route = self
            .route(route_id)
            .cloned()
            .ok_or(TransportError::UnknownRoute { route: route_id })?;
        self.validate_scheduler_route(scheduler, route_id, &route)?;

        let source_partition = route.source_partition();
        let start_tick = scheduler.now();
        let fabric = self.fabric.clone();
        scheduler
            .schedule_parallel_at(source_partition, start_tick, move |context| {
                let request_id = request.id();
                trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    route.source().clone(),
                    MemoryTraceKind::RequestSent,
                    request_id,
                ));

                Self::schedule_parallel_request_hop(
                    context,
                    route_id,
                    route,
                    0,
                    request,
                    trace,
                    fabric,
                    responder,
                    response_sink,
                );
            })
            .map_err(TransportError::Scheduler)
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_request_hop<F, G>(
        context: &mut SchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        request: MemoryRequest,
        trace: MemoryTrace,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        responder: F,
        response_sink: G,
    ) where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let hop = route.hops()[hop_index].clone();
        let delay = request_hop_delay(&fabric, context.now(), route_id, &route, &hop, &request)
            .expect("validated request fabric timing");
        context
            .schedule_remote_after(hop.partition(), delay, move |context| {
                trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    hop.endpoint().clone(),
                    MemoryTraceKind::RequestArrived,
                    request.id(),
                ));

                if hop_index + 1 == route.hops().len() {
                    let delivery = RequestDelivery {
                        tick: context.now(),
                        route: route_id,
                        endpoint: hop.endpoint().clone(),
                        request,
                    };

                    match responder(delivery, context) {
                        TargetOutcome::Respond(response) => {
                            Self::schedule_response_hop(
                                context,
                                route_id,
                                route,
                                hop_index,
                                response,
                                trace,
                                fabric,
                                response_sink,
                            );
                        }
                        TargetOutcome::RespondAfter { delay, response } => {
                            context
                                .schedule_local_after(delay, move |context| {
                                    Self::schedule_response_hop(
                                        context,
                                        route_id,
                                        route,
                                        hop_index,
                                        response,
                                        trace,
                                        fabric,
                                        response_sink,
                                    );
                                })
                                .expect("validated target response delay");
                        }
                        TargetOutcome::NoResponse => {}
                    }
                } else {
                    Self::schedule_request_hop(
                        context,
                        route_id,
                        route,
                        hop_index + 1,
                        request,
                        trace,
                        fabric,
                        responder,
                        response_sink,
                    );
                }
            })
            .expect("validated request transport latency");
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_parallel_request_hop<F, G>(
        context: &mut ParallelSchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        request: MemoryRequest,
        trace: MemoryTrace,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        responder: F,
        response_sink: G,
    ) where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let hop = route.hops()[hop_index].clone();
        let delay = request_hop_delay(&fabric, context.now(), route_id, &route, &hop, &request)
            .expect("validated request fabric timing");
        context
            .schedule_remote_after(hop.partition(), delay, move |context| {
                trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    hop.endpoint().clone(),
                    MemoryTraceKind::RequestArrived,
                    request.id(),
                ));

                if hop_index + 1 == route.hops().len() {
                    let delivery = RequestDelivery {
                        tick: context.now(),
                        route: route_id,
                        endpoint: hop.endpoint().clone(),
                        request,
                    };

                    match responder(delivery, context) {
                        TargetOutcome::Respond(response) => {
                            Self::schedule_parallel_response_hop(
                                context,
                                route_id,
                                route,
                                hop_index,
                                response,
                                trace,
                                fabric,
                                response_sink,
                            );
                        }
                        TargetOutcome::RespondAfter { delay, response } => {
                            context
                                .schedule_local_after(delay, move |context| {
                                    Self::schedule_parallel_response_hop(
                                        context,
                                        route_id,
                                        route,
                                        hop_index,
                                        response,
                                        trace,
                                        fabric,
                                        response_sink,
                                    );
                                })
                                .expect("validated target response delay");
                        }
                        TargetOutcome::NoResponse => {}
                    }
                } else {
                    Self::schedule_parallel_request_hop(
                        context,
                        route_id,
                        route,
                        hop_index + 1,
                        request,
                        trace,
                        fabric,
                        responder,
                        response_sink,
                    );
                }
            })
            .expect("validated request transport latency");
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_response_hop<G>(
        context: &mut SchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        response: MemoryResponse,
        trace: MemoryTrace,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        response_sink: G,
    ) where
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let hop = route.hops()[hop_index].clone();
        let endpoint = if hop_index == 0 {
            route.source().clone()
        } else {
            route.hops()[hop_index - 1].endpoint().clone()
        };
        let partition = if hop_index == 0 {
            route.source_partition()
        } else {
            route.hops()[hop_index - 1].partition()
        };
        let delay = response_hop_delay(&fabric, context.now(), route_id, &route, &hop, &response)
            .expect("validated response fabric timing");
        context
            .schedule_remote_after(partition, delay, move |context| {
                trace.record(MemoryTraceEvent::response(
                    context.now(),
                    route_id,
                    endpoint.clone(),
                    response.request_id(),
                    response.status(),
                ));

                if hop_index == 0 {
                    response_sink(ResponseDelivery {
                        tick: context.now(),
                        route: route_id,
                        endpoint,
                        response,
                    });
                } else {
                    Self::schedule_response_hop(
                        context,
                        route_id,
                        route,
                        hop_index - 1,
                        response,
                        trace,
                        fabric,
                        response_sink,
                    );
                }
            })
            .expect("validated response transport latency");
    }

    #[allow(clippy::too_many_arguments)]
    fn schedule_parallel_response_hop<G>(
        context: &mut ParallelSchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        response: MemoryResponse,
        trace: MemoryTrace,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        response_sink: G,
    ) where
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let hop = route.hops()[hop_index].clone();
        let endpoint = if hop_index == 0 {
            route.source().clone()
        } else {
            route.hops()[hop_index - 1].endpoint().clone()
        };
        let partition = if hop_index == 0 {
            route.source_partition()
        } else {
            route.hops()[hop_index - 1].partition()
        };
        let delay = response_hop_delay(&fabric, context.now(), route_id, &route, &hop, &response)
            .expect("validated response fabric timing");
        context
            .schedule_remote_after(partition, delay, move |context| {
                trace.record(MemoryTraceEvent::response(
                    context.now(),
                    route_id,
                    endpoint.clone(),
                    response.request_id(),
                    response.status(),
                ));

                if hop_index == 0 {
                    response_sink(ResponseDelivery {
                        tick: context.now(),
                        route: route_id,
                        endpoint,
                        response,
                    });
                } else {
                    Self::schedule_parallel_response_hop(
                        context,
                        route_id,
                        route,
                        hop_index - 1,
                        response,
                        trace,
                        fabric,
                        response_sink,
                    );
                }
            })
            .expect("validated response transport latency");
    }

    fn validate_scheduler_route(
        &self,
        scheduler: &PartitionedScheduler,
        route_id: MemoryRouteId,
        route: &MemoryRoute,
    ) -> Result<(), TransportError> {
        let mut previous_partition = route.source_partition();
        scheduler
            .partition_now(previous_partition)
            .map_err(TransportError::Scheduler)?;

        for hop in route.hops() {
            if self.fabric.is_none()
                && (hop.request_fabric_path().is_some() || hop.response_fabric_path().is_some())
            {
                return Err(TransportError::MissingFabricModel { route: route_id });
            }
            scheduler
                .partition_now(hop.partition())
                .map_err(TransportError::Scheduler)?;

            if previous_partition != hop.partition() {
                let minimum = scheduler.min_remote_delay();
                if hop.request_latency() < minimum {
                    return Err(TransportError::LatencyBelowLookahead {
                        route: route_id,
                        latency: TransportLatency::Request,
                        delay: hop.request_latency(),
                        minimum,
                    });
                }
                if hop.response_latency() < minimum {
                    return Err(TransportError::LatencyBelowLookahead {
                        route: route_id,
                        latency: TransportLatency::Response,
                        delay: hop.response_latency(),
                        minimum,
                    });
                }
            }
            previous_partition = hop.partition();
        }

        Ok(())
    }
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

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn request_hop_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: Tick,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    hop: &MemoryRouteHop,
    request: &MemoryRequest,
) -> Result<Tick, TransportError> {
    let Some(path) = hop.request_fabric_path() else {
        return Ok(hop.request_latency());
    };
    let Some(fabric) = fabric else {
        return Err(TransportError::MissingFabricModel { route: route_id });
    };
    let packet = FabricPacket::new(
        fabric_packet_id(route_id, request.id(), TransportLatency::Request),
        request.size().bytes(),
        route.request_virtual_network(),
    )
    .map_err(TransportError::Fabric)?;
    let arrival = fabric
        .lock()
        .expect("fabric lock")
        .transmit(now, packet, path.clone())
        .map_err(TransportError::Fabric)?
        .arrival_tick();
    arrival
        .checked_sub(now)
        .ok_or(TransportError::Fabric(FabricError::TickOverflow))
}

fn response_hop_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: Tick,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    hop: &MemoryRouteHop,
    response: &MemoryResponse,
) -> Result<Tick, TransportError> {
    let Some(path) = hop.response_fabric_path() else {
        return Ok(hop.response_latency());
    };
    let Some(fabric) = fabric else {
        return Err(TransportError::MissingFabricModel { route: route_id });
    };
    let packet = FabricPacket::new(
        fabric_packet_id(route_id, response.request_id(), TransportLatency::Response),
        response_packet_bytes(response),
        route.response_virtual_network(),
    )
    .map_err(TransportError::Fabric)?;
    let arrival = fabric
        .lock()
        .expect("fabric lock")
        .transmit(now, packet, path.clone())
        .map_err(TransportError::Fabric)?
        .arrival_tick();
    arrival
        .checked_sub(now)
        .ok_or(TransportError::Fabric(FabricError::TickOverflow))
}

fn response_packet_bytes(response: &MemoryResponse) -> u64 {
    response
        .data()
        .map_or(1, |bytes| (bytes.len() as u64).max(1))
}

fn fabric_packet_id(
    route: MemoryRouteId,
    request: MemoryRequestId,
    latency: TransportLatency,
) -> FabricPacketId {
    let direction = match latency {
        TransportLatency::Request => 0,
        TransportLatency::Response => 1,
    };
    let value = (direction << 63)
        | ((route.get() & 0x7fff) << 48)
        | ((u64::from(request.agent().get()) & 0xffff) << 32)
        | (request.sequence() & 0xffff_ffff);
    FabricPacketId::new(value)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestDelivery {
    tick: Tick,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    request: MemoryRequest,
}

impl RequestDelivery {
    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn request(&self) -> &MemoryRequest {
        &self.request
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResponseDelivery {
    tick: Tick,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    response: MemoryResponse,
}

impl ResponseDelivery {
    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn response(&self) -> &MemoryResponse {
        &self.response
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TargetOutcome {
    Respond(MemoryResponse),
    RespondAfter {
        delay: Tick,
        response: MemoryResponse,
    },
    NoResponse,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryTraceKind {
    RequestSent,
    RequestArrived,
    ResponseArrived,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MemoryTraceEvent {
    tick: Tick,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    kind: MemoryTraceKind,
    request: MemoryRequestId,
    response_status: Option<ResponseStatus>,
}

impl MemoryTraceEvent {
    pub fn request(
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        kind: MemoryTraceKind,
        request: MemoryRequestId,
    ) -> Self {
        Self {
            tick,
            route,
            endpoint,
            kind,
            request,
            response_status: None,
        }
    }

    pub fn response(
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        request: MemoryRequestId,
        response_status: ResponseStatus,
    ) -> Self {
        Self {
            tick,
            route,
            endpoint,
            kind: MemoryTraceKind::ResponseArrived,
            request,
            response_status: Some(response_status),
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn kind(&self) -> MemoryTraceKind {
        self.kind
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.request
    }

    pub fn response_status(&self) -> Option<ResponseStatus> {
        self.response_status
    }
}

#[derive(Clone, Default)]
pub struct MemoryTrace {
    events: Arc<Mutex<Vec<MemoryTraceEvent>>>,
}

impl MemoryTrace {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn record(&self, event: MemoryTraceEvent) {
        self.events.lock().expect("memory trace lock").push(event);
    }

    pub fn snapshot(&self) -> Vec<MemoryTraceEvent> {
        self.events.lock().expect("memory trace lock").clone()
    }

    pub fn len(&self) -> usize {
        self.events.lock().expect("memory trace lock").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
