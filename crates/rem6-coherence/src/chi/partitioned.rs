use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::ChiCacheController;
use rem6_directory::{ChiDirectoryDataSource, ChiDirectoryDecision};
use rem6_dram::DramMemoryController;
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_chi::ChiEvent;
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, TargetOutcome,
    TransportEndpointId,
};

use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerError};

use super::{
    chi_response_record, decision_downgrades_unique_owner, map_chi_cache_error,
    ChiCpuResponseRecord, ChiHarnessError, PartitionedChiRoute,
};
use crate::{LineBackingStore, PartitionedDramQosState, PartitionedRouteHopConfig};

pub(super) fn expand_partition_count_for_chi_hops(
    partition_count: &mut u32,
    hops: &[PartitionedRouteHopConfig],
) -> Result<(), ChiHarnessError> {
    for hop in hops {
        *partition_count = (*partition_count).max(
            hop.partition()
                .index()
                .checked_add(1)
                .ok_or(ChiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
    }

    Ok(())
}

pub(super) fn chi_route_hops_use_fabric(hops: &[PartitionedRouteHopConfig]) -> bool {
    hops.iter()
        .any(|hop| hop.request_fabric_path().is_some() || hop.response_fabric_path().is_some())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn chi_route_from_config(
    source_endpoint: TransportEndpointId,
    source_partition: PartitionId,
    target_endpoint: TransportEndpointId,
    target_partition: PartitionId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: rem6_fabric::VirtualNetworkId,
    response_virtual_network: rem6_fabric::VirtualNetworkId,
    route_hops: &[PartitionedRouteHopConfig],
) -> Result<MemoryRoute, ChiHarnessError> {
    if route_hops.is_empty() {
        return Ok(MemoryRoute::new(
            source_endpoint,
            source_partition,
            target_endpoint,
            target_partition,
            request_latency,
            response_latency,
        )
        .map_err(ChiHarnessError::Transport)?
        .with_virtual_networks(request_virtual_network, response_virtual_network));
    }

    let hops = route_hops
        .iter()
        .map(|hop| {
            let mut route_hop = MemoryRouteHop::new(
                hop.endpoint().clone(),
                hop.partition(),
                hop.request_latency(),
                hop.response_latency(),
            )
            .map_err(ChiHarnessError::Transport)?;
            if let Some(path) = hop.request_fabric_path() {
                route_hop = route_hop.with_request_fabric_path(path.clone());
            }
            if let Some(path) = hop.response_fabric_path() {
                route_hop = route_hop.with_response_fabric_path(path.clone());
            }
            Ok(route_hop)
        })
        .collect::<Result<Vec<_>, ChiHarnessError>>()?;

    Ok(
        MemoryRoute::new_path(source_endpoint, source_partition, hops)
            .map_err(ChiHarnessError::Transport)?
            .with_virtual_networks(request_virtual_network, response_virtual_network),
    )
}

pub(super) fn line_backing_from_chi_dram_memory(
    layout: CacheLineLayout,
    line_address: Address,
    controller: &DramMemoryController,
) -> Result<LineBackingStore, ChiHarnessError> {
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 0),
        line_address,
        AccessSize::new(layout.bytes()).map_err(ChiHarnessError::Memory)?,
        layout,
    )
    .map_err(ChiHarnessError::Memory)?;
    let mut probe = controller.clone();
    let outcome = probe.accept(0, &request).map_err(ChiHarnessError::Dram)?;
    let data = outcome
        .response()
        .and_then(MemoryResponse::data)
        .map(<[u8]>::to_vec)
        .ok_or(ChiHarnessError::Memory(MemoryError::MissingResponseData {
            request: request.id(),
        }))?;

    LineBackingStore::new(layout, line_address, data).map_err(ChiHarnessError::Backing)
}

pub(super) fn partitioned_chi_directory_response(
    request: &MemoryRequest,
    decision: &ChiDirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<ChiCacheController>>>,
    backing: &Arc<Mutex<LineBackingStore>>,
) -> Result<MemoryResponse, ChiHarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(ChiHarnessError::MissingDirectoryGrant {
            request: request.id(),
        })?;
    let source_data = partitioned_chi_source_data(decision, caches)?;

    if decision_downgrades_unique_owner(decision) {
        if let Some(data) = &source_data {
            backing
                .lock()
                .expect("backing lock")
                .replace_data(data.clone())
                .map_err(ChiHarnessError::Backing)?;
        }
    }

    match grant.data_source() {
        ChiDirectoryDataSource::BackingMemory => backing
            .lock()
            .expect("backing lock")
            .respond(request)
            .map_err(ChiHarnessError::Backing),
        ChiDirectoryDataSource::OwnerCache(_) if request.returns_data() => {
            MemoryResponse::completed(request, source_data).map_err(ChiHarnessError::Memory)
        }
        ChiDirectoryDataSource::OwnerCache(_) | ChiDirectoryDataSource::NoData => {
            MemoryResponse::completed(request, None).map_err(ChiHarnessError::Memory)
        }
    }
}

pub(super) fn decision_uses_chi_backing_memory(decision: &ChiDirectoryDecision) -> bool {
    decision
        .grant()
        .is_some_and(|grant| matches!(grant.data_source(), ChiDirectoryDataSource::BackingMemory))
}

fn partitioned_chi_source_data(
    decision: &ChiDirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<ChiCacheController>>>,
) -> Result<Option<Vec<u8>>, ChiHarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(ChiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?;
    Ok(match grant.data_source() {
        ChiDirectoryDataSource::BackingMemory | ChiDirectoryDataSource::NoData => None,
        ChiDirectoryDataSource::OwnerCache(agent) => {
            let cache = caches
                .get(&agent)
                .ok_or(ChiHarnessError::UnknownCache { agent })?;
            let locked = cache.lock().expect("cache lock");
            Some(
                locked
                    .cached_data()
                    .ok_or(ChiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    })?
                    .to_vec(),
            )
        }
    })
}

pub(super) fn schedule_partitioned_chi_snoops(
    context: &mut ParallelSchedulerContext<'_>,
    decision: &ChiDirectoryDecision,
    cache_routes: &BTreeMap<AgentId, PartitionedChiRoute>,
    caches: &BTreeMap<AgentId, Arc<Mutex<ChiCacheController>>>,
) -> Result<u64, ChiHarnessError> {
    let mut max_delay = 0;
    for snoop in decision.snoops() {
        let route = cache_routes
            .get(&snoop.target())
            .ok_or(ChiHarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let delay = route.route.response_latency();
        max_delay = max_delay.max(delay);
        let cache = caches
            .get(&snoop.target())
            .cloned()
            .ok_or(ChiHarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let event = snoop.event();
        context
            .schedule_remote_after(route.route.source_partition(), delay, move |_| {
                cache
                    .lock()
                    .expect("cache lock")
                    .accept_snoop(event)
                    .map_err(map_chi_cache_error)
                    .expect("scheduled CHI snoop");
            })
            .map_err(ChiHarnessError::Scheduler)?;
    }

    Ok(max_delay)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_chi_memory_response(
    context: &mut ParallelSchedulerContext<'_>,
    request: MemoryRequest,
    fill_event: ChiEvent,
    requester_route: PartitionedChiRoute,
    memory_route: PartitionedChiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<Arc<Mutex<PartitionedDramQosState>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<ChiCacheController>>,
    responses: Arc<Mutex<Vec<ChiCpuResponseRecord>>>,
    snoop_delay: u64,
) -> Result<(), ChiHarnessError> {
    trace.record(MemoryTraceEvent::request(
        context.now(),
        memory_route.id,
        memory_route.route.source().clone(),
        MemoryTraceKind::RequestSent,
        request.id(),
    ));
    let directory_tick = context.now();
    schedule_partitioned_chi_memory_request_hop(
        context,
        0,
        request,
        fill_event,
        requester_route,
        memory_route,
        backing,
        dram_memory,
        dram_qos,
        trace,
        response_cache,
        responses,
        snoop_delay,
        directory_tick,
    )
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_chi_memory_request_hop(
    context: &mut ParallelSchedulerContext<'_>,
    hop_index: usize,
    request: MemoryRequest,
    fill_event: ChiEvent,
    requester_route: PartitionedChiRoute,
    memory_route: PartitionedChiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<Arc<Mutex<PartitionedDramQosState>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<ChiCacheController>>,
    responses: Arc<Mutex<Vec<ChiCpuResponseRecord>>>,
    snoop_delay: u64,
    directory_tick: u64,
) -> Result<(), ChiHarnessError> {
    let hop = memory_route.route.hops()[hop_index].clone();
    let route_id = memory_route.id;
    context
        .schedule_remote_after(hop.partition(), hop.request_latency(), move |context| {
            trace.record(MemoryTraceEvent::request(
                context.now(),
                route_id,
                hop.endpoint().clone(),
                MemoryTraceKind::RequestArrived,
                request.id(),
            ));

            if hop_index + 1 == memory_route.route.hops().len() {
                let (ready_tick, response) = complete_partitioned_chi_memory_request(
                    context.now(),
                    &request,
                    &backing,
                    dram_memory.as_ref(),
                    dram_qos.as_ref(),
                )
                .expect("CHI memory response");
                context
                    .schedule_local_after(
                        ready_tick
                            .checked_sub(context.now())
                            .expect("DRAM ready tick is not in the past"),
                        move |context| {
                            let last_hop = memory_route.route.hops().len() - 1;
                            schedule_partitioned_chi_memory_response_hop(
                                context,
                                last_hop,
                                response,
                                fill_event,
                                requester_route,
                                memory_route,
                                trace,
                                response_cache,
                                responses,
                                snoop_delay,
                                directory_tick,
                            )
                            .expect("scheduled CHI memory response hop");
                        },
                    )
                    .expect("validated CHI DRAM ready latency");
            } else {
                schedule_partitioned_chi_memory_request_hop(
                    context,
                    hop_index + 1,
                    request,
                    fill_event,
                    requester_route,
                    memory_route,
                    backing,
                    dram_memory,
                    dram_qos,
                    trace,
                    response_cache,
                    responses,
                    snoop_delay,
                    directory_tick,
                )
                .expect("scheduled CHI memory request hop");
            }
        })
        .map(|_| ())
        .map_err(ChiHarnessError::Scheduler)
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_chi_memory_response_hop(
    context: &mut ParallelSchedulerContext<'_>,
    hop_index: usize,
    response: MemoryResponse,
    fill_event: ChiEvent,
    requester_route: PartitionedChiRoute,
    memory_route: PartitionedChiRoute,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<ChiCacheController>>,
    responses: Arc<Mutex<Vec<ChiCpuResponseRecord>>>,
    snoop_delay: u64,
    directory_tick: u64,
) -> Result<(), ChiHarnessError> {
    let hop = memory_route.route.hops()[hop_index].clone();
    let (endpoint, partition) =
        partitioned_chi_route_response_destination(&memory_route.route, hop_index);
    let route_id = memory_route.id;
    context
        .schedule_remote_after(partition, hop.response_latency(), move |context| {
            trace.record(MemoryTraceEvent::response(
                context.now(),
                route_id,
                endpoint,
                response.request_id(),
                response.status(),
            ));

            if hop_index == 0 {
                let elapsed = context
                    .now()
                    .checked_sub(directory_tick)
                    .expect("memory response is after directory request");
                let wait_for_snoops = snoop_delay.saturating_sub(elapsed);
                schedule_partitioned_chi_cache_response(
                    context,
                    wait_for_snoops,
                    requester_route,
                    response,
                    fill_event,
                    trace,
                    response_cache,
                    responses,
                )
                .expect("scheduled CHI cache response");
            } else {
                schedule_partitioned_chi_memory_response_hop(
                    context,
                    hop_index - 1,
                    response,
                    fill_event,
                    requester_route,
                    memory_route,
                    trace,
                    response_cache,
                    responses,
                    snoop_delay,
                    directory_tick,
                )
                .expect("scheduled CHI memory response hop");
            }
        })
        .map(|_| ())
        .map_err(ChiHarnessError::Scheduler)
}

fn complete_partitioned_chi_memory_request(
    now: u64,
    request: &MemoryRequest,
    backing: &Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<&Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<&Arc<Mutex<PartitionedDramQosState>>>,
) -> Result<(u64, MemoryResponse), ChiHarnessError> {
    let Some(dram_memory) = dram_memory else {
        let response = backing
            .lock()
            .expect("backing lock")
            .respond(request)
            .map_err(ChiHarnessError::Backing)?;
        return Ok((now, response));
    };

    let mut controller = dram_memory.lock().expect("DRAM memory lock");
    let outcome = match dram_qos {
        Some(qos) => qos
            .lock()
            .expect("DRAM QoS lock")
            .accept(&mut controller, now, request)
            .map_err(ChiHarnessError::Dram)?,
        None => controller
            .accept(now, request)
            .map_err(ChiHarnessError::Dram)?,
    };
    let response = outcome.response().cloned().ok_or(ChiHarnessError::Memory(
        MemoryError::MissingResponseData {
            request: request.id(),
        },
    ))?;

    Ok((outcome.ready_cycle(), response))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_chi_cache_response(
    context: &mut ParallelSchedulerContext<'_>,
    pre_response_delay: u64,
    requester_route: PartitionedChiRoute,
    response: MemoryResponse,
    fill_event: ChiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<ChiCacheController>>,
    responses: Arc<Mutex<Vec<ChiCpuResponseRecord>>>,
) -> Result<(), ChiHarnessError> {
    if pre_response_delay == 0 {
        let last_hop = requester_route.route.hops().len() - 1;
        return schedule_partitioned_chi_cache_response_hop(
            context,
            last_hop,
            requester_route,
            response,
            fill_event,
            trace,
            response_cache,
            responses,
        );
    }

    context
        .schedule_local_after(pre_response_delay, move |context| {
            let last_hop = requester_route.route.hops().len() - 1;
            schedule_partitioned_chi_cache_response_hop(
                context,
                last_hop,
                requester_route,
                response,
                fill_event,
                trace,
                response_cache,
                responses,
            )
            .expect("scheduled CHI response hop");
        })
        .map(|_| ())
        .map_err(ChiHarnessError::Scheduler)
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_chi_cache_response_hop(
    context: &mut ParallelSchedulerContext<'_>,
    hop_index: usize,
    requester_route: PartitionedChiRoute,
    response: MemoryResponse,
    fill_event: ChiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<ChiCacheController>>,
    responses: Arc<Mutex<Vec<ChiCpuResponseRecord>>>,
) -> Result<(), ChiHarnessError> {
    let hop = requester_route.route.hops()[hop_index].clone();
    let (endpoint, partition) =
        partitioned_chi_route_response_destination(&requester_route.route, hop_index);
    let route_id = requester_route.id;
    context
        .schedule_remote_after(partition, hop.response_latency(), move |context| {
            trace.record(MemoryTraceEvent::response(
                context.now(),
                route_id,
                endpoint,
                response.request_id(),
                response.status(),
            ));

            if hop_index == 0 {
                let result = response_cache
                    .lock()
                    .expect("cache lock")
                    .accept_fill(response, fill_event)
                    .map_err(map_chi_cache_error)
                    .expect("scheduled CHI fill");
                if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                    responses
                        .lock()
                        .expect("response lock")
                        .push(chi_response_record(context.now(), result.kind(), response));
                }
            } else {
                schedule_partitioned_chi_cache_response_hop(
                    context,
                    hop_index - 1,
                    requester_route,
                    response,
                    fill_event,
                    trace,
                    response_cache,
                    responses,
                )
                .expect("scheduled CHI response hop");
            }
        })
        .map(|_| ())
        .map_err(ChiHarnessError::Scheduler)
}

fn partitioned_chi_route_response_destination(
    route: &MemoryRoute,
    hop_index: usize,
) -> (TransportEndpointId, PartitionId) {
    if hop_index == 0 {
        (route.source().clone(), route.source_partition())
    } else {
        let hop = &route.hops()[hop_index - 1];
        (hop.endpoint().clone(), hop.partition())
    }
}
