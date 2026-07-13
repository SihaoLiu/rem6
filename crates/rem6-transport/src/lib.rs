use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_fabric::{
    FabricActivityMarker, FabricActivityProfile, FabricError, FabricHopActivity,
    FabricLaneActivity, FabricModel, FabricPacket, FabricPacketId, FabricTransfer,
    FabricWaitForMarker, QosFixedPriorityPolicy, QosPriorityPolicy, QosQueueArbiter,
};
pub use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler,
    SchedulerContext, SchedulerError, Tick, WaitForGraph,
};
use rem6_memory::{MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_topology::{Endpoint, Topology};

mod message_buffer;
mod ordering;
mod parallel_qos;
mod parallel_submit;
mod qos_activity;
mod response_qos;
mod route;
mod trace;

pub use message_buffer::{
    TransportMessageAdmission, TransportMessageBuffer, TransportMessageBufferConfig,
    TransportMessageBufferError, TransportMessageBufferSnapshot, TransportQueuedMessage,
};
pub use qos_activity::{
    FabricQosGrantActivity, FabricQosGrantDirection, FabricQosSuppressedRequest,
    FabricQosSuppressionReason, SharedFabricQosState,
};
use route::StoredRoute;
pub use route::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, TopologyRouteError, TransportEndpointId,
    TransportError, TransportLatency, TransportQosClass,
};
pub use trace::{
    MemoryTrace, MemoryTraceEvent, MemoryTraceKind, RequestDelivery, ResponseDelivery,
    TargetBatchOutcome, TargetOutcome,
};

type ParallelRequestResponder =
    Box<dyn FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome + Send>;
type ResponseSink = Box<dyn FnOnce(ResponseDelivery) + Send>;
type ParallelTargetBatchResponder = Arc<
    dyn for<'a> Fn(
            Vec<RequestDelivery>,
            &mut ParallelSchedulerContext<'a>,
        ) -> Option<Vec<TargetBatchOutcome>>
        + Send
        + Sync,
>;
type DirectTargetBatchKey = (Tick, PartitionId, TransportEndpointId);
type SharedPendingDirectTargetBatch = Arc<Mutex<PendingDirectTargetBatch>>;

pub struct ParallelMemoryTransaction {
    route: MemoryRouteId,
    request: MemoryRequest,
    trace: MemoryTrace,
    responder: ParallelRequestResponder,
    response_sink: ResponseSink,
    qos_requestor: Option<QosRequestorId>,
    qos_priority: Option<QosPriority>,
}

impl ParallelMemoryTransaction {
    pub fn new<F, G>(
        route: MemoryRouteId,
        request: MemoryRequest,
        trace: MemoryTrace,
        responder: F,
        response_sink: G,
    ) -> Self
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        Self {
            route,
            request,
            trace,
            responder: Box::new(responder),
            response_sink: Box::new(response_sink),
            qos_requestor: None,
            qos_priority: None,
        }
    }

    pub fn with_qos_priority(mut self, priority: QosPriority) -> Self {
        self.qos_priority = Some(priority);
        self
    }

    pub fn with_qos_requestor(mut self, requestor: QosRequestorId) -> Self {
        self.qos_requestor = Some(requestor);
        self
    }

    pub fn with_qos_class(mut self, qos: TransportQosClass) -> Self {
        self.qos_requestor = Some(qos.requestor());
        self.qos_priority = Some(qos.priority());
        self
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn request(&self) -> &MemoryRequest {
        &self.request
    }
}

struct PreparedParallelTransaction {
    route_id: MemoryRouteId,
    route: MemoryRoute,
    request: MemoryRequest,
    trace: MemoryTrace,
    responder: ParallelRequestResponder,
    response_sink: ResponseSink,
    first_hop_delay: Tick,
    qos_requestor: QosRequestorId,
    qos_priority: QosPriority,
    response_qos: Option<response_qos::ResponseQosContext>,
}

struct PendingDirectTargetBatch {
    event: Option<PartitionEventId>,
    transactions: Vec<PreparedParallelTransaction>,
}

pub struct MemoryTransport {
    next_route_id: u64,
    routes: Vec<StoredRoute>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    qos_state: Option<SharedFabricQosState>,
    qos_priority_policy: Option<Arc<Mutex<QosPriorityPolicy>>>,
    direct_target_batch_responder: Option<ParallelTargetBatchResponder>,
    direct_target_batches:
        Arc<Mutex<BTreeMap<DirectTargetBatchKey, SharedPendingDirectTargetBatch>>>,
}

impl MemoryTransport {
    pub fn new() -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: None,
            qos_state: None,
            qos_priority_policy: None,
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_fabric(fabric: FabricModel) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(Arc::new(Mutex::new(fabric))),
            qos_state: None,
            qos_priority_policy: None,
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_shared_fabric(fabric: Arc<Mutex<FabricModel>>) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(fabric),
            qos_state: None,
            qos_priority_policy: None,
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_fabric_qos(fabric: FabricModel, arbiter: QosQueueArbiter) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(Arc::new(Mutex::new(fabric))),
            qos_state: Some(SharedFabricQosState::new(arbiter)),
            qos_priority_policy: None,
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_qos_policy(
        arbiter: QosQueueArbiter,
        priority_policy: QosFixedPriorityPolicy,
    ) -> Self {
        Self::with_qos_priority_policy(arbiter, priority_policy.into())
    }

    pub fn with_qos_priority_policy(
        arbiter: QosQueueArbiter,
        priority_policy: QosPriorityPolicy,
    ) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: None,
            qos_state: Some(SharedFabricQosState::new(arbiter)),
            qos_priority_policy: Some(Arc::new(Mutex::new(priority_policy))),
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_fabric_qos_policy(
        fabric: FabricModel,
        arbiter: QosQueueArbiter,
        priority_policy: QosFixedPriorityPolicy,
    ) -> Self {
        Self::with_fabric_qos_priority_policy(fabric, arbiter, priority_policy.into())
    }

    pub fn with_fabric_qos_priority_policy(
        fabric: FabricModel,
        arbiter: QosQueueArbiter,
        priority_policy: QosPriorityPolicy,
    ) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(Arc::new(Mutex::new(fabric))),
            qos_state: Some(SharedFabricQosState::new(arbiter)),
            qos_priority_policy: Some(Arc::new(Mutex::new(priority_policy))),
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_shared_fabric_qos(
        fabric: Arc<Mutex<FabricModel>>,
        qos_state: SharedFabricQosState,
    ) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(fabric),
            qos_state: Some(qos_state),
            qos_priority_policy: None,
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_shared_fabric_qos_policy(
        fabric: Arc<Mutex<FabricModel>>,
        qos_state: SharedFabricQosState,
        priority_policy: QosFixedPriorityPolicy,
    ) -> Self {
        Self {
            next_route_id: 0,
            routes: Vec::new(),
            fabric: Some(fabric),
            qos_state: Some(qos_state),
            qos_priority_policy: Some(Arc::new(Mutex::new(priority_policy.into()))),
            direct_target_batch_responder: None,
            direct_target_batches: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    pub fn with_direct_target_batch_responder<F>(mut self, responder: F) -> Self
    where
        F: for<'a> Fn(
                Vec<RequestDelivery>,
                &mut ParallelSchedulerContext<'a>,
            ) -> Option<Vec<TargetBatchOutcome>>
            + Send
            + Sync
            + 'static,
    {
        self.direct_target_batch_responder = Some(Arc::new(responder));
        self
    }

    pub fn fabric(&self) -> Option<Arc<Mutex<FabricModel>>> {
        self.fabric.as_ref().map(Arc::clone)
    }

    pub fn mark_fabric_activity(&self) -> Option<FabricActivityMarker> {
        self.fabric
            .as_ref()
            .map(|fabric| fabric.lock().expect("fabric lock").mark_activity())
    }

    pub fn mark_fabric_wait_for(&self) -> Option<FabricWaitForMarker> {
        self.fabric
            .as_ref()
            .map(|fabric| fabric.lock().expect("fabric lock").mark_wait_for())
    }

    pub fn fabric_lane_activities(&self) -> Option<Vec<FabricLaneActivity>> {
        self.fabric
            .as_ref()
            .map(|fabric| fabric.lock().expect("fabric lock").lane_activities())
    }

    pub fn fabric_hop_activities(&self) -> Option<Vec<FabricHopActivity>> {
        self.fabric
            .as_ref()
            .map(|fabric| fabric.lock().expect("fabric lock").hop_activities())
    }

    pub fn fabric_qos_grant_activities(&self) -> Vec<FabricQosGrantActivity> {
        self.qos_state
            .as_ref()
            .map(|state| {
                state
                    .inner
                    .lock()
                    .expect("fabric QoS state lock")
                    .activity
                    .grants()
                    .to_vec()
            })
            .unwrap_or_default()
    }

    pub fn fabric_lane_activities_since(
        &self,
        marker: FabricActivityMarker,
    ) -> Option<Vec<FabricLaneActivity>> {
        self.fabric.as_ref().map(|fabric| {
            fabric
                .lock()
                .expect("fabric lock")
                .lane_activities_since(marker)
        })
    }

    pub fn fabric_hop_activities_since(
        &self,
        marker: FabricActivityMarker,
    ) -> Option<Vec<FabricHopActivity>> {
        self.fabric.as_ref().map(|fabric| {
            fabric
                .lock()
                .expect("fabric lock")
                .hop_activities_since(marker)
        })
    }

    pub fn fabric_activity_profile(&self) -> Option<FabricActivityProfile> {
        self.fabric
            .as_ref()
            .map(|fabric| fabric.lock().expect("fabric lock").activity_profile())
    }

    pub fn fabric_activity_profile_since(
        &self,
        marker: FabricActivityMarker,
    ) -> Option<FabricActivityProfile> {
        self.fabric.as_ref().map(|fabric| {
            fabric
                .lock()
                .expect("fabric lock")
                .activity_profile_since(marker)
        })
    }

    pub fn fabric_wait_for_graph_at(&self, tick: Tick) -> Option<WaitForGraph> {
        self.fabric
            .as_ref()
            .map(|fabric| fabric.lock().expect("fabric lock").wait_for_graph_at(tick))
    }

    pub fn fabric_wait_for_graph_since(&self, marker: FabricWaitForMarker) -> Option<WaitForGraph> {
        self.fabric.as_ref().map(|fabric| {
            fabric
                .lock()
                .expect("fabric lock")
                .wait_for_graph_since(marker)
        })
    }

    pub fn add_route(&mut self, route: MemoryRoute) -> Result<MemoryRouteId, TransportError> {
        if self.routes.iter().any(|stored| {
            stored.route.source() == route.source() && stored.route.target() == route.target()
        }) {
            return Err(TransportError::DuplicateRoute {
                source: route.source().clone(),
                target: route.target().clone(),
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
        let source_partition = route.source_partition();
        let start_tick = scheduler.now();
        self.validate_scheduler_route(scheduler, route_id, &route, start_tick)?;
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

    pub fn submit_parallel_batch<I>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transactions: I,
    ) -> Result<Vec<PartitionEventId>, TransportError>
    where
        I: IntoIterator<Item = ParallelMemoryTransaction>,
    {
        let start_tick = scheduler.now();
        let mut priority_policy = self
            .qos_priority_policy
            .as_ref()
            .map(|policy| policy.lock().expect("QoS priority policy lock").clone());
        let mut prepared = Vec::new();
        for transaction in transactions {
            prepared.push(self.prepare_parallel_transaction(
                scheduler,
                start_tick,
                transaction,
                priority_policy.as_mut(),
            )?);
        }

        if self.can_submit_direct_qos_parallel_batch(&prepared) {
            let result = self.submit_direct_qos_parallel_batch(scheduler, start_tick, prepared);
            if result.is_ok() {
                self.commit_qos_priority_policy(priority_policy);
            }
            return result;
        }

        let (delays, fabric_requests) = self.prepare_parallel_batch_first_hops(&prepared)?;
        let result =
            self.reserve_parallel_batch_first_hops(start_tick, delays, fabric_requests, |delays| {
                self.schedule_prepared_parallel_batch(scheduler, start_tick, prepared, delays)
            });
        if result.is_ok() {
            self.commit_qos_priority_policy(priority_policy);
        }
        result
    }

    fn commit_qos_priority_policy(&self, priority_policy: Option<QosPriorityPolicy>) {
        if let (Some(staged), Some(policy)) = (priority_policy, &self.qos_priority_policy) {
            *policy.lock().expect("QoS priority policy lock") = staged;
        }
    }

    fn prepare_parallel_transaction(
        &self,
        scheduler: &PartitionedScheduler,
        start_tick: Tick,
        transaction: ParallelMemoryTransaction,
        priority_policy: Option<&mut QosPriorityPolicy>,
    ) -> Result<PreparedParallelTransaction, TransportError> {
        let route = self
            .route(transaction.route)
            .cloned()
            .ok_or(TransportError::UnknownRoute {
                route: transaction.route,
            })?;
        self.validate_scheduler_route(scheduler, transaction.route, &route, start_tick)?;
        let source_now = scheduler
            .partition_now(route.source_partition())
            .map_err(TransportError::Scheduler)?;
        if start_tick < source_now {
            return Err(TransportError::Scheduler(SchedulerError::InThePast {
                partition: route.source_partition(),
                now: source_now,
                requested: start_tick,
            }));
        }

        let qos_requestor = transaction
            .qos_requestor
            .unwrap_or_else(|| QosRequestorId::new(transaction.request.id().agent().get()));
        let qos_priority = match transaction.qos_priority {
            Some(priority) => priority,
            None => match priority_policy {
                Some(policy) => policy
                    .priority_for(qos_requestor, transaction.request.size().bytes())
                    .map_err(|source| TransportError::Qos { source })?,
                None => QosPriority::new(0),
            },
        };

        Ok(PreparedParallelTransaction {
            route_id: transaction.route,
            route,
            request: transaction.request,
            trace: transaction.trace,
            responder: transaction.responder,
            response_sink: transaction.response_sink,
            first_hop_delay: 0,
            qos_requestor,
            qos_priority,
            response_qos: self.response_qos_context(qos_requestor, qos_priority),
        })
    }

    fn prepare_parallel_batch_first_hops(
        &self,
        transactions: &[PreparedParallelTransaction],
    ) -> Result<(Vec<Tick>, Vec<ordering::OrderedFabricQosRequest>), TransportError> {
        let mut delays = vec![0; transactions.len()];
        let mut fabric_requests = Vec::new();

        for (index, transaction) in transactions.iter().enumerate() {
            let hop = &transaction.route.hops()[0];
            let Some(path) = hop.request_fabric_path() else {
                delays[index] = hop.request_latency();
                continue;
            };
            if self.fabric.is_none() {
                return Err(TransportError::MissingFabricModel {
                    route: transaction.route_id,
                });
            }
            let packet = FabricPacket::new(
                fabric_packet_id(
                    transaction.route_id,
                    transaction.request.id(),
                    TransportLatency::Request,
                ),
                transaction.request.size().bytes(),
                transaction.route.request_virtual_network(),
            )
            .map_err(TransportError::Fabric)?;
            fabric_requests.push(ordering::OrderedFabricQosRequest::new(
                index,
                packet,
                path.clone(),
                transaction.request.clone(),
                transaction.qos_requestor,
                transaction.qos_priority,
            ));
        }

        Ok((delays, fabric_requests))
    }

    fn reserve_parallel_batch_first_hops<T, F>(
        &self,
        now: Tick,
        mut delays: Vec<Tick>,
        fabric_requests: Vec<ordering::OrderedFabricQosRequest>,
        commit: F,
    ) -> Result<T, TransportError>
    where
        F: FnOnce(Vec<Tick>) -> Result<T, TransportError>,
    {
        if fabric_requests.is_empty() {
            return commit(delays);
        }

        let fabric = self
            .fabric
            .as_ref()
            .expect("fabric presence is checked before batch reservation");
        if let Some(qos_state) = &self.qos_state {
            let mut fabric = fabric.lock().expect("fabric lock");
            let mut qos_state = qos_state.inner.lock().expect("fabric QoS state lock");
            let arbiter_checkpoint = qos_state.request_arbiter.clone();
            let batch = qos_state.activity.next_batch();
            let result = fabric.try_transaction(|fabric| {
                let (transfers, activities) = ordering::transmit_ordered_qos_fabric_batch(
                    now,
                    batch,
                    &fabric_requests,
                    fabric,
                    &mut qos_state.request_arbiter,
                )
                .map_err(TransportError::Fabric)?;
                apply_fabric_transfer_delays(now, &fabric_requests, transfers, &mut delays)?;
                commit(delays).map(|value| (value, activities))
            });
            return match result {
                Ok((value, activities)) => {
                    qos_state.activity.commit_batch(batch, activities);
                    Ok(value)
                }
                Err(error) => {
                    qos_state.request_arbiter = arbiter_checkpoint;
                    Err(error)
                }
            };
        }

        fabric
            .lock()
            .expect("fabric lock")
            .try_transaction(|fabric| {
                let transfers = fabric
                    .transmit_batch(
                        now,
                        fabric_requests
                            .iter()
                            .map(|request| (request.packet().clone(), request.path().clone())),
                    )
                    .map_err(TransportError::Fabric)?;
                apply_fabric_transfer_delays(now, &fabric_requests, transfers, &mut delays)?;
                commit(delays)
            })
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
    fn schedule_parallel_request_hop_with_delay<F, G>(
        context: &mut ParallelSchedulerContext<'_>,
        route_id: MemoryRouteId,
        route: MemoryRoute,
        hop_index: usize,
        request: MemoryRequest,
        trace: MemoryTrace,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        response_qos: Option<response_qos::ResponseQosContext>,
        responder: F,
        response_sink: G,
        delay: Tick,
    ) where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
        G: FnOnce(ResponseDelivery) + Send + 'static,
    {
        let hop = route.hops()[hop_index].clone();
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
                                response_qos,
                                Box::new(response_sink),
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
                                        response_qos,
                                        Box::new(response_sink),
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
                        response_qos,
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
        response_qos: Option<response_qos::ResponseQosContext>,
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
                                response_qos,
                                Box::new(response_sink),
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
                                        response_qos,
                                        Box::new(response_sink),
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
                        response_qos,
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

    fn validate_scheduler_route(
        &self,
        scheduler: &PartitionedScheduler,
        route_id: MemoryRouteId,
        route: &MemoryRoute,
        start_tick: Tick,
    ) -> Result<(), TransportError> {
        let mut previous_partition = route.source_partition();
        scheduler
            .partition_now(previous_partition)
            .map_err(TransportError::Scheduler)?;
        let mut request_tick = start_tick;

        for hop in route.hops() {
            if self.fabric.is_none()
                && (hop.request_fabric_path().is_some() || hop.response_fabric_path().is_some())
            {
                return Err(TransportError::MissingFabricModel { route: route_id });
            }
            let target_now = scheduler
                .partition_now(hop.partition())
                .map_err(TransportError::Scheduler)?;

            request_tick = validate_transport_hop_boundary(
                previous_partition,
                hop.partition(),
                request_tick,
                hop.request_latency(),
                scheduler.min_remote_delay(),
            )?;
            if request_tick < target_now {
                return Err(TransportError::Scheduler(SchedulerError::InThePast {
                    partition: hop.partition(),
                    now: target_now,
                    requested: request_tick,
                }));
            }
            previous_partition = hop.partition();
        }

        let mut response_tick = request_tick;
        let mut response_source = route.target_partition();
        for (hop_index, hop) in route.hops().iter().enumerate().rev() {
            let response_target = if hop_index == 0 {
                route.source_partition()
            } else {
                route.hops()[hop_index - 1].partition()
            };
            response_tick = validate_transport_hop_boundary(
                response_source,
                response_target,
                response_tick,
                hop.response_latency(),
                scheduler.min_remote_delay(),
            )?;
            let target_now = scheduler
                .partition_now(response_target)
                .map_err(TransportError::Scheduler)?;
            if response_tick < target_now {
                return Err(TransportError::Scheduler(SchedulerError::InThePast {
                    partition: response_target,
                    now: target_now,
                    requested: response_tick,
                }));
            }
            response_source = response_target;
        }

        Ok(())
    }
}

fn validate_transport_hop_boundary(
    source: PartitionId,
    target: PartitionId,
    source_tick: Tick,
    delay: Tick,
    min_remote_delay: Tick,
) -> Result<Tick, TransportError> {
    let delivery_tick = source_tick
        .checked_add(delay)
        .ok_or(TransportError::Scheduler(SchedulerError::TickOverflow {
            now: source_tick,
            delay,
        }))?;
    if source == target {
        return Ok(delivery_tick);
    }

    let minimum_delivery_tick =
        source_tick
            .checked_add(min_remote_delay)
            .ok_or(TransportError::Scheduler(SchedulerError::TickOverflow {
                now: source_tick,
                delay: min_remote_delay,
            }))?;
    if delivery_tick < minimum_delivery_tick {
        return Err(TransportError::Scheduler(
            SchedulerError::RemoteDeliveryBeforeLookaheadBoundary {
                source,
                target,
                source_tick,
                delivery_tick,
                minimum_delivery_tick,
            },
        ));
    }

    Ok(delivery_tick)
}

impl Default for MemoryTransport {
    fn default() -> Self {
        Self::new()
    }
}

fn apply_fabric_transfer_delays(
    now: Tick,
    requests: &[ordering::OrderedFabricQosRequest],
    transfers: Vec<FabricTransfer>,
    delays: &mut [Tick],
) -> Result<(), TransportError> {
    let mut arrivals = transfers
        .into_iter()
        .map(|transfer| {
            let delay = transfer
                .arrival_tick()
                .checked_sub(now)
                .ok_or(TransportError::Fabric(FabricError::TickOverflow))?;
            Ok((transfer.packet().id(), delay))
        })
        .collect::<Result<BTreeMap<_, _>, TransportError>>()?;

    for request in requests {
        delays[request.transaction_index()] = arrivals
            .remove(&request.packet().id())
            .expect("fabric batch returns every accepted packet");
    }
    Ok(())
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
