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
pub struct MemoryRoute {
    source: TransportEndpointId,
    source_partition: PartitionId,
    target: TransportEndpointId,
    target_partition: PartitionId,
    request_latency: Tick,
    response_latency: Tick,
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
            source,
            source_partition,
            target,
            target_partition,
            request_latency,
            response_latency,
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

                let target_route = route.clone();
                let target_trace = trace.clone();
                context
                    .schedule_remote_after(
                        route.target_partition(),
                        route.request_latency(),
                        move |context| {
                            target_trace.record(MemoryTraceEvent::request(
                                context.now(),
                                route_id,
                                target_route.target().clone(),
                                MemoryTraceKind::RequestArrived,
                                request_id,
                            ));

                            let delivery = RequestDelivery {
                                tick: context.now(),
                                route: route_id,
                                endpoint: target_route.target().clone(),
                                request,
                            };

                            match responder(delivery, context) {
                                TargetOutcome::Respond(response) => {
                                    let response_status = response.status();
                                    let source_endpoint = target_route.source().clone();
                                    let response_trace = target_trace.clone();
                                    context
                                        .schedule_remote_after(
                                            target_route.source_partition(),
                                            target_route.response_latency(),
                                            move |context| {
                                                response_trace.record(MemoryTraceEvent::response(
                                                    context.now(),
                                                    route_id,
                                                    source_endpoint.clone(),
                                                    response.request_id(),
                                                    response_status,
                                                ));
                                                response_sink(ResponseDelivery {
                                                    tick: context.now(),
                                                    route: route_id,
                                                    endpoint: source_endpoint,
                                                    response,
                                                });
                                            },
                                        )
                                        .expect("validated response transport latency");
                                }
                                TargetOutcome::NoResponse => {}
                            }
                        },
                    )
                    .expect("validated request transport latency");
            })
            .map_err(TransportError::Scheduler)
    }

    fn validate_scheduler_route(
        &self,
        scheduler: &PartitionedScheduler,
        route_id: MemoryRouteId,
        route: &MemoryRoute,
    ) -> Result<(), TransportError> {
        scheduler
            .partition_now(route.source_partition())
            .map_err(TransportError::Scheduler)?;
        scheduler
            .partition_now(route.target_partition())
            .map_err(TransportError::Scheduler)?;

        if route.source_partition() != route.target_partition() {
            let minimum = scheduler.min_remote_delay();
            if route.request_latency() < minimum {
                return Err(TransportError::LatencyBelowLookahead {
                    route: route_id,
                    latency: TransportLatency::Request,
                    delay: route.request_latency(),
                    minimum,
                });
            }
            if route.response_latency() < minimum {
                return Err(TransportError::LatencyBelowLookahead {
                    route: route_id,
                    latency: TransportLatency::Response,
                    delay: route.response_latency(),
                    minimum,
                });
            }
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
