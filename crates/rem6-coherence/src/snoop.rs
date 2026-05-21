use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::MsiCacheController;
use rem6_directory::DirectoryDecision;
use rem6_fabric::{FabricError, FabricModel, FabricPacket, FabricPacketId};
use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext, Tick};
use rem6_memory::{AgentId, MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, TransportEndpointId,
    TransportError,
};

use super::{
    map_cache_error, partitioned_directory_source_data, response_record, CpuResponseRecord,
    DirectoryDecisionRecord, HarnessError,
};
use rem6_transport::TargetOutcome;

#[derive(Clone)]
pub(super) struct SnoopRoute {
    id: MemoryRouteId,
    route: MemoryRoute,
}

impl SnoopRoute {
    pub(super) fn new(id: MemoryRouteId, route: MemoryRoute) -> Self {
        Self { id, route }
    }
}

pub(super) struct DirectorySnoopWork {
    request: MemoryRequest,
    decision: DirectoryDecision,
    requester_route: SnoopRoute,
    cache_routes: BTreeMap<AgentId, SnoopRoute>,
    caches: BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MsiCacheController>>,
    responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
    decisions: Arc<Mutex<Vec<DirectoryDecisionRecord>>>,
}

impl DirectorySnoopWork {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        request: MemoryRequest,
        decision: DirectoryDecision,
        requester_route: SnoopRoute,
        cache_routes: BTreeMap<AgentId, SnoopRoute>,
        caches: BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
        fabric: Option<Arc<Mutex<FabricModel>>>,
        trace: MemoryTrace,
        response_cache: Arc<Mutex<MsiCacheController>>,
        responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
        decisions: Arc<Mutex<Vec<DirectoryDecisionRecord>>>,
    ) -> Self {
        Self {
            request,
            decision,
            requester_route,
            cache_routes,
            caches,
            fabric,
            trace,
            response_cache,
            responses,
            decisions,
        }
    }

    pub(super) fn schedule(
        self,
        context: &mut SchedulerContext<'_>,
        decision_tick: Tick,
    ) -> Result<(), HarnessError> {
        let source_data = partitioned_directory_source_data(&self.decision, &self.caches)?;
        self.decisions
            .lock()
            .expect("decision lock")
            .push(DirectoryDecisionRecord::new(
                decision_tick,
                self.request.id().agent(),
                self.decision.clone(),
            ));

        let snoop_ready_tick = schedule_directory_snoops(
            context,
            &self.decision,
            self.request.id(),
            &self.cache_routes,
            &self.caches,
            &self.fabric,
        )?;
        let snoop_delay =
            snoop_ready_tick
                .checked_sub(context.now())
                .ok_or(HarnessError::Transport(TransportError::Fabric(
                    FabricError::TickOverflow,
                )))?;

        let response =
            MemoryResponse::completed(&self.request, source_data).map_err(HarnessError::Memory)?;
        let response_work = SnoopResponseWork {
            route: self.requester_route,
            fabric: self.fabric,
            trace: self.trace,
            response_cache: self.response_cache,
            responses: self.responses,
        };
        context
            .schedule_local_after(snoop_delay, move |context| {
                response_work.schedule(context, response);
            })
            .map_err(HarnessError::Scheduler)?;

        Ok(())
    }

    pub(super) fn schedule_parallel(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        decision_tick: Tick,
    ) -> Result<(), HarnessError> {
        let source_data = partitioned_directory_source_data(&self.decision, &self.caches)?;
        self.decisions
            .lock()
            .expect("decision lock")
            .push(DirectoryDecisionRecord::new(
                decision_tick,
                self.request.id().agent(),
                self.decision.clone(),
            ));

        let snoop_ready_tick = schedule_directory_snoops_parallel(
            context,
            &self.decision,
            self.request.id(),
            &self.cache_routes,
            &self.caches,
            &self.fabric,
        )?;
        let snoop_delay =
            snoop_ready_tick
                .checked_sub(context.now())
                .ok_or(HarnessError::Transport(TransportError::Fabric(
                    FabricError::TickOverflow,
                )))?;

        let response =
            MemoryResponse::completed(&self.request, source_data).map_err(HarnessError::Memory)?;
        let response_work = SnoopResponseWork {
            route: self.requester_route,
            fabric: self.fabric,
            trace: self.trace,
            response_cache: self.response_cache,
            responses: self.responses,
        };
        context
            .schedule_local_after(snoop_delay, move |context| {
                response_work.schedule_parallel(context, response);
            })
            .map_err(HarnessError::Scheduler)?;

        Ok(())
    }
}

pub(super) fn schedule_directory_snoops(
    context: &mut SchedulerContext<'_>,
    decision: &DirectoryDecision,
    request: MemoryRequestId,
    cache_routes: &BTreeMap<AgentId, SnoopRoute>,
    caches: &BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    fabric: &Option<Arc<Mutex<FabricModel>>>,
) -> Result<Tick, HarnessError> {
    let mut max_snoop_delay = 0;
    for snoop in decision.snoops() {
        let snoop_route = cache_routes
            .get(&snoop.target())
            .ok_or(HarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let delay = route_response_delay(
            fabric,
            context.now(),
            snoop_route.id,
            &snoop_route.route,
            request,
            1,
        )
        .map_err(HarnessError::Transport)?;
        max_snoop_delay = max_snoop_delay.max(delay);

        let cache = caches
            .get(&snoop.target())
            .cloned()
            .ok_or(HarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let event = snoop.event();
        context
            .schedule_remote_after(snoop_route.route.source_partition(), delay, move |_| {
                cache
                    .lock()
                    .expect("cache lock")
                    .accept_snoop(event)
                    .map_err(map_cache_error)
                    .expect("scheduled snoop");
            })
            .map_err(HarnessError::Scheduler)?;
    }

    context
        .now()
        .checked_add(max_snoop_delay)
        .ok_or(HarnessError::Transport(TransportError::Fabric(
            FabricError::TickOverflow,
        )))
}

pub(super) fn schedule_directory_snoops_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    decision: &DirectoryDecision,
    request: MemoryRequestId,
    cache_routes: &BTreeMap<AgentId, SnoopRoute>,
    caches: &BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    fabric: &Option<Arc<Mutex<FabricModel>>>,
) -> Result<Tick, HarnessError> {
    let mut max_snoop_delay = 0;
    for snoop in decision.snoops() {
        let snoop_route = cache_routes
            .get(&snoop.target())
            .ok_or(HarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let delay = route_response_delay(
            fabric,
            context.now(),
            snoop_route.id,
            &snoop_route.route,
            request,
            1,
        )
        .map_err(HarnessError::Transport)?;
        max_snoop_delay = max_snoop_delay.max(delay);

        let cache = caches
            .get(&snoop.target())
            .cloned()
            .ok_or(HarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let event = snoop.event();
        context
            .schedule_remote_after(snoop_route.route.source_partition(), delay, move |_| {
                cache
                    .lock()
                    .expect("cache lock")
                    .accept_snoop(event)
                    .map_err(map_cache_error)
                    .expect("scheduled snoop");
            })
            .map_err(HarnessError::Scheduler)?;
    }

    context
        .now()
        .checked_add(max_snoop_delay)
        .ok_or(HarnessError::Transport(TransportError::Fabric(
            FabricError::TickOverflow,
        )))
}

struct SnoopResponseWork {
    route: SnoopRoute,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MsiCacheController>>,
    responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
}

impl SnoopResponseWork {
    fn schedule(self, context: &mut SchedulerContext<'_>, response: MemoryResponse) {
        let last_hop = self.route.route.hops().len() - 1;
        self.schedule_response_hop(context, last_hop, response);
    }

    fn schedule_parallel(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        response: MemoryResponse,
    ) {
        let last_hop = self.route.route.hops().len() - 1;
        self.schedule_response_hop_parallel(context, last_hop, response);
    }

    fn schedule_response_hop(
        self,
        context: &mut SchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) {
        let hop = self.route.route.hops()[hop_index].clone();
        let (endpoint, partition) = route_response_destination(&self.route.route, hop_index);
        let route_id = self.route.id;
        let delay = response_hop_delay(
            &self.fabric,
            context.now(),
            route_id,
            &self.route.route,
            &hop,
            response.request_id(),
            response_packet_bytes(&response),
        )
        .expect("validated snoop response fabric timing");
        context
            .schedule_remote_after(partition, delay, move |context| {
                self.trace.record(MemoryTraceEvent::response(
                    context.now(),
                    route_id,
                    endpoint,
                    response.request_id(),
                    response.status(),
                ));

                if hop_index == 0 {
                    let result = self
                        .response_cache
                        .lock()
                        .expect("cache lock")
                        .accept_fill(response)
                        .expect("cache fill");
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        self.responses
                            .lock()
                            .expect("response lock")
                            .push(response_record(context.now(), result.kind(), response));
                    }
                } else {
                    self.schedule_response_hop(context, hop_index - 1, response);
                }
            })
            .expect("validated snoop response latency");
    }

    fn schedule_response_hop_parallel(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) {
        let hop = self.route.route.hops()[hop_index].clone();
        let (endpoint, partition) = route_response_destination(&self.route.route, hop_index);
        let route_id = self.route.id;
        let delay = response_hop_delay(
            &self.fabric,
            context.now(),
            route_id,
            &self.route.route,
            &hop,
            response.request_id(),
            response_packet_bytes(&response),
        )
        .expect("validated snoop response fabric timing");
        context
            .schedule_remote_after(partition, delay, move |context| {
                self.trace.record(MemoryTraceEvent::response(
                    context.now(),
                    route_id,
                    endpoint,
                    response.request_id(),
                    response.status(),
                ));

                if hop_index == 0 {
                    let result = self
                        .response_cache
                        .lock()
                        .expect("cache lock")
                        .accept_fill(response)
                        .expect("cache fill");
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        self.responses
                            .lock()
                            .expect("response lock")
                            .push(response_record(context.now(), result.kind(), response));
                    }
                } else {
                    self.schedule_response_hop_parallel(context, hop_index - 1, response);
                }
            })
            .expect("validated snoop response latency");
    }
}

fn route_response_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: Tick,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    request: MemoryRequestId,
    packet_bytes: u64,
) -> Result<Tick, TransportError> {
    let mut tick = now;
    for hop in route.hops().iter().rev() {
        let delay = response_hop_delay(fabric, tick, route_id, route, hop, request, packet_bytes)?;
        tick = tick
            .checked_add(delay)
            .ok_or(TransportError::Fabric(FabricError::TickOverflow))?;
    }

    tick.checked_sub(now)
        .ok_or(TransportError::Fabric(FabricError::TickOverflow))
}

fn response_hop_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: Tick,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    hop: &MemoryRouteHop,
    request: MemoryRequestId,
    packet_bytes: u64,
) -> Result<Tick, TransportError> {
    let Some(path) = hop.response_fabric_path() else {
        return Ok(hop.response_latency());
    };
    let Some(fabric) = fabric else {
        return Err(TransportError::MissingFabricModel { route: route_id });
    };
    let packet = FabricPacket::new(
        fabric_packet_id(route_id, request),
        packet_bytes.max(1),
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

fn fabric_packet_id(route: MemoryRouteId, request: MemoryRequestId) -> FabricPacketId {
    let value = (1_u64 << 63)
        | ((route.get() & 0x7fff) << 48)
        | ((u64::from(request.agent().get()) & 0xffff) << 32)
        | (request.sequence() & 0xffff_ffff);
    FabricPacketId::new(value)
}

fn route_response_destination(
    route: &MemoryRoute,
    hop_index: usize,
) -> (TransportEndpointId, PartitionId) {
    if hop_index == 0 {
        (route.source().clone(), route.source_partition())
    } else {
        let previous_hop = &route.hops()[hop_index - 1];
        (previous_hop.endpoint().clone(), previous_hop.partition())
    }
}
