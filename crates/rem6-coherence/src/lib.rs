use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use rem6_cache::{CacheControllerError, CacheControllerResultKind, MsiCacheController};
use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryGrant, DirectoryLineState, MsiDirectory,
};
use rem6_dram::DramMemoryController;
use rem6_fabric::{FabricModel, VirtualNetworkId};
use rem6_kernel::{
    ConservativeRunSummary, PartitionId, PartitionedScheduler, SchedulerError, Tick,
};
use rem6_memory::{Address, AgentId, CacheLineLayout, MemoryRequest, MemoryResponse};
use rem6_protocol_msi::{MsiLineId, MsiState};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport,
    TargetOutcome, TransportEndpointId, TransportError,
};

mod backing;
mod bank;
mod bank_snapshot_codec;
mod chi;
mod deferred;
mod directory_snapshot;
mod error;
mod mesi;
mod moesi;
mod partitioned_config;
mod partitioned_parallel;
mod partitioned_snapshot;
mod response;
#[cfg(test)]
mod response_record_tests;
mod snoop;
mod summary;
mod topology;
mod wait_for;

use deferred::{DeferredMemoryPath, DeferredMemoryWork};
use snoop::{DirectorySnoopWork, SnoopRoute};

pub use backing::LineBackingStore;
pub use bank::{
    MsiBankBackingLineSnapshot, MsiBankCycleAccepted, MsiBankCycleEntry, MsiBankCycleHistory,
    MsiBankCyclePlan, MsiBankCycleRun, MsiBankDirectoryHarness, MsiBankDirectoryHarnessSnapshot,
    MsiBankDownstreamTransportRequest, MsiBankTransportCycleRun,
};
pub use chi::{
    ChiCpuResponseRecord, ChiDirectoryDecisionRecord, ChiDirectoryLineHarness,
    ChiDirectoryLineHarnessSnapshot, ChiHarnessError, ChiSubmitResult,
    PartitionedChiDirectoryLineHarness, PartitionedChiDirectoryLineHarnessSnapshot,
};
pub use directory_snapshot::DirectoryLineHarnessSnapshot;
pub use error::HarnessError;
pub use mesi::{
    MesiCpuResponseRecord, MesiDirectoryDecisionRecord, MesiDirectoryLineHarness,
    MesiDirectoryLineHarnessSnapshot, MesiHarnessError, MesiSubmitResult,
    PartitionedMesiDirectoryLineHarness, PartitionedMesiDirectoryLineHarnessSnapshot,
};
pub use moesi::{
    MoesiCpuResponseRecord, MoesiDirectoryDecisionRecord, MoesiDirectoryLineHarness,
    MoesiDirectoryLineHarnessSnapshot, MoesiHarnessError, MoesiSubmitResult,
    PartitionedMoesiDirectoryLineHarness, PartitionedMoesiDirectoryLineHarnessSnapshot,
};
pub use partitioned_config::{
    DirectoryDecisionRecord, DramMemoryAccessRecord, PartitionedCacheAgentConfig,
    PartitionedDramMemoryConfig, PartitionedMemoryConfig, PartitionedRouteHopConfig,
};
pub use partitioned_snapshot::PartitionedDirectoryLineHarnessSnapshot;
use response::{push_locked_response_records_from_outcomes, push_response_records_from_outcomes};
pub use response::{CpuResponseRecord, SubmitKind, SubmitResult};
use summary::CoherenceResourceActivityWindow;
pub use summary::{
    ParallelCoherenceRunHistory, ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs,
};
pub use topology::{
    TopologyCacheAgentConfig, TopologyChiDirectoryHarnessConfig, TopologyDirectoryConfig,
    TopologyDirectoryHarnessConfig, TopologyDramMemoryConfig,
};

pub struct DirectoryLineHarness {
    line: MsiLineId,
    directory: MsiDirectory,
    caches: BTreeMap<AgentId, MsiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<CpuResponseRecord>,
    directory_decisions: Vec<DirectoryDecision>,
}

impl DirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        agents: I,
    ) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            });
        }

        let line = MsiLineId::new(line_address);
        let caches = agents
            .into_iter()
            .map(|agent| (agent, MsiCacheController::new(agent, layout, line_address)))
            .collect();

        Ok(Self {
            line,
            directory: MsiDirectory::new(),
            caches,
            backing,
            cpu_responses: Vec::new(),
            directory_decisions: Vec::new(),
        })
    }

    pub fn submit_cpu_request(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(map_cache_error)?;
        let cache_result = result.kind();

        if self.record_target_outcomes(0, cache_result, result.target_outcomes()) > 0 {
            return Ok(SubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(HarnessError::Cache(CacheControllerError::NoPendingMiss))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(HarnessError::Directory)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response)
            .map_err(map_cache_error)?;
        self.record_target_outcomes(0, fill.kind(), fill.target_outcomes());

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
            .with_directory_decision(decision))
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<MsiState, HarnessError> {
        Ok(self.cache(agent)?.state())
    }

    pub fn directory_state(&self) -> DirectoryLineState {
        self.directory.line_state(self.line)
    }

    pub fn cpu_responses(&self) -> Vec<CpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[DirectoryDecision] {
        &self.directory_decisions
    }

    pub fn cache_data(&self, agent: AgentId) -> Result<Option<Vec<u8>>, HarnessError> {
        Ok(self.cache(agent)?.cached_data().map(<[u8]>::to_vec))
    }

    pub const fn line(&self) -> MsiLineId {
        self.line
    }

    fn directory_response(
        &mut self,
        request: &MemoryRequest,
        decision: &DirectoryDecision,
    ) -> Result<MemoryResponse, HarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(HarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(snoop.event())
                .map_err(map_cache_error)?;
        }

        match grant.data_source() {
            DirectoryDataSource::BackingMemory => self.backing.respond(request),
            DirectoryDataSource::ModifiedOwner(_) => {
                MemoryResponse::completed(request, source_data).map_err(HarnessError::Memory)
            }
            DirectoryDataSource::NoData => {
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
        }
    }

    fn source_data(&self, grant: DirectoryGrant) -> Result<Option<Vec<u8>>, HarnessError> {
        match grant.data_source() {
            DirectoryDataSource::BackingMemory | DirectoryDataSource::NoData => Ok(None),
            DirectoryDataSource::ModifiedOwner(agent) => {
                let data =
                    self.cache(agent)?
                        .cached_data()
                        .ok_or(HarnessError::GrantDataUnavailable {
                            agent,
                            line: grant.line(),
                        })?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn cache(&self, agent: AgentId) -> Result<&MsiCacheController, HarnessError> {
        self.caches
            .get(&agent)
            .ok_or(HarnessError::UnknownCache { agent })
    }

    fn cache_mut(&mut self, agent: AgentId) -> Result<&mut MsiCacheController, HarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(HarnessError::UnknownCache { agent })
    }

    fn record_target_outcomes(
        &mut self,
        tick: u64,
        cache_result: CacheControllerResultKind,
        outcomes: &[TargetOutcome],
    ) -> usize {
        push_response_records_from_outcomes(&mut self.cpu_responses, tick, cache_result, outcomes)
    }
}

pub struct PartitionedDirectoryLineHarness {
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    line: MsiLineId,
    directory: Arc<Mutex<MsiDirectory>>,
    caches: BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    routes: BTreeMap<AgentId, MemoryRouteId>,
    backing: Option<Arc<Mutex<LineBackingStore>>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    memory_route: Option<MemoryRouteId>,
    memory_route_info: Option<MemoryRoute>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
    directory_decisions: Arc<Mutex<Vec<DirectoryDecisionRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    parallel_runs: Vec<ParallelCoherenceRunSummary>,
    wait_for: wait_for::CoherenceWaitFor,
}

impl PartitionedDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        agents: I,
    ) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        Self::new_internal(
            layout,
            line_address,
            Some(backing),
            directory_partition,
            directory_endpoint,
            None,
            None,
            agents,
        )
    }

    pub fn new_with_memory<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: PartitionedMemoryConfig,
        agents: I,
    ) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        Self::new_internal(
            layout,
            line_address,
            Some(backing),
            directory_partition,
            directory_endpoint,
            Some(memory),
            None,
            agents,
        )
    }

    pub fn new_with_dram_memory<I>(
        layout: CacheLineLayout,
        line_address: Address,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: PartitionedDramMemoryConfig,
        agents: I,
    ) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        Self::new_internal(
            layout,
            line_address,
            None,
            directory_partition,
            directory_endpoint,
            None,
            Some(memory),
            agents,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn new_internal<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: Option<LineBackingStore>,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: Option<PartitionedMemoryConfig>,
        dram_memory: Option<PartitionedDramMemoryConfig>,
        agents: I,
    ) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);
        if let Some(backing) = &backing {
            if backing.line_address() != line_address {
                return Err(HarnessError::WrongLine {
                    expected: line_address,
                    actual: backing.line_address(),
                });
            }
        }

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?;
        let agent_configs: Vec<_> = agents.into_iter().collect();
        let uses_fabric = memory
            .as_ref()
            .is_some_and(|memory| route_hops_use_fabric(memory.route_hops()))
            || dram_memory
                .as_ref()
                .is_some_and(|memory| route_hops_use_fabric(memory.route_hops()))
            || agent_configs
                .iter()
                .any(|config| route_hops_use_fabric(config.route_hops()));
        let fabric = uses_fabric.then(|| Arc::new(Mutex::new(FabricModel::new())));
        let mut transport = if let Some(fabric) = &fabric {
            MemoryTransport::with_shared_fabric(Arc::clone(fabric))
        } else {
            MemoryTransport::new()
        };
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();
        let mut memory_route = None;
        let mut memory_route_info = None;
        let mut dram_controller = None;

        if let Some(memory) = memory {
            partition_count = partition_count.max(
                memory
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_hops(&mut partition_count, memory.route_hops())?;
            let route = memory_route_from_config(
                directory_endpoint.clone(),
                directory_partition,
                memory.endpoint().clone(),
                memory.partition(),
                memory.request_latency(),
                memory.response_latency(),
                memory.request_virtual_network(),
                memory.response_virtual_network(),
                memory.route_hops(),
            )
            .map_err(HarnessError::Transport)?;
            memory_route = Some(
                transport
                    .add_route(route.clone())
                    .map_err(HarnessError::Transport)?,
            );
            memory_route_info = Some(route);
        }

        if let Some(memory) = dram_memory {
            partition_count = partition_count.max(
                memory
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_hops(&mut partition_count, memory.route_hops())?;
            let route = memory_route_from_config(
                directory_endpoint.clone(),
                directory_partition,
                memory.endpoint().clone(),
                memory.partition(),
                memory.request_latency(),
                memory.response_latency(),
                memory.request_virtual_network(),
                memory.response_virtual_network(),
                memory.route_hops(),
            )
            .map_err(HarnessError::Transport)?;
            memory_route = Some(
                transport
                    .add_route(route.clone())
                    .map_err(HarnessError::Transport)?,
            );
            memory_route_info = Some(route);
            dram_controller = Some(Arc::new(Mutex::new(memory.into_controller())));
        }

        for config in agent_configs {
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_hops(&mut partition_count, config.route_hops())?;
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(MsiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(HarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }
            let route = transport
                .add_route(
                    memory_route_from_config(
                        config.endpoint().clone(),
                        config.partition(),
                        directory_endpoint.clone(),
                        directory_partition,
                        config.request_latency(),
                        config.response_latency(),
                        config.request_virtual_network(),
                        config.response_virtual_network(),
                        config.route_hops(),
                    )
                    .map_err(HarnessError::Transport)?,
                )
                .map_err(HarnessError::Transport)?;
            routes.insert(config.agent(), route);
        }

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(HarnessError::Scheduler)?,
            transport,
            line: MsiLineId::new(line_address),
            directory: Arc::new(Mutex::new(MsiDirectory::new())),
            caches,
            routes,
            backing: backing.map(|backing| Arc::new(Mutex::new(backing))),
            dram_memory: dram_controller,
            fabric,
            memory_route,
            memory_route_info,
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
            dram_accesses: Arc::new(Mutex::new(Vec::new())),
            parallel_runs: Vec::new(),
            wait_for: wait_for::CoherenceWaitFor::new(),
        })
    }

    pub fn submit_cpu_request(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        let cache = self.cache_arc(agent)?;
        let result = cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
            .map_err(map_cache_error)?;
        let cache_result = result.kind();

        if push_locked_response_records_from_outcomes(
            &self.cpu_responses,
            self.scheduler.now(),
            cache_result,
            result.target_outcomes(),
        ) > 0
        {
            return Ok(SubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(HarnessError::Cache(CacheControllerError::NoPendingMiss))?;
        let route = *self
            .routes
            .get(&agent)
            .ok_or(HarnessError::UnknownCache { agent })?;
        let cache_route = self
            .transport
            .route(route)
            .cloned()
            .ok_or(HarnessError::Transport(TransportError::UnknownRoute {
                route,
            }))?;
        let mut cache_routes = BTreeMap::new();
        for (agent, route_id) in &self.routes {
            let route_info =
                self.transport
                    .route(*route_id)
                    .cloned()
                    .ok_or(HarnessError::Transport(TransportError::UnknownRoute {
                        route: *route_id,
                    }))?;
            cache_routes.insert(*agent, SnoopRoute::new(*route_id, route_info));
        }
        let directory = Arc::clone(&self.directory);
        let caches = self.caches.clone();
        let backing = self.backing.clone();
        let dram_memory = self.dram_memory.clone();
        let decisions = Arc::clone(&self.directory_decisions);
        let dram_accesses = Arc::clone(&self.dram_accesses);
        let fabric = self.fabric.clone();
        let trace = self.trace.clone();
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let memory_path = self.memory_route.zip(self.memory_route_info.clone()).map(
            |(memory_route, memory_route_info)| DeferredMemoryPath {
                cache_route_id: route,
                cache_route: cache_route.clone(),
                memory_route_id: memory_route,
                memory_route: memory_route_info,
            },
        );
        let deferred = memory_path.map(|path| DeferredMemoryWork {
            path,
            cache_routes: cache_routes.clone(),
            caches: caches.clone(),
            backing: backing.clone(),
            dram_memory: dram_memory.clone(),
            fabric: fabric.clone(),
            trace: trace.clone(),
            response_cache: Arc::clone(&response_cache),
            responses: Arc::clone(&responses),
            decisions: Arc::clone(&decisions),
            dram_accesses: Arc::clone(&dram_accesses),
            wait_for: None,
        });
        let response_cache_for_snoop = Arc::clone(&response_cache);
        let responses_for_snoop = Arc::clone(&responses);
        self.transport
            .submit(
                &mut self.scheduler,
                route,
                downstream,
                trace.clone(),
                move |delivery, context| {
                    let decision = directory
                        .lock()
                        .expect("directory lock")
                        .accept(delivery.request().clone())
                        .expect("directory decision");
                    if decision_uses_backing_memory(&decision) {
                        if let Some(deferred) = deferred {
                            deferred
                                .schedule(context, delivery.request().clone(), decision)
                                .expect("deferred memory response");
                            return TargetOutcome::NoResponse;
                        }
                    }
                    if !decision.snoops().is_empty() && !decision_uses_backing_memory(&decision) {
                        DirectorySnoopWork::new(
                            delivery.request().clone(),
                            decision,
                            SnoopRoute::new(route, cache_route.clone()),
                            cache_routes,
                            caches,
                            fabric,
                            trace.clone(),
                            Arc::clone(&response_cache_for_snoop),
                            Arc::clone(&responses_for_snoop),
                            Arc::clone(&decisions),
                        )
                        .schedule(context, delivery.tick())
                        .expect("scheduled directory snoops");
                        return TargetOutcome::NoResponse;
                    }
                    let response = partitioned_directory_response(
                        delivery.request(),
                        &decision,
                        &caches,
                        &backing,
                    )
                    .expect("directory response");
                    decisions
                        .lock()
                        .expect("decision lock")
                        .push(DirectoryDecisionRecord::new(
                            delivery.tick(),
                            delivery.request().id().agent(),
                            decision,
                        ));
                    TargetOutcome::Respond(response)
                },
                move |delivery| {
                    let result = response_cache
                        .lock()
                        .expect("cache lock")
                        .accept_fill(delivery.response().clone())
                        .expect("cache fill");
                    push_locked_response_records_from_outcomes(
                        &responses,
                        delivery.tick(),
                        result.kind(),
                        result.target_outcomes(),
                    );
                },
            )
            .map_err(HarnessError::Transport)?;

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result))
    }

    pub fn run_until_idle(&mut self) -> ConservativeRunSummary {
        self.scheduler.run_until_idle_conservative()
    }

    pub fn now(&self) -> Tick {
        self.scheduler.now()
    }

    pub fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, HarnessError> {
        let cpu_responses_before = self.cpu_responses.lock().expect("response lock").len();
        let directory_decisions_before = self
            .directory_decisions
            .lock()
            .expect("decision lock")
            .len();
        let dram_accesses_before = self.dram_accesses.lock().expect("DRAM access lock").len();
        let resource_window =
            CoherenceResourceActivityWindow::mark(&self.transport, self.dram_memory.as_ref());
        let wait_for_graph_before = self.wait_for.graph();
        let scheduler_run = self
            .scheduler
            .run_until_idle_parallel_recorded()
            .map_err(HarnessError::Scheduler)?;
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
            .expect("DRAM access lock")
            .len()
            .saturating_sub(dram_accesses_before);
        let (fabric_activity, dram_activity) =
            resource_window.collect(&self.transport, self.dram_memory.as_ref());
        let run = ParallelCoherenceRunSummary::new(
            scheduler_run,
            cpu_response_count,
            directory_decision_count,
            dram_access_count,
            fabric_activity,
            dram_activity,
            ParallelCoherenceWaitForGraphs::new(wait_for_graph_before, self.wait_for.graph()),
        );
        if run.has_parallel_observation() {
            self.parallel_runs.push(run.clone());
        }
        Ok(run)
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<MsiState, HarnessError> {
        Ok(self.cache_arc(agent)?.lock().expect("cache lock").state())
    }

    pub fn directory_state(&self) -> DirectoryLineState {
        self.directory
            .lock()
            .expect("directory lock")
            .line_state(self.line)
    }

    pub fn route(&self, agent: AgentId) -> Result<MemoryRouteId, HarnessError> {
        self.routes
            .get(&agent)
            .copied()
            .ok_or(HarnessError::UnknownCache { agent })
    }

    pub const fn memory_route(&self) -> Option<MemoryRouteId> {
        self.memory_route
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.snapshot()
    }

    pub fn cpu_responses(&self) -> Vec<CpuResponseRecord> {
        self.cpu_responses.lock().expect("response lock").clone()
    }

    pub fn directory_decisions(&self) -> Vec<DirectoryDecisionRecord> {
        self.directory_decisions
            .lock()
            .expect("decision lock")
            .clone()
    }

    pub fn dram_memory_accesses(&self) -> Vec<DramMemoryAccessRecord> {
        self.dram_accesses.lock().expect("DRAM access lock").clone()
    }

    pub fn parallel_runs(&self) -> &[ParallelCoherenceRunSummary] {
        &self.parallel_runs
    }

    pub fn parallel_run_history(&self) -> ParallelCoherenceRunHistory {
        ParallelCoherenceRunHistory::from_runs(&self.parallel_runs)
    }

    pub const fn line(&self) -> MsiLineId {
        self.line
    }

    fn cache_arc(&self, agent: AgentId) -> Result<Arc<Mutex<MsiCacheController>>, HarnessError> {
        self.caches
            .get(&agent)
            .cloned()
            .ok_or(HarnessError::UnknownCache { agent })
    }
}

fn expand_partition_count_for_hops(
    partition_count: &mut u32,
    hops: &[PartitionedRouteHopConfig],
) -> Result<(), HarnessError> {
    for hop in hops {
        *partition_count = (*partition_count).max(
            hop.partition()
                .index()
                .checked_add(1)
                .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
    }

    Ok(())
}

fn route_hops_use_fabric(hops: &[PartitionedRouteHopConfig]) -> bool {
    hops.iter()
        .any(|hop| hop.request_fabric_path().is_some() || hop.response_fabric_path().is_some())
}

#[allow(clippy::too_many_arguments)]
fn memory_route_from_config(
    source_endpoint: TransportEndpointId,
    source_partition: PartitionId,
    target_endpoint: TransportEndpointId,
    target_partition: PartitionId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    route_hops: &[PartitionedRouteHopConfig],
) -> Result<MemoryRoute, TransportError> {
    if route_hops.is_empty() {
        return Ok(MemoryRoute::new(
            source_endpoint,
            source_partition,
            target_endpoint,
            target_partition,
            request_latency,
            response_latency,
        )?
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
            )?;
            if let Some(path) = hop.request_fabric_path() {
                route_hop = route_hop.with_request_fabric_path(path.clone());
            }
            if let Some(path) = hop.response_fabric_path() {
                route_hop = route_hop.with_response_fabric_path(path.clone());
            }
            Ok(route_hop)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        MemoryRoute::new_path(source_endpoint, source_partition, hops)?
            .with_virtual_networks(request_virtual_network, response_virtual_network),
    )
}

fn decision_uses_backing_memory(decision: &DirectoryDecision) -> bool {
    decision
        .grant()
        .is_some_and(|grant| grant.data_source() == DirectoryDataSource::BackingMemory)
}

fn apply_directory_snoops(
    decision: &DirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
) -> Result<(), HarnessError> {
    for snoop in decision.snoops() {
        let cache = caches
            .get(&snoop.target())
            .ok_or(HarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        cache
            .lock()
            .expect("cache lock")
            .accept_snoop(snoop.event())
            .map_err(map_cache_error)?;
    }

    Ok(())
}

fn partitioned_directory_source_data(
    decision: &DirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
) -> Result<Option<Vec<u8>>, HarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(HarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?;
    Ok(match grant.data_source() {
        DirectoryDataSource::BackingMemory | DirectoryDataSource::NoData => None,
        DirectoryDataSource::ModifiedOwner(agent) => {
            let cache = caches
                .get(&agent)
                .ok_or(HarnessError::UnknownCache { agent })?;
            let locked = cache.lock().expect("cache lock");
            Some(
                locked
                    .cached_data()
                    .ok_or(HarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    })?
                    .to_vec(),
            )
        }
    })
}

fn partitioned_directory_response(
    request: &MemoryRequest,
    decision: &DirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    backing: &Option<Arc<Mutex<LineBackingStore>>>,
) -> Result<MemoryResponse, HarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(HarnessError::MissingDirectoryGrant {
            request: request.id(),
        })?;
    let source_data = partitioned_directory_source_data(decision, caches)?;

    apply_directory_snoops(decision, caches)?;

    match grant.data_source() {
        DirectoryDataSource::BackingMemory => backing
            .as_ref()
            .ok_or(HarnessError::MissingBackingMemory {
                line: request.line_address(),
            })?
            .lock()
            .expect("backing lock")
            .respond(request),
        DirectoryDataSource::ModifiedOwner(_) => {
            MemoryResponse::completed(request, source_data).map_err(HarnessError::Memory)
        }
        DirectoryDataSource::NoData => {
            MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
        }
    }
}

pub struct CoherentLineHarness {
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    route: MemoryRouteId,
    cache: Arc<Mutex<MsiCacheController>>,
    backing: Arc<Mutex<LineBackingStore>>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
}

impl CoherentLineHarness {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        cache_agent: rem6_memory::AgentId,
        layout: CacheLineLayout,
        line_address: Address,
        cache_partition: PartitionId,
        memory_partition: PartitionId,
        cache_endpoint: TransportEndpointId,
        memory_endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
        backing: LineBackingStore,
    ) -> Result<Self, HarnessError> {
        let partitions = cache_partition
            .index()
            .max(memory_partition.index())
            .checked_add(1)
            .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?;
        let scheduler = PartitionedScheduler::with_min_remote_delay(partitions, 1)
            .map_err(HarnessError::Scheduler)?;
        let mut transport = MemoryTransport::new();
        let route = transport
            .add_route(
                MemoryRoute::new(
                    cache_endpoint,
                    cache_partition,
                    memory_endpoint,
                    memory_partition,
                    request_latency,
                    response_latency,
                )
                .map_err(HarnessError::Transport)?,
            )
            .map_err(HarnessError::Transport)?;

        Ok(Self {
            scheduler,
            transport,
            route,
            cache: Arc::new(Mutex::new(MsiCacheController::new(
                cache_agent,
                layout,
                line_address,
            ))),
            backing: Arc::new(Mutex::new(backing)),
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn submit_cpu_request(
        &mut self,
        request: MemoryRequest,
    ) -> Result<SubmitResult, HarnessError> {
        let result = self
            .cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
            .map_err(map_cache_error)?;
        let cache_result = result.kind();

        if push_locked_response_records_from_outcomes(
            &self.cpu_responses,
            self.scheduler.now(),
            cache_result,
            result.target_outcomes(),
        ) > 0
        {
            return Ok(SubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(HarnessError::Cache(CacheControllerError::NoPendingMiss))?;
        let backing = Arc::clone(&self.backing);
        let cache = Arc::clone(&self.cache);
        let responses = Arc::clone(&self.cpu_responses);
        self.transport
            .submit(
                &mut self.scheduler,
                self.route,
                downstream,
                self.trace.clone(),
                move |delivery, _context| {
                    let response = backing
                        .lock()
                        .expect("backing lock")
                        .respond(delivery.request())
                        .expect("backing store response");
                    TargetOutcome::Respond(response)
                },
                move |delivery| {
                    let result = cache
                        .lock()
                        .expect("cache lock")
                        .accept_fill(delivery.response().clone())
                        .expect("cache fill");
                    push_locked_response_records_from_outcomes(
                        &responses,
                        delivery.tick(),
                        result.kind(),
                        result.target_outcomes(),
                    );
                },
            )
            .map_err(HarnessError::Transport)?;

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result))
    }

    pub fn run_until_idle(&mut self) -> ConservativeRunSummary {
        self.scheduler.run_until_idle_conservative()
    }

    pub fn cache_state(&self) -> MsiState {
        self.cache.lock().expect("cache lock").state()
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.snapshot()
    }

    pub fn cpu_responses(&self) -> Vec<CpuResponseRecord> {
        self.cpu_responses.lock().expect("response lock").clone()
    }

    pub fn backing_data(&self) -> Vec<u8> {
        self.backing.lock().expect("backing lock").data().to_vec()
    }
}

fn map_cache_error(error: CacheControllerError) -> HarnessError {
    match error {
        CacheControllerError::LineBusy { state } => HarnessError::LineBusy { state },
        error => HarnessError::Cache(error),
    }
}
