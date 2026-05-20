use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_cache::{CacheControllerError, CacheControllerResultKind, MsiCacheController};
use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryError, DirectoryGrant, DirectoryLineState,
    MsiDirectory,
};
use rem6_kernel::{ConservativeRunSummary, PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse, ResponseStatus,
};
use rem6_protocol_msi::{MsiLineId, MsiState};
use rem6_transport::{
    MemoryRoute, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind, MemoryTransport,
    TargetOutcome, TransportEndpointId, TransportError,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SubmitKind {
    ImmediateHit,
    ScheduledMiss,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SubmitResult {
    kind: SubmitKind,
    cache_result: CacheControllerResultKind,
    directory_decision: Option<DirectoryDecision>,
}

impl SubmitResult {
    fn new(kind: SubmitKind, cache_result: CacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
        }
    }

    fn with_directory_decision(mut self, decision: DirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> CacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&DirectoryDecision> {
        self.directory_decision.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuResponseRecord {
    tick: u64,
    cache_result: CacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl CpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: CacheControllerResultKind,
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

    pub const fn cache_result(&self) -> CacheControllerResultKind {
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HarnessError {
    LineBusy { state: MsiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    WrongLine { expected: Address, actual: Address },
    LineDataSizeMismatch { expected: u64, actual: u64 },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: MsiLineId },
    Cache(CacheControllerError),
    Directory(DirectoryError),
    Memory(MemoryError),
    Scheduler(SchedulerError),
    Transport(TransportError),
}

impl fmt::Display for HarnessError {
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
            Self::WrongLine { expected, actual } => write!(
                formatter,
                "request for line {:#x} reached backing line {:#x}",
                actual.get(),
                expected.get()
            ),
            Self::LineDataSizeMismatch { expected, actual } => write!(
                formatter,
                "line data has {actual} bytes but line expects {expected}"
            ),
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
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for HarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Directory(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LineBackingStore {
    layout: CacheLineLayout,
    line_address: Address,
    data: Vec<u8>,
}

impl LineBackingStore {
    pub fn new(
        layout: CacheLineLayout,
        line_address: Address,
        data: Vec<u8>,
    ) -> Result<Self, HarnessError> {
        let line_address = layout.line_address(line_address);
        if data.len() as u64 != layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: layout.bytes(),
                actual: data.len() as u64,
            });
        }

        Ok(Self {
            layout,
            line_address,
            data,
        })
    }

    pub fn line_address(&self) -> Address {
        self.line_address
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }

    pub fn respond(&mut self, request: &MemoryRequest) -> Result<MemoryResponse, HarnessError> {
        self.check_line(request)?;
        match request.operation() {
            MemoryOperation::ReadShared | MemoryOperation::ReadUnique => {
                MemoryResponse::completed(request, Some(self.data.clone()))
                    .map_err(HarnessError::Memory)
            }
            MemoryOperation::Upgrade => {
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::Write | MemoryOperation::Atomic => {
                self.apply_write(request)?;
                MemoryResponse::completed(request, None).map_err(HarnessError::Memory)
            }
            MemoryOperation::WritebackClean | MemoryOperation::WritebackDirty => {
                self.replace_line(request)?;
                Ok(MemoryResponse::retry(request))
            }
            _ => MemoryResponse::completed(request, None).map_err(HarnessError::Memory),
        }
    }

    fn check_line(&self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let actual = request.line_address();
        if actual != self.line_address {
            return Err(HarnessError::WrongLine {
                expected: self.line_address,
                actual,
            });
        }

        Ok(())
    }

    fn apply_write(&mut self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let offset = request.line_offset() as usize;
        let payload =
            request
                .data()
                .ok_or(HarnessError::Memory(MemoryError::MissingRequestData {
                    request: request.id(),
                }))?;
        let mask = request.byte_mask();
        for (index, byte) in payload.iter().enumerate() {
            if mask.is_none_or(|mask| mask.bits()[index]) {
                self.data[offset + index] = *byte;
            }
        }

        Ok(())
    }

    fn replace_line(&mut self, request: &MemoryRequest) -> Result<(), HarnessError> {
        let data = request
            .data()
            .ok_or(HarnessError::Memory(MemoryError::MissingRequestData {
                request: request.id(),
            }))?;
        if data.len() as u64 != self.layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.data = data.to_vec();
        Ok(())
    }
}

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

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
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
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

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

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: CacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(response_record(tick, cache_result, response));
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedCacheAgentConfig {
    agent: AgentId,
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
}

impl PartitionedCacheAgentConfig {
    pub fn new(
        agent: AgentId,
        partition: PartitionId,
        endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
    ) -> Self {
        Self {
            agent,
            partition,
            endpoint,
            request_latency,
            response_latency,
        }
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn request_latency(&self) -> u64 {
        self.request_latency
    }

    pub const fn response_latency(&self) -> u64 {
        self.response_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMemoryConfig {
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
}

impl PartitionedMemoryConfig {
    pub fn new(
        partition: PartitionId,
        endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
    ) -> Self {
        Self {
            partition,
            endpoint,
            request_latency,
            response_latency,
        }
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub const fn request_latency(&self) -> u64 {
        self.request_latency
    }

    pub const fn response_latency(&self) -> u64 {
        self.response_latency
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectoryDecisionRecord {
    tick: u64,
    requester: AgentId,
    decision: DirectoryDecision,
}

impl DirectoryDecisionRecord {
    pub const fn new(tick: u64, requester: AgentId, decision: DirectoryDecision) -> Self {
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

    pub const fn decision(&self) -> &DirectoryDecision {
        &self.decision
    }
}

pub struct PartitionedDirectoryLineHarness {
    scheduler: PartitionedScheduler,
    transport: MemoryTransport,
    line: MsiLineId,
    directory: Arc<Mutex<MsiDirectory>>,
    caches: BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    routes: BTreeMap<AgentId, MemoryRouteId>,
    backing: Arc<Mutex<LineBackingStore>>,
    memory_route: Option<MemoryRouteId>,
    memory_route_info: Option<MemoryRoute>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
    directory_decisions: Arc<Mutex<Vec<DirectoryDecisionRecord>>>,
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
            backing,
            directory_partition,
            directory_endpoint,
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
            backing,
            directory_partition,
            directory_endpoint,
            Some(memory),
            agents,
        )
    }

    fn new_internal<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: Option<PartitionedMemoryConfig>,
        agents: I,
    ) -> Result<Self, HarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            });
        }

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?;
        let mut transport = MemoryTransport::new();
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();
        let mut memory_route = None;
        let mut memory_route_info = None;

        if let Some(memory) = memory {
            partition_count = partition_count.max(
                memory
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            let route = MemoryRoute::new(
                directory_endpoint.clone(),
                directory_partition,
                memory.endpoint().clone(),
                memory.partition(),
                memory.request_latency(),
                memory.response_latency(),
            )
            .map_err(HarnessError::Transport)?;
            memory_route = Some(
                transport
                    .add_route(route.clone())
                    .map_err(HarnessError::Transport)?,
            );
            memory_route_info = Some(route);
        }

        for config in agents {
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(HarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
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
                    MemoryRoute::new(
                        config.endpoint().clone(),
                        config.partition(),
                        directory_endpoint.clone(),
                        directory_partition,
                        config.request_latency(),
                        config.response_latency(),
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
            backing: Arc::new(Mutex::new(backing)),
            memory_route,
            memory_route_info,
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
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

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.cpu_responses
                .lock()
                .expect("response lock")
                .push(response_record(
                    self.scheduler.now(),
                    cache_result,
                    response,
                ));
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
        let directory = Arc::clone(&self.directory);
        let caches = self.caches.clone();
        let backing = Arc::clone(&self.backing);
        let decisions = Arc::clone(&self.directory_decisions);
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let memory_path = self.memory_route.zip(self.memory_route_info.clone()).map(
            |(memory_route, memory_route_info)| DeferredMemoryPath {
                cache_route_id: route,
                cache_route,
                memory_route_id: memory_route,
                memory_route: memory_route_info,
            },
        );
        let deferred = memory_path.map(|path| DeferredMemoryWork {
            path,
            caches: caches.clone(),
            backing: Arc::clone(&backing),
            trace: self.trace.clone(),
            response_cache: Arc::clone(&response_cache),
            responses: Arc::clone(&responses),
            decisions: Arc::clone(&decisions),
        });
        self.transport
            .submit(
                &mut self.scheduler,
                route,
                downstream,
                self.trace.clone(),
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
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        responses
                            .lock()
                            .expect("response lock")
                            .push(response_record(delivery.tick(), result.kind(), response));
                    }
                },
            )
            .map_err(HarnessError::Transport)?;

        Ok(SubmitResult::new(SubmitKind::ScheduledMiss, cache_result))
    }

    pub fn run_until_idle(&mut self) -> ConservativeRunSummary {
        self.scheduler.run_until_idle_conservative()
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

#[derive(Clone)]
struct DeferredMemoryPath {
    cache_route_id: MemoryRouteId,
    cache_route: MemoryRoute,
    memory_route_id: MemoryRouteId,
    memory_route: MemoryRoute,
}

struct DeferredMemoryWork {
    path: DeferredMemoryPath,
    caches: BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    backing: Arc<Mutex<LineBackingStore>>,
    trace: MemoryTrace,
    response_cache: Arc<Mutex<MsiCacheController>>,
    responses: Arc<Mutex<Vec<CpuResponseRecord>>>,
    decisions: Arc<Mutex<Vec<DirectoryDecisionRecord>>>,
}

impl DeferredMemoryWork {
    fn schedule(
        self,
        context: &mut rem6_kernel::SchedulerContext<'_>,
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

        let memory_route = self.path.memory_route.clone();
        let memory_route_id = self.path.memory_route_id;
        let cache_route = self.path.cache_route.clone();
        let cache_route_id = self.path.cache_route_id;
        let trace = self.trace.clone();
        let backing = Arc::clone(&self.backing);
        let response_cache = Arc::clone(&self.response_cache);
        let responses = Arc::clone(&self.responses);
        context
            .schedule_remote_after(
                memory_route.target_partition(),
                memory_route.request_latency(),
                move |context| {
                    trace.record(MemoryTraceEvent::request(
                        context.now(),
                        memory_route_id,
                        memory_route.target().clone(),
                        MemoryTraceKind::RequestArrived,
                        request.id(),
                    ));
                    let response = backing
                        .lock()
                        .expect("backing lock")
                        .respond(&request)
                        .expect("backing store response");
                    let response_status = response.status();
                    let response_request = response.request_id();
                    let trace_to_directory = trace.clone();
                    context
                        .schedule_remote_after(
                            memory_route.source_partition(),
                            memory_route.response_latency(),
                            move |context| {
                                trace_to_directory.record(MemoryTraceEvent::response(
                                    context.now(),
                                    memory_route_id,
                                    memory_route.source().clone(),
                                    response_request,
                                    response_status,
                                ));
                                let trace_to_cache = trace_to_directory.clone();
                                let response_status = response.status();
                                let response_request = response.request_id();
                                context
                                    .schedule_remote_after(
                                        cache_route.source_partition(),
                                        cache_route.response_latency(),
                                        move |context| {
                                            trace_to_cache.record(MemoryTraceEvent::response(
                                                context.now(),
                                                cache_route_id,
                                                cache_route.source().clone(),
                                                response_request,
                                                response_status,
                                            ));
                                            let result = response_cache
                                                .lock()
                                                .expect("cache lock")
                                                .accept_fill(response)
                                                .expect("cache fill");
                                            if let Some(TargetOutcome::Respond(response)) =
                                                result.target_outcome()
                                            {
                                                responses.lock().expect("response lock").push(
                                                    response_record(
                                                        context.now(),
                                                        result.kind(),
                                                        response,
                                                    ),
                                                );
                                            }
                                        },
                                    )
                                    .expect("validated cache response latency");
                            },
                        )
                        .expect("validated memory response latency");
                },
            )
            .expect("validated memory request latency");

        Ok(())
    }
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

fn partitioned_directory_response(
    request: &MemoryRequest,
    decision: &DirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MsiCacheController>>>,
    backing: &Arc<Mutex<LineBackingStore>>,
) -> Result<MemoryResponse, HarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(HarnessError::MissingDirectoryGrant {
            request: request.id(),
        })?;
    let source_data = match grant.data_source() {
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
    };

    apply_directory_snoops(decision, caches)?;

    match grant.data_source() {
        DirectoryDataSource::BackingMemory => {
            backing.lock().expect("backing lock").respond(request)
        }
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

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(self.scheduler.now(), cache_result, response);
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
                    if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
                        responses
                            .lock()
                            .expect("response lock")
                            .push(response_record(delivery.tick(), result.kind(), response));
                    }
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

    fn record_cpu_response(
        &self,
        tick: u64,
        cache_result: CacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .lock()
            .expect("response lock")
            .push(response_record(tick, cache_result, response));
    }
}

fn response_record(
    tick: u64,
    cache_result: CacheControllerResultKind,
    response: &MemoryResponse,
) -> CpuResponseRecord {
    CpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_cache_error(error: CacheControllerError) -> HarnessError {
    match error {
        CacheControllerError::LineBusy { state } => HarnessError::LineBusy { state },
        error => HarnessError::Cache(error),
    }
}
