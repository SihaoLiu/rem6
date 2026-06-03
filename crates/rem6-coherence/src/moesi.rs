use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_cache::{
    MoesiCacheController, MoesiCacheControllerError, MoesiCacheControllerResultKind,
    MoesiCacheControllerSnapshot,
};
use rem6_directory::{
    MoesiDirectory, MoesiDirectoryDataSource, MoesiDirectoryDecision, MoesiDirectoryError,
    MoesiDirectoryGrant, MoesiDirectoryLineState,
};
use rem6_dram::{DramMemoryController, DramMemoryError};
use rem6_fabric::{FabricError, FabricModel};
use rem6_kernel::{
    ConservativeRunSummary, DeadlockDiagnostic, ParallelSchedulerContext, PartitionId,
    PartitionedScheduler, SchedulerError, Tick, WaitForGraph,
};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryRequest, MemoryRequestId, MemoryResponse,
    ResponseStatus,
};
use rem6_protocol_moesi::{MoesiEvent, MoesiLineId, MoesiState};
use rem6_topology::{Endpoint, TopologyError};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, TargetOutcome,
    TransportEndpointId, TransportError,
};

use crate::summary::CoherenceResourceActivityWindow;
use crate::wait_for::CoherenceWaitFor;
use crate::{
    DramMemoryAccessRecord, HarnessError, LineBackingStore, ParallelCoherenceRunHistory,
    ParallelCoherenceRunSummary, ParallelCoherenceWaitForGraphs, PartitionedCacheAgentConfig,
    PartitionedDramMemoryConfig, PartitionedDramQosState, PartitionedRouteHopConfig, SubmitKind,
};

mod response;
mod snapshot;

use response::{
    moesi_route_response_delay, schedule_partitioned_moesi_cache_response_parallel,
    schedule_partitioned_moesi_memory_response_parallel,
};

pub use snapshot::PartitionedMoesiDirectoryLineHarnessSnapshot;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MoesiHarnessError {
    LineBusy { state: MoesiState },
    UnknownCache { agent: AgentId },
    DuplicateCache { agent: AgentId },
    MissingTopologyConnection { from: Endpoint, to: Endpoint },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: MoesiLineId },
    UnexpectedGrantState { state: MoesiState },
    Cache(MoesiCacheControllerError),
    Directory(MoesiDirectoryError),
    Memory(MemoryError),
    Dram(DramMemoryError),
    Fabric(FabricError),
    Scheduler(SchedulerError),
    SnapshotResourceMismatch { resource: &'static str },
    Topology(TopologyError),
    Transport(TransportError),
    Backing(HarnessError),
}

impl fmt::Display for MoesiHarnessError {
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
                "agent {} has no data for line {:#x}",
                agent.get(),
                line.address().get()
            ),
            Self::UnexpectedGrantState { state } => {
                write!(formatter, "directory granted transient state {state:?}")
            }
            Self::Cache(error) => write!(formatter, "{error}"),
            Self::Directory(error) => write!(formatter, "{error}"),
            Self::Memory(error) => write!(formatter, "{error}"),
            Self::Dram(error) => write!(formatter, "{error}"),
            Self::Fabric(error) => write!(formatter, "{error}"),
            Self::Scheduler(error) => write!(formatter, "{error}"),
            Self::SnapshotResourceMismatch { resource } => write!(
                formatter,
                "snapshot resource {resource} does not match harness configuration"
            ),
            Self::Topology(error) => write!(formatter, "{error}"),
            Self::Transport(error) => write!(formatter, "{error}"),
            Self::Backing(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for MoesiHarnessError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Cache(error) => Some(error),
            Self::Directory(error) => Some(error),
            Self::Memory(error) => Some(error),
            Self::Dram(error) => Some(error),
            Self::Fabric(error) => Some(error),
            Self::Scheduler(error) => Some(error),
            Self::Topology(error) => Some(error),
            Self::Transport(error) => Some(error),
            Self::Backing(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiSubmitResult {
    kind: SubmitKind,
    cache_result: MoesiCacheControllerResultKind,
    directory_decision: Option<MoesiDirectoryDecision>,
}

impl MoesiSubmitResult {
    fn new(kind: SubmitKind, cache_result: MoesiCacheControllerResultKind) -> Self {
        Self {
            kind,
            cache_result,
            directory_decision: None,
        }
    }

    fn with_directory_decision(mut self, decision: MoesiDirectoryDecision) -> Self {
        self.directory_decision = Some(decision);
        self
    }

    pub const fn kind(&self) -> SubmitKind {
        self.kind
    }

    pub const fn cache_result(&self) -> MoesiCacheControllerResultKind {
        self.cache_result
    }

    pub const fn directory_decision(&self) -> Option<&MoesiDirectoryDecision> {
        self.directory_decision.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiCpuResponseRecord {
    tick: u64,
    cache_result: MoesiCacheControllerResultKind,
    request: MemoryRequestId,
    status: ResponseStatus,
    data: Option<Vec<u8>>,
}

impl MoesiCpuResponseRecord {
    pub fn new(
        tick: u64,
        cache_result: MoesiCacheControllerResultKind,
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

    pub const fn cache_result(&self) -> MoesiCacheControllerResultKind {
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
pub struct MoesiDirectoryDecisionRecord {
    tick: u64,
    requester: AgentId,
    decision: MoesiDirectoryDecision,
}

impl MoesiDirectoryDecisionRecord {
    pub const fn new(tick: u64, requester: AgentId, decision: MoesiDirectoryDecision) -> Self {
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

    pub const fn decision(&self) -> &MoesiDirectoryDecision {
        &self.decision
    }
}

pub struct MoesiDirectoryLineHarness {
    line: MoesiLineId,
    directory: MoesiDirectory,
    caches: BTreeMap<AgentId, MoesiCacheController>,
    backing: LineBackingStore,
    cpu_responses: Vec<MoesiCpuResponseRecord>,
    directory_decisions: Vec<MoesiDirectoryDecision>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MoesiDirectoryLineHarnessSnapshot {
    line: MoesiLineId,
    directory: MoesiDirectoryLineState,
    caches: BTreeMap<AgentId, MoesiCacheControllerSnapshot>,
    backing: LineBackingStore,
    cpu_responses: Vec<MoesiCpuResponseRecord>,
    directory_decisions: Vec<MoesiDirectoryDecision>,
}

impl MoesiDirectoryLineHarnessSnapshot {
    pub fn new(
        line: MoesiLineId,
        directory: MoesiDirectoryLineState,
        caches: BTreeMap<AgentId, MoesiCacheControllerSnapshot>,
        backing: LineBackingStore,
        cpu_responses: Vec<MoesiCpuResponseRecord>,
        directory_decisions: Vec<MoesiDirectoryDecision>,
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

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    pub const fn directory(&self) -> &MoesiDirectoryLineState {
        &self.directory
    }

    pub fn caches(&self) -> &BTreeMap<AgentId, MoesiCacheControllerSnapshot> {
        &self.caches
    }

    pub const fn backing(&self) -> &LineBackingStore {
        &self.backing
    }

    pub fn cpu_responses(&self) -> &[MoesiCpuResponseRecord] {
        &self.cpu_responses
    }

    pub fn directory_decisions(&self) -> &[MoesiDirectoryDecision] {
        &self.directory_decisions
    }
}

#[derive(Clone)]
struct PartitionedMoesiRoute {
    id: MemoryRouteId,
    route: MemoryRoute,
}

impl PartitionedMoesiRoute {
    fn new(id: MemoryRouteId, route: MemoryRoute) -> Self {
        Self { id, route }
    }
}

pub struct PartitionedMoesiDirectoryLineHarness {
    scheduler: PartitionedScheduler,
    transport: rem6_transport::MemoryTransport,
    line: MoesiLineId,
    directory: Arc<Mutex<MoesiDirectory>>,
    caches: BTreeMap<AgentId, Arc<Mutex<MoesiCacheController>>>,
    routes: BTreeMap<AgentId, PartitionedMoesiRoute>,
    memory_route: Option<PartitionedMoesiRoute>,
    backing: Arc<Mutex<LineBackingStore>>,
    dram_memory: Option<Arc<Mutex<DramMemoryController>>>,
    dram_qos: Option<Arc<Mutex<PartitionedDramQosState>>>,
    fabric: Option<Arc<Mutex<FabricModel>>>,
    trace: MemoryTrace,
    cpu_responses: Arc<Mutex<Vec<MoesiCpuResponseRecord>>>,
    directory_decisions: Arc<Mutex<Vec<MoesiDirectoryDecisionRecord>>>,
    dram_accesses: Arc<Mutex<Vec<DramMemoryAccessRecord>>>,
    parallel_runs: Vec<ParallelCoherenceRunSummary>,
    wait_for: CoherenceWaitFor,
}

impl MoesiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        agents: I,
    ) -> Result<Self, MoesiHarnessError>
    where
        I: IntoIterator<Item = AgentId>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut caches = BTreeMap::new();
        for agent in agents {
            if caches
                .insert(
                    agent,
                    MoesiCacheController::new(agent, layout, line_address),
                )
                .is_some()
            {
                return Err(MoesiHarnessError::DuplicateCache { agent });
            }
        }

        Ok(Self {
            line: MoesiLineId::new(line_address),
            directory: MoesiDirectory::new(),
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
    ) -> Result<MoesiSubmitResult, MoesiHarnessError> {
        let result = self
            .cache_mut(agent)?
            .accept_cpu_request(request)
            .map_err(map_moesi_cache_error)?;
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.record_cpu_response(0, cache_result, response);
            return Ok(MoesiSubmitResult::new(
                SubmitKind::ImmediateHit,
                cache_result,
            ));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(MoesiHarnessError::Cache(
                MoesiCacheControllerError::NoPendingMiss,
            ))?;
        let decision = self
            .directory
            .accept(downstream.clone())
            .map_err(MoesiHarnessError::Directory)?;
        let fill_event = moesi_fill_event(&decision)?;
        let response = self.directory_response(&downstream, &decision)?;
        self.directory_decisions.push(decision.clone());
        let fill = self
            .cache_mut(agent)?
            .accept_fill(response, fill_event)
            .map_err(map_moesi_cache_error)?;
        if let Some(TargetOutcome::Respond(response)) = fill.target_outcome() {
            self.record_cpu_response(0, fill.kind(), response);
        }

        Ok(
            MoesiSubmitResult::new(SubmitKind::ScheduledMiss, cache_result)
                .with_directory_decision(decision),
        )
    }

    pub fn cache_state(&self, agent: AgentId) -> Result<MoesiState, MoesiHarnessError> {
        Ok(self.cache(agent)?.state())
    }

    pub fn directory_state(&self) -> MoesiDirectoryLineState {
        self.directory.line_state(self.line)
    }

    pub fn cpu_responses(&self) -> Vec<MoesiCpuResponseRecord> {
        self.cpu_responses.clone()
    }

    pub fn directory_decisions(&self) -> &[MoesiDirectoryDecision] {
        &self.directory_decisions
    }

    pub fn cache_data(&self, agent: AgentId) -> Result<Option<Vec<u8>>, MoesiHarnessError> {
        Ok(self.cache(agent)?.cached_data().map(<[u8]>::to_vec))
    }

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    pub fn snapshot(&self) -> MoesiDirectoryLineHarnessSnapshot {
        MoesiDirectoryLineHarnessSnapshot::new(
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
        snapshot: &MoesiDirectoryLineHarnessSnapshot,
    ) -> Result<(), MoesiHarnessError> {
        self.validate_snapshot_identity(snapshot)?;
        let mut directory = self.directory.clone();
        directory
            .restore_line_state(snapshot.directory())
            .map_err(MoesiHarnessError::Directory)?;
        let mut caches = self.caches.clone();
        for (agent, cache_snapshot) in snapshot.caches() {
            caches
                .get_mut(agent)
                .ok_or(MoesiHarnessError::UnknownCache { agent: *agent })?
                .restore(cache_snapshot)
                .map_err(map_moesi_cache_error)?;
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
        decision: &MoesiDirectoryDecision,
    ) -> Result<MemoryResponse, MoesiHarnessError> {
        let grant = decision
            .grant()
            .copied()
            .ok_or(MoesiHarnessError::MissingDirectoryGrant {
                request: request.id(),
            })?;
        let source_data = self.source_data(grant)?;

        for snoop in decision.snoops() {
            self.cache_mut(snoop.target())?
                .accept_snoop(snoop.event())
                .map_err(map_moesi_cache_error)?;
        }

        match grant.data_source() {
            MoesiDirectoryDataSource::BackingMemory => self
                .backing
                .respond(request)
                .map_err(MoesiHarnessError::Backing),
            MoesiDirectoryDataSource::OwnerCache(_) if request.returns_data() => {
                MemoryResponse::completed(request, source_data).map_err(MoesiHarnessError::Memory)
            }
            MoesiDirectoryDataSource::OwnerCache(_) => {
                MemoryResponse::completed(request, None).map_err(MoesiHarnessError::Memory)
            }
            MoesiDirectoryDataSource::NoData => {
                MemoryResponse::completed(request, None).map_err(MoesiHarnessError::Memory)
            }
        }
    }

    fn source_data(
        &self,
        grant: MoesiDirectoryGrant,
    ) -> Result<Option<Vec<u8>>, MoesiHarnessError> {
        match grant.data_source() {
            MoesiDirectoryDataSource::BackingMemory | MoesiDirectoryDataSource::NoData => Ok(None),
            MoesiDirectoryDataSource::OwnerCache(agent) => {
                let data = self.cache(agent)?.cached_data().ok_or(
                    MoesiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    },
                )?;
                Ok(Some(data.to_vec()))
            }
        }
    }

    fn cache(&self, agent: AgentId) -> Result<&MoesiCacheController, MoesiHarnessError> {
        self.caches
            .get(&agent)
            .ok_or(MoesiHarnessError::UnknownCache { agent })
    }

    fn cache_mut(
        &mut self,
        agent: AgentId,
    ) -> Result<&mut MoesiCacheController, MoesiHarnessError> {
        self.caches
            .get_mut(&agent)
            .ok_or(MoesiHarnessError::UnknownCache { agent })
    }

    fn record_cpu_response(
        &mut self,
        tick: u64,
        cache_result: MoesiCacheControllerResultKind,
        response: &MemoryResponse,
    ) {
        self.cpu_responses
            .push(moesi_response_record(tick, cache_result, response));
    }

    fn validate_snapshot_identity(
        &self,
        snapshot: &MoesiDirectoryLineHarnessSnapshot,
    ) -> Result<(), MoesiHarnessError> {
        if self.line != snapshot.line() {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.line().address(),
            }));
        }
        if snapshot.backing().line_address() != self.line.address() {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.backing().line_address(),
            }));
        }
        if snapshot.directory().line() != self.line {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: self.line.address(),
                actual: snapshot.directory().line().address(),
            }));
        }
        for agent in self.caches.keys() {
            if !snapshot.caches().contains_key(agent) {
                return Err(MoesiHarnessError::UnknownCache { agent: *agent });
            }
        }
        for agent in snapshot.caches().keys() {
            if !self.caches.contains_key(agent) {
                return Err(MoesiHarnessError::UnknownCache { agent: *agent });
            }
        }

        Ok(())
    }
}

impl PartitionedMoesiDirectoryLineHarness {
    pub fn new<I>(
        layout: CacheLineLayout,
        line_address: Address,
        backing: LineBackingStore,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        agents: I,
    ) -> Result<Self, MoesiHarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);
        if backing.line_address() != line_address {
            return Err(MoesiHarnessError::Backing(HarnessError::WrongLine {
                expected: line_address,
                actual: backing.line_address(),
            }));
        }

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(MoesiHarnessError::Scheduler(SchedulerError::NoPartitions))?;
        let agent_configs: Vec<_> = agents.into_iter().collect();
        let uses_fabric = agent_configs
            .iter()
            .any(|config| moesi_route_hops_use_fabric(config.route_hops()));
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
                    .ok_or(MoesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_moesi_hops(&mut partition_count, config.route_hops())?;
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(MoesiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(MoesiHarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }

            let route = moesi_route_from_config(
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
                .map_err(MoesiHarnessError::Transport)?;
            routes.insert(config.agent(), PartitionedMoesiRoute::new(route_id, route));
        }

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(MoesiHarnessError::Scheduler)?,
            transport,
            line: MoesiLineId::new(line_address),
            directory: Arc::new(Mutex::new(MoesiDirectory::new())),
            caches,
            routes,
            memory_route: None,
            backing: Arc::new(Mutex::new(backing)),
            dram_memory: None,
            dram_qos: None,
            fabric,
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
            dram_accesses: Arc::new(Mutex::new(Vec::new())),
            parallel_runs: Vec::new(),
            wait_for: CoherenceWaitFor::new(),
        })
    }

    pub fn new_with_dram_memory<I>(
        layout: CacheLineLayout,
        line_address: Address,
        directory_partition: PartitionId,
        directory_endpoint: TransportEndpointId,
        memory: PartitionedDramMemoryConfig,
        agents: I,
    ) -> Result<Self, MoesiHarnessError>
    where
        I: IntoIterator<Item = PartitionedCacheAgentConfig>,
    {
        let line_address = layout.line_address(line_address);

        let mut partition_count = directory_partition
            .index()
            .checked_add(1)
            .ok_or(MoesiHarnessError::Scheduler(SchedulerError::NoPartitions))?;
        partition_count = partition_count.max(
            memory
                .partition()
                .index()
                .checked_add(1)
                .ok_or(MoesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
        expand_partition_count_for_moesi_hops(&mut partition_count, memory.route_hops())?;
        let agent_configs: Vec<_> = agents.into_iter().collect();
        let uses_fabric = moesi_route_hops_use_fabric(memory.route_hops())
            || agent_configs
                .iter()
                .any(|config| moesi_route_hops_use_fabric(config.route_hops()));
        let fabric = uses_fabric.then(|| Arc::new(Mutex::new(FabricModel::new())));
        let mut transport = if let Some(fabric) = &fabric {
            rem6_transport::MemoryTransport::with_shared_fabric(Arc::clone(fabric))
        } else {
            rem6_transport::MemoryTransport::new()
        };
        let memory_route = moesi_route_from_config(
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
            .map_err(MoesiHarnessError::Transport)?;
        let mut caches = BTreeMap::new();
        let mut routes = BTreeMap::new();

        for config in agent_configs {
            partition_count = partition_count.max(
                config
                    .partition()
                    .index()
                    .checked_add(1)
                    .ok_or(MoesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
            );
            expand_partition_count_for_moesi_hops(&mut partition_count, config.route_hops())?;
            if caches
                .insert(
                    config.agent(),
                    Arc::new(Mutex::new(MoesiCacheController::new(
                        config.agent(),
                        layout,
                        line_address,
                    ))),
                )
                .is_some()
            {
                return Err(MoesiHarnessError::DuplicateCache {
                    agent: config.agent(),
                });
            }

            let route = moesi_route_from_config(
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
                .map_err(MoesiHarnessError::Transport)?;
            routes.insert(config.agent(), PartitionedMoesiRoute::new(route_id, route));
        }

        let dram_qos = memory.qos().cloned().map(|qos| Arc::new(Mutex::new(qos)));
        let dram_controller = memory.into_controller();
        let backing = line_backing_from_moesi_dram_memory(layout, line_address, &dram_controller)?;

        Ok(Self {
            scheduler: PartitionedScheduler::with_min_remote_delay(partition_count, 1)
                .map_err(MoesiHarnessError::Scheduler)?,
            transport,
            line: MoesiLineId::new(line_address),
            directory: Arc::new(Mutex::new(MoesiDirectory::new())),
            caches,
            routes,
            memory_route: Some(PartitionedMoesiRoute::new(memory_route_id, memory_route)),
            backing: Arc::new(Mutex::new(backing)),
            dram_memory: Some(Arc::new(Mutex::new(dram_controller))),
            dram_qos,
            fabric,
            trace: MemoryTrace::new(),
            cpu_responses: Arc::new(Mutex::new(Vec::new())),
            directory_decisions: Arc::new(Mutex::new(Vec::new())),
            dram_accesses: Arc::new(Mutex::new(Vec::new())),
            parallel_runs: Vec::new(),
            wait_for: CoherenceWaitFor::new(),
        })
    }

    pub fn submit_cpu_request_parallel(
        &mut self,
        agent: AgentId,
        request: MemoryRequest,
    ) -> Result<MoesiSubmitResult, MoesiHarnessError> {
        let cache = self.cache_arc(agent)?;
        let request_id = request.id();
        let result = match cache
            .lock()
            .expect("cache lock")
            .accept_cpu_request(request)
        {
            Ok(result) => result,
            Err(MoesiCacheControllerError::LineBusy { state }) => {
                self.wait_for.record_cache_busy(
                    agent,
                    self.line.address().get(),
                    request_id,
                    self.scheduler.now(),
                );
                return Err(MoesiHarnessError::LineBusy { state });
            }
            Err(error) => return Err(map_moesi_cache_error(error)),
        };
        let cache_result = result.kind();

        if let Some(TargetOutcome::Respond(response)) = result.target_outcome() {
            self.cpu_responses
                .lock()
                .expect("response lock")
                .push(moesi_response_record(
                    self.scheduler.now(),
                    cache_result,
                    response,
                ));
            return Ok(MoesiSubmitResult::new(
                SubmitKind::ImmediateHit,
                cache_result,
            ));
        }

        let downstream = result
            .downstream_request()
            .cloned()
            .ok_or(MoesiHarnessError::Cache(
                MoesiCacheControllerError::NoPendingMiss,
            ))?;
        let requester_route = self
            .routes
            .get(&agent)
            .cloned()
            .ok_or(MoesiHarnessError::UnknownCache { agent })?;
        let directory = Arc::clone(&self.directory);
        let caches = self.caches.clone();
        let cache_routes = self.routes.clone();
        let memory_route = self.memory_route.clone();
        let backing = Arc::clone(&self.backing);
        let dram_memory = self.dram_memory.clone();
        let dram_qos = self.dram_qos.clone();
        let fabric = self.fabric.clone();
        let trace = self.trace.clone();
        let response_cache = Arc::clone(&cache);
        let responses = Arc::clone(&self.cpu_responses);
        let decisions = Arc::clone(&self.directory_decisions);
        let dram_accesses = Arc::clone(&self.dram_accesses);
        let wait_for = self.wait_for.clone();
        let line = self.line;

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
                    let fill_event =
                        moesi_fill_event(&decision).expect("directory grant fill event");
                    if let Some(memory_route) =
                        memory_route.filter(|_| decision_uses_moesi_backing_memory(&decision))
                    {
                        decisions.lock().expect("decision lock").push(
                            MoesiDirectoryDecisionRecord::new(
                                delivery.tick(),
                                delivery.request().id().agent(),
                                decision.clone(),
                            ),
                        );
                        let snoop_delay = schedule_partitioned_moesi_snoops_parallel(
                            context,
                            &decision,
                            &cache_routes,
                            &caches,
                            &fabric,
                        )
                        .expect("scheduled directory snoops");
                        schedule_partitioned_moesi_memory_response_parallel(
                            context,
                            delivery.request().clone(),
                            fill_event,
                            requester_route,
                            memory_route,
                            backing,
                            dram_memory,
                            dram_qos,
                            fabric.clone(),
                            trace.clone(),
                            response_cache,
                            responses,
                            dram_accesses,
                            wait_for.clone(),
                            line,
                            snoop_delay,
                        )
                        .expect("scheduled memory response");
                        return TargetOutcome::NoResponse;
                    }
                    let response = partitioned_moesi_directory_response(
                        delivery.request(),
                        &decision,
                        &caches,
                        &backing,
                    )
                    .expect("directory response");
                    decisions.lock().expect("decision lock").push(
                        MoesiDirectoryDecisionRecord::new(
                            delivery.tick(),
                            delivery.request().id().agent(),
                            decision.clone(),
                        ),
                    );

                    let snoop_delay = schedule_partitioned_moesi_snoops_parallel(
                        context,
                        &decision,
                        &cache_routes,
                        &caches,
                        &fabric,
                    )
                    .expect("scheduled directory snoops");
                    let route_delay = requester_route.route.response_latency();
                    let pre_response_delay = snoop_delay.saturating_sub(route_delay);
                    schedule_partitioned_moesi_cache_response_parallel(
                        context,
                        pre_response_delay,
                        requester_route,
                        response,
                        fill_event,
                        fabric,
                        trace,
                        response_cache,
                        responses,
                        wait_for.clone(),
                        line,
                    )
                    .expect("scheduled cache response");

                    TargetOutcome::NoResponse
                },
                |_| {},
            )
            .map_err(MoesiHarnessError::Transport)?;

        Ok(MoesiSubmitResult::new(
            SubmitKind::ScheduledMiss,
            cache_result,
        ))
    }

    pub fn run_until_idle_parallel(&mut self) -> Result<ConservativeRunSummary, MoesiHarnessError> {
        self.run_until_idle_parallel_recorded()
            .map(|run| run.summary())
    }
    pub fn now(&self) -> Tick {
        self.scheduler.now()
    }
    pub fn run_until_idle_parallel_recorded(
        &mut self,
    ) -> Result<ParallelCoherenceRunSummary, MoesiHarnessError> {
        let cpu_responses_before = self.cpu_responses.lock().expect("response lock").len();
        let directory_decisions_before = self
            .directory_decisions
            .lock()
            .expect("decision lock")
            .len();
        let dram_accesses_before = self.dram_accesses.lock().expect("dram access lock").len();
        let resource_window =
            CoherenceResourceActivityWindow::mark(&self.transport, self.dram_memory.as_ref());
        let wait_for_graph_before = self.wait_for.graph();

        let scheduler_run = self
            .scheduler
            .run_until_idle_parallel_recorded()
            .map_err(MoesiHarnessError::Scheduler)?;

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

    pub fn cache_state(&self, agent: AgentId) -> Result<MoesiState, MoesiHarnessError> {
        Ok(self.cache_arc(agent)?.lock().expect("cache lock").state())
    }

    pub fn directory_state(&self) -> MoesiDirectoryLineState {
        self.directory
            .lock()
            .expect("directory lock")
            .line_state(self.line)
    }

    pub fn route(&self, agent: AgentId) -> Result<MemoryRouteId, MoesiHarnessError> {
        self.routes
            .get(&agent)
            .map(|route| route.id)
            .ok_or(MoesiHarnessError::UnknownCache { agent })
    }

    pub fn memory_route(&self) -> Option<MemoryRouteId> {
        self.memory_route.as_ref().map(|route| route.id)
    }

    pub fn trace(&self) -> Vec<MemoryTraceEvent> {
        self.trace.snapshot()
    }

    pub fn cpu_responses(&self) -> Vec<MoesiCpuResponseRecord> {
        self.cpu_responses.lock().expect("response lock").clone()
    }

    pub fn directory_decisions(&self) -> Vec<MoesiDirectoryDecisionRecord> {
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

    pub fn wait_for_graph(&self) -> WaitForGraph {
        self.wait_for.graph()
    }

    pub fn deadlock_diagnostic(&self) -> Option<DeadlockDiagnostic> {
        self.wait_for.deadlock_diagnostic()
    }

    pub const fn line(&self) -> MoesiLineId {
        self.line
    }

    fn cache_arc(
        &self,
        agent: AgentId,
    ) -> Result<Arc<Mutex<MoesiCacheController>>, MoesiHarnessError> {
        self.caches
            .get(&agent)
            .cloned()
            .ok_or(MoesiHarnessError::UnknownCache { agent })
    }
}

fn moesi_fill_event(decision: &MoesiDirectoryDecision) -> Result<MoesiEvent, MoesiHarnessError> {
    let state = decision
        .grant()
        .ok_or(MoesiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?
        .state();
    match state {
        MoesiState::Shared => Ok(MoesiEvent::DataShared),
        MoesiState::Exclusive => Ok(MoesiEvent::DataExclusive),
        MoesiState::Modified => Ok(MoesiEvent::DataModified),
        state => Err(MoesiHarnessError::UnexpectedGrantState { state }),
    }
}

fn expand_partition_count_for_moesi_hops(
    partition_count: &mut u32,
    hops: &[PartitionedRouteHopConfig],
) -> Result<(), MoesiHarnessError> {
    for hop in hops {
        *partition_count = (*partition_count).max(
            hop.partition()
                .index()
                .checked_add(1)
                .ok_or(MoesiHarnessError::Scheduler(SchedulerError::NoPartitions))?,
        );
    }

    Ok(())
}

fn moesi_route_hops_use_fabric(hops: &[PartitionedRouteHopConfig]) -> bool {
    hops.iter()
        .any(|hop| hop.request_fabric_path().is_some() || hop.response_fabric_path().is_some())
}

#[allow(clippy::too_many_arguments)]
fn moesi_route_from_config(
    source_endpoint: TransportEndpointId,
    source_partition: PartitionId,
    target_endpoint: TransportEndpointId,
    target_partition: PartitionId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: rem6_fabric::VirtualNetworkId,
    response_virtual_network: rem6_fabric::VirtualNetworkId,
    route_hops: &[PartitionedRouteHopConfig],
) -> Result<MemoryRoute, MoesiHarnessError> {
    if route_hops.is_empty() {
        return Ok(MemoryRoute::new(
            source_endpoint,
            source_partition,
            target_endpoint,
            target_partition,
            request_latency,
            response_latency,
        )
        .map_err(MoesiHarnessError::Transport)?
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
            .map_err(MoesiHarnessError::Transport)?;
            if let Some(path) = hop.request_fabric_path() {
                route_hop = route_hop.with_request_fabric_path(path.clone());
            }
            if let Some(path) = hop.response_fabric_path() {
                route_hop = route_hop.with_response_fabric_path(path.clone());
            }
            Ok(route_hop)
        })
        .collect::<Result<Vec<_>, MoesiHarnessError>>()?;

    Ok(
        MemoryRoute::new_path(source_endpoint, source_partition, hops)
            .map_err(MoesiHarnessError::Transport)?
            .with_virtual_networks(request_virtual_network, response_virtual_network),
    )
}

fn partitioned_moesi_directory_response(
    request: &MemoryRequest,
    decision: &MoesiDirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MoesiCacheController>>>,
    backing: &Arc<Mutex<LineBackingStore>>,
) -> Result<MemoryResponse, MoesiHarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(MoesiHarnessError::MissingDirectoryGrant {
            request: request.id(),
        })?;
    let source_data = partitioned_moesi_source_data(decision, caches)?;

    match grant.data_source() {
        MoesiDirectoryDataSource::BackingMemory => backing
            .lock()
            .expect("backing lock")
            .respond(request)
            .map_err(MoesiHarnessError::Backing),
        MoesiDirectoryDataSource::OwnerCache(_) if request.returns_data() => {
            MemoryResponse::completed(request, source_data).map_err(MoesiHarnessError::Memory)
        }
        MoesiDirectoryDataSource::OwnerCache(_) | MoesiDirectoryDataSource::NoData => {
            MemoryResponse::completed(request, None).map_err(MoesiHarnessError::Memory)
        }
    }
}

fn partitioned_moesi_source_data(
    decision: &MoesiDirectoryDecision,
    caches: &BTreeMap<AgentId, Arc<Mutex<MoesiCacheController>>>,
) -> Result<Option<Vec<u8>>, MoesiHarnessError> {
    let grant = decision
        .grant()
        .copied()
        .ok_or(MoesiHarnessError::MissingDirectoryGrant {
            request: decision.request(),
        })?;
    Ok(match grant.data_source() {
        MoesiDirectoryDataSource::BackingMemory | MoesiDirectoryDataSource::NoData => None,
        MoesiDirectoryDataSource::OwnerCache(agent) => {
            let cache = caches
                .get(&agent)
                .ok_or(MoesiHarnessError::UnknownCache { agent })?;
            let locked = cache.lock().expect("cache lock");
            Some(
                locked
                    .cached_data()
                    .ok_or(MoesiHarnessError::GrantDataUnavailable {
                        agent,
                        line: grant.line(),
                    })?
                    .to_vec(),
            )
        }
    })
}

fn decision_uses_moesi_backing_memory(decision: &MoesiDirectoryDecision) -> bool {
    decision
        .grant()
        .is_some_and(|grant| matches!(grant.data_source(), MoesiDirectoryDataSource::BackingMemory))
}

fn line_backing_from_moesi_dram_memory(
    layout: CacheLineLayout,
    line_address: Address,
    controller: &DramMemoryController,
) -> Result<LineBackingStore, MoesiHarnessError> {
    let request = MemoryRequest::read_shared(
        MemoryRequestId::new(AgentId::new(0), 0),
        line_address,
        rem6_memory::AccessSize::new(layout.bytes()).map_err(MoesiHarnessError::Memory)?,
        layout,
    )
    .map_err(MoesiHarnessError::Memory)?;
    let mut probe = controller.clone();
    let outcome = probe.accept(0, &request).map_err(MoesiHarnessError::Dram)?;
    let data = outcome
        .response()
        .and_then(MemoryResponse::data)
        .map(<[u8]>::to_vec)
        .ok_or(MoesiHarnessError::Memory(
            MemoryError::MissingResponseData {
                request: request.id(),
            },
        ))?;

    LineBackingStore::new(layout, line_address, data).map_err(MoesiHarnessError::Backing)
}

fn schedule_partitioned_moesi_snoops_parallel(
    context: &mut ParallelSchedulerContext<'_>,
    decision: &MoesiDirectoryDecision,
    cache_routes: &BTreeMap<AgentId, PartitionedMoesiRoute>,
    caches: &BTreeMap<AgentId, Arc<Mutex<MoesiCacheController>>>,
    fabric: &Option<Arc<Mutex<FabricModel>>>,
) -> Result<u64, MoesiHarnessError> {
    let mut max_delay = 0;
    for snoop in decision.snoops() {
        let route = cache_routes
            .get(&snoop.target())
            .ok_or(MoesiHarnessError::UnknownCache {
                agent: snoop.target(),
            })?;
        let delay = moesi_route_response_delay(
            fabric,
            context.now(),
            route.id,
            &route.route,
            decision.request(),
            1,
        )
        .map_err(MoesiHarnessError::Transport)?;
        max_delay = max_delay.max(delay);
        let cache =
            caches
                .get(&snoop.target())
                .cloned()
                .ok_or(MoesiHarnessError::UnknownCache {
                    agent: snoop.target(),
                })?;
        let event = snoop.event();
        context
            .schedule_remote_after(route.route.source_partition(), delay, move |_| {
                cache
                    .lock()
                    .expect("cache lock")
                    .accept_snoop(event)
                    .map_err(map_moesi_cache_error)
                    .expect("scheduled snoop");
            })
            .map_err(MoesiHarnessError::Scheduler)?;
    }

    Ok(max_delay)
}

fn moesi_response_record(
    tick: u64,
    cache_result: MoesiCacheControllerResultKind,
    response: &MemoryResponse,
) -> MoesiCpuResponseRecord {
    MoesiCpuResponseRecord::new(
        tick,
        cache_result,
        response.request_id(),
        response.status(),
        response.data().map(<[u8]>::to_vec),
    )
}

fn map_moesi_cache_error(error: MoesiCacheControllerError) -> MoesiHarnessError {
    match error {
        MoesiCacheControllerError::LineBusy { state } => MoesiHarnessError::LineBusy { state },
        error => MoesiHarnessError::Cache(error),
    }
}
