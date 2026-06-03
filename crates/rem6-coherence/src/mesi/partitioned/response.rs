use std::sync::{Arc, Mutex};

use rem6_cache::MesiCacheController;
use rem6_dram::DramMemoryController;
use rem6_kernel::{ParallelSchedulerContext, PartitionId, SchedulerContext};
use rem6_memory::{MemoryError, MemoryRequest, MemoryResponse};
use rem6_protocol_mesi::{MesiEvent, MesiLineId};
use rem6_transport::{
    MemoryRoute, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, TargetOutcome, TransportEndpointId,
};

use crate::wait_for::CoherenceWaitFor;
use crate::{DramMemoryAccessRecord, LineBackingStore, PartitionedDramQosState};

use super::{mesi_response_record, MesiCpuResponseRecord, MesiHarnessError, PartitionedMesiRoute};

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_mesi_memory_response(
    context: &mut SchedulerContext<'_>,
    request: MemoryRequest,
    fill_event: MesiEvent,
    requester_route: PartitionedMesiRoute,
    memory_route: PartitionedMesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<Arc<Mutex<PartitionedDramQosState>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    snoop_delay: u64,
) -> Result<(), MesiHarnessError> {
    SerialMesiMemoryResponseWork {
        directory_tick: context.now(),
        fill_event,
        requester_route,
        memory_route,
        backing,
        dram_memory,
        dram_qos,
        trace,
        response_cache,
        responses,
        dram_accesses,
        snoop_delay,
    }
    .schedule(context, request)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_mesi_cache_response(
    context: &mut SchedulerContext<'_>,
    pre_response_delay: u64,
    requester_route: PartitionedMesiRoute,
    response: MemoryResponse,
    fill_event: MesiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
) -> Result<(), MesiHarnessError> {
    SerialMesiCacheResponseWork {
        requester_route,
        fill_event,
        trace,
        response_cache,
        responses,
    }
    .schedule(context, pre_response_delay, response)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_mesi_memory_response_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    request: MemoryRequest,
    fill_event: MesiEvent,
    requester_route: PartitionedMesiRoute,
    memory_route: PartitionedMesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<Arc<Mutex<PartitionedDramQosState>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MesiLineId,
    snoop_delay: u64,
) -> Result<(), MesiHarnessError> {
    ParallelMesiMemoryResponseWork {
        directory_tick: context.now(),
        fill_event,
        requester_route,
        memory_route,
        backing,
        dram_memory,
        dram_qos,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn schedule_partitioned_mesi_cache_response_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    pre_response_delay: u64,
    requester_route: PartitionedMesiRoute,
    response: MemoryResponse,
    fill_event: MesiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MesiLineId,
) -> Result<(), MesiHarnessError> {
    ParallelMesiCacheResponseWork {
        requester_route,
        fill_event,
        trace,
        response_cache,
        responses,
        wait_for,
        line,
    }
    .schedule(context, pre_response_delay, response)
}

struct SerialMesiMemoryResponseWork {
    directory_tick: u64,
    fill_event: MesiEvent,
    requester_route: PartitionedMesiRoute,
    memory_route: PartitionedMesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<Arc<Mutex<PartitionedDramQosState>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    snoop_delay: u64,
}

impl SerialMesiMemoryResponseWork {
    fn schedule(
        self,
        context: &mut SchedulerContext<'_>,
        request: MemoryRequest,
    ) -> Result<(), MesiHarnessError> {
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
        context: &mut SchedulerContext<'_>,
        hop_index: usize,
        request: MemoryRequest,
    ) -> Result<(), MesiHarnessError> {
        let hop = self.memory_route.route.hops()[hop_index].clone();
        let route_id = self.memory_route.id;
        context
            .schedule_remote_after(hop.partition(), hop.request_latency(), move |context| {
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
            .map_err(MesiHarnessError::Scheduler)
    }

    fn complete_target(self, context: &mut SchedulerContext<'_>, request: MemoryRequest) {
        let (ready_tick, response) = complete_partitioned_mesi_memory_request(
            context.now(),
            &request,
            &self.backing,
            self.dram_memory.as_ref(),
            self.dram_qos.as_ref(),
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
        context: &mut SchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) -> Result<(), MesiHarnessError> {
        let hop = self.memory_route.route.hops()[hop_index].clone();
        let (endpoint, partition) =
            mesi_route_response_destination(&self.memory_route.route, hop_index);
        let route_id = self.memory_route.id;
        context
            .schedule_remote_after(partition, hop.response_latency(), move |context| {
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
                    schedule_partitioned_mesi_cache_response(
                        context,
                        wait_for_snoops,
                        self.requester_route,
                        response,
                        self.fill_event,
                        self.trace,
                        self.response_cache,
                        self.responses,
                    )
                    .expect("scheduled cache response");
                } else {
                    self.schedule_response_hop(context, hop_index - 1, response)
                        .expect("scheduled memory response hop");
                }
            })
            .map(|_| ())
            .map_err(MesiHarnessError::Scheduler)
    }
}

struct SerialMesiCacheResponseWork {
    requester_route: PartitionedMesiRoute,
    fill_event: MesiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
}

impl SerialMesiCacheResponseWork {
    fn schedule(
        self,
        context: &mut SchedulerContext<'_>,
        pre_response_delay: u64,
        response: MemoryResponse,
    ) -> Result<(), MesiHarnessError> {
        if pre_response_delay == 0 {
            let last_hop = self.requester_route.route.hops().len() - 1;
            return self.schedule_hop(context, last_hop, response);
        }

        context
            .schedule_local_after(pre_response_delay, move |context| {
                let last_hop = self.requester_route.route.hops().len() - 1;
                self.schedule_hop(context, last_hop, response)
                    .expect("scheduled cache response hop");
            })
            .map(|_| ())
            .map_err(MesiHarnessError::Scheduler)
    }

    fn schedule_hop(
        self,
        context: &mut SchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) -> Result<(), MesiHarnessError> {
        let hop = self.requester_route.route.hops()[hop_index].clone();
        let (endpoint, partition) =
            mesi_route_response_destination(&self.requester_route.route, hop_index);
        let route_id = self.requester_route.id;
        context
            .schedule_remote_after(partition, hop.response_latency(), move |context| {
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
                        .accept_fill(response, self.fill_event)
                        .expect("cache fill");
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        self.responses
                            .lock()
                            .expect("response lock")
                            .push(mesi_response_record(context.now(), result.kind(), response));
                    }
                } else {
                    self.schedule_hop(context, hop_index - 1, response)
                        .expect("scheduled cache response hop");
                }
            })
            .map(|_| ())
            .map_err(MesiHarnessError::Scheduler)
    }
}

struct ParallelMesiMemoryResponseWork {
    directory_tick: u64,
    fill_event: MesiEvent,
    requester_route: PartitionedMesiRoute,
    memory_route: PartitionedMesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<Arc<Mutex<PartitionedDramQosState>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MesiLineId,
    snoop_delay: u64,
}

impl ParallelMesiMemoryResponseWork {
    fn schedule(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        request: MemoryRequest,
    ) -> Result<(), MesiHarnessError> {
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
    ) -> Result<(), MesiHarnessError> {
        let hop = self.memory_route.route.hops()[hop_index].clone();
        let route_id = self.memory_route.id;
        context
            .schedule_remote_after(hop.partition(), hop.request_latency(), move |context| {
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
            .map_err(MesiHarnessError::Scheduler)
    }

    fn complete_target(self, context: &mut ParallelSchedulerContext<'_>, request: MemoryRequest) {
        let (ready_tick, response) = complete_partitioned_mesi_memory_request(
            context.now(),
            &request,
            &self.backing,
            self.dram_memory.as_ref(),
            self.dram_qos.as_ref(),
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
    ) -> Result<(), MesiHarnessError> {
        let hop = self.memory_route.route.hops()[hop_index].clone();
        let (endpoint, partition) =
            mesi_route_response_destination(&self.memory_route.route, hop_index);
        let route_id = self.memory_route.id;
        context
            .schedule_remote_after(partition, hop.response_latency(), move |context| {
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
                    schedule_partitioned_mesi_cache_response_parallel(
                        context,
                        wait_for_snoops,
                        self.requester_route,
                        response,
                        self.fill_event,
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
            .map_err(MesiHarnessError::Scheduler)
    }
}

struct ParallelMesiCacheResponseWork {
    requester_route: PartitionedMesiRoute,
    fill_event: MesiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    wait_for: CoherenceWaitFor,
    line: MesiLineId,
}

impl ParallelMesiCacheResponseWork {
    fn schedule(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        pre_response_delay: u64,
        response: MemoryResponse,
    ) -> Result<(), MesiHarnessError> {
        if pre_response_delay == 0 {
            let last_hop = self.requester_route.route.hops().len() - 1;
            return self.schedule_hop(context, last_hop, response);
        }

        context
            .schedule_local_after(pre_response_delay, move |context| {
                let last_hop = self.requester_route.route.hops().len() - 1;
                self.schedule_hop(context, last_hop, response)
                    .expect("scheduled cache response hop");
            })
            .map(|_| ())
            .map_err(MesiHarnessError::Scheduler)
    }

    fn schedule_hop(
        self,
        context: &mut ParallelSchedulerContext<'_>,
        hop_index: usize,
        response: MemoryResponse,
    ) -> Result<(), MesiHarnessError> {
        let hop = self.requester_route.route.hops()[hop_index].clone();
        let (endpoint, partition) =
            mesi_route_response_destination(&self.requester_route.route, hop_index);
        let route_id = self.requester_route.id;
        context
            .schedule_remote_after(partition, hop.response_latency(), move |context| {
                self.trace.record(MemoryTraceEvent::response(
                    context.now(),
                    route_id,
                    endpoint,
                    response.request_id(),
                    response.status(),
                ));

                if hop_index == 0 {
                    let response_request = response.request_id();
                    let result = self
                        .response_cache
                        .lock()
                        .expect("cache lock")
                        .accept_fill(response, self.fill_event)
                        .expect("cache fill");
                    self.wait_for
                        .clear_cache_line(response_request.agent(), self.line.address().get());
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        self.responses
                            .lock()
                            .expect("response lock")
                            .push(mesi_response_record(context.now(), result.kind(), response));
                    }
                } else {
                    self.schedule_hop(context, hop_index - 1, response)
                        .expect("scheduled cache response hop");
                }
            })
            .map(|_| ())
            .map_err(MesiHarnessError::Scheduler)
    }
}

fn complete_partitioned_mesi_memory_request(
    now: u64,
    request: &MemoryRequest,
    backing: &Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<&Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<&Arc<Mutex<PartitionedDramQosState>>>,
    dram_accesses: &Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
) -> Result<(u64, MemoryResponse), MesiHarnessError> {
    let Some(dram_memory) = dram_memory else {
        let response = backing
            .lock()
            .expect("backing lock")
            .respond(request)
            .map_err(MesiHarnessError::Backing)?;
        return Ok((now, response));
    };

    let mut controller = dram_memory.lock().expect("DRAM memory lock");
    let outcome = match dram_qos {
        Some(qos) => qos
            .lock()
            .expect("DRAM QoS lock")
            .accept(&mut controller, now, request)
            .map_err(MesiHarnessError::Dram)?,
        None => controller
            .accept(now, request)
            .map_err(MesiHarnessError::Dram)?,
    };
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
    let response = outcome.response().cloned().ok_or(MesiHarnessError::Memory(
        MemoryError::MissingResponseData {
            request: request.id(),
        },
    ))?;

    Ok((outcome.ready_cycle(), response))
}

fn mesi_route_response_destination(
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
