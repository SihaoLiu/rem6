use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_isa_riscv::{
    MemoryAccessKind, Register, RiscvExecutionRecord, RiscvHartState, RiscvInstruction, RiscvTrap,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler,
    SchedulerContext, Tick,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, CacheLineLayout, MemoryOperation, MemoryRequest,
    MemoryRequestId, ResponseStatus, TranslationRequestId,
};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome, TransportEndpointId, TransportError,
};

mod bimode_predictor;
mod branch_predictor;
mod data_config;
mod error;
mod fetch_config;
mod gshare_predictor;
mod indirect_target_predictor;
mod loop_predictor;
mod ltage_predictor;
mod multiperspective_perceptron;
mod o3_dependency;
mod o3_pipeline;
mod parallel_flow;
mod riscv_activity;
mod riscv_cluster;
mod riscv_cluster_run;
mod riscv_data_access;
mod riscv_data_issue;
mod riscv_reservation;
mod riscv_translation;
mod statistical_corrector;
mod tage_predictor;
mod tage_sc_l_predictor;
mod topology;
mod tournament_predictor;
mod translation;

pub use bimode_predictor::{
    BiModeBranchPredictor, BiModeBranchPredictorConfig, BiModeBranchPredictorError,
    BiModeBranchPredictorSnapshot, BiModeDirectionArray, BiModeHistory, BiModeHistoryUpdate,
    BiModePrediction, BiModeSquash, BiModeThreadSnapshot, BiModeTrainingUpdate,
};
pub use branch_predictor::{
    BranchPrediction, BranchPredictor, BranchPredictorConfig, BranchPredictorError,
    BranchPredictorSnapshot, BranchSpeculation, BranchSpeculationId, BranchSpeculationRepair,
    BranchTargetBuffer, BranchTargetBufferConfig, BranchTargetBufferError,
    BranchTargetBufferSnapshot, BranchTargetEntry, BranchTargetKind, BranchTargetLookup,
    BranchTargetSafetyConfig, BranchTargetSafetyProfile, BranchTargetUpdate, BranchUpdate,
    ReturnAddressStack, ReturnAddressStackConfig, ReturnAddressStackError,
    ReturnAddressStackOperation, ReturnAddressStackOperationId, ReturnAddressStackOperationKind,
    ReturnAddressStackRepair, ReturnAddressStackSnapshot,
};
pub use data_config::CpuDataConfig;
pub use error::{CpuClusterError, CpuError, RiscvCpuError};
pub use fetch_config::CpuFetchConfig;
pub use gshare_predictor::{
    GShareBranchPredictor, GShareBranchPredictorConfig, GShareBranchPredictorError,
    GShareBranchPredictorSnapshot, GShareHistory, GShareHistoryUpdate, GSharePrediction,
    GShareSquash, GShareThreadSnapshot, GShareTrainingUpdate,
};
pub use indirect_target_predictor::{
    IndirectTargetCommit, IndirectTargetEntry, IndirectTargetHistory, IndirectTargetPathEntry,
    IndirectTargetPrediction, IndirectTargetPredictor, IndirectTargetPredictorConfig,
    IndirectTargetPredictorError, IndirectTargetPredictorSnapshot, IndirectTargetSequence,
    IndirectTargetSquash, IndirectTargetThreadSnapshot, IndirectTargetUpdate,
};
pub use loop_predictor::{
    LoopBranchPredictor, LoopBranchPredictorConfig, LoopBranchPredictorError,
    LoopBranchPredictorSnapshot, LoopEntrySnapshot, LoopHistory, LoopPrediction, LoopSquash,
    LoopTrainingUpdate,
};
pub use ltage_predictor::{
    LTageBranchPredictor, LTageBranchPredictorConfig, LTageBranchPredictorError,
    LTageBranchPredictorSnapshot, LTageHistory, LTagePrediction, LTageProvider, LTageRepair,
    LTageTrainingUpdate,
};
pub use multiperspective_perceptron::{
    MultiperspectivePerceptron, MultiperspectivePerceptronConfig, MultiperspectivePerceptronError,
    MultiperspectivePerceptronFeature, MultiperspectivePerceptronFeatureKind,
    MultiperspectivePerceptronFeatureUpdate, MultiperspectivePerceptronFilterEntry,
    MultiperspectivePerceptronHistory, MultiperspectivePerceptronPrediction,
    MultiperspectivePerceptronSnapshot, MultiperspectivePerceptronThreadSnapshot,
    MultiperspectivePerceptronTrainingUpdate,
};
pub use o3_dependency::{
    O3DependencyProducerKind, O3DependencyReleasePlan, O3DependencyReleaseReason,
    O3DependencyReleaseStage, O3DestinationRegister, O3DestinationRelease, O3DestinationVisibility,
    O3PhysicalRegisterId, O3RegisterClass, O3SourceRegister, O3SourceRenameDecision,
    O3SourceRenamePlan, O3SourceRenameReason,
};
pub use o3_pipeline::{
    O3DistributedIssuePlan, O3DistributedIssueScheduler, O3IssueOpClass, O3IssueQueueCapacity,
    O3IssueQueueId, O3PipelineError, O3PipelineStage, O3ReadyInstruction, O3UnblockDecision,
    O3UnblockDecisionReason, O3UnblockPolicy, O3WritebackAdmission, O3WritebackTransferPlan,
    O3WritebackTransferPolicy,
};
pub use riscv_activity::RiscvCoreDriveActivity;
pub use riscv_cluster::{RiscvCluster, RiscvClusterError};
pub use riscv_cluster_run::{
    RiscvClusterDriveEvent, RiscvClusterParallelBatchTimelineRecord, RiscvClusterRun,
    RiscvClusterSchedulerEpoch, RiscvClusterStopReason, RiscvClusterTurn,
};
pub use riscv_data_access::{
    RiscvDataAccessEvent, RiscvDataAccessEventKind, RiscvDataAccessRecord, RiscvDataAccessTarget,
    RiscvLoadReservation,
};
pub use statistical_corrector::{
    StatisticalCorrector, StatisticalCorrectorBranchKind, StatisticalCorrectorConfig,
    StatisticalCorrectorError, StatisticalCorrectorHistory, StatisticalCorrectorHistoryUpdate,
    StatisticalCorrectorInput, StatisticalCorrectorPrediction, StatisticalCorrectorSnapshot,
    StatisticalCorrectorThreadSnapshot, StatisticalCorrectorTrainingUpdate,
};
pub use tage_predictor::{
    FoldedHistorySnapshot, TageBranchPredictor, TageBranchPredictorConfig,
    TageBranchPredictorError, TageBranchPredictorSnapshot, TageHistory, TageHistoryUpdate,
    TagePrediction, TageProvider, TageTableEntry, TageThreadSnapshot, TageTrainingUpdate,
};
pub use tage_sc_l_predictor::{
    TageScLBranchPredictor, TageScLBranchPredictorConfig, TageScLBranchPredictorError,
    TageScLBranchPredictorSnapshot, TageScLHistory, TageScLHistoryUpdate, TageScLPrediction,
    TageScLProvider, TageScLRepair, TageScLTrainingUpdate,
};
pub use topology::{
    CpuTopologyError, RiscvClusterTopologyConfig, RiscvCoreTopologyConfig,
    RiscvCoreTopologyDataTranslationConfig,
};
pub use tournament_predictor::{
    TournamentBranchPredictor, TournamentBranchPredictorConfig, TournamentBranchPredictorError,
    TournamentBranchPredictorSnapshot, TournamentHistory, TournamentHistoryUpdate,
    TournamentPrediction, TournamentPredictorSelection, TournamentSquash, TournamentThreadSnapshot,
    TournamentTrainingUpdate,
};
pub use translation::{
    CpuSegmentedTranslationOutcome, CpuTranslatedMemoryOperation, CpuTranslatedMemoryRequest,
    CpuTranslatedMemorySegment, CpuTranslationFaultRecord, CpuTranslationFrontend,
    CpuTranslationFrontendError, CpuTranslationFrontendSnapshot, CpuTranslationOutcome,
    CpuTranslationRequest,
};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CpuId(u32);

impl CpuId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }

    pub const fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuResetState {
    cpu: CpuId,
    partition: PartitionId,
    agent: AgentId,
    entry: Address,
}

impl CpuResetState {
    pub const fn new(cpu: CpuId, partition: PartitionId, agent: AgentId, entry: Address) -> Self {
        Self {
            cpu,
            partition,
            agent,
            entry,
        }
    }

    pub fn from_boot_image(
        cpu: CpuId,
        partition: PartitionId,
        agent: AgentId,
        image: &BootImage,
    ) -> Self {
        Self::new(cpu, partition, agent, image.entry())
    }

    pub const fn cpu(&self) -> CpuId {
        self.cpu
    }

    pub const fn partition(&self) -> PartitionId {
        self.partition
    }

    pub const fn agent(&self) -> AgentId {
        self.agent
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }
}

#[derive(Clone)]
pub struct CpuCore {
    state: Arc<Mutex<CpuCoreState>>,
}

impl CpuCore {
    pub fn new(reset: CpuResetState, fetch: CpuFetchConfig) -> Result<Self, CpuError> {
        Ok(Self {
            state: Arc::new(Mutex::new(CpuCoreState::new(reset, fetch))),
        })
    }

    pub fn id(&self) -> CpuId {
        self.state.lock().expect("cpu core lock").reset.cpu()
    }

    pub fn partition(&self) -> PartitionId {
        self.state.lock().expect("cpu core lock").reset.partition()
    }

    pub fn agent(&self) -> AgentId {
        self.state.lock().expect("cpu core lock").reset.agent()
    }

    pub fn pc(&self) -> Address {
        self.state.lock().expect("cpu core lock").pc
    }

    pub fn fetch_endpoint(&self) -> TransportEndpointId {
        self.state
            .lock()
            .expect("cpu core lock")
            .fetch
            .endpoint()
            .clone()
    }

    pub fn fetch_route(&self) -> MemoryRouteId {
        self.state.lock().expect("cpu core lock").fetch.route()
    }

    pub fn next_sequence(&self) -> u64 {
        self.state.lock().expect("cpu core lock").next_sequence
    }

    pub fn fetch_events(&self) -> Vec<CpuFetchEvent> {
        self.state.lock().expect("cpu core lock").events.clone()
    }

    fn has_pending_fetch(&self) -> bool {
        !self
            .state
            .lock()
            .expect("cpu core lock")
            .outstanding
            .is_empty()
    }

    fn set_pc(&self, pc: Address) {
        self.state.lock().expect("cpu core lock").pc = pc;
    }

    pub fn add_fetch_line_layout_range(&self, range: AddressRange, line_layout: CacheLineLayout) {
        self.state
            .lock()
            .expect("cpu core lock")
            .fetch
            .add_line_layout_range(range, line_layout);
    }

    fn advance_sequence_past(&self, request: MemoryRequestId) {
        let mut state = self.state.lock().expect("cpu core lock");
        if state.next_sequence <= request.sequence() {
            state.next_sequence = request.sequence() + 1;
        }
    }

    pub fn issue_next_fetch<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let issue = self.prepare_fetch(scheduler.now(), transport)?;
        let request = MemoryRequest::instruction_fetch(
            issue.request_id,
            issue.pc,
            issue.size,
            issue.line_layout,
        )
        .map_err(CpuError::Memory)?;

        let core = self.clone();
        let event = transport
            .submit(
                scheduler,
                issue.route,
                request,
                trace,
                responder,
                move |delivery| core.record_response(delivery),
            )
            .map_err(CpuError::Transport)?;

        self.record_issue(issue);
        Ok(event)
    }

    pub fn issue_next_fetch_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let (issue, transaction) =
            self.prepare_fetch_parallel_transaction(scheduler.now(), transport, trace, responder)?;
        let event = transport
            .submit_parallel_batch(scheduler, [transaction])
            .map_err(CpuError::Transport)?
            .into_iter()
            .next()
            .expect("single fetch transaction returns one event");

        self.record_issue(issue);
        Ok(event)
    }

    fn prepare_fetch_parallel_transaction<F>(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<(OutstandingFetch, ParallelMemoryTransaction), CpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let issue = self.prepare_fetch(tick, transport)?;
        let request = MemoryRequest::instruction_fetch(
            issue.request_id,
            issue.pc,
            issue.size,
            issue.line_layout,
        )
        .map_err(CpuError::Memory)?;

        let core = self.clone();
        let transaction = ParallelMemoryTransaction::new(
            issue.route,
            request,
            trace,
            responder,
            move |delivery| core.record_response(delivery),
        );
        Ok((issue, transaction))
    }

    fn prepare_fetch(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<OutstandingFetch, CpuError> {
        let state = self.state.lock().expect("cpu core lock");
        let route = transport
            .route(state.fetch.route())
            .ok_or(CpuError::Transport(TransportError::UnknownRoute {
                route: state.fetch.route(),
            }))?;

        if route.source_partition() != state.reset.partition() {
            return Err(CpuError::RoutePartitionMismatch {
                route: state.fetch.route(),
                expected: state.reset.partition(),
                actual: route.source_partition(),
            });
        }
        if route.source() != state.fetch.endpoint() {
            return Err(CpuError::RouteEndpointMismatch {
                route: state.fetch.route(),
                expected: state.fetch.endpoint().clone(),
                actual: route.source().clone(),
            });
        }

        let line_layout = state
            .fetch
            .line_layout_for_fetch(state.pc)
            .map_err(CpuError::Memory)?;
        let line_offset = line_layout.line_offset(state.pc);
        if line_offset + state.fetch.width().bytes() > line_layout.bytes() {
            return Err(CpuError::FetchCrossesLine {
                pc: state.pc,
                size: state.fetch.width(),
                line_size: line_layout.bytes(),
            });
        }

        Ok(OutstandingFetch {
            tick,
            partition: state.reset.partition(),
            route: state.fetch.route(),
            endpoint: state.fetch.endpoint().clone(),
            request_id: MemoryRequestId::new(state.reset.agent(), state.next_sequence),
            pc: state.pc,
            size: state.fetch.width(),
            line_layout,
        })
    }

    fn record_issue(&self, issue: OutstandingFetch) {
        let mut state = self.state.lock().expect("cpu core lock");
        state.next_sequence += 1;
        state
            .outstanding
            .insert(issue.request_id, issue.clone_without_layout());
        state.events.push(CpuFetchEvent::issued(CpuFetchRecord::new(
            issue.tick,
            issue.partition,
            issue.route,
            issue.endpoint,
            issue.request_id,
            issue.pc,
            issue.size,
        )));
    }

    fn record_response(&self, delivery: ResponseDelivery) {
        let mut state = self.state.lock().expect("cpu core lock");
        let Some(fetch) = state.outstanding.remove(&delivery.response().request_id()) else {
            return;
        };

        match delivery.response().status() {
            ResponseStatus::Completed => {
                let data = delivery.response().data().unwrap_or_default().to_vec();
                if let Some(next_pc) = fetch.pc.get().checked_add(data.len() as u64) {
                    state.pc = Address::new(next_pc);
                }
                state.events.push(CpuFetchEvent::completed(
                    fetch.record(
                        delivery.tick(),
                        delivery.route(),
                        delivery.endpoint().clone(),
                    ),
                    data,
                ));
            }
            ResponseStatus::Retry => {
                state.events.push(CpuFetchEvent::retry(fetch.record(
                    delivery.tick(),
                    delivery.route(),
                    delivery.endpoint().clone(),
                )));
            }
        }
    }
}

impl fmt::Debug for CpuCore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = self.state.lock().expect("cpu core lock");
        formatter
            .debug_struct("CpuCore")
            .field("cpu", &state.reset.cpu())
            .field("partition", &state.reset.partition())
            .field("pc", &state.pc)
            .field("next_sequence", &state.next_sequence)
            .finish()
    }
}

#[derive(Clone, Debug)]
pub struct CpuCluster {
    cores: BTreeMap<CpuId, CpuCore>,
}

impl CpuCluster {
    pub fn new<I>(cores: I) -> Result<Self, CpuClusterError>
    where
        I: IntoIterator<Item = CpuCore>,
    {
        let mut by_cpu = BTreeMap::new();
        let mut by_agent = BTreeMap::new();
        let mut by_endpoint = BTreeMap::new();

        for core in cores {
            let cpu = core.id();
            if by_cpu.contains_key(&cpu) {
                return Err(CpuClusterError::DuplicateCpu { cpu });
            }

            let agent = core.agent();
            if let Some(existing) = by_agent.insert(agent, cpu) {
                return Err(CpuClusterError::DuplicateAgent {
                    agent,
                    existing,
                    duplicate: cpu,
                });
            }

            let endpoint = core.fetch_endpoint();
            if let Some(existing) = by_endpoint.insert(endpoint.clone(), cpu) {
                return Err(CpuClusterError::DuplicateFetchEndpoint {
                    endpoint,
                    existing,
                    duplicate: cpu,
                });
            }

            by_cpu.insert(cpu, core);
        }

        Ok(Self { cores: by_cpu })
    }

    pub fn core_count(&self) -> usize {
        self.cores.len()
    }

    pub fn core_ids(&self) -> Vec<CpuId> {
        self.cores.keys().copied().collect()
    }

    pub fn core(&self, cpu: CpuId) -> Result<CpuCore, CpuClusterError> {
        self.cores
            .get(&cpu)
            .cloned()
            .ok_or(CpuClusterError::UnknownCpu { cpu })
    }

    pub fn issue_next_fetch<F>(
        &self,
        cpu: CpuId,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuClusterError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        self.core(cpu)?
            .issue_next_fetch(scheduler, transport, trace, responder)
            .map_err(CpuClusterError::Cpu)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CpuCoreState {
    reset: CpuResetState,
    fetch: CpuFetchConfig,
    pc: Address,
    next_sequence: u64,
    outstanding: BTreeMap<MemoryRequestId, IssuedFetch>,
    events: Vec<CpuFetchEvent>,
}

impl CpuCoreState {
    fn new(reset: CpuResetState, fetch: CpuFetchConfig) -> Self {
        let pc = reset.entry();
        Self {
            reset,
            fetch,
            pc,
            next_sequence: 0,
            outstanding: BTreeMap::new(),
            events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct OutstandingFetch {
    tick: Tick,
    partition: PartitionId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    request_id: MemoryRequestId,
    pc: Address,
    size: AccessSize,
    line_layout: CacheLineLayout,
}

impl OutstandingFetch {
    fn clone_without_layout(&self) -> IssuedFetch {
        IssuedFetch {
            partition: self.partition,
            request: self.request_id,
            pc: self.pc,
            size: self.size,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IssuedFetch {
    partition: PartitionId,
    request: MemoryRequestId,
    pc: Address,
    size: AccessSize,
}

impl IssuedFetch {
    fn record(
        &self,
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
    ) -> CpuFetchRecord {
        CpuFetchRecord::new(
            tick,
            self.partition,
            route,
            endpoint,
            self.request,
            self.pc,
            self.size,
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CpuFetchEventKind {
    Issued,
    Completed,
    Retry,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuFetchRecord {
    tick: Tick,
    partition: PartitionId,
    route: MemoryRouteId,
    endpoint: TransportEndpointId,
    request: MemoryRequestId,
    pc: Address,
    size: AccessSize,
}

impl CpuFetchRecord {
    pub fn new(
        tick: Tick,
        partition: PartitionId,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
        request: MemoryRequestId,
        pc: Address,
        size: AccessSize,
    ) -> Self {
        Self {
            tick,
            partition,
            route,
            endpoint,
            request,
            pc,
            size,
        }
    }

    pub fn tick(&self) -> Tick {
        self.tick
    }

    pub fn partition(&self) -> PartitionId {
        self.partition
    }

    pub fn route(&self) -> MemoryRouteId {
        self.route
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        &self.endpoint
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.request
    }

    pub fn pc(&self) -> Address {
        self.pc
    }

    pub fn size(&self) -> AccessSize {
        self.size
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CpuFetchEvent {
    record: CpuFetchRecord,
    kind: CpuFetchEventKind,
    data: Option<Vec<u8>>,
}

impl CpuFetchEvent {
    pub fn issued(record: CpuFetchRecord) -> Self {
        Self {
            record,
            kind: CpuFetchEventKind::Issued,
            data: None,
        }
    }

    pub fn completed(record: CpuFetchRecord, data: Vec<u8>) -> Self {
        Self {
            record,
            kind: CpuFetchEventKind::Completed,
            data: Some(data),
        }
    }

    pub fn retry(record: CpuFetchRecord) -> Self {
        Self {
            record,
            kind: CpuFetchEventKind::Retry,
            data: None,
        }
    }

    pub fn tick(&self) -> Tick {
        self.record.tick()
    }

    pub fn partition(&self) -> PartitionId {
        self.record.partition()
    }

    pub fn route(&self) -> MemoryRouteId {
        self.record.route()
    }

    pub fn endpoint(&self) -> &TransportEndpointId {
        self.record.endpoint()
    }

    pub fn request_id(&self) -> MemoryRequestId {
        self.record.request_id()
    }

    pub fn pc(&self) -> Address {
        self.record.pc()
    }

    pub fn size(&self) -> AccessSize {
        self.record.size()
    }

    pub fn kind(&self) -> CpuFetchEventKind {
        self.kind
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.data.as_deref()
    }
}

#[derive(Clone, Debug)]
pub struct RiscvCore {
    core: CpuCore,
    state: Arc<Mutex<RiscvCoreState>>,
}

impl RiscvCore {
    pub fn new(core: CpuCore) -> Self {
        let pc = core.pc().get();
        Self {
            core,
            state: Arc::new(Mutex::new(RiscvCoreState::new(pc))),
        }
    }

    pub fn with_data(core: CpuCore, data: CpuDataConfig) -> Self {
        let core = Self::new(core);
        core.state.lock().expect("riscv core lock").data = Some(data);
        core
    }

    pub fn inner(&self) -> CpuCore {
        self.core.clone()
    }

    pub fn id(&self) -> CpuId {
        self.core.id()
    }

    pub fn partition(&self) -> PartitionId {
        self.core.partition()
    }

    pub fn agent(&self) -> AgentId {
        self.core.agent()
    }

    pub fn fetch_endpoint(&self) -> TransportEndpointId {
        self.core.fetch_endpoint()
    }

    pub fn fetch_route(&self) -> MemoryRouteId {
        self.core.fetch_route()
    }

    pub fn data_endpoint(&self) -> Option<TransportEndpointId> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data
            .as_ref()
            .map(|data| data.endpoint().clone())
    }

    pub fn data_route(&self) -> Option<MemoryRouteId> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data
            .as_ref()
            .map(CpuDataConfig::route)
    }

    pub fn pc(&self) -> Address {
        Address::new(self.state.lock().expect("riscv core lock").hart.pc())
    }

    pub fn read_register(&self, register: Register) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .read(register)
    }

    pub fn pending_trap(&self) -> Option<RiscvTrap> {
        self.state.lock().expect("riscv core lock").pending_trap
    }

    pub fn has_pending_trap(&self) -> bool {
        self.pending_trap().is_some()
    }

    pub fn has_pending_fetch(&self) -> bool {
        self.core.has_pending_fetch()
    }

    pub fn has_pending_data_access(&self) -> bool {
        let state = self.state.lock().expect("riscv core lock");
        !state.outstanding_data.is_empty()
            || !state.pending_data_translations.is_empty()
            || !state.ready_translated_data.is_empty()
    }

    fn has_outstanding_data_request(&self) -> bool {
        !self
            .state
            .lock()
            .expect("riscv core lock")
            .outstanding_data
            .is_empty()
    }

    pub fn has_unissued_data_access(&self) -> bool {
        self.next_unissued_data_access().is_some()
    }

    pub fn write_register(&self, register: Register, value: u64) {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .write(register, value);
    }

    pub fn redirect_pc(&self, pc: Address) {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .set_pc(pc.get());
        self.core.set_pc(pc);
    }

    pub fn add_memory_line_layout_range(&self, range: AddressRange, line_layout: CacheLineLayout) {
        self.core.add_fetch_line_layout_range(range, line_layout);
        if let Some(data) = &mut self.state.lock().expect("riscv core lock").data {
            data.add_line_layout_range(range, line_layout);
        }
    }

    pub fn execution_events(&self) -> Vec<RiscvCpuExecutionEvent> {
        self.state.lock().expect("riscv core lock").events.clone()
    }

    pub fn data_access_events(&self) -> Vec<RiscvDataAccessEvent> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data_events
            .clone()
    }

    pub fn load_reservation(&self) -> Option<RiscvLoadReservation> {
        self.state.lock().expect("riscv core lock").reservation
    }

    pub(crate) fn invalidate_load_reservation_if_overlaps(
        &self,
        address: Address,
        size: AccessSize,
    ) -> Option<RiscvLoadReservation> {
        let mut state = self.state.lock().expect("riscv core lock");
        let reservation = state.reservation?;
        if !reservation.overlaps(address, size) {
            return None;
        }
        state.reservation = None;
        Some(reservation)
    }

    pub fn issue_next_fetch<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        self.core
            .issue_next_fetch(scheduler, transport, trace, responder)
    }

    pub fn issue_next_fetch_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<PartitionEventId, CpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        self.core
            .issue_next_fetch_parallel(scheduler, transport, trace, responder)
    }

    fn prepare_fetch_parallel_transaction<F>(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<(OutstandingFetch, ParallelMemoryTransaction), CpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        self.core
            .prepare_fetch_parallel_transaction(tick, transport, trace, responder)
    }

    fn record_prepared_fetch_issue(&self, issue: OutstandingFetch) {
        self.core.record_issue(issue);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_next_action<F, D>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        D: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        if self.core.has_pending_fetch() || self.has_pending_data_access() {
            return Ok(None);
        }
        if self.has_pending_trap() {
            return Ok(None);
        }

        if let Some(event) = self.execute_next_completed_fetch()? {
            return Ok(Some(RiscvCoreDriveAction::InstructionExecuted(Box::new(
                event,
            ))));
        }

        if let Some(event) =
            self.issue_next_data_access(scheduler, transport, data_trace, data_responder)?
        {
            return Ok(Some(RiscvCoreDriveAction::DataAccessIssued { event }));
        }

        let event = self
            .issue_next_fetch(scheduler, transport, fetch_trace, fetch_responder)
            .map_err(RiscvCpuError::Cpu)?;
        Ok(Some(RiscvCoreDriveAction::FetchIssued { event }))
    }

    pub fn execute_next_completed_fetch(
        &self,
    ) -> Result<Option<RiscvCpuExecutionEvent>, RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some() {
            return Ok(None);
        }
        let Some(fetch) = fetch_events.into_iter().find(|event| {
            event.kind() == CpuFetchEventKind::Completed
                && !state.executed_fetches.contains(&event.request_id())
        }) else {
            return Ok(None);
        };

        let architectural = Address::new(state.hart.pc());
        if fetch.pc() != architectural {
            return Err(RiscvCpuError::PcMismatch {
                fetch: fetch.pc(),
                architectural,
            });
        }

        let data = fetch.data().ok_or(RiscvCpuError::MissingFetchData {
            request: fetch.request_id(),
        })?;
        if data.len() != 4 {
            return Err(RiscvCpuError::InvalidFetchWidth {
                request: fetch.request_id(),
                bytes: data.len() as u64,
            });
        }
        let raw = u32::from_le_bytes(data.try_into().expect("fetch width checked"));
        let instruction = RiscvInstruction::decode(raw).map_err(RiscvCpuError::Isa)?;
        let execution = state
            .hart
            .execute(instruction)
            .map_err(RiscvCpuError::Isa)?;
        let next_pc = Address::new(execution.next_pc());
        self.core.set_pc(next_pc);
        if let Some(trap) = execution.trap().copied() {
            state.pending_trap = Some(trap);
        }

        let event = RiscvCpuExecutionEvent::new(fetch.clone(), instruction, execution);
        state.executed_fetches.insert(fetch.request_id());
        state.events.push(event.clone());
        Ok(Some(event))
    }

    fn next_unissued_data_access(&self) -> Option<(MemoryRequestId, MemoryAccessKind)> {
        let state = self.state.lock().expect("riscv core lock");
        state.next_unissued_data_access()
    }

    fn data_translation_page_map_required_fetch(&self) -> Option<MemoryRequestId> {
        let state = self.state.lock().expect("riscv core lock");
        state.data_translation.as_ref()?;
        state
            .next_unissued_data_access()
            .map(|(fetch_request, _access)| fetch_request)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvCoreState {
    hart: RiscvHartState,
    data: Option<CpuDataConfig>,
    data_translation: Option<CpuTranslationFrontend>,
    executed_fetches: BTreeSet<MemoryRequestId>,
    issued_data_for_fetches: BTreeSet<MemoryRequestId>,
    pending_data_translations:
        BTreeMap<TranslationRequestId, riscv_translation::PendingDataTranslation>,
    ready_translated_data: BTreeMap<MemoryRequestId, riscv_translation::TranslatedDataAccess>,
    outstanding_data: BTreeMap<MemoryRequestId, riscv_data_issue::IssuedDataAccess>,
    pending_trap: Option<RiscvTrap>,
    reservation: Option<RiscvLoadReservation>,
    events: Vec<RiscvCpuExecutionEvent>,
    data_events: Vec<RiscvDataAccessEvent>,
}

impl RiscvCoreState {
    fn new(pc: u64) -> Self {
        Self {
            hart: RiscvHartState::new(pc),
            data: None,
            data_translation: None,
            executed_fetches: BTreeSet::new(),
            issued_data_for_fetches: BTreeSet::new(),
            pending_data_translations: BTreeMap::new(),
            ready_translated_data: BTreeMap::new(),
            outstanding_data: BTreeMap::new(),
            pending_trap: None,
            reservation: None,
            events: Vec::new(),
            data_events: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvCpuExecutionEvent {
    fetch: CpuFetchEvent,
    instruction: RiscvInstruction,
    execution: RiscvExecutionRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvCoreDriveAction {
    FetchIssued { event: PartitionEventId },
    InstructionExecuted(Box<RiscvCpuExecutionEvent>),
    DataAccessIssued { event: PartitionEventId },
}

impl RiscvCpuExecutionEvent {
    pub const fn new(
        fetch: CpuFetchEvent,
        instruction: RiscvInstruction,
        execution: RiscvExecutionRecord,
    ) -> Self {
        Self {
            fetch,
            instruction,
            execution,
        }
    }

    pub fn fetch(&self) -> &CpuFetchEvent {
        &self.fetch
    }

    pub fn fetch_pc(&self) -> Address {
        self.fetch.pc()
    }

    pub const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub fn execution(&self) -> &RiscvExecutionRecord {
        &self.execution
    }
}

pub fn is_fetch_request(request: &MemoryRequest) -> bool {
    request.operation() == MemoryOperation::InstructionFetch
}
