use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::MsiCacheController;
use rem6_directory::DirectoryDecision;
use rem6_dram::DramMemoryController;
use rem6_fabric::{FabricError, FabricModel, FabricPacket, FabricPacketId};
use rem6_kernel::{PartitionId, SchedulerContext, Tick};
use rem6_memory::{AgentId, MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind,
    TargetOutcome, TransportEndpointId, TransportError,
};

use super::{
    apply_directory_snoops, response_record, CpuResponseRecord, DirectoryDecisionRecord,
    DramMemoryAccessRecord, HarnessError, LineBackingStore,
};

#[derive(Clone)]
pub(super) struct DeferredMemoryPath {
    pub(super) cache_route_id: MemoryRouteId,
    pub(super) cache_route: MemoryRoute,
    pub(super) memory_route_id: MemoryRouteId,
    pub(super) memory_route: MemoryRoute,
}

pub(super) struct DeferredMemoryWork {
    pub(super) path: DeferredMemoryPath,
    pub(super) caches: BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    pub(super) backing: Option<Arc<Mutex<LineBackingStore>>>,
    pub(super) dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    pub(super) fabric: Option<Arc<Mutex<FabricModel>>>,
    pub(super) trace: MemoryTrace,
    pub(super) response_cache: Arc<Mutex<MsiCacheController>>,
    pub(super) responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
    pub(super) decisions: Arc<Mutex<Vec<DirectoryDecisionRecord>>>,
    pub(super) dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
}

impl DeferredMemoryWork {
    pub(super) fn schedule(
        self,
        context: &mut SchedulerContext<'_>,
        request: MemoryRequest,
        decision: DirectoryDecision,
    ) -> Result<(), HarnessError> {
        apply_directory_snoops(&decision, &self.caches)?;
        self.decisions
            .lock()
            .expect("decision lock")
            .push(DirectoryDecisionRecord::new(
                context.now(),
                request.id().agent(),
                decision,
            ));
        self.trace.record(MemoryTraceEvent::request(
            context.now(),
            self.path.memory_route_id,
            self.path.memory_route.source().clone(),
            MemoryTraceKind::RequestSent,
            request.id(),
        ));

        let request_work = DeferredMemoryRequestWork {
            path: self.path,
            backing: self.backing,
            dram_memory: self.dram_memory,
            fabric: self.fabric,
            trace: self.trace,
            response_cache: self.response_cache,
            responses: self.responses,
            dram_accesses: self.dram_accesses,
        };
        request_work.schedule_hop(context, 0, request);

        Ok(())
    }
}

struct DeferredMemoryRequestWork {
    path: DeferredMemoryPath,
    backing: Option<Arc<Mutex<LineBackingStore>>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MsiCacheController>>,
    responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
}

impl DeferredMemoryRequestWork {
    fn schedule_hop(
        self,
        context: &mut SchedulerContext<'_>,
        hop_index: usize,
        request: MemoryRequest,
    ) {
        let hop = self.path.memory_route.hops()[hop_index].clone();
        let route_id = self.path.memory_route_id;
        let delay = deferred_request_hop_delay(
            &self.fabric,
            context.now(),
            route_id,
            &self.path.memory_route,
            &hop,
            &request,
        )
        .expect("validated deferred request fabric timing");
        context
            .schedule_remote_after(hop.partition(), delay, move |context| {
                self.trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    hop.endpoint().clone(),
                    MemoryTraceKind::RequestArrived,
                    request.id(),
                ));

                if hop_index + 1 == self.path.memory_route.hops().len() {
                    self.complete_target(context, request);
                } else {
                    self.schedule_hop(context, hop_index + 1, request);
                }
            })
            .expect("validated memory request latency");
    }

    fn complete_target(self, context: &mut SchedulerContext<'_>, request: MemoryRequest) {
        let Self {
            path,
            backing,
            dram_memory,
            fabric,
            trace,
            response_cache,
            responses,
            dram_accesses,
        } = self;
        let response_work = DeferredMemoryResponseWork {
            path,
            fabric,
            trace,
            response_cache,
            responses,
        };

        if let Some(dram_memory) = dram_memory {
            let outcome = dram_memory
                .lock()
                .expect("DRAM memory lock")
                .accept(context.now(), &request)
                .expect("DRAM memory response");
            dram_accesses
                .lock()
                .expect("DRAM access lock")
                .push(DramMemoryAccessRecord::new(
                    context.now(),
                    outcome.target(),
                    outcome.dram_access().request(),
                    outcome.dram_access().bank(),
                    outcome.dram_access().row(),
                    outcome.dram_access().row_hit(),
                    outcome.ready_cycle(),
                ));
            let ready_delay = outcome
                .ready_cycle()
                .checked_sub(context.now())
                .expect("DRAM ready cycle is not in the past");
            let response = outcome
                .response()
                .cloned()
                .expect("directory backing read expects memory response");
            context
                .schedule_local_after(ready_delay, move |context| {
                    response_work.schedule(context, response);
                })
                .expect("validated DRAM ready latency");
        } else {
            let response = backing
                .as_ref()
                .expect("line backing memory")
                .lock()
                .expect("backing lock")
                .respond(&request)
                .expect("backing store response");
            response_work.schedule(context, response);
        }
    }
}

struct DeferredMemoryResponseWork {
    path: DeferredMemoryPath,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MsiCacheController>>,
    responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
}

impl DeferredMemoryResponseWork {
    fn schedule(self, context: &mut SchedulerContext<'_>, response: MemoryResponse) {
        let last_hop = self.path.memory_route.hops().len() - 1;
        self.schedule_memory_response_hop(context, last_hop, response);
    }

    fn schedule_memory_response_hop(
        self,
        context: &mut SchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) {
        let hop = self.path.memory_route.hops()[hop_index].clone();
        let (endpoint, partition) = route_response_destination(&self.path.memory_route, hop_index);
        let route_id = self.path.memory_route_id;
        let delay = deferred_response_hop_delay(
            &self.fabric,
            context.now(),
            route_id,
            &self.path.memory_route,
            &hop,
            &response,
        )
        .expect("validated deferred memory response fabric timing");
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
                    let last_hop = self.path.cache_route.hops().len() - 1;
                    self.schedule_cache_response_hop(context, last_hop, response);
                } else {
                    self.schedule_memory_response_hop(context, hop_index - 1, response);
                }
            })
            .expect("validated memory response latency");
    }

    fn schedule_cache_response_hop(
        self,
        context: &mut SchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) {
        let hop = self.path.cache_route.hops()[hop_index].clone();
        let (endpoint, partition) = route_response_destination(&self.path.cache_route, hop_index);
        let route_id = self.path.cache_route_id;
        let delay = deferred_response_hop_delay(
            &self.fabric,
            context.now(),
            route_id,
            &self.path.cache_route,
            &hop,
            &response,
        )
        .expect("validated deferred cache response fabric timing");
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
                    self.schedule_cache_response_hop(context, hop_index - 1, response);
                }
            })
            .expect("validated cache response latency");
    }
}

fn deferred_request_hop_delay(
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
        deferred_fabric_packet_id(route_id, request.id(), false),
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

fn deferred_response_hop_delay(
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
        deferred_fabric_packet_id(route_id, response.request_id(), true),
        deferred_response_packet_bytes(response),
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

fn deferred_response_packet_bytes(response: &MemoryResponse) -> u64 {
    response
        .data()
        .map_or(1, |bytes| (bytes.len() as u64).max(1))
}

fn deferred_fabric_packet_id(
    route: MemoryRouteId,
    request: MemoryRequestId,
    response: bool,
) -> FabricPacketId {
    let direction = u64::from(response);
    let value = (direction << 63)
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
