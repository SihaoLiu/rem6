use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_cache::{MesiCacheController, MesiCacheControllerError, MesiCacheControllerResultKind};
use rem6_directory::{
    MesiDirectory, MesiDirectoryDataSource, MesiDirectoryDecision, MesiDirectoryError,
    MesiDirectoryGrant, MesiDirectoryLineState,
};
use rem6_kernel::{
    ConservativeRunSummary, ParallelSchedulerContext, PartitionId, PartitionedScheduler,
    SchedulerError,
};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_protocol_mesi::{MesiEvent, MesiLineId, MesiState};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport, TargetOutcome,
    TransportEndpointId, TransportError,
};

use crate::{HarnessError, LineBackingStore, PartitionedCacheAgentConfig, SubmitKind};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MesiHarnessError {
    LineBusy { state: MesiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: MesiLineId },
    UnexpectedGrantState { state: MesiState },
    UnsupportedRouteHops { agent: AgentId },
    Cache(MesiCacheControllerError),
    Directory(MesiDirectoryError),
    Memory(MemoryError),
    Scheduler(SchedulerError),
    Transport(TransportError),
    Backing(HarnessError),
}

impl fmt::Display for MesiHarnessError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LineBusy { state } => write!(formatter, "cache line is busy in {state:?}"),
            Self::UnknownCache { agent } => {
                write!(formatter, "cache agent {} is not registered", agent.get())
            }
            Self::DuplicateCache { agent } => {
                write!(
                    formatter,
                    "cache agent {} is already registered",
                    agent.get()
                )
            }
            Self::MissingDirectoryGrant { request } => write!(
                formatter,
                "directory did not grant request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::GrantDataUnavailable { agent, line } => write!(
                formatter,
                "agent {} has no data for line {:#x}",
                agent.get(),
                line.address().get()
            ),
            Self::UnexpectedGrantState { state } => {
                write!(formatter, "directory granted transient state {state:?}")
            }
            Self::UnsupportedRouteHops { agent } => write!(
                formatter,
                "cache agent {} uses route hops that are not supported by this harness",
                agent.get()
            ),
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Backing(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for MesiHarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Directory(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::Backing(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiSubmitResult {
    kind: SubmitKind,
    cache_result: MesiCacheControllerResultKind,
    directory_decision: Option<MesiDirectoryDecision>,
}

impl MesiSubmitResult {
    fn new(kind: SubmitKind, cache_result: MesiCacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
        }
    }

    fn with_directory_decision(mut self, decision: MesiDirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> MesiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&MesiDirectoryDecision> {
        self.directory_decision.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiCpuResponseRecord {
    tick: u64,
    cache_result: MesiCacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl MesiCpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: MesiCacheControllerResultKind,
        request: MemoryRequestId,
        status: ResponseStatus,
        data: Option<Vec<u8>>,
    ) -> Self {
        Self {
            tick,
            cache_result,
            request,
            status,
            data,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn cache_result(&self) -> MesiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn status(&self) -> ResponseStatus {
        self.status
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

pub struct MesiDirectoryLineHarness {
    line: MesiLineId,
    directory: MesiDirectory,
    caches: BTreeMap<AgentId, MesiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<MesiCpuResponseRecord>,
    directory_decisions: Vec<MesiDirectoryDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MesiDirectoryDecisionRecord {
    tick: u64,
    requester: AgentId,
    decision: MesiDirectoryDecision,
}

impl MesiDirectoryDecisionRecord {
    pub const fn new(tick: u64, requester: AgentId, decision: MesiDirectoryDecision) -> Self {
        Self {
            tick,
            requester,
            decision,
        }
    }

    pub const fn tick(&self) -> u64 {
        self.tick
    }

    pub const fn requester(&self) -> AgentId {
        self.requester
    }

    pub const fn decision(&self) -> &MesiDirectoryDecision {
        &self.decision
    }
}

impl MesiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        agents: I,
    ) -> Result<Self, MesiHarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(MesiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(agent, MesiCacheController::new(agent, layout, line_address))
                .is_some()
            {
                return Err(MesiHarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            line: MesiLineId::new(line_address),
            directory: MesiDirectory::new(),
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
    ) -> Result<MesiSubmitResult, MesiHarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(map_mesi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
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
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(MesiHarnessError::Directory)?;
        let fill_event = fill_event(&decision)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response, fill_event)
            .map_err(map_mesi_cache_error)?;
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

        Ok(
            MesiSubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
                .with_directory_decision(decision),
        )
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<MesiState, MesiHarnessError> {
        Ok(self.cache(agent)?.state())
    }

    pub fn directory_state(&self) -> MesiDirectoryLineState {
        self.directory.line_state(self.line)
    }

    pub fn cpu_responses(&self) -> Vec<MesiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[MesiDirectoryDecision] {
        &self.directory_decisions
    }

    pub fn cache_data(&self, agent: AgentId) -> Result<Option<Vec<u8>>, MesiHarnessError> {
        Ok(self.cache(agent)?.cached_data().map(<[u8]>::to_vec))
    }

    pub const fn line(&self) -> MesiLineId {
        self.line
    }

    fn directory_response(
        &mut self,
        request: &MemoryRequest,
        decision: &MesiDirectoryDecision,
    ) -> Result<MemoryResponse, MesiHarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(MesiHarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        if decision
            .snoops()
            .iter()
            .any(|snoop| snoop.event() == MesiEvent::SnoopRead)
        {
            if let Some(data) = &source_data {
                self.backing
                    .replace_data(data.clone())
                    .map_err(MesiHarnessError::Backing)?;
            }
        }

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(snoop.event())
                .map_err(map_mesi_cache_error)?;
        }

        match grant.data_source() {
            MesiDirectoryDataSource::BackingMemory => self
                .backing
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

    fn source_data(&self, grant: MesiDirectoryGrant) -> Result<Option<Vec<u8>>, MesiHarnessError> {
        match grant.data_source() {
            MesiDirectoryDataSource::BackingMemory | MesiDirectoryDataSource::NoData => Ok(None),
            MesiDirectoryDataSource::OwnedCache(agent) => {
                let data = self.cache(agent)?.cached_data().ok_or(
                    MesiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    },
                )?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn cache(&self, agent: AgentId) -> Result<&MesiCacheController, MesiHarnessError> {
        self.caches
            .get(&agent)
            .ok_or(MesiHarnessError::UnknownCache { agent })
    }

    fn cache_mut(&mut self, agent: AgentId) -> Result<&mut MesiCacheController, MesiHarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(MesiHarnessError::UnknownCache { agent })
    }

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: MesiCacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(mesi_response_record(tick, cache_result, response));
    }
}

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
    backing: Arc<Mutex<LineBackingStore>>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<MesiCpuResponseRecord>>>,
    directory_decisions: Arc<Mutex<Vec<MesiDirectoryDecisionRecord>>>,
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
            backing: Arc::new(Mutex::new(backing)),
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
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
        let backing = Arc::clone(&self.backing);
        let trace = self.trace.clone();
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let decisions = Arc::clone(&self.directory_decisions);

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
        let backing = Arc::clone(&self.backing);
        let trace = self.trace.clone();
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let decisions = Arc::clone(&self.directory_decisions);

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

fn fill_event(decision: &MesiDirectoryDecision) -> Result<MesiEvent, MesiHarnessError> {
    let state = decision
        .grant()
        .ok_or(MesiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?
        .state();
    match state {
        MesiState::Shared => Ok(MesiEvent::DataShared),
        MesiState::Exclusive => Ok(MesiEvent::DataExclusive),
        MesiState::Modified => Ok(MesiEvent::DataModified),
        state => Err(MesiHarnessError::UnexpectedGrantState { state }),
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
    context: &mut rem6_kernel::SchedulerContext<'_>,
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

fn mesi_response_record(
    tick: u64,
    cache_result: MesiCacheControllerResultKind,
    response: &MemoryResponse,
) -> MesiCpuResponseRecord {
    MesiCpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_mesi_cache_error(error: MesiCacheControllerError) -> MesiHarnessError {
    match error {
        MesiCacheControllerError::LineBusy { state } => MesiHarnessError::LineBusy { state },
        error => MesiHarnessError::Cache(error),
    }
}
