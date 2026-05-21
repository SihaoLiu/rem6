use std::sync::{Arc, Mutex};

use rem6_cache::MoesiCacheController;
use rem6_dram::DramMemoryController;
use rem6_fabric::{FabricError, FabricModel, FabricPacket, FabricPacketId};
use rem6_kernel::{ParallelSchedulerContext, PartitionId};
use rem6_memory::{MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse};
use rem6_protocol_moesi::{MoesiEvent, MoesiLineId};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind,
    TargetOutcome, TransportEndpointId, TransportError,
};

use crate::wait_for::CoherenceWaitFor;
use crate::{DramMemoryAccessRecord, LineBackingStore};

use super::{
    moesi_response_record, MoesiCpuResponseRecord, MoesiHarnessError, PartitionedMoesiRoute,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_moesi_memory_response_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    request: MemoryRequest,
    fill_event: MoesiEvent,
    requester_route: PartitionedMoesiRoute,
    memory_route: PartitionedMoesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MoesiCacheController>>,
    responses: Arc<Mutex<Vec<MoesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MoesiLineId,
    snoop_delay: u64,
) -> Result<(), MoesiHarnessError> {
    PartitionedMoesiMemoryResponseWork {
        directory_tick: context.now(),
        fill_event,
        requester_route,
        memory_route,
        backing,
        dram_memory,
        fabric,
        trace,
        response_cache,
        responses,
        dram_accesses,
        wait_for,
        line,
        snoop_delay,
    }
    .schedule(context, request)
}

struct PartitionedMoesiMemoryResponseWork {
    directory_tick: u64,
    fill_event: MoesiEvent,
    requester_route: PartitionedMoesiRoute,
    memory_route: PartitionedMoesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MoesiCacheController>>,
    responses: Arc<Mutex<Vec<MoesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MoesiLineId,
    snoop_delay: u64,
}

impl PartitionedMoesiMemoryResponseWork {
    fn schedule(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MemoryRequest,
    ) -> Result<(), MoesiHarnessError> {
        self.trace.record(MemoryTraceEvent::request(
            context.now(),
            self.memory_route.id,
            self.memory_route.route.source().clone(),
            MemoryTraceKind::RequestSent,
            request.id(),
        ));
        self.schedule_request_hop(context, 0, request)
    }

    fn schedule_request_hop(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        hop_index: usize,
        request: MemoryRequest,
    ) -> Result<(), MoesiHarnessError> {
        let hop = self.memory_route.route.hops()[hop_index].clone();
        let route_id = self.memory_route.id;
        let delay = moesi_request_hop_delay(
            &self.fabric,
            context.now(),
            route_id,
            &self.memory_route.route,
            &hop,
            &request,
        )
        .expect("validated MOESI memory request fabric timing");
        context
            .schedule_remote_after(hop.partition(), delay, move |context| {
                self.trace.record(MemoryTraceEvent::request(
                    context.now(),
                    route_id,
                    hop.endpoint().clone(),
                    MemoryTraceKind::RequestArrived,
                    request.id(),
                ));

                if hop_index + 1 == self.memory_route.route.hops().len() {
                    self.complete_target(context, request);
                } else {
                    self.schedule_request_hop(context, hop_index + 1, request)
                        .expect("scheduled memory request hop");
                }
            })
            .map(|_| ())
            .map_err(MoesiHarnessError::Scheduler)
    }

    fn complete_target(self, context: &mut ParallelSchedulerContext<'_>, request: MemoryRequest) {
        let (ready_tick, response) = complete_partitioned_moesi_memory_request(
            context.now(),
            &request,
            &self.backing,
            self.dram_memory.as_ref(),
            &self.dram_accesses,
        )
        .expect("memory response");
        context
            .schedule_local_after(
                ready_tick
                    .checked_sub(context.now())
                    .expect("DRAM ready tick is not in the past"),
                move |context| {
                    let last_hop = self.memory_route.route.hops().len() - 1;
                    self.schedule_response_hop(context, last_hop, response)
                        .expect("scheduled memory response hop");
                },
            )
            .expect("validated DRAM ready latency");
    }

    fn schedule_response_hop(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) -> Result<(), MoesiHarnessError> {
        let hop = self.memory_route.route.hops()[hop_index].clone();
        let (endpoint, partition) =
            moesi_route_response_destination(&self.memory_route.route, hop_index);
        let route_id = self.memory_route.id;
        let delay = moesi_response_hop_delay(
            &self.fabric,
            context.now(),
            route_id,
            &self.memory_route.route,
            &hop,
            &response,
        )
        .expect("validated MOESI memory response fabric timing");
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
                    let elapsed = context
                        .now()
                        .checked_sub(self.directory_tick)
                        .expect("memory response is after directory request");
                    let wait_for_snoops = self.snoop_delay.saturating_sub(elapsed);
                    schedule_partitioned_moesi_cache_response_parallel(
                        context,
                        wait_for_snoops,
                        self.requester_route,
                        response,
                        self.fill_event,
                        self.fabric,
                        self.trace,
                        self.response_cache,
                        self.responses,
                        self.wait_for,
                        self.line,
                    )
                    .expect("scheduled cache response");
                } else {
                    self.schedule_response_hop(context, hop_index - 1, response)
                        .expect("scheduled memory response hop");
                }
            })
            .map(|_| ())
            .map_err(MoesiHarnessError::Scheduler)
    }
}

fn complete_partitioned_moesi_memory_request(
    now: u64,
    request: &MemoryRequest,
    backing: &Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<&Arc<Mutex<DramMemoryController>>>,
    dram_accesses: &Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
) -> Result<(u64, MemoryResponse), MoesiHarnessError> {
    let Some(dram_memory) = dram_memory else {
        let response = backing
            .lock()
            .expect("backing lock")
            .respond(request)
            .map_err(MoesiHarnessError::Backing)?;
        return Ok((now, response));
    };

    let outcome = dram_memory
        .lock()
        .expect("DRAM memory lock")
        .accept(now, request)
        .map_err(MoesiHarnessError::Dram)?;
    dram_accesses
        .lock()
        .expect("DRAM access lock")
        .push(DramMemoryAccessRecord::new(
            now,
            outcome.target(),
            outcome.dram_access().request(),
            outcome.dram_access().bank(),
            outcome.dram_access().row(),
            outcome.dram_access().row_hit(),
            outcome.ready_cycle(),
        ));
    let response = outcome
        .response()
        .cloned()
        .ok_or(MoesiHarnessError::Memory(
            MemoryError::MissingResponseData {
                request: request.id(),
            },
        ))?;

    Ok((outcome.ready_cycle(), response))
}

pub(super) fn moesi_route_response_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: u64,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    request: MemoryRequestId,
    packet_bytes: u64,
) -> Result<u64, TransportError> {
    let mut tick = now;
    for hop in route.hops().iter().rev() {
        let delay = moesi_response_packet_hop_delay(
            fabric,
            tick,
            route_id,
            route,
            hop,
            request,
            packet_bytes,
        )?;
        tick = tick
            .checked_add(delay)
            .ok_or(TransportError::Fabric(FabricError::TickOverflow))?;
    }

    tick.checked_sub(now)
        .ok_or(TransportError::Fabric(FabricError::TickOverflow))
}

fn moesi_request_hop_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: u64,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    hop: &MemoryRouteHop,
    request: &MemoryRequest,
) -> Result<u64, TransportError> {
    let Some(path) = hop.request_fabric_path() else {
        return Ok(hop.request_latency());
    };
    let Some(fabric) = fabric else {
        return Err(TransportError::MissingFabricModel { route: route_id });
    };
    let packet = FabricPacket::new(
        moesi_fabric_packet_id(route_id, request.id(), false),
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

fn moesi_response_hop_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: u64,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    hop: &MemoryRouteHop,
    response: &MemoryResponse,
) -> Result<u64, TransportError> {
    moesi_response_packet_hop_delay(
        fabric,
        now,
        route_id,
        route,
        hop,
        response.request_id(),
        moesi_response_packet_bytes(response),
    )
}

fn moesi_response_packet_hop_delay(
    fabric: &Option<Arc<Mutex<FabricModel>>>,
    now: u64,
    route_id: MemoryRouteId,
    route: &MemoryRoute,
    hop: &MemoryRouteHop,
    request: MemoryRequestId,
    packet_bytes: u64,
) -> Result<u64, TransportError> {
    let Some(path) = hop.response_fabric_path() else {
        return Ok(hop.response_latency());
    };
    let Some(fabric) = fabric else {
        return Err(TransportError::MissingFabricModel { route: route_id });
    };
    let packet = FabricPacket::new(
        moesi_fabric_packet_id(route_id, request, true),
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

fn moesi_response_packet_bytes(response: &MemoryResponse) -> u64 {
    response
        .data()
        .map_or(1, |bytes| (bytes.len() as u64).max(1))
}

fn moesi_fabric_packet_id(
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

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_moesi_cache_response_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    pre_response_delay: u64,
    requester_route: PartitionedMoesiRoute,
    response: MemoryResponse,
    fill_event: MoesiEvent,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MoesiCacheController>>,
    responses: Arc<Mutex<Vec<MoesiCpuResponseRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MoesiLineId,
) -> Result<(), MoesiHarnessError> {
    if pre_response_delay == 0 {
        let last_hop = requester_route.route.hops().len() - 1;
        return schedule_partitioned_moesi_cache_response_hop_parallel(
            context,
            requester_route,
            last_hop,
            response,
            fill_event,
            fabric,
            trace,
            response_cache,
            responses,
            wait_for,
            line,
        );
    }

    context
        .schedule_local_after(pre_response_delay, move |context| {
            let last_hop = requester_route.route.hops().len() - 1;
            schedule_partitioned_moesi_cache_response_hop_parallel(
                context,
                requester_route,
                last_hop,
                response,
                fill_event,
                fabric,
                trace,
                response_cache,
                responses,
                wait_for,
                line,
            )
            .expect("scheduled cache response hop");
        })
        .map(|_| ())
        .map_err(MoesiHarnessError::Scheduler)
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_moesi_cache_response_hop_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    requester_route: PartitionedMoesiRoute,
    hop_index: usize,
    response: MemoryResponse,
    fill_event: MoesiEvent,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MoesiCacheController>>,
    responses: Arc<Mutex<Vec<MoesiCpuResponseRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MoesiLineId,
) -> Result<(), MoesiHarnessError> {
    let hop = requester_route.route.hops()[hop_index].clone();
    let (endpoint, partition) = moesi_route_response_destination(&requester_route.route, hop_index);
    let route_id = requester_route.id;
    let delay = moesi_response_hop_delay(
        &fabric,
        context.now(),
        route_id,
        &requester_route.route,
        &hop,
        &response,
    )
    .expect("validated MOESI cache response fabric timing");
    context
        .schedule_remote_after(partition, delay, move |context| {
            trace.record(MemoryTraceEvent::response(
                context.now(),
                route_id,
                endpoint,
                response.request_id(),
                response.status(),
            ));

            if hop_index == 0 {
                let response_request = response.request_id();
                let result = response_cache
                    .lock()
                    .expect("cache lock")
                    .accept_fill(response, fill_event)
                    .expect("cache fill");
                wait_for.clear_cache_line(response_request.agent(), line.address().get());
                if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                    responses
                        .lock()
                        .expect("response lock")
                        .push(moesi_response_record(
                            context.now(),
                            result.kind(),
                            response,
                        ));
                }
            } else {
                schedule_partitioned_moesi_cache_response_hop_parallel(
                    context,
                    requester_route,
                    hop_index - 1,
                    response,
                    fill_event,
                    fabric,
                    trace,
                    response_cache,
                    responses,
                    wait_for,
                    line,
                )
                .expect("scheduled cache response hop");
            }
        })
        .map(|_| ())
        .map_err(MoesiHarnessError::Scheduler)
}

fn moesi_route_response_destination(
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
