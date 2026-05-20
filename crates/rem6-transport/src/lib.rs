use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_kernel::{
    PartitionEventId, PartitionId, PartitionedScheduler, SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{MemoryRequest, MemoryRequestId, MemoryResponse, ResponseStatus};

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
            Self::Scheduler(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for TransportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Scheduler(error) => Some(error),
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
        })
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
        })
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
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StoredRoute {
    id: MemoryRouteId,
    route: MemoryRoute,
}

pub struct MemoryTransport {
    next_route_id: u64,
    routes: Vec<StoredRoute>,
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
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
        responder: F,
        response_sink: G,
    ) where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let hop = route.hops()[hop_index].clone();
        context
            .schedule_remote_after(hop.partition(), hop.request_latency(), move |context| {
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

                    if let TargetOutcome::Respond(response) = responder(delivery, context) {
                        Self::schedule_response_hop(
                            context,
                            route_id,
                            route,
                            hop_index,
                            response,
                            trace,
                            response_sink,
                        );
                    }
                } else {
                    Self::schedule_request_hop(
                        context,
                        route_id,
                        route,
                        hop_index + 1,
                        request,
                        trace,
                        responder,
                        response_sink,
                    );
                }
            })
            .expect("validated request transport latency");
    }

    fn schedule_response_hop<G>(
        context: &mut SchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        response: MemoryResponse,
        trace: MemoryTrace,
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
        context
            .schedule_remote_after(partition, hop.response_latency(), move |context| {
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

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
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
