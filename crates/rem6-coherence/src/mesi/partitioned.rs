use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::{MesiCacheController, MesiCacheControllerError};
use rem6_directory::{
    MesiDirectory, MesiDirectoryDataSource, MesiDirectoryDecision, MesiDirectoryLineState,
};
use rem6_dram::DramMemoryController;
use rem6_kernel::{
    ConservativeRunSummary, ParallelSchedulerContext, PartitionId, PartitionedScheduler,
    SchedulerContext, SchedulerError, Tick,
};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId,
    MemoryResponse,
};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport,
    TargetOutcome, TransportEndpointId,
};

use crate::summary::CoherenceResourceActivityWindow;
use crate::{
    DramMemoryAccessRecord, HarnessError, LineBackingStore, ParallelCoherenceRunSummary,
    PartitionedCacheAgentConfig, PartitionedDramMemoryConfig, PartitionedMemoryConfig,
    PartitionedRouteHopConfig, SubmitKind,
};

use super::{
    decision_uses_mesi_backing_memory, fill_event, map_mesi_cache_error, mesi_response_record,
    MesiCpuResponseRecord, MesiDirectoryDecisionRecord, MesiHarnessError, MesiSubmitResult,
};

mod response;
mod snapshot;

use response::{
    schedule_partitioned_mesi_cache_response, schedule_partitioned_mesi_cache_response_parallel,
    schedule_partitioned_mesi_memory_response, schedule_partitioned_mesi_memory_response_parallel,
};
pub use snapshot::PartitionedMesiDirectoryLineHarnessSnapshot;

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
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_mesi_hops(&mut partition_count, config.route_hops())?;
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

            let route = mesi_route_from_config(
                config.endpoint().clone(),
                config.partition(),
                directory_endpoint.clone(),
                directory_partition,
                config.request_latency(),
                config.response_latency(),
                config.request_virtual_network(),
                config.response_virtual_network(),
                config.route_hops(),
            )?;
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
        expand_partition_count_for_mesi_hops(&mut partition_count, memory.route_hops())?;
        let mut transport = MemoryTransport::new();
        let memory_route = mesi_route_from_config(
            directory_endpoint.clone(),
            directory_partition,
            memory.endpoint().clone(),
            memory.partition(),
            memory.request_latency(),
            memory.response_latency(),
            memory.request_virtual_network(),
            memory.response_virtual_network(),
            memory.route_hops(),
        )?;
        let memory_route_id = transport
            .add_route(memory_route.clone())
            .map_err(MesiHarnessError::Transport)?;
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agents {
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_mesi_hops(&mut partition_count, config.route_hops())?;
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

            let route = mesi_route_from_config(
                config.endpoint().clone(),
                config.partition(),
                directory_endpoint.clone(),
                directory_partition,
                config.request_latency(),
                config.response_latency(),
                config.request_virtual_network(),
                config.response_virtual_network(),
                config.route_hops(),
            )?;
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
        expand_partition_count_for_mesi_hops(&mut partition_count, memory.route_hops())?;
        let mut transport = MemoryTransport::new();
        let memory_route = mesi_route_from_config(
            directory_endpoint.clone(),
            directory_partition,
            memory.endpoint().clone(),
            memory.partition(),
            memory.request_latency(),
            memory.response_latency(),
            memory.request_virtual_network(),
            memory.response_virtual_network(),
            memory.route_hops(),
        )?;
        let memory_route_id = transport
            .add_route(memory_route.clone())
            .map_err(MesiHarnessError::Transport)?;
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agents {
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_mesi_hops(&mut partition_count, config.route_hops())?;
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

            let route = mesi_route_from_config(
                config.endpoint().clone(),
                config.partition(),
                directory_endpoint.clone(),
                directory_partition,
                config.request_latency(),
                config.response_latency(),
                config.request_virtual_network(),
                config.response_virtual_network(),
                config.route_hops(),
            )?;
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
                    let route_delay = requester_route.route.response_latency();
                    let pre_response_delay = snoop_delay.saturating_sub(route_delay);
                    schedule_partitioned_mesi_cache_response(
                        context,
                        pre_response_delay,
                        requester_route,
                        response,
                        fill_event,
                        trace,
                        response_cache,
                        responses,
                    )
                    .expect("scheduled cache response");

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
                    let route_delay = requester_route.route.response_latency();
                    let pre_response_delay = snoop_delay.saturating_sub(route_delay);
                    schedule_partitioned_mesi_cache_response_parallel(
                        context,
                        pre_response_delay,
                        requester_route,
                        response,
                        fill_event,
                        trace,
                        response_cache,
                        responses,
                    )
                    .expect("scheduled cache response");

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

    pub fn now(&self) -> Tick {
        self.scheduler.now()
    }

    pub fn run_until_idle_parallel(&mut self) -> Result<ConservativeRunSummary, MesiHarnessError> {
        self.run_until_idle_parallel_recorded()
            .map(|run| run.summary())
    }

    pub fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, MesiHarnessError> {
        let cpu_responses_before = self.cpu_responses.lock().expect("response lock").len();
        let directory_decisions_before = self
            .directory_decisions
            .lock()
            .expect("decision lock")
            .len();
        let dram_accesses_before = self.dram_accesses.lock().expect("dram access lock").len();
        let resource_window =
            CoherenceResourceActivityWindow::mark(&self.transport, self.dram_memory.as_ref());

        let scheduler_run = self
            .scheduler
            .run_until_idle_parallel_recorded()
            .map_err(MesiHarnessError::Scheduler)?;

        let cpu_response_count = self
            .cpu_responses
            .lock()
            .expect("response lock")
            .len()
            .saturating_sub(cpu_responses_before);
        let directory_decision_count = self
            .directory_decisions
            .lock()
            .expect("decision lock")
            .len()
            .saturating_sub(directory_decisions_before);
        let dram_access_count = self
            .dram_accesses
            .lock()
            .expect("dram access lock")
            .len()
            .saturating_sub(dram_accesses_before);
        let (fabric_activity, dram_activity) =
            resource_window.collect(&self.transport, self.dram_memory.as_ref());

        Ok(ParallelCoherenceRunSummary::new(
            scheduler_run,
            cpu_response_count,
            directory_decision_count,
            dram_access_count,
            fabric_activity,
            dram_activity,
        ))
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

fn expand_partition_count_for_mesi_hops(
    partition_count: &mut u32,
    hops: &[PartitionedRouteHopConfig],
) -> Result<(), MesiHarnessError> {
    for hop in hops {
        *partition_count = (*partition_count).max(
            hop.partition()
                .index()
                .checked_add(1)
                .ok_or(MesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
    }

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn mesi_route_from_config(
    source_endpoint: TransportEndpointId,
    source_partition: PartitionId,
    target_endpoint: TransportEndpointId,
    target_partition: PartitionId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: rem6_fabric::VirtualNetworkId,
    response_virtual_network: rem6_fabric::VirtualNetworkId,
    route_hops: &[PartitionedRouteHopConfig],
) -> Result<MemoryRoute, MesiHarnessError> {
    if route_hops.is_empty() {
        return Ok(MemoryRoute::new(
            source_endpoint,
            source_partition,
            target_endpoint,
            target_partition,
            request_latency,
            response_latency,
        )
        .map_err(MesiHarnessError::Transport)?
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
            .map_err(MesiHarnessError::Transport)?;
            if let Some(path) = hop.request_fabric_path() {
                route_hop = route_hop.with_request_fabric_path(path.clone());
            }
            if let Some(path) = hop.response_fabric_path() {
                route_hop = route_hop.with_response_fabric_path(path.clone());
            }
            Ok(route_hop)
        })
        .collect::<Result<Vec<_>, MesiHarnessError>>()?;

    Ok(
        MemoryRoute::new_path(source_endpoint, source_partition, hops)
            .map_err(MesiHarnessError::Transport)?
            .with_virtual_networks(request_virtual_network, response_virtual_network),
    )
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
