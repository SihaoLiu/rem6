use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::{MesiCacheController, MesiCacheControllerError};
use rem6_directory::{
    MesiDirectory, MesiDirectoryDataSource, MesiDirectoryDecision, MesiDirectoryLineState,
};
use rem6_dram::DramMemoryController;
use rem6_kernel::{
    ConservativeRunSummary, ParallelSchedulerContext, PartitionId, PartitionedScheduler,
    SchedulerContext, SchedulerError,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    TargetOutcome, TransportEndpointId,
};

use crate::{
    DramMemoryAccessRecord, HarnessError, LineBackingStore, PartitionedCacheAgentConfig,
    PartitionedDramMemoryConfig, PartitionedMemoryConfig, SubmitKind,
};

use super::{
    decision_uses_mesi_backing_memory, fill_event, map_mesi_cache_error, mesi_response_record,
    MesiCpuResponseRecord, MesiDirectoryDecisionRecord, MesiHarnessError, MesiSubmitResult,
};

#[derive(Clone)]
struct PartitionedMesiRoute {
    id: MemoryRouteId,
    route: MemoryRoute,
}

impl PartitionedMesiRoute {
    fn new(id: MemoryRouteId, route: MemoryRoute) -> Self {
        Self { id, route }
    }
}

pub struct PartitionedMesiDirectoryLineHarness {
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    line: MesiLineId,
    directory: Arc<Mutex<MesiDirectory>>,
    caches: BTreeMap<AgentId, Arc<Mutex<MesiCacheController>>>,
    routes: BTreeMap<AgentId, PartitionedMesiRoute>,
    memory_route: Option<PartitionedMesiRoute>,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    directory_decisions: Arc<Mutex<Vec<MesiDirectoryDecisionRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
}

impl PartitionedMesiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        agents: I,
    ) -> Result<Self, MesiHarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(MesiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?;
        let mut transport = MemoryTransport::new();
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agents {
            if !config.route_hops().is_empty() {
                return Err(MesiHarnessError::UnsupportedRouteHops {
                    agent: config.agent(),
                });
            }

            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(MesiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(MesiHarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }

            let route = MemoryRoute::new(
                config.endpoint().clone(),
                config.partition(),
                directory_endpoint.clone(),
                directory_partition,
                config.request_latency(),
                config.response_latency(),
            )
            .map_err(MesiHarnessError::Transport)?
            .with_virtual_networks(
                config.request_virtual_network(),
                config.response_virtual_network(),
            );
            let route_id = transport
                .add_route(route.clone())
                .map_err(MesiHarnessError::Transport)?;
            routes.insert(config.agent(), PartitionedMesiRoute::new(route_id, route));
        }

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(MesiHarnessError::Scheduler)?,
            transport,
            line: MesiLineId::new(line_address),
            directory: Arc::new(Mutex::new(MesiDirectory::new())),
            caches,
            routes,
            memory_route: None,
            backing: Arc::new(Mutex::new(backing)),
            dram_memory: None,
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
            dram_accesses: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn new_with_memory<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: PartitionedMemoryConfig,
        agents: I,
    ) -> Result<Self, MesiHarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(MesiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }
        if !memory.route_hops().is_empty() {
            return Err(MesiHarnessError::UnsupportedMemoryRouteHops);
        }

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?;
        partition_count = partition_count.max(
            memory
                .partition()
                .index()
                .checked_add(1)
                .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
        let mut transport = MemoryTransport::new();
        let memory_route = MemoryRoute::new(
            directory_endpoint.clone(),
            directory_partition,
            memory.endpoint().clone(),
            memory.partition(),
            memory.request_latency(),
            memory.response_latency(),
        )
        .map_err(MesiHarnessError::Transport)?
        .with_virtual_networks(
            memory.request_virtual_network(),
            memory.response_virtual_network(),
        );
        let memory_route_id = transport
            .add_route(memory_route.clone())
            .map_err(MesiHarnessError::Transport)?;
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agents {
            if !config.route_hops().is_empty() {
                return Err(MesiHarnessError::UnsupportedRouteHops {
                    agent: config.agent(),
                });
            }

            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(MesiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(MesiHarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }

            let route = MemoryRoute::new(
                config.endpoint().clone(),
                config.partition(),
                directory_endpoint.clone(),
                directory_partition,
                config.request_latency(),
                config.response_latency(),
            )
            .map_err(MesiHarnessError::Transport)?
            .with_virtual_networks(
                config.request_virtual_network(),
                config.response_virtual_network(),
            );
            let route_id = transport
                .add_route(route.clone())
                .map_err(MesiHarnessError::Transport)?;
            routes.insert(config.agent(), PartitionedMesiRoute::new(route_id, route));
        }

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(MesiHarnessError::Scheduler)?,
            transport,
            line: MesiLineId::new(line_address),
            directory: Arc::new(Mutex::new(MesiDirectory::new())),
            caches,
            routes,
            memory_route: Some(PartitionedMesiRoute::new(memory_route_id, memory_route)),
            backing: Arc::new(Mutex::new(backing)),
            dram_memory: None,
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
            dram_accesses: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn new_with_dram_memory<I>(
        layout: CacheLineLayout,
        line_address: Address,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: PartitionedDramMemoryConfig,
        agents: I,
    ) -> Result<Self, MesiHarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);
        if !memory.route_hops().is_empty() {
            return Err(MesiHarnessError::UnsupportedMemoryRouteHops);
        }

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?;
        partition_count = partition_count.max(
            memory
                .partition()
                .index()
                .checked_add(1)
                .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
        let mut transport = MemoryTransport::new();
        let memory_route = MemoryRoute::new(
            directory_endpoint.clone(),
            directory_partition,
            memory.endpoint().clone(),
            memory.partition(),
            memory.request_latency(),
            memory.response_latency(),
        )
        .map_err(MesiHarnessError::Transport)?
        .with_virtual_networks(
            memory.request_virtual_network(),
            memory.response_virtual_network(),
        );
        let memory_route_id = transport
            .add_route(memory_route.clone())
            .map_err(MesiHarnessError::Transport)?;
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agents {
            if !config.route_hops().is_empty() {
                return Err(MesiHarnessError::UnsupportedRouteHops {
                    agent: config.agent(),
                });
            }

            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(MesiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(MesiHarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }

            let route = MemoryRoute::new(
                config.endpoint().clone(),
                config.partition(),
                directory_endpoint.clone(),
                directory_partition,
                config.request_latency(),
                config.response_latency(),
            )
            .map_err(MesiHarnessError::Transport)?
            .with_virtual_networks(
                config.request_virtual_network(),
                config.response_virtual_network(),
            );
            let route_id = transport
                .add_route(route.clone())
                .map_err(MesiHarnessError::Transport)?;
            routes.insert(config.agent(), PartitionedMesiRoute::new(route_id, route));
        }

        let dram_controller = memory.into_controller();
        let backing = line_backing_from_dram_memory(layout, line_address, &dram_controller)?;

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(MesiHarnessError::Scheduler)?,
            transport,
            line: MesiLineId::new(line_address),
            directory: Arc::new(Mutex::new(MesiDirectory::new())),
            caches,
            routes,
            memory_route: Some(PartitionedMesiRoute::new(memory_route_id, memory_route)),
            backing: Arc::new(Mutex::new(backing)),
            dram_memory: Some(Arc::new(Mutex::new(dram_controller))),
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
            dram_accesses: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn submit_cpu_request(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<MesiSubmitResult, MesiHarnessError> {
        let cache = self.cache_arc(agent)?;
        let result = cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
            .map_err(map_mesi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.cpu_responses
                .lock()
                .expect("response lock")
                .push(mesi_response_record(
                    self.scheduler.now(),
                    cache_result,
                    response,
                ));
            return Ok(MesiSubmitResult::new(
                SubmitKind::ImmediateHit,
                cache_result,
            ));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(MesiHarnessError::Cache(
                MesiCacheControllerError::NoPendingMiss,
            ))?;
        let requester_route = self
            .routes
            .get(&agent)
            .cloned()
            .ok_or(MesiHarnessError::UnknownCache { agent })?;
        let directory = Arc::clone(&self.directory);
        let caches = self.caches.clone();
        let cache_routes = self.routes.clone();
        let memory_route = self.memory_route.clone();
        let backing = Arc::clone(&self.backing);
        let dram_memory = self.dram_memory.clone();
        let trace = self.trace.clone();
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let decisions = Arc::clone(&self.directory_decisions);
        let dram_accesses = Arc::clone(&self.dram_accesses);

        self.transport
            .submit(
                &mut self.scheduler,
                requester_route.id,
                downstream,
                trace.clone(),
                move |delivery, context| {
                    let decision = directory
                        .lock()
                        .expect("directory lock")
                        .accept(delivery.request().clone())
                        .expect("directory decision");
                    let fill_event = fill_event(&decision).expect("directory grant fill event");
                    if let Some(memory_route) =
                        memory_route.filter(|_| decision_uses_mesi_backing_memory(&decision))
                    {
                        decisions.lock().expect("decision lock").push(
                            MesiDirectoryDecisionRecord::new(
                                delivery.tick(),
                                delivery.request().id().agent(),
                                decision.clone(),
                            ),
                        );
                        let snoop_delay = schedule_partitioned_mesi_snoops(
                            context,
                            &decision,
                            &cache_routes,
                            &caches,
                        )
                        .expect("scheduled directory snoops");
                        schedule_partitioned_mesi_memory_response(
                            context,
                            delivery.request().clone(),
                            fill_event,
                            requester_route,
                            memory_route,
                            backing,
                            dram_memory,
                            trace.clone(),
                            response_cache,
                            responses,
                            dram_accesses,
                            snoop_delay,
                        )
                        .expect("scheduled memory response");
                        return TargetOutcome::NoResponse;
                    }
                    let response = partitioned_mesi_directory_response(
                        delivery.request(),
                        &decision,
                        &caches,
                        &backing,
                    )
                    .expect("directory response");
                    decisions.lock().expect("decision lock").push(
                        MesiDirectoryDecisionRecord::new(
                            delivery.tick(),
                            delivery.request().id().agent(),
                            decision.clone(),
                        ),
                    );

                    let snoop_delay = schedule_partitioned_mesi_snoops(
                        context,
                        &decision,
                        &cache_routes,
                        &caches,
                    )
                    .expect("scheduled directory snoops");
                    let response_delay = requester_route.route.response_latency().max(snoop_delay);
                    let route_id = requester_route.id;
                    let response_endpoint = requester_route.route.source().clone();
                    let response_partition = requester_route.route.source_partition();
                    let response_trace = trace.clone();
                    let response_cache = Arc::clone(&response_cache);
                    let responses = Arc::clone(&responses);
                    context
                        .schedule_remote_after(response_partition, response_delay, move |context| {
                            response_trace.record(MemoryTraceEvent::response(
                                context.now(),
                                route_id,
                                response_endpoint,
                                response.request_id(),
                                response.status(),
                            ));
                            let result = response_cache
                                .lock()
                                .expect("cache lock")
                                .accept_fill(response, fill_event)
                                .expect("cache fill");
                            if let Some(TargetOutcome::Respond(response)) = result.target_outcome()
                            {
                                responses.lock().expect("response lock").push(
                                    mesi_response_record(context.now(), result.kind(), response),
                                );
                            }
                        })
                        .expect("validated response latency");

                    TargetOutcome::NoResponse
                },
                |_| {},
            )
            .map_err(MesiHarnessError::Transport)?;

        Ok(MesiSubmitResult::new(
            SubmitKind::ScheduledMiss,
            cache_result,
        ))
    }

    pub fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<MesiSubmitResult, MesiHarnessError> {
        let cache = self.cache_arc(agent)?;
        let result = cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
            .map_err(map_mesi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.cpu_responses
                .lock()
                .expect("response lock")
                .push(mesi_response_record(
                    self.scheduler.now(),
                    cache_result,
                    response,
                ));
            return Ok(MesiSubmitResult::new(
                SubmitKind::ImmediateHit,
                cache_result,
            ));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(MesiHarnessError::Cache(
                MesiCacheControllerError::NoPendingMiss,
            ))?;
        let requester_route = self
            .routes
            .get(&agent)
            .cloned()
            .ok_or(MesiHarnessError::UnknownCache { agent })?;
        let directory = Arc::clone(&self.directory);
        let caches = self.caches.clone();
        let cache_routes = self.routes.clone();
        let memory_route = self.memory_route.clone();
        let backing = Arc::clone(&self.backing);
        let dram_memory = self.dram_memory.clone();
        let trace = self.trace.clone();
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let decisions = Arc::clone(&self.directory_decisions);
        let dram_accesses = Arc::clone(&self.dram_accesses);

        self.transport
            .submit_parallel(
                &mut self.scheduler,
                requester_route.id,
                downstream,
                trace.clone(),
                move |delivery, context| {
                    let decision = directory
                        .lock()
                        .expect("directory lock")
                        .accept(delivery.request().clone())
                        .expect("directory decision");
                    let fill_event = fill_event(&decision).expect("directory grant fill event");
                    if let Some(memory_route) =
                        memory_route.filter(|_| decision_uses_mesi_backing_memory(&decision))
                    {
                        decisions.lock().expect("decision lock").push(
                            MesiDirectoryDecisionRecord::new(
                                delivery.tick(),
                                delivery.request().id().agent(),
                                decision.clone(),
                            ),
                        );
                        let snoop_delay = schedule_partitioned_mesi_snoops_parallel(
                            context,
                            &decision,
                            &cache_routes,
                            &caches,
                        )
                        .expect("scheduled directory snoops");
                        schedule_partitioned_mesi_memory_response_parallel(
                            context,
                            delivery.request().clone(),
                            fill_event,
                            requester_route,
                            memory_route,
                            backing,
                            dram_memory,
                            trace.clone(),
                            response_cache,
                            responses,
                            dram_accesses,
                            snoop_delay,
                        )
                        .expect("scheduled memory response");
                        return TargetOutcome::NoResponse;
                    }
                    let response = partitioned_mesi_directory_response(
                        delivery.request(),
                        &decision,
                        &caches,
                        &backing,
                    )
                    .expect("directory response");
                    decisions.lock().expect("decision lock").push(
                        MesiDirectoryDecisionRecord::new(
                            delivery.tick(),
                            delivery.request().id().agent(),
                            decision.clone(),
                        ),
                    );

                    let snoop_delay = schedule_partitioned_mesi_snoops_parallel(
                        context,
                        &decision,
                        &cache_routes,
                        &caches,
                    )
                    .expect("scheduled directory snoops");
                    let response_delay = requester_route.route.response_latency().max(snoop_delay);
                    let route_id = requester_route.id;
                    let response_endpoint = requester_route.route.source().clone();
                    let response_partition = requester_route.route.source_partition();
                    let response_trace = trace.clone();
                    let response_cache = Arc::clone(&response_cache);
                    let responses = Arc::clone(&responses);
                    context
                        .schedule_remote_after(response_partition, response_delay, move |context| {
                            response_trace.record(MemoryTraceEvent::response(
                                context.now(),
                                route_id,
                                response_endpoint,
                                response.request_id(),
                                response.status(),
                            ));
                            let result = response_cache
                                .lock()
                                .expect("cache lock")
                                .accept_fill(response, fill_event)
                                .expect("cache fill");
                            if let Some(TargetOutcome::Respond(response)) = result.target_outcome()
                            {
                                responses.lock().expect("response lock").push(
                                    mesi_response_record(context.now(), result.kind(), response),
                                );
                            }
                        })
                        .expect("validated response latency");

                    TargetOutcome::NoResponse
                },
                |_| {},
            )
            .map_err(MesiHarnessError::Transport)?;

        Ok(MesiSubmitResult::new(
            SubmitKind::ScheduledMiss,
            cache_result,
        ))
    }

    pub fn run_until_idle(&mut self) -> ConservativeRunSummary {
        self.scheduler.run_until_idle_conservative()
    }

    pub fn run_until_idle_parallel(&mut self) -> Result<ConservativeRunSummary, MesiHarnessError> {
        self.scheduler
            .run_until_idle_parallel()
            .map_err(MesiHarnessError::Scheduler)
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<MesiState, MesiHarnessError> {
        Ok(self.cache_arc(agent)?.lock().expect("cache lock").state())
    }

    pub fn directory_state(&self) -> MesiDirectoryLineState {
        self.directory
            .lock()
            .expect("directory lock")
            .line_state(self.line)
    }

    pub fn route(&self, agent: AgentId) -> Result<MemoryRouteId, MesiHarnessError> {
        self.routes
            .get(&agent)
            .map(|route| route.id)
            .ok_or(MesiHarnessError::UnknownCache { agent })
    }

    pub fn memory_route(&self) -> Option<MemoryRouteId> {
        self.memory_route.as_ref().map(|route| route.id)
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.snapshot()
    }

    pub fn cpu_responses(&self) -> Vec<MesiCpuResponseRecord> {
        self.cpu_responses.lock().expect("response lock").clone()
    }

    pub fn directory_decisions(&self) -> Vec<MesiDirectoryDecisionRecord> {
        self.directory_decisions
            .lock()
            .expect("decision lock")
            .clone()
    }

    pub fn dram_memory_accesses(&self) -> Vec<DramMemoryAccessRecord> {
        self.dram_accesses.lock().expect("DRAM access lock").clone()
    }

    pub const fn line(&self) -> MesiLineId {
        self.line
    }

    fn cache_arc(
        &self,
        agent: AgentId,
    ) -> Result<Arc<Mutex<MesiCacheController>>, MesiHarnessError> {
        self.caches
            .get(&agent)
            .cloned()
            .ok_or(MesiHarnessError::UnknownCache { agent })
    }
}

fn partitioned_mesi_directory_response(
    request: &MemoryRequest,
    decision: &MesiDirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MesiCacheController>>>,
    backing: &Arc<Mutex<LineBackingStore>>,
) -> Result<MemoryResponse, MesiHarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(MesiHarnessError::MissingDirectoryGrant {
            request: request.id(),
        })?;
    let source_data = partitioned_mesi_source_data(decision, caches)?;

    if decision
        .snoops()
        .iter()
        .any(|snoop| snoop.event() == MesiEvent::SnoopRead)
    {
        if let Some(data) = &source_data {
            backing
                .lock()
                .expect("backing lock")
                .replace_data(data.clone())
                .map_err(MesiHarnessError::Backing)?;
        }
    }

    match grant.data_source() {
        MesiDirectoryDataSource::BackingMemory => backing
            .lock()
            .expect("backing lock")
            .respond(request)
            .map_err(MesiHarnessError::Backing),
        MesiDirectoryDataSource::OwnedCache(_) => {
            MemoryResponse::completed(request, source_data).map_err(MesiHarnessError::Memory)
        }
        MesiDirectoryDataSource::NoData => {
            MemoryResponse::completed(request, None).map_err(MesiHarnessError::Memory)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_mesi_memory_response(
    context: &mut SchedulerContext<'_>,
    request: MemoryRequest,
    fill_event: MesiEvent,
    requester_route: PartitionedMesiRoute,
    memory_route: PartitionedMesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    snoop_delay: u64,
) -> Result<(), MesiHarnessError> {
    let directory_tick = context.now();
    trace.record(MemoryTraceEvent::request(
        context.now(),
        memory_route.id,
        memory_route.route.source().clone(),
        MemoryTraceKind::RequestSent,
        request.id(),
    ));
    let memory_response_delay = memory_route.route.response_latency();
    let memory_route_id = memory_route.id;
    let memory_endpoint = memory_route.route.target().clone();
    let memory_source = memory_route.route.source().clone();
    let memory_partition = memory_route.route.target_partition();
    let directory_partition = memory_route.route.source_partition();
    let memory_trace = trace.clone();
    context
        .schedule_remote_after(
            memory_partition,
            memory_route.route.request_latency(),
            move |context| {
                memory_trace.record(MemoryTraceEvent::request(
                    context.now(),
                    memory_route_id,
                    memory_endpoint,
                    MemoryTraceKind::RequestArrived,
                    request.id(),
                ));
                let (ready_tick, response) = complete_partitioned_mesi_memory_request(
                    context.now(),
                    &request,
                    &backing,
                    dram_memory.as_ref(),
                    &dram_accesses,
                )
                .expect("memory response");
                let response_trace = memory_trace.clone();
                context
                    .schedule_local_after(
                        ready_tick
                            .checked_sub(context.now())
                            .expect("DRAM ready tick is not in the past"),
                        move |context| {
                            context
                                .schedule_remote_after(
                                    directory_partition,
                                    memory_response_delay,
                                    move |context| {
                                        response_trace.record(MemoryTraceEvent::response(
                                            context.now(),
                                            memory_route_id,
                                            memory_source,
                                            response.request_id(),
                                            response.status(),
                                        ));
                                        let elapsed = context
                                            .now()
                                            .checked_sub(directory_tick)
                                            .expect("memory response is after directory request");
                                        let wait_for_snoops = snoop_delay.saturating_sub(elapsed);
                                        schedule_partitioned_mesi_cache_response(
                                            context,
                                            wait_for_snoops,
                                            requester_route,
                                            response,
                                            fill_event,
                                            trace,
                                            response_cache,
                                            responses,
                                        )
                                        .expect("scheduled cache response");
                                    },
                                )
                                .expect("validated memory response latency");
                        },
                    )
                    .expect("validated DRAM ready latency");
            },
        )
        .map_err(MesiHarnessError::Scheduler)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_mesi_cache_response(
    context: &mut SchedulerContext<'_>,
    pre_response_delay: u64,
    requester_route: PartitionedMesiRoute,
    response: MemoryResponse,
    fill_event: MesiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
) -> Result<(), MesiHarnessError> {
    let response_delay = pre_response_delay + requester_route.route.response_latency();
    let route_id = requester_route.id;
    let response_endpoint = requester_route.route.source().clone();
    let response_partition = requester_route.route.source_partition();
    context
        .schedule_remote_after(response_partition, response_delay, move |context| {
            trace.record(MemoryTraceEvent::response(
                context.now(),
                route_id,
                response_endpoint,
                response.request_id(),
                response.status(),
            ));
            let result = response_cache
                .lock()
                .expect("cache lock")
                .accept_fill(response, fill_event)
                .expect("cache fill");
            if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                responses
                    .lock()
                    .expect("response lock")
                    .push(mesi_response_record(context.now(), result.kind(), response));
            }
        })
        .map_err(MesiHarnessError::Scheduler)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_mesi_memory_response_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    request: MemoryRequest,
    fill_event: MesiEvent,
    requester_route: PartitionedMesiRoute,
    memory_route: PartitionedMesiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    snoop_delay: u64,
) -> Result<(), MesiHarnessError> {
    let directory_tick = context.now();
    trace.record(MemoryTraceEvent::request(
        context.now(),
        memory_route.id,
        memory_route.route.source().clone(),
        MemoryTraceKind::RequestSent,
        request.id(),
    ));
    let memory_response_delay = memory_route.route.response_latency();
    let memory_route_id = memory_route.id;
    let memory_endpoint = memory_route.route.target().clone();
    let memory_source = memory_route.route.source().clone();
    let memory_partition = memory_route.route.target_partition();
    let directory_partition = memory_route.route.source_partition();
    let memory_trace = trace.clone();
    context
        .schedule_remote_after(
            memory_partition,
            memory_route.route.request_latency(),
            move |context| {
                memory_trace.record(MemoryTraceEvent::request(
                    context.now(),
                    memory_route_id,
                    memory_endpoint,
                    MemoryTraceKind::RequestArrived,
                    request.id(),
                ));
                let (ready_tick, response) = complete_partitioned_mesi_memory_request(
                    context.now(),
                    &request,
                    &backing,
                    dram_memory.as_ref(),
                    &dram_accesses,
                )
                .expect("memory response");
                let response_trace = memory_trace.clone();
                context
                    .schedule_local_after(
                        ready_tick
                            .checked_sub(context.now())
                            .expect("DRAM ready tick is not in the past"),
                        move |context| {
                            context
                                .schedule_remote_after(
                                    directory_partition,
                                    memory_response_delay,
                                    move |context| {
                                        response_trace.record(MemoryTraceEvent::response(
                                            context.now(),
                                            memory_route_id,
                                            memory_source,
                                            response.request_id(),
                                            response.status(),
                                        ));
                                        let elapsed = context
                                            .now()
                                            .checked_sub(directory_tick)
                                            .expect("memory response is after directory request");
                                        let wait_for_snoops = snoop_delay.saturating_sub(elapsed);
                                        schedule_partitioned_mesi_cache_response_parallel(
                                            context,
                                            wait_for_snoops,
                                            requester_route,
                                            response,
                                            fill_event,
                                            trace,
                                            response_cache,
                                            responses,
                                        )
                                        .expect("scheduled cache response");
                                    },
                                )
                                .expect("validated memory response latency");
                        },
                    )
                    .expect("validated DRAM ready latency");
            },
        )
        .map_err(MesiHarnessError::Scheduler)?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_mesi_cache_response_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    pre_response_delay: u64,
    requester_route: PartitionedMesiRoute,
    response: MemoryResponse,
    fill_event: MesiEvent,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MesiCacheController>>,
    responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
) -> Result<(), MesiHarnessError> {
    let response_delay = pre_response_delay + requester_route.route.response_latency();
    let route_id = requester_route.id;
    let response_endpoint = requester_route.route.source().clone();
    let response_partition = requester_route.route.source_partition();
    context
        .schedule_remote_after(response_partition, response_delay, move |context| {
            trace.record(MemoryTraceEvent::response(
                context.now(),
                route_id,
                response_endpoint,
                response.request_id(),
                response.status(),
            ));
            let result = response_cache
                .lock()
                .expect("cache lock")
                .accept_fill(response, fill_event)
                .expect("cache fill");
            if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                responses
                    .lock()
                    .expect("response lock")
                    .push(mesi_response_record(context.now(), result.kind(), response));
            }
        })
        .map_err(MesiHarnessError::Scheduler)?;

    Ok(())
}

fn line_backing_from_dram_memory(
    layout: CacheLineLayout,
    line_address: Address,
    controller: &DramMemoryController,
) -> Result<LineBackingStore, MesiHarnessError> {
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 0),
        line_address,
        AccessSize::new(layout.bytes()).map_err(MesiHarnessError::Memory)?,
        layout,
    )
    .map_err(MesiHarnessError::Memory)?;
    let mut probe = controller.clone();
    let outcome = probe.accept(0, &request).map_err(MesiHarnessError::Dram)?;
    let data = outcome
        .response()
        .and_then(MemoryResponse::data)
        .map(<[u8]>::to_vec)
        .ok_or(MesiHarnessError::Memory(MemoryError::MissingResponseData {
            request: request.id(),
        }))?;

    LineBackingStore::new(layout, line_address, data).map_err(MesiHarnessError::Backing)
}

fn complete_partitioned_mesi_memory_request(
    now: u64,
    request: &MemoryRequest,
    backing: &Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<&Arc<Mutex<DramMemoryController>>>,
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

    let outcome = dram_memory
        .lock()
        .expect("DRAM memory lock")
        .accept(now, request)
        .map_err(MesiHarnessError::Dram)?;
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

fn partitioned_mesi_source_data(
    decision: &MesiDirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MesiCacheController>>>,
) -> Result<Option<Vec<u8>>, MesiHarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(MesiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?;
    Ok(match grant.data_source() {
        MesiDirectoryDataSource::BackingMemory | MesiDirectoryDataSource::NoData => None,
        MesiDirectoryDataSource::OwnedCache(agent) => {
            let cache = caches
                .get(&agent)
                .ok_or(MesiHarnessError::UnknownCache { agent })?;
            let locked = cache.lock().expect("cache lock");
            Some(
                locked
                    .cached_data()
                    .ok_or(MesiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    })?
                    .to_vec(),
            )
        }
    })
}

fn schedule_partitioned_mesi_snoops(
    context: &mut SchedulerContext<'_>,
    decision: &MesiDirectoryDecision,
    cache_routes: &BTreeMap<AgentId, PartitionedMesiRoute>,
    caches: &BTreeMap<AgentId, Arc<Mutex<MesiCacheController>>>,
) -> Result<u64, MesiHarnessError> {
    let mut max_delay = 0;
    for snoop in decision.snoops() {
        let route = cache_routes
            .get(&snoop.target())
            .ok_or(MesiHarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let delay = route.route.response_latency();
        max_delay = max_delay.max(delay);
        let cache = caches
            .get(&snoop.target())
            .cloned()
            .ok_or(MesiHarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let event = snoop.event();
        context
            .schedule_remote_after(route.route.source_partition(), delay, move |_| {
                cache
                    .lock()
                    .expect("cache lock")
                    .accept_snoop(event)
                    .map_err(map_mesi_cache_error)
                    .expect("scheduled snoop");
            })
            .map_err(MesiHarnessError::Scheduler)?;
    }

    Ok(max_delay)
}

fn schedule_partitioned_mesi_snoops_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    decision: &MesiDirectoryDecision,
    cache_routes: &BTreeMap<AgentId, PartitionedMesiRoute>,
    caches: &BTreeMap<AgentId, Arc<Mutex<MesiCacheController>>>,
) -> Result<u64, MesiHarnessError> {
    let mut max_delay = 0;
    for snoop in decision.snoops() {
        let route = cache_routes
            .get(&snoop.target())
            .ok_or(MesiHarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let delay = route.route.response_latency();
        max_delay = max_delay.max(delay);
        let cache = caches
            .get(&snoop.target())
            .cloned()
            .ok_or(MesiHarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let event = snoop.event();
        context
            .schedule_remote_after(route.route.source_partition(), delay, move |_| {
                cache
                    .lock()
                    .expect("cache lock")
                    .accept_snoop(event)
                    .map_err(map_mesi_cache_error)
                    .expect("scheduled snoop");
            })
            .map_err(MesiHarnessError::Scheduler)?;
    }

    Ok(max_delay)
}
