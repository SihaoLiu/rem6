use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_cache::{
    ChiCacheController, ChiCacheControllerError, ChiCacheControllerResultKind,
    ChiCacheControllerSnapshot,
};
use rem6_directory::{
    ChiDirectory, ChiDirectoryDataSource, ChiDirectoryDecision, ChiDirectoryError,
    ChiDirectoryGrant, ChiDirectoryLineState,
};
use rem6_dram::{DramMemoryController, DramMemoryError};
use rem6_memory::{
    AccessSize, Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId,
    MemoryResponse, ResponseStatus,
};
use rem6_protocol_chi::{ChiEvent, ChiLineId, ChiState};
use rem6_topology::{Endpoint, TopologyError};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTraceKind,
    TargetOutcome, TransportEndpointId, TransportError,
};

use rem6_fabric::FabricModel;
use rem6_kernel::{
    ConservativeRunSummary, PartitionId, PartitionedScheduler, SchedulerError, SchedulerSnapshot,
    Tick,
};

use crate::{
    HarnessError, LineBackingStore, PartitionedCacheAgentConfig, PartitionedDramMemoryConfig,
    PartitionedRouteHopConfig, SubmitKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChiHarnessError {
    LineBusy { state: ChiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    MissingTopologyConnection { from: Endpoint, to: Endpoint },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: ChiLineId },
    UnexpectedGrantState { state: ChiState },
    Cache(ChiCacheControllerError),
    Directory(ChiDirectoryError),
    Dram(DramMemoryError),
    Memory(MemoryError),
    Scheduler(SchedulerError),
    Backing(HarnessError),
    SnapshotResourceMismatch { resource: &'static str },
    Topology(TopologyError),
    Transport(TransportError),
}

impl fmt::Display for ChiHarnessError {
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
            Self::MissingTopologyConnection { from, to } => write!(
                formatter,
                "topology connection {}.{} to {}.{} is not declared",
                from.component().as_str(),
                from.port().as_str(),
                to.component().as_str(),
                to.port().as_str()
            ),
            Self::MissingDirectoryGrant { request } => write!(
                formatter,
                "directory did not grant request {} from agent {}",
                request.sequence(),
                request.agent().get()
            ),
            Self::GrantDataUnavailable { agent, line } => write!(
                formatter,
                "agent {} has no data for CHI line {:#x}",
                agent.get(),
                line.address().get()
            ),
            Self::UnexpectedGrantState { state } => {
                write!(formatter, "directory granted invalid CHI state {state:?}")
            }
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Dram(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::Backing(error) => write!(formatter, "{error}"),
            Self::SnapshotResourceMismatch { resource } => write!(
                formatter,
                "snapshot resource {resource} does not match harness configuration"
            ),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for ChiHarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Directory(error) => Some(error),
            Self::Dram(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Backing(error) => Some(error),
            Self::Topology(error) => Some(error),
            Self::Transport(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiSubmitResult {
    kind: SubmitKind,
    cache_result: ChiCacheControllerResultKind,
    directory_decision: Option<ChiDirectoryDecision>,
}

impl ChiSubmitResult {
    fn new(kind: SubmitKind, cache_result: ChiCacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
        }
    }

    fn with_directory_decision(mut self, decision: ChiDirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> ChiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&ChiDirectoryDecision> {
        self.directory_decision.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiCpuResponseRecord {
    tick: u64,
    cache_result: ChiCacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl ChiCpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: ChiCacheControllerResultKind,
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

    pub const fn cache_result(&self) -> ChiCacheControllerResultKind {
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

pub struct ChiDirectoryLineHarness {
    line: ChiLineId,
    directory: ChiDirectory,
    caches: BTreeMap<AgentId, ChiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<ChiCpuResponseRecord>,
    directory_decisions: Vec<ChiDirectoryDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiDirectoryDecisionRecord {
    tick: u64,
    requester: AgentId,
    decision: ChiDirectoryDecision,
}

impl ChiDirectoryDecisionRecord {
    pub const fn new(tick: u64, requester: AgentId, decision: ChiDirectoryDecision) -> Self {
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

    pub const fn decision(&self) -> &ChiDirectoryDecision {
        &self.decision
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChiDirectoryLineHarnessSnapshot {
    line: ChiLineId,
    directory: ChiDirectoryLineState,
    caches: BTreeMap<AgentId, ChiCacheControllerSnapshot>,
    backing: LineBackingStore,
    cpu_responses: Vec<ChiCpuResponseRecord>,
    directory_decisions: Vec<ChiDirectoryDecision>,
}

impl ChiDirectoryLineHarnessSnapshot {
    pub fn new(
        line: ChiLineId,
        directory: ChiDirectoryLineState,
        caches: BTreeMap<AgentId, ChiCacheControllerSnapshot>,
        backing: LineBackingStore,
        cpu_responses: Vec<ChiCpuResponseRecord>,
        directory_decisions: Vec<ChiDirectoryDecision>,
    ) -> Self {
        Self {
            line,
            directory,
            caches,
            backing,
            cpu_responses,
            directory_decisions,
        }
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn directory(&self) -> &ChiDirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, ChiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> &LineBackingStore {
        &self.backing
    }

    pub fn cpu_responses(&self) -> &[ChiCpuResponseRecord] {
        &self.cpu_responses
    }

    pub fn directory_decisions(&self) -> &[ChiDirectoryDecision] {
        &self.directory_decisions
    }
}

#[derive(Clone)]
struct PartitionedChiRoute {
    id: MemoryRouteId,
    route: MemoryRoute,
}

impl PartitionedChiRoute {
    fn new(id: MemoryRouteId, route: MemoryRoute) -> Self {
        Self { id, route }
    }
}

pub struct PartitionedChiDirectoryLineHarness {
    scheduler: PartitionedScheduler,
    transport: rem6_transport::MemoryTransport,
    line: ChiLineId,
    directory: Arc<Mutex<ChiDirectory>>,
    caches: BTreeMap<AgentId, Arc<Mutex<ChiCacheController>>>,
    routes: BTreeMap<AgentId, PartitionedChiRoute>,
    memory_route: Option<PartitionedChiRoute>,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<ChiCpuResponseRecord>>>,
    directory_decisions: Arc<Mutex<Vec<ChiDirectoryDecisionRecord>>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedChiDirectoryLineHarnessSnapshot {
    line: ChiLineId,
    scheduler: SchedulerSnapshot,
    directory: ChiDirectoryLineState,
    caches: BTreeMap<AgentId, ChiCacheControllerSnapshot>,
    backing: LineBackingStore,
    dram_memory: Option<DramMemoryController>,
    trace: Vec<MemoryTraceEvent>,
    cpu_responses: Vec<ChiCpuResponseRecord>,
    directory_decisions: Vec<ChiDirectoryDecisionRecord>,
}

impl PartitionedChiDirectoryLineHarnessSnapshot {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        line: ChiLineId,
        scheduler: SchedulerSnapshot,
        directory: ChiDirectoryLineState,
        caches: BTreeMap<AgentId, ChiCacheControllerSnapshot>,
        backing: LineBackingStore,
        dram_memory: Option<DramMemoryController>,
        trace: Vec<MemoryTraceEvent>,
        cpu_responses: Vec<ChiCpuResponseRecord>,
        directory_decisions: Vec<ChiDirectoryDecisionRecord>,
    ) -> Self {
        Self {
            line,
            scheduler,
            directory,
            caches,
            backing,
            dram_memory,
            trace,
            cpu_responses,
            directory_decisions,
        }
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub const fn scheduler(&self) -> &SchedulerSnapshot {
        &self.scheduler
    }

    pub const fn directory(&self) -> &ChiDirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, ChiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> &LineBackingStore {
        &self.backing
    }

    pub const fn dram_memory(&self) -> Option<&DramMemoryController> {
        self.dram_memory.as_ref()
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.clone()
    }

    pub fn cpu_responses(&self) -> Vec<ChiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> Vec<ChiDirectoryDecisionRecord> {
        self.directory_decisions.clone()
    }
}

impl ChiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        agents: I,
    ) -> Result<Self, ChiHarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(ChiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(agent, ChiCacheController::new(agent, layout, line_address))
                .is_some()
            {
                return Err(ChiHarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            line: ChiLineId::new(line_address),
            directory: ChiDirectory::new(),
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
    ) -> Result<ChiSubmitResult, ChiHarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(map_chi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
            return Ok(ChiSubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(ChiHarnessError::Cache(
                ChiCacheControllerError::NoPendingMiss,
            ))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(ChiHarnessError::Directory)?;
        let fill_event = chi_fill_event(&decision)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response, fill_event)
            .map_err(map_chi_cache_error)?;
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

        Ok(
            ChiSubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
                .with_directory_decision(decision),
        )
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<ChiState, ChiHarnessError> {
        Ok(self.cache(agent)?.state())
    }

    pub fn directory_state(&self) -> ChiDirectoryLineState {
        self.directory.line_state(self.line)
    }

    pub fn cpu_responses(&self) -> Vec<ChiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[ChiDirectoryDecision] {
        &self.directory_decisions
    }

    pub fn cache_data(&self, agent: AgentId) -> Result<Option<Vec<u8>>, ChiHarnessError> {
        Ok(self.cache(agent)?.cached_data().map(<[u8]>::to_vec))
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub fn snapshot(&self) -> ChiDirectoryLineHarnessSnapshot {
        ChiDirectoryLineHarnessSnapshot::new(
            self.line,
            self.directory.line_state(self.line),
            self.caches
                .iter()
                .map(|(agent, cache)| (*agent, cache.snapshot()))
                .collect(),
            self.backing.clone(),
            self.cpu_responses.clone(),
            self.directory_decisions.clone(),
        )
    }

    pub fn restore(
        &mut self,
        snapshot: &ChiDirectoryLineHarnessSnapshot,
    ) -> Result<(), ChiHarnessError> {
        self.validate_snapshot_identity(snapshot)?;
        let mut directory = self.directory.clone();
        directory
            .restore_line_state(snapshot.directory())
            .map_err(ChiHarnessError::Directory)?;
        let mut caches = self.caches.clone();
        for (agent, cache_snapshot) in snapshot.caches() {
            caches
                .get_mut(agent)
                .ok_or(ChiHarnessError::UnknownCache { agent: *agent })?
                .restore(cache_snapshot)
                .map_err(map_chi_cache_error)?;
        }

        self.directory = directory;
        self.caches = caches;
        self.backing = snapshot.backing.clone();
        self.cpu_responses = snapshot.cpu_responses.clone();
        self.directory_decisions = snapshot.directory_decisions.clone();
        Ok(())
    }

    fn directory_response(
        &mut self,
        request: &MemoryRequest,
        decision: &ChiDirectoryDecision,
    ) -> Result<MemoryResponse, ChiHarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(ChiHarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        if decision_downgrades_unique_owner(decision) {
            if let Some(data) = &source_data {
                self.backing
                    .replace_data(data.clone())
                    .map_err(ChiHarnessError::Backing)?;
            }
        }

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(snoop.event())
                .map_err(map_chi_cache_error)?;
        }

        match grant.data_source() {
            ChiDirectoryDataSource::BackingMemory => self
                .backing
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

    fn source_data(&self, grant: ChiDirectoryGrant) -> Result<Option<Vec<u8>>, ChiHarnessError> {
        match grant.data_source() {
            ChiDirectoryDataSource::BackingMemory | ChiDirectoryDataSource::NoData => Ok(None),
            ChiDirectoryDataSource::OwnerCache(agent) => {
                let data = self.cache(agent)?.cached_data().ok_or(
                    ChiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    },
                )?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn cache(&self, agent: AgentId) -> Result<&ChiCacheController, ChiHarnessError> {
        self.caches
            .get(&agent)
            .ok_or(ChiHarnessError::UnknownCache { agent })
    }

    fn cache_mut(&mut self, agent: AgentId) -> Result<&mut ChiCacheController, ChiHarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(ChiHarnessError::UnknownCache { agent })
    }

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: ChiCacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(chi_response_record(tick, cache_result, response));
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &ChiDirectoryLineHarnessSnapshot,
    ) -> Result<(), ChiHarnessError> {
        if self.line != snapshot.line() {
            return Err(ChiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            }));
        }
        for agent in self.caches.keys() {
            if !snapshot.caches().contains_key(agent) {
                return Err(ChiHarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.caches().keys() {
            if !self.caches.contains_key(agent) {
                return Err(ChiHarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

impl PartitionedChiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        agents: I,
    ) -> Result<Self, ChiHarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(ChiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(ChiHarnessError::Scheduler(SchedulerError::NoPartitions))?;
        let agent_configs: Vec<_> = agents.into_iter().collect();
        let uses_fabric = agent_configs
            .iter()
            .any(|config| chi_route_hops_use_fabric(config.route_hops()));
        let fabric = uses_fabric.then(|| Arc::new(Mutex::new(FabricModel::new())));
        let mut transport = if let Some(fabric) = &fabric {
            rem6_transport::MemoryTransport::with_shared_fabric(Arc::clone(fabric))
        } else {
            rem6_transport::MemoryTransport::new()
        };
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agent_configs {
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(ChiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_chi_hops(&mut partition_count, config.route_hops())?;
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(ChiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(ChiHarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }

            let route = chi_route_from_config(
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
                .map_err(ChiHarnessError::Transport)?;
            routes.insert(config.agent(), PartitionedChiRoute::new(route_id, route));
        }

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(ChiHarnessError::Scheduler)?,
            transport,
            line: ChiLineId::new(line_address),
            directory: Arc::new(Mutex::new(ChiDirectory::new())),
            caches,
            routes,
            memory_route: None,
            backing: Arc::new(Mutex::new(backing)),
            dram_memory: None,
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn new_with_dram_memory<I>(
        layout: CacheLineLayout,
        line_address: Address,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: PartitionedDramMemoryConfig,
        agents: I,
    ) -> Result<Self, ChiHarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(ChiHarnessError::Scheduler(SchedulerError::NoPartitions))?;
        partition_count = partition_count.max(
            memory
                .partition()
                .index()
                .checked_add(1)
                .ok_or(ChiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
        expand_partition_count_for_chi_hops(&mut partition_count, memory.route_hops())?;
        let agent_configs: Vec<_> = agents.into_iter().collect();
        let uses_fabric = chi_route_hops_use_fabric(memory.route_hops())
            || agent_configs
                .iter()
                .any(|config| chi_route_hops_use_fabric(config.route_hops()));
        let fabric = uses_fabric.then(|| Arc::new(Mutex::new(FabricModel::new())));
        let mut transport = if let Some(fabric) = &fabric {
            rem6_transport::MemoryTransport::with_shared_fabric(Arc::clone(fabric))
        } else {
            rem6_transport::MemoryTransport::new()
        };
        let memory_route = chi_route_from_config(
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
            .map_err(ChiHarnessError::Transport)?;
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agent_configs {
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(ChiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_chi_hops(&mut partition_count, config.route_hops())?;
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(ChiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(ChiHarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }

            let route = chi_route_from_config(
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
                .map_err(ChiHarnessError::Transport)?;
            routes.insert(config.agent(), PartitionedChiRoute::new(route_id, route));
        }

        let dram_controller = memory.into_controller();
        let backing = line_backing_from_chi_dram_memory(layout, line_address, &dram_controller)?;

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(ChiHarnessError::Scheduler)?,
            transport,
            line: ChiLineId::new(line_address),
            directory: Arc::new(Mutex::new(ChiDirectory::new())),
            caches,
            routes,
            memory_route: Some(PartitionedChiRoute::new(memory_route_id, memory_route)),
            backing: Arc::new(Mutex::new(backing)),
            dram_memory: Some(Arc::new(Mutex::new(dram_controller))),
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
        })
    }

    pub fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<ChiSubmitResult, ChiHarnessError> {
        let cache = self.cache_arc(agent)?;
        let result = cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
            .map_err(map_chi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.cpu_responses
                .lock()
                .expect("response lock")
                .push(chi_response_record(
                    self.scheduler.now(),
                    cache_result,
                    response,
                ));
            return Ok(ChiSubmitResult::new(SubmitKind::ImmediateHit, cache_result));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(ChiHarnessError::Cache(
                ChiCacheControllerError::NoPendingMiss,
            ))?;
        let requester_route = self
            .routes
            .get(&agent)
            .cloned()
            .ok_or(ChiHarnessError::UnknownCache { agent })?;
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
                    let fill_event = chi_fill_event(&decision).expect("directory grant fill event");
                    if let Some(memory_route) =
                        memory_route.filter(|_| decision_uses_chi_backing_memory(&decision))
                    {
                        decisions.lock().expect("decision lock").push(
                            ChiDirectoryDecisionRecord::new(
                                delivery.tick(),
                                delivery.request().id().agent(),
                                decision.clone(),
                            ),
                        );
                        let snoop_delay = schedule_partitioned_chi_snoops(
                            context,
                            &decision,
                            &cache_routes,
                            &caches,
                        )
                        .expect("scheduled directory snoops");
                        schedule_partitioned_chi_memory_response(
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
                            snoop_delay,
                        )
                        .expect("scheduled memory response");
                        return TargetOutcome::NoResponse;
                    }
                    let response = partitioned_chi_directory_response(
                        delivery.request(),
                        &decision,
                        &caches,
                        &backing,
                    )
                    .expect("directory response");
                    decisions
                        .lock()
                        .expect("decision lock")
                        .push(ChiDirectoryDecisionRecord::new(
                            delivery.tick(),
                            delivery.request().id().agent(),
                            decision.clone(),
                        ));

                    let snoop_delay =
                        schedule_partitioned_chi_snoops(context, &decision, &cache_routes, &caches)
                            .expect("scheduled directory snoops");
                    let route_delay = requester_route.route.response_latency();
                    let pre_response_delay = snoop_delay.saturating_sub(route_delay);
                    schedule_partitioned_chi_cache_response(
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
            .map_err(ChiHarnessError::Transport)?;

        Ok(ChiSubmitResult::new(
            SubmitKind::ScheduledMiss,
            cache_result,
        ))
    }

    pub fn run_until_idle_parallel(&mut self) -> Result<ConservativeRunSummary, ChiHarnessError> {
        self.scheduler
            .run_until_idle_parallel_recorded()
            .map(|run| run.summary())
            .map_err(ChiHarnessError::Scheduler)
    }

    pub fn now(&self) -> Tick {
        self.scheduler.now()
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<ChiState, ChiHarnessError> {
        Ok(self.cache_arc(agent)?.lock().expect("cache lock").state())
    }

    pub fn directory_state(&self) -> ChiDirectoryLineState {
        self.directory
            .lock()
            .expect("directory lock")
            .line_state(self.line)
    }

    pub fn route(&self, agent: AgentId) -> Result<MemoryRouteId, ChiHarnessError> {
        self.routes
            .get(&agent)
            .map(|route| route.id)
            .ok_or(ChiHarnessError::UnknownCache { agent })
    }

    pub fn memory_route(&self) -> Option<MemoryRouteId> {
        self.memory_route.as_ref().map(|route| route.id)
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.snapshot()
    }

    pub fn cpu_responses(&self) -> Vec<ChiCpuResponseRecord> {
        self.cpu_responses.lock().expect("response lock").clone()
    }

    pub fn directory_decisions(&self) -> Vec<ChiDirectoryDecisionRecord> {
        self.directory_decisions
            .lock()
            .expect("decision lock")
            .clone()
    }

    pub const fn line(&self) -> ChiLineId {
        self.line
    }

    pub fn quiescent_snapshot(
        &self,
    ) -> Result<PartitionedChiDirectoryLineHarnessSnapshot, ChiHarnessError> {
        let scheduler = self
            .scheduler
            .quiescent_snapshot()
            .map_err(ChiHarnessError::Scheduler)?;
        Ok(PartitionedChiDirectoryLineHarnessSnapshot::new(
            self.line,
            scheduler,
            self.directory
                .lock()
                .expect("directory lock")
                .line_state(self.line),
            self.caches
                .iter()
                .map(|(agent, cache)| (*agent, cache.lock().expect("cache lock").snapshot()))
                .collect(),
            self.backing.lock().expect("backing lock").clone(),
            self.dram_memory
                .as_ref()
                .map(|memory| memory.lock().expect("DRAM memory lock").clone()),
            self.trace.snapshot(),
            self.cpu_responses.lock().expect("response lock").clone(),
            self.directory_decisions
                .lock()
                .expect("decision lock")
                .clone(),
        ))
    }

    pub fn restore_quiescent(
        &mut self,
        snapshot: &PartitionedChiDirectoryLineHarnessSnapshot,
    ) -> Result<(), ChiHarnessError> {
        self.validate_quiescent_snapshot_identity(snapshot)?;
        self.scheduler
            .quiescent_snapshot()
            .map_err(ChiHarnessError::Scheduler)?;

        let mut directory = self.directory.lock().expect("directory lock").clone();
        directory
            .restore_line_state(snapshot.directory())
            .map_err(ChiHarnessError::Directory)?;

        let mut caches = BTreeMap::new();
        for (agent, current) in &self.caches {
            let cache_snapshot = snapshot
                .caches()
                .get(agent)
                .ok_or(ChiHarnessError::UnknownCache { agent: *agent })?;
            let mut cache = current.lock().expect("cache lock").clone();
            cache.restore(cache_snapshot).map_err(map_chi_cache_error)?;
            caches.insert(*agent, Arc::new(Mutex::new(cache)));
        }

        self.scheduler
            .restore_quiescent(snapshot.scheduler())
            .map_err(ChiHarnessError::Scheduler)?;
        self.directory = Arc::new(Mutex::new(directory));
        self.caches = caches;
        self.backing = Arc::new(Mutex::new(snapshot.backing.clone()));
        self.dram_memory = snapshot
            .dram_memory
            .clone()
            .map(|memory| Arc::new(Mutex::new(memory)));
        self.trace = MemoryTrace::from_events(snapshot.trace.clone());
        *self.cpu_responses.lock().expect("response lock") = snapshot.cpu_responses.clone();
        *self.directory_decisions.lock().expect("decision lock") =
            snapshot.directory_decisions.clone();
        Ok(())
    }

    fn validate_quiescent_snapshot_identity(
        &self,
        snapshot: &PartitionedChiDirectoryLineHarnessSnapshot,
    ) -> Result<(), ChiHarnessError> {
        if self.line != snapshot.line() {
            return Err(ChiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            }));
        }
        for agent in self.caches.keys() {
            if !snapshot.caches().contains_key(agent) {
                return Err(ChiHarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.caches().keys() {
            if !self.caches.contains_key(agent) {
                return Err(ChiHarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }

    fn cache_arc(&self, agent: AgentId) -> Result<Arc<Mutex<ChiCacheController>>, ChiHarnessError> {
        self.caches
            .get(&agent)
            .cloned()
            .ok_or(ChiHarnessError::UnknownCache { agent })
    }
}

fn chi_fill_event(decision: &ChiDirectoryDecision) -> Result<ChiEvent, ChiHarnessError> {
    let state = decision
        .grant()
        .ok_or(ChiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?
        .state();
    match state {
        ChiState::SharedClean => Ok(ChiEvent::CompDataSharedClean),
        ChiState::SharedDirty => Ok(ChiEvent::CompDataSharedDirty),
        ChiState::UniqueClean => Ok(ChiEvent::CompDataUniqueClean),
        ChiState::UniqueDirty => Ok(ChiEvent::CompDataUniqueDirty),
        state => Err(ChiHarnessError::UnexpectedGrantState { state }),
    }
}

fn decision_downgrades_unique_owner(decision: &ChiDirectoryDecision) -> bool {
    decision.before().unique_owner().is_some()
        && decision
            .snoops()
            .iter()
            .any(|snoop| snoop.event() == ChiEvent::SnoopShared)
}

fn chi_response_record(
    tick: u64,
    cache_result: ChiCacheControllerResultKind,
    response: &MemoryResponse,
) -> ChiCpuResponseRecord {
    ChiCpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_chi_cache_error(error: ChiCacheControllerError) -> ChiHarnessError {
    match error {
        ChiCacheControllerError::LineBusy { state } => ChiHarnessError::LineBusy { state },
        error => ChiHarnessError::Cache(error),
    }
}

fn expand_partition_count_for_chi_hops(
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

fn chi_route_hops_use_fabric(hops: &[PartitionedRouteHopConfig]) -> bool {
    hops.iter()
        .any(|hop| hop.request_fabric_path().is_some() || hop.response_fabric_path().is_some())
}

#[allow(clippy::too_many_arguments)]
fn chi_route_from_config(
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

fn line_backing_from_chi_dram_memory(
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

fn partitioned_chi_directory_response(
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

fn decision_uses_chi_backing_memory(decision: &ChiDirectoryDecision) -> bool {
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

fn schedule_partitioned_chi_snoops(
    context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
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
fn schedule_partitioned_chi_memory_response(
    context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
    request: MemoryRequest,
    fill_event: ChiEvent,
    requester_route: PartitionedChiRoute,
    memory_route: PartitionedChiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
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
    schedule_partitioned_chi_memory_request_hop(
        context,
        0,
        request,
        fill_event,
        requester_route,
        memory_route,
        backing,
        dram_memory,
        trace,
        response_cache,
        responses,
        snoop_delay,
        context.now(),
    )
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_chi_memory_request_hop(
    context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
    hop_index: usize,
    request: MemoryRequest,
    fill_event: ChiEvent,
    requester_route: PartitionedChiRoute,
    memory_route: PartitionedChiRoute,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
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
    context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
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
) -> Result<(u64, MemoryResponse), ChiHarnessError> {
    let Some(dram_memory) = dram_memory else {
        let response = backing
            .lock()
            .expect("backing lock")
            .respond(request)
            .map_err(ChiHarnessError::Backing)?;
        return Ok((now, response));
    };

    let outcome = dram_memory
        .lock()
        .expect("DRAM memory lock")
        .accept(now, request)
        .map_err(ChiHarnessError::Dram)?;
    let response = outcome.response().cloned().ok_or(ChiHarnessError::Memory(
        MemoryError::MissingResponseData {
            request: request.id(),
        },
    ))?;

    Ok((outcome.ready_cycle(), response))
}

#[allow(clippy::too_many_arguments)]
fn schedule_partitioned_chi_cache_response(
    context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
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
    context: &mut rem6_kernel::ParallelSchedulerContext<'_>,
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
