use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_isa_riscv::{
    FloatRegister, MemoryAccessKind, Register, RiscvHartState, RiscvPmaError, RiscvPmaRange,
    RiscvPmaTable, RiscvPmpConfig, RiscvPmpError, RiscvPmpSnapshot, RiscvPmpTable,
    RiscvPrivilegeMode, RiscvTrap, RiscvTrapKind,
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
mod fetch_event;
mod gshare_predictor;
mod htm_transaction;
mod in_order_pipeline;
mod indirect_target_predictor;
mod loop_predictor;
mod ltage_predictor;
mod multiperspective_perceptron;
mod o3_dependency;
mod o3_pipeline;
mod parallel_flow;
mod riscv_activity;
mod riscv_cluster;
mod riscv_cluster_drive;
mod riscv_cluster_error;
mod riscv_cluster_htm;
mod riscv_cluster_run;
mod riscv_cluster_scheduler;
mod riscv_data_access;
mod riscv_data_issue;
mod riscv_drive;
mod riscv_execute;
mod riscv_execution_event;
mod riscv_fetch;
mod riscv_fetch_ahead;
mod riscv_hart_run_state;
mod riscv_htm;
mod riscv_reservation;
mod riscv_sc_progress;
mod riscv_sv39_memory_walker;
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
    BranchPredictorSnapshot, BranchSpeculation, BranchSpeculationDiscard, BranchSpeculationId,
    BranchSpeculationRepair, BranchTargetBuffer, BranchTargetBufferConfig, BranchTargetBufferError,
    BranchTargetBufferSnapshot, BranchTargetEntry, BranchTargetKind, BranchTargetLookup,
    BranchTargetSafetyConfig, BranchTargetSafetyProfile, BranchTargetUpdate, BranchUpdate,
    ReturnAddressStack, ReturnAddressStackConfig, ReturnAddressStackError,
    ReturnAddressStackOperation, ReturnAddressStackOperationId, ReturnAddressStackOperationKind,
    ReturnAddressStackRepair, ReturnAddressStackSnapshot,
};
pub use data_config::CpuDataConfig;
pub use error::{CpuClusterError, CpuError, RiscvCpuError};
pub use fetch_config::CpuFetchConfig;
pub use fetch_event::{CpuFetchEvent, CpuFetchEventKind, CpuFetchRecord};
pub use gshare_predictor::{
    GShareBranchPredictor, GShareBranchPredictorConfig, GShareBranchPredictorError,
    GShareBranchPredictorSnapshot, GShareHistory, GShareHistoryUpdate, GSharePrediction,
    GShareSquash, GShareThreadSnapshot, GShareTrainingUpdate,
};
pub use htm_transaction::{
    HtmAbortRecord, HtmActiveTransactionSnapshot, HtmBeginRecord, HtmCommitRecord, HtmFailureCause,
    HtmTransactionError, HtmTransactionSnapshot, HtmTransactionState, HtmTransactionUid,
};
pub use in_order_pipeline::{
    InOrderBranchPrediction, InOrderBranchPredictionRecord, InOrderBranchRedirect,
    InOrderPipelineAdvance, InOrderPipelineCheckpointPayload, InOrderPipelineConfig,
    InOrderPipelineCycleRecord, InOrderPipelineCycleSummary, InOrderPipelineError,
    InOrderPipelineInstruction, InOrderPipelinePlan, InOrderPipelineRunSummary,
    InOrderPipelineScheduler, InOrderPipelineSnapshot, InOrderPipelineStage,
    InOrderPipelineStageWidth, InOrderPipelineState,
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
    O3DependencyScopeId, O3DistributedIssuePlan, O3DistributedIssueScheduler, O3IssueOpClass,
    O3IssueQueueCapacity, O3IssueQueueId, O3PipelineError, O3PipelineStage, O3ReadyInstruction,
    O3ScopedIssuePlan, O3ScopedIssueScheduler, O3ScopedReadyInstruction, O3UnblockDecision,
    O3UnblockDecisionReason, O3UnblockPolicy, O3VectorReductionDependencyPlan,
    O3VectorReductionGroupId, O3VectorReductionMicroOp, O3VectorReductionOrdering,
    O3WritebackAdmission, O3WritebackCompletion, O3WritebackCompletionAdmission,
    O3WritebackTransferBuffer, O3WritebackTransferCheckpointPayload, O3WritebackTransferCycle,
    O3WritebackTransferPlan, O3WritebackTransferPolicy, O3WritebackTransferSnapshot,
};
pub use riscv_activity::RiscvCoreDriveActivity;
pub use riscv_cluster::{
    RiscvCluster, RiscvClusterError, RiscvClusterHtmAbortOutcome, RiscvClusterHtmBeginOutcome,
};
pub use riscv_cluster_run::{
    RiscvClusterDriveEvent, RiscvClusterParallelBatchTimelineRecord, RiscvClusterRun,
    RiscvClusterSchedulerEpoch, RiscvClusterStopReason, RiscvClusterTurn,
};
pub use riscv_data_access::{
    RiscvDataAccessEvent, RiscvDataAccessEventKind, RiscvDataAccessRecord, RiscvDataAccessTarget,
    RiscvLoadReservation,
};
pub use riscv_execution_event::{
    RiscvCoreDriveAction, RiscvCpuExecutionEvent, RiscvGShareBranchUpdate,
    RiscvTournamentBranchUpdate,
};
pub use riscv_hart_run_state::RiscvHartRunState;
pub use riscv_sc_progress::{
    RiscvStoreConditionalFailureDiagnostic, RiscvStoreConditionalFailureStreak,
    RiscvStoreConditionalProgress, RiscvStoreConditionalProgressCheckpointPayload,
    RiscvStoreConditionalProgressConfig, RiscvStoreConditionalProgressError,
    RiscvStoreConditionalProgressSnapshot, DEFAULT_RISCV_SC_DIAGNOSTIC_THRESHOLD,
};
pub use riscv_sv39_memory_walker::{
    RiscvSv39MemoryWalker, RiscvSv39MemoryWalkerAdvance, RiscvSv39MemoryWalkerError,
    RiscvSv39MemoryWalkerParallelSubmission,
};
pub use riscv_translation::{
    decode_sv39_pte_read_response, RiscvSv39MemoryWalk, RiscvSv39MemoryWalkAdvance,
    RiscvSv39MemoryWalkError, RiscvSv39PageTableResolver, RiscvSv39PteReadRequestError,
    RiscvSv39PteReadResponseError, RiscvSv39TranslationResult,
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

pub const DEFAULT_RISCV_PMP_ENTRIES: usize = 16;
pub const DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_GSHARE_BRANCH_PREDICTOR_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_LOCAL_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_LOCAL_HISTORY_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_GLOBAL_ENTRIES: usize = 1024;
pub const DEFAULT_RISCV_TOURNAMENT_CHOICE_ENTRIES: usize = 1024;
pub const RISCV_LOCAL_GSHARE_THREAD: CpuId = CpuId::new(0);
pub const RISCV_LOCAL_TOURNAMENT_THREAD: CpuId = CpuId::new(0);

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

    fn reset_fetch_stream_to_pc(&self, pc: Address) {
        let mut state = self.state.lock().expect("cpu core lock");
        state.pc = pc;
        state.outstanding.clear();
        state.events.clear();
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

    pub(crate) fn prepare_fetch(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<OutstandingFetch, CpuError> {
        self.prepare_fetch_with_size(tick, transport, None)
    }

    pub(crate) fn prepare_fetch_with_explicit_size(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        size: AccessSize,
    ) -> Result<OutstandingFetch, CpuError> {
        self.prepare_fetch_with_size(tick, transport, Some(size))
    }

    fn prepare_fetch_with_size(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        size: Option<AccessSize>,
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

        let size = size.unwrap_or_else(|| state.fetch.width());
        let range = AddressRange::new(state.pc, size).map_err(CpuError::Memory)?;
        let line_layout = state.fetch.line_layout_for_range(range);
        let line_offset = line_layout.line_offset(state.pc);
        if line_offset + size.bytes() > line_layout.bytes() {
            return Err(CpuError::FetchCrossesLine {
                pc: state.pc,
                size,
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
            size,
            line_layout,
        })
    }

    pub(crate) fn record_issue(&self, issue: OutstandingFetch) {
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

    pub(crate) fn record_response(&self, delivery: ResponseDelivery) {
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
            ResponseStatus::Retry | ResponseStatus::StoreConditionalFailed => {
                state.events.push(CpuFetchEvent::retry(fetch.record(
                    delivery.tick(),
                    delivery.route(),
                    delivery.endpoint().clone(),
                )));
            }
        }
    }

    pub(crate) fn discard_outstanding_fetches<I>(&self, request_ids: I)
    where
        I: IntoIterator<Item = MemoryRequestId>,
    {
        let mut state = self.state.lock().expect("cpu core lock");
        for request_id in request_ids {
            state.outstanding.remove(&request_id);
        }
    }

    pub(crate) fn record_fetch_failure(
        &self,
        request_id: MemoryRequestId,
        tick: Tick,
        route: MemoryRouteId,
        endpoint: TransportEndpointId,
    ) {
        let mut state = self.state.lock().expect("cpu core lock");
        let Some(fetch) = state.outstanding.remove(&request_id) else {
            return;
        };
        state
            .events
            .push(CpuFetchEvent::failed(fetch.record(tick, route, endpoint)));
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
pub(crate) struct OutstandingFetch {
    pub(crate) tick: Tick,
    pub(crate) partition: PartitionId,
    pub(crate) route: MemoryRouteId,
    pub(crate) endpoint: TransportEndpointId,
    pub(crate) request_id: MemoryRequestId,
    pub(crate) pc: Address,
    pub(crate) size: AccessSize,
    pub(crate) line_layout: CacheLineLayout,
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

#[derive(Clone, Debug)]
pub struct RiscvCore {
    core: CpuCore,
    state: Arc<Mutex<RiscvCoreState>>,
}

impl RiscvCore {
    pub fn new(core: CpuCore) -> Self {
        let pc = core.pc().get();
        let hart_id = u64::from(core.id().get());
        Self {
            core,
            state: Arc::new(Mutex::new(RiscvCoreState::new(pc, hart_id))),
        }
    }

    pub fn with_data(core: CpuCore, data: CpuDataConfig) -> Self {
        let core = Self::new(core);
        core.state.lock().expect("riscv core lock").data = Some(data);
        core
    }

    pub fn with_data_and_store_conditional_progress_config(
        core: CpuCore,
        data: CpuDataConfig,
        sc_progress_config: RiscvStoreConditionalProgressConfig,
    ) -> Self {
        let core = Self::with_data(core, data);
        core.state.lock().expect("riscv core lock").sc_progress =
            RiscvStoreConditionalProgress::new(sc_progress_config);
        core
    }

    pub fn inner(&self) -> CpuCore {
        self.core.clone()
    }

    pub fn id(&self) -> CpuId {
        self.core.id()
    }

    pub fn hart_id(&self) -> u64 {
        self.state.lock().expect("riscv core lock").hart.hart_id()
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

    pub fn read_float_register(&self, register: FloatRegister) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .read_float(register)
    }

    pub fn add_pma_misaligned_range(&self, range: RiscvPmaRange) -> Result<(), RiscvPmaError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pma
            .add_misaligned_range(range)
    }

    pub fn pma_misaligned_ranges(&self) -> Vec<RiscvPmaRange> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pma
            .misaligned_ranges()
            .to_vec()
    }

    pub fn add_pma_uncacheable_range(&self, range: RiscvPmaRange) -> Result<(), RiscvPmaError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pma
            .add_uncacheable_range(range)
    }

    pub fn pma_uncacheable_ranges(&self) -> Vec<RiscvPmaRange> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pma
            .uncacheable_ranges()
            .to_vec()
    }

    pub fn pmp_entry_count(&self) -> usize {
        self.state
            .lock()
            .expect("riscv core lock")
            .pmp
            .entries()
            .len()
    }

    pub fn pmp_snapshot(&self) -> RiscvPmpSnapshot {
        self.state.lock().expect("riscv core lock").pmp.snapshot()
    }

    pub fn restore_pmp_snapshot(&self, snapshot: &RiscvPmpSnapshot) -> Result<(), RiscvPmpError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pmp
            .restore(snapshot)
    }

    pub fn default_in_order_pipeline_snapshot() -> InOrderPipelineSnapshot {
        InOrderPipelineState::new(default_riscv_in_order_pipeline_config()).snapshot()
    }

    pub fn restore_in_order_pipeline_snapshot(
        &self,
        snapshot: InOrderPipelineSnapshot,
    ) -> Result<(), InOrderPipelineError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .in_order_pipeline = InOrderPipelineState::restore(snapshot)?;
        Ok(())
    }

    pub(crate) fn sync_in_order_fetch_state(&self) -> Result<(), RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        sync_in_order_fetch_state(&mut state, &fetch_events)
    }

    pub fn write_pmp_config(
        &self,
        index: usize,
        config: RiscvPmpConfig,
    ) -> Result<(), RiscvPmpError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pmp
            .write_config(index, config)
    }

    pub fn write_pmp_config_bits(&self, index: usize, bits: u8) -> Result<(), RiscvPmpError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pmp
            .write_config_bits(index, bits)
    }

    pub fn write_pmp_addr(&self, index: usize, raw_addr: u64) -> Result<(), RiscvPmpError> {
        self.state
            .lock()
            .expect("riscv core lock")
            .pmp
            .write_addr(index, raw_addr)
    }

    pub fn pending_trap(&self) -> Option<RiscvTrap> {
        self.state.lock().expect("riscv core lock").pending_trap
    }

    pub fn has_pending_trap(&self) -> bool {
        self.pending_trap().is_some()
    }

    pub fn pending_trap_return_privilege_mode(&self) -> Option<RiscvPrivilegeMode> {
        let state = self.state.lock().expect("riscv core lock");
        state.pending_trap?;
        Some(match state.hart.privilege_mode() {
            RiscvPrivilegeMode::Machine => state.hart.status().mpp(),
            RiscvPrivilegeMode::Supervisor => state.hart.status().spp(),
            RiscvPrivilegeMode::User => RiscvPrivilegeMode::User,
        })
    }

    pub fn complete_pending_user_environment_call(&self, return_value: u64) -> Option<RiscvTrap> {
        let mut state = self.state.lock().expect("riscv core lock");
        let trap = state.pending_trap?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        let return_privilege = match state.hart.privilege_mode() {
            RiscvPrivilegeMode::Machine => state.hart.status().mpp(),
            RiscvPrivilegeMode::Supervisor => state.hart.status().spp(),
            RiscvPrivilegeMode::User => RiscvPrivilegeMode::User,
        };
        if return_privilege != RiscvPrivilegeMode::User {
            return None;
        }

        state.pending_trap = None;
        state.hart.set_privilege_mode(RiscvPrivilegeMode::User);
        state.hart.write(
            Register::new(10).expect("valid RISC-V integer register"),
            return_value,
        );
        let next_pc = Address::new(trap.pc().wrapping_add(4));
        state.hart.set_pc(next_pc.get());
        drop(state);
        self.core.set_pc(next_pc);
        Some(trap)
    }

    pub fn complete_pending_supervisor_environment_call(
        &self,
        error: u64,
        value: u64,
    ) -> Option<RiscvTrap> {
        let mut state = self.state.lock().expect("riscv core lock");
        let trap = state.pending_trap?;
        if !matches!(trap.kind(), RiscvTrapKind::EnvironmentCall) {
            return None;
        }
        let return_privilege = match state.hart.privilege_mode() {
            RiscvPrivilegeMode::Machine => state.hart.status().mpp(),
            RiscvPrivilegeMode::Supervisor => state.hart.status().spp(),
            RiscvPrivilegeMode::User => RiscvPrivilegeMode::User,
        };
        if return_privilege != RiscvPrivilegeMode::Supervisor {
            return None;
        }

        state.pending_trap = None;
        state
            .hart
            .set_privilege_mode(RiscvPrivilegeMode::Supervisor);
        state.hart.write(
            Register::new(10).expect("valid RISC-V integer register"),
            error,
        );
        state.hart.write(
            Register::new(11).expect("valid RISC-V integer register"),
            value,
        );
        let next_pc = Address::new(trap.pc().wrapping_add(4));
        state.hart.set_pc(next_pc.get());
        drop(state);
        self.core.set_pc(next_pc);
        Some(trap)
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

    pub fn write_float_register(&self, register: FloatRegister, value: u64) {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .write_float(register, value);
    }

    pub fn redirect_pc(&self, pc: Address) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_pc(pc.get());
        state.pending_fetch_prefix = None;
        state.discard_branch_speculations();
        drop(state);
        self.core.reset_fetch_stream_to_pc(pc);
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

    pub fn store_conditional_progress_snapshot(&self) -> RiscvStoreConditionalProgressSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .sc_progress
            .snapshot()
    }

    pub fn store_conditional_failure_streak(&self) -> Option<RiscvStoreConditionalFailureStreak> {
        self.state
            .lock()
            .expect("riscv core lock")
            .sc_progress
            .streak(self.id())
            .copied()
    }

    pub fn store_conditional_failure_diagnostics(
        &self,
    ) -> Vec<RiscvStoreConditionalFailureDiagnostic> {
        self.state
            .lock()
            .expect("riscv core lock")
            .sc_progress
            .diagnostics()
            .to_vec()
    }

    pub fn branch_predictor_snapshot(&self) -> BranchPredictorSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .branch_predictor
            .snapshot()
    }

    pub fn gshare_branch_predictor_snapshot(&self) -> GShareBranchPredictorSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .gshare_branch_predictor
            .snapshot()
    }

    pub fn tournament_branch_predictor_snapshot(&self) -> TournamentBranchPredictorSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .tournament_branch_predictor
            .snapshot()
    }

    pub fn in_order_pipeline_snapshot(&self) -> InOrderPipelineSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .in_order_pipeline
            .snapshot()
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

fn sync_in_order_fetch_state(
    state: &mut RiscvCoreState,
    fetch_events: &[CpuFetchEvent],
) -> Result<(), RiscvCpuError> {
    let failed_or_retried = fetch_events
        .iter()
        .filter(|event| {
            matches!(
                event.kind(),
                CpuFetchEventKind::Retry | CpuFetchEventKind::Failed
            )
        })
        .map(CpuFetchEvent::request_id)
        .collect::<BTreeSet<_>>();
    let failed_or_retried_sequences = failed_or_retried
        .iter()
        .map(|request| request.sequence())
        .collect::<BTreeSet<_>>();
    remove_fetch_sequences_from_pipeline(state, &failed_or_retried_sequences)?;
    let mut fetches = fetch_events
        .iter()
        .filter(|event| {
            !failed_or_retried.contains(&event.request_id())
                && !state.executed_fetches.contains(&event.request_id())
                && match event.kind() {
                    CpuFetchEventKind::Issued => event.size().bytes() == 4,
                    CpuFetchEventKind::Completed => {
                        event.data().is_some_and(|data| data.len() == 4)
                    }
                    CpuFetchEventKind::Retry | CpuFetchEventKind::Failed => false,
                }
        })
        .collect::<Vec<_>>();
    fetches.sort_by_key(|event| event.request_id().sequence());

    for fetch in fetches {
        state
            .in_order_pipeline
            .enqueue_fetch(fetch.request_id().sequence())
            .map_err(RiscvCpuError::InOrderPipeline)?;
    }
    Ok(())
}

fn remove_fetch_sequences_from_pipeline(
    state: &mut RiscvCoreState,
    sequences: &BTreeSet<u64>,
) -> Result<(), RiscvCpuError> {
    if sequences.is_empty() {
        return Ok(());
    }

    let retained = state
        .in_order_pipeline
        .in_flight()
        .iter()
        .copied()
        .filter(|instruction| !sequences.contains(&instruction.sequence()))
        .collect::<Vec<_>>();
    state
        .in_order_pipeline
        .replace_in_flight(retained)
        .map_err(RiscvCpuError::InOrderPipeline)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RiscvCoreState {
    hart: RiscvHartState,
    data: Option<CpuDataConfig>,
    data_translation: Option<CpuTranslationFrontend>,
    executed_fetches: BTreeSet<MemoryRequestId>,
    pending_fetch_prefix: Option<riscv_execute::RiscvPendingFetchPrefix>,
    issued_data_for_fetches: BTreeSet<MemoryRequestId>,
    pending_data_translations:
        BTreeMap<TranslationRequestId, riscv_translation::PendingDataTranslation>,
    ready_translated_data: BTreeMap<MemoryRequestId, riscv_translation::TranslatedDataAccess>,
    outstanding_data: BTreeMap<MemoryRequestId, riscv_data_issue::IssuedDataAccess>,
    pending_trap: Option<RiscvTrap>,
    pending_trap_event: Option<RiscvCpuExecutionEvent>,
    reservation: Option<RiscvLoadReservation>,
    sc_progress: RiscvStoreConditionalProgress,
    htm: HtmTransactionState,
    htm_hart_checkpoint: Option<RiscvHartState>,
    branch_predictor: BranchPredictor,
    branch_speculations: BTreeMap<u64, BranchSpeculationId>,
    gshare_branch_predictor: GShareBranchPredictor,
    tournament_branch_predictor: TournamentBranchPredictor,
    in_order_pipeline: InOrderPipelineState,
    events: Vec<RiscvCpuExecutionEvent>,
    data_events: Vec<RiscvDataAccessEvent>,
    pma: RiscvPmaTable,
    pmp: RiscvPmpTable,
    run_state: RiscvHartRunState,
    run_state_explicit: bool,
}

impl RiscvCoreState {
    fn new(pc: u64, hart_id: u64) -> Self {
        Self {
            hart: RiscvHartState::with_hart_id(pc, hart_id),
            data: None,
            data_translation: None,
            executed_fetches: BTreeSet::new(),
            pending_fetch_prefix: None,
            issued_data_for_fetches: BTreeSet::new(),
            pending_data_translations: BTreeMap::new(),
            ready_translated_data: BTreeMap::new(),
            outstanding_data: BTreeMap::new(),
            pending_trap: None,
            pending_trap_event: None,
            reservation: None,
            sc_progress: RiscvStoreConditionalProgress::default(),
            htm: HtmTransactionState::new(),
            htm_hart_checkpoint: None,
            branch_predictor: BranchPredictor::new(
                BranchPredictorConfig::new(DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES)
                    .expect("default RISC-V branch predictor entries are valid"),
            ),
            branch_speculations: BTreeMap::new(),
            gshare_branch_predictor: GShareBranchPredictor::new(
                GShareBranchPredictorConfig::new(1, DEFAULT_RISCV_GSHARE_BRANCH_PREDICTOR_ENTRIES)
                    .expect("default RISC-V gshare branch predictor config is valid"),
            ),
            tournament_branch_predictor: TournamentBranchPredictor::new(
                TournamentBranchPredictorConfig::new(
                    1,
                    DEFAULT_RISCV_TOURNAMENT_LOCAL_ENTRIES,
                    DEFAULT_RISCV_TOURNAMENT_LOCAL_HISTORY_ENTRIES,
                    DEFAULT_RISCV_TOURNAMENT_GLOBAL_ENTRIES,
                    DEFAULT_RISCV_TOURNAMENT_CHOICE_ENTRIES,
                )
                .expect("default RISC-V tournament branch predictor config is valid"),
            ),
            in_order_pipeline: InOrderPipelineState::new(default_riscv_in_order_pipeline_config()),
            events: Vec::new(),
            data_events: Vec::new(),
            pma: RiscvPmaTable::new(),
            pmp: RiscvPmpTable::new(DEFAULT_RISCV_PMP_ENTRIES)
                .expect("default RISC-V PMP entry count is valid"),
            run_state: RiscvHartRunState::Started,
            run_state_explicit: false,
        }
    }

    fn discard_branch_speculations(&mut self) {
        self.branch_predictor.discard_all_speculations();
        self.branch_speculations.clear();
    }
}

fn default_riscv_in_order_pipeline_config() -> InOrderPipelineConfig {
    InOrderPipelineConfig::new([
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Fetch1, 1)
            .expect("default RISC-V fetch1 width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Fetch2, 1)
            .expect("default RISC-V fetch2 width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Decode, 1)
            .expect("default RISC-V decode width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Execute, 1)
            .expect("default RISC-V execute width is valid"),
        InOrderPipelineStageWidth::new(InOrderPipelineStage::Commit, 1)
            .expect("default RISC-V commit width is valid"),
    ])
    .expect("default RISC-V in-order pipeline config covers every stage")
}

pub fn is_fetch_request(request: &MemoryRequest) -> bool {
    request.operation() == MemoryOperation::InstructionFetch
}
