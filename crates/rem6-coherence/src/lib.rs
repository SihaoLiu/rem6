use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_cache::{CacheControllerError, CacheControllerResultKind, MsiCacheController};
use rem6_directory::{
    DirectoryDataSource, DirectoryDecision, DirectoryError, DirectoryGrant, DirectoryLineState,
    MsiDirectory,
};
use rem6_dram::DramMemoryController;
use rem6_fabric::{FabricModel, FabricPath, VirtualNetworkId};
use rem6_kernel::{ConservativeRunSummary, PartitionId, PartitionedScheduler, SchedulerError};
use rem6_memory::{
    Address, AgentId, CacheLineLayout, MemoryError, MemoryOperation, MemoryRequest,
    MemoryRequestId, MemoryResponse, MemoryTargetId, ResponseStatus,
};
use rem6_protocol_msi::{MsiLineId, MsiState};
use rem6_topology::{Endpoint, TopologyError};
use rem6_transport::{
    MemoryRoute, MemoryRouteHop, MemoryRouteId, MemoryTrace, MemoryTraceEvent, MemoryTransport,
    TargetOutcome, TransportEndpointId, TransportError,
};

mod deferred;
mod mesi;
mod moesi;
mod snoop;
mod topology;

use deferred::{DeferredMemoryPath, DeferredMemoryWork};
use snoop::{DirectorySnoopWork, SnoopRoute};

pub use mesi::{
    MesiCpuResponseRecord, MesiDirectoryDecisionRecord, MesiDirectoryLineHarness, MesiHarnessError,
    MesiSubmitResult, PartitionedMesiDirectoryLineHarness,
};
pub use moesi::{
    MoesiCpuResponseRecord, MoesiDirectoryDecisionRecord, MoesiDirectoryLineHarness,
    MoesiHarnessError, MoesiSubmitResult, PartitionedMoesiDirectoryLineHarness,
};
pub use topology::{
    TopologyCacheAgentConfig, TopologyDirectoryConfig, TopologyDirectoryHarnessConfig,
    TopologyDramMemoryConfig,
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
    MissingTopologyConnection { from: Endpoint, to: Endpoint },
    MissingBackingMemory { line: Address },
    WrongLine { expected: Address, actual: Address },
    LineDataSizeMismatch { expected: u64, actual: u64 },
    MissingDirectoryGrant { request: MemoryRequestId },
    GrantDataUnavailable { agent: AgentId, line: MsiLineId },
    Cache(CacheControllerError),
    Directory(DirectoryError),
    Memory(MemoryError),
    Scheduler(SchedulerError),
    Topology(TopologyError),
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
            Self::MissingTopologyConnection { from, to } => write!(
                formatter,
                "topology connection {}.{} to {}.{} is not declared",
                from.component().as_str(),
                from.port().as_str(),
                to.component().as_str(),
                to.port().as_str()
            ),
            Self::MissingBackingMemory { line } => {
                write!(formatter, "line {:#x} has no backing memory", line.get())
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
            Self::Topology(error) => write!(formatter, "{error}"),
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
            Self::Topology(error) => Some(error),
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

    pub fn replace_data(&mut self, data: Vec<u8>) -> Result<(), HarnessError> {
        if data.len() as u64 != self.layout.bytes() {
            return Err(HarnessError::LineDataSizeMismatch {
                expected: self.layout.bytes(),
                actual: data.len() as u64,
            });
        }

        self.data = data;
        Ok(())
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
pub struct PartitionedRouteHopConfig {
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_fabric_path: Option<FabricPath>,
    response_fabric_path: Option<FabricPath>,
}

impl PartitionedRouteHopConfig {
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
            request_fabric_path: None,
            response_fabric_path: None,
        }
    }

    pub fn with_request_fabric_path(mut self, path: FabricPath) -> Self {
        self.request_fabric_path = Some(path);
        self
    }

    pub fn with_response_fabric_path(mut self, path: FabricPath) -> Self {
        self.response_fabric_path = Some(path);
        self
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

    pub fn request_fabric_path(&self) -> Option<&FabricPath> {
        self.request_fabric_path.as_ref()
    }

    pub fn response_fabric_path(&self) -> Option<&FabricPath> {
        self.response_fabric_path.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedCacheAgentConfig {
    agent: AgentId,
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    route_hops: Vec<PartitionedRouteHopConfig>,
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
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
            route_hops: Vec::new(),
        }
    }

    pub fn with_virtual_networks(
        mut self,
        request_virtual_network: VirtualNetworkId,
        response_virtual_network: VirtualNetworkId,
    ) -> Self {
        self.request_virtual_network = request_virtual_network;
        self.response_virtual_network = response_virtual_network;
        self
    }

    pub fn with_route_hops<I>(mut self, route_hops: I) -> Self
    where
        I: IntoIterator<Item = PartitionedRouteHopConfig>,
    {
        self.route_hops = route_hops.into_iter().collect();
        self
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

    pub const fn request_virtual_network(&self) -> VirtualNetworkId {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> VirtualNetworkId {
        self.response_virtual_network
    }

    pub fn route_hops(&self) -> &[PartitionedRouteHopConfig] {
        &self.route_hops
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedMemoryConfig {
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    route_hops: Vec<PartitionedRouteHopConfig>,
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
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
            route_hops: Vec::new(),
        }
    }

    pub fn with_virtual_networks(
        mut self,
        request_virtual_network: VirtualNetworkId,
        response_virtual_network: VirtualNetworkId,
    ) -> Self {
        self.request_virtual_network = request_virtual_network;
        self.response_virtual_network = response_virtual_network;
        self
    }

    pub fn with_route_hops<I>(mut self, route_hops: I) -> Self
    where
        I: IntoIterator<Item = PartitionedRouteHopConfig>,
    {
        self.route_hops = route_hops.into_iter().collect();
        self
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

    pub const fn request_virtual_network(&self) -> VirtualNetworkId {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> VirtualNetworkId {
        self.response_virtual_network
    }

    pub fn route_hops(&self) -> &[PartitionedRouteHopConfig] {
        &self.route_hops
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PartitionedDramMemoryConfig {
    partition: PartitionId,
    endpoint: TransportEndpointId,
    request_latency: u64,
    response_latency: u64,
    request_virtual_network: VirtualNetworkId,
    response_virtual_network: VirtualNetworkId,
    controller: DramMemoryController,
    route_hops: Vec<PartitionedRouteHopConfig>,
}

impl PartitionedDramMemoryConfig {
    pub fn new(
        partition: PartitionId,
        endpoint: TransportEndpointId,
        request_latency: u64,
        response_latency: u64,
        controller: DramMemoryController,
    ) -> Self {
        Self {
            partition,
            endpoint,
            request_latency,
            response_latency,
            request_virtual_network: VirtualNetworkId::new(0),
            response_virtual_network: VirtualNetworkId::new(0),
            controller,
            route_hops: Vec::new(),
        }
    }

    pub fn with_virtual_networks(
        mut self,
        request_virtual_network: VirtualNetworkId,
        response_virtual_network: VirtualNetworkId,
    ) -> Self {
        self.request_virtual_network = request_virtual_network;
        self.response_virtual_network = response_virtual_network;
        self
    }

    pub fn with_route_hops<I>(mut self, route_hops: I) -> Self
    where
        I: IntoIterator<Item = PartitionedRouteHopConfig>,
    {
        self.route_hops = route_hops.into_iter().collect();
        self
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

    pub const fn request_virtual_network(&self) -> VirtualNetworkId {
        self.request_virtual_network
    }

    pub const fn response_virtual_network(&self) -> VirtualNetworkId {
        self.response_virtual_network
    }

    pub fn route_hops(&self) -> &[PartitionedRouteHopConfig] {
        &self.route_hops
    }

    fn into_controller(self) -> DramMemoryController {
        self.controller
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DramMemoryAccessRecord {
    arrival_tick: u64,
    target: MemoryTargetId,
    request: MemoryRequestId,
    bank: u32,
    row: u64,
    row_hit: bool,
    ready_cycle: u64,
}

impl DramMemoryAccessRecord {
    pub const fn new(
        arrival_tick: u64,
        target: MemoryTargetId,
        request: MemoryRequestId,
        bank: u32,
        row: u64,
        row_hit: bool,
        ready_cycle: u64,
    ) -> Self {
        Self {
            arrival_tick,
            target,
            request,
            bank,
            row,
            row_hit,
            ready_cycle,
        }
    }

    pub const fn arrival_tick(&self) -> u64 {
        self.arrival_tick
    }

    pub const fn target(&self) -> MemoryTargetId {
        self.target
    }

    pub const fn request(&self) -> MemoryRequestId {
        self.request
    }

    pub const fn bank(&self) -> u32 {
        self.bank
    }

    pub const fn row(&self) -> u64 {
        self.row
    }

    pub const fn row_hit(&self) -> bool {
        self.row_hit
    }

    pub const fn ready_cycle(&self) -> u64 {
        self.ready_cycle
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

    pub fn dram_memory_accesses(&self) -> Vec<DramMemoryAccessRecord> {
        self.dram_accesses.lock().expect("DRAM access lock").clone()
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
