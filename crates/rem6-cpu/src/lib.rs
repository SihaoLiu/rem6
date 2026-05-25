use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_boot::BootImage;
use rem6_isa_riscv::{
    AtomicMemoryOp, MemoryAccessKind, MemoryWidth, Register, RiscvExecutionRecord, RiscvHartState,
    RiscvInstruction, RiscvTrap,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler,
    SchedulerContext, Tick,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, AgentId, ByteMask, CacheLineLayout, MemoryAtomicOp,
    MemoryOperation, MemoryRequest, MemoryRequestId, ResponseStatus, TranslationRequestId,
};
use rem6_mmio::{MmioBus, MmioCompletion, MmioError, MmioRequest, MmioRequestId};
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
mod riscv_activity;
mod riscv_cluster;
mod riscv_data_access;
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
    BranchTargetUpdate, BranchUpdate, ReturnAddressStack, ReturnAddressStackConfig,
    ReturnAddressStackError, ReturnAddressStackOperation, ReturnAddressStackOperationId,
    ReturnAddressStackOperationKind, ReturnAddressStackRepair, ReturnAddressStackSnapshot,
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
pub use riscv_activity::RiscvCoreDriveActivity;
pub use riscv_cluster::{
    RiscvCluster, RiscvClusterDriveEvent, RiscvClusterError, RiscvClusterRun,
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

    pub fn issue_next_data_access<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let Some(issue) = self.prepare_data_access(scheduler.now(), transport)? else {
            return Ok(None);
        };
        if self.store_conditional_fails(&issue) {
            return self
                .schedule_store_conditional_failure(scheduler, issue)
                .map(Some);
        }
        let request = issue.memory_request()?;

        let core = self.clone();
        let event = transport
            .submit(
                scheduler,
                issue.memory_route(),
                request,
                trace,
                responder,
                move |delivery| core.record_data_response(delivery),
            )
            .map_err(RiscvCpuError::Transport)?;

        self.record_data_issue(issue);
        Ok(Some(event))
    }

    pub fn issue_next_data_access_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(prepared) =
            self.prepare_data_parallel_access(scheduler.now(), transport, trace, responder)?
        else {
            return Ok(None);
        };

        match prepared {
            PreparedDataParallelAccess::Transaction { issue, transaction } => {
                let event = transport
                    .submit_parallel_batch(scheduler, [transaction])
                    .map_err(RiscvCpuError::Transport)?
                    .into_iter()
                    .next()
                    .expect("single data transaction returns one event");

                self.record_data_issue(issue);
                Ok(Some(event))
            }
            PreparedDataParallelAccess::ConditionalFailed { issue } => self
                .schedule_store_conditional_failure_parallel(scheduler, issue)
                .map(Some),
        }
    }

    fn prepare_data_parallel_access<F>(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<PreparedDataParallelAccess>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(issue) = self.prepare_data_access(tick, transport)? else {
            return Ok(None);
        };
        if self.store_conditional_fails(&issue) {
            return Ok(Some(PreparedDataParallelAccess::ConditionalFailed {
                issue,
            }));
        }
        let request = issue.memory_request()?;
        let core = self.clone();
        let transaction = ParallelMemoryTransaction::new(
            issue.memory_route(),
            request,
            trace,
            responder,
            move |delivery| core.record_data_response(delivery),
        );
        Ok(Some(PreparedDataParallelAccess::Transaction {
            issue,
            transaction,
        }))
    }

    fn record_prepared_data_issue(&self, issue: OutstandingDataAccess) {
        self.record_data_issue(issue);
    }

    fn schedule_prepared_store_conditional_failure_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        self.schedule_store_conditional_failure_parallel(scheduler, issue)
    }

    pub fn issue_next_mmio_data_access_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        bus: &MmioBus,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError> {
        let Some(issue) = self.prepare_mmio_data_access(scheduler.now(), bus)? else {
            return Ok(None);
        };
        if self.store_conditional_fails(&issue) {
            return self
                .schedule_store_conditional_failure_parallel(scheduler, issue)
                .map(Some);
        }
        let request = issue.mmio_request()?;
        let bus = bus.clone();
        let core = self.clone();
        let request_id = issue.request_id;
        let event = scheduler
            .schedule_parallel_at(self.partition(), scheduler.now(), move |context| {
                bus.submit_parallel(context, request, move |completion| {
                    core.record_mmio_completion(request_id, completion);
                })
                .expect("validated parallel MMIO data access submission");
            })
            .map_err(RiscvCpuError::Scheduler)?;

        self.record_data_issue(issue);
        Ok(Some(event))
    }

    fn prepare_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        if let Some(fetch) = self.data_translation_page_map_required_fetch() {
            return Err(RiscvCpuError::DataTranslationPageMapRequired { fetch });
        }
        let Some((fetch_request, access)) = self.next_unissued_data_access() else {
            return Ok(None);
        };

        let state = self.state.lock().expect("riscv core lock");
        let data = state.data.clone().ok_or(RiscvCpuError::MissingDataConfig {
            fetch: fetch_request,
        })?;
        drop(state);
        let route = transport
            .route(data.route())
            .ok_or(RiscvCpuError::Transport(TransportError::UnknownRoute {
                route: data.route(),
            }))?;
        if route.source_partition() != self.core.partition() {
            return Err(RiscvCpuError::DataRoutePartitionMismatch {
                route: data.route(),
                expected: self.core.partition(),
                actual: route.source_partition(),
            });
        }
        if route.source() != data.endpoint() {
            return Err(RiscvCpuError::DataRouteEndpointMismatch {
                route: data.route(),
                expected: data.endpoint().clone(),
                actual: route.source().clone(),
            });
        }

        let size = memory_width_size(access_width(&access))?;
        let address = Address::new(access_address(&access));
        let line_layout = data
            .line_layout_for_access(address, size)
            .map_err(RiscvCpuError::Memory)?;
        let line_offset = line_layout.line_offset(address);
        if line_offset + size.bytes() > line_layout.bytes() {
            return Err(RiscvCpuError::DataAccessCrossesLine {
                address,
                size,
                line_size: line_layout.bytes(),
            });
        }

        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());

        Ok(Some(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Memory {
                route: data.route(),
                endpoint: data.endpoint().clone(),
            },
            request_id,
            fetch_request,
            access,
            size,
            physical_address: address,
            line_layout: Some(line_layout),
        }))
    }

    fn prepare_mmio_data_access(
        &self,
        tick: Tick,
        bus: &MmioBus,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        if let Some(fetch) = self.data_translation_page_map_required_fetch() {
            return Err(RiscvCpuError::DataTranslationPageMapRequired { fetch });
        }
        let Some((fetch_request, access)) = self.next_unissued_data_access() else {
            return Ok(None);
        };
        let size = memory_width_size(access_width(&access))?;
        let address = Address::new(access_address(&access));
        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());
        let request = mmio_request(request_id, &access, size, address)?;
        let route = match bus.route_for(&request) {
            Ok(route) => route,
            Err(MmioError::UnmappedAddress { .. }) => return Ok(None),
            Err(error) => return Err(RiscvCpuError::Mmio(error)),
        };
        if route.source_partition() != self.core.partition() {
            return Err(RiscvCpuError::MmioRoutePartitionMismatch {
                expected: self.core.partition(),
                actual: route.source_partition(),
            });
        }

        Ok(Some(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Mmio { route },
            request_id,
            fetch_request,
            access,
            size,
            physical_address: address,
            line_layout: None,
        }))
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

    fn record_data_issue(&self, issue: OutstandingDataAccess) {
        self.core.advance_sequence_past(issue.request_id);
        let mut state = self.state.lock().expect("riscv core lock");
        state.issued_data_for_fetches.insert(issue.fetch_request);
        state
            .outstanding_data
            .insert(issue.request_id, issue.clone_without_layout());
        state
            .data_events
            .push(RiscvDataAccessEvent::issued(issue.record(issue.tick)));
    }

    fn schedule_store_conditional_failure(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let request_id = issue.request_id;
        let core = self.clone();
        let event = scheduler
            .schedule_at(self.partition(), scheduler.now(), move |context| {
                core.record_store_conditional_failure(request_id, context.now());
            })
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_data_issue(issue);
        Ok(event)
    }

    fn schedule_store_conditional_failure_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        issue: OutstandingDataAccess,
    ) -> Result<PartitionEventId, RiscvCpuError> {
        let request_id = issue.request_id;
        let core = self.clone();
        let event = scheduler
            .schedule_parallel_at(self.partition(), scheduler.now(), move |context| {
                core.record_store_conditional_failure(request_id, context.now());
            })
            .map_err(RiscvCpuError::Scheduler)?;
        self.record_data_issue(issue);
        Ok(event)
    }

    fn store_conditional_fails(&self, issue: &OutstandingDataAccess) -> bool {
        if !matches!(issue.access, MemoryAccessKind::StoreConditional { .. }) {
            return false;
        }
        let expected = RiscvLoadReservation::new(issue.physical_address, issue.size);
        self.state.lock().expect("riscv core lock").reservation != Some(expected)
    }

    fn record_store_conditional_failure(&self, request_id: MemoryRequestId, tick: Tick) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(access) = state.outstanding_data.remove(&request_id) else {
            return;
        };
        let MemoryAccessKind::StoreConditional { rd, .. } = &access.access else {
            debug_assert!(
                false,
                "store-conditional failure recorded for non-SC access"
            );
            return;
        };
        state.hart.write(*rd, 1);
        state.reservation = None;
        state
            .data_events
            .push(RiscvDataAccessEvent::conditional_failed(
                access.record(tick),
            ));
    }

    fn record_data_response(&self, delivery: ResponseDelivery) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(access) = state
            .outstanding_data
            .remove(&delivery.response().request_id())
        else {
            return;
        };

        match delivery.response().status() {
            ResponseStatus::Completed => {
                let data = delivery.response().data().map(ToOwned::to_owned);
                record_load_completion(&mut state, &access, data.as_deref(), "load response data");
                state.data_events.push(RiscvDataAccessEvent::completed(
                    access.record(delivery.tick()),
                    data,
                ));
            }
            ResponseStatus::Retry => {
                state
                    .data_events
                    .push(RiscvDataAccessEvent::retry(access.record(delivery.tick())));
            }
        }
    }

    fn record_mmio_completion(&self, request_id: MemoryRequestId, completion: MmioCompletion) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(access) = state.outstanding_data.remove(&request_id) else {
            return;
        };

        match completion.response() {
            Ok(response) => {
                let data = response.data().map(ToOwned::to_owned);
                record_load_completion(
                    &mut state,
                    &access,
                    data.as_deref(),
                    "MMIO load response data",
                );
                state.data_events.push(RiscvDataAccessEvent::completed(
                    access.record(completion.tick()),
                    data,
                ));
            }
            Err(_) => {
                state.data_events.push(RiscvDataAccessEvent::retry(
                    access.record(completion.tick()),
                ));
            }
        }
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
    outstanding_data: BTreeMap<MemoryRequestId, IssuedDataAccess>,
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
struct OutstandingDataAccess {
    tick: Tick,
    partition: PartitionId,
    target: RiscvDataAccessTarget,
    request_id: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    size: AccessSize,
    physical_address: Address,
    line_layout: Option<CacheLineLayout>,
}

impl OutstandingDataAccess {
    fn memory_route(&self) -> MemoryRouteId {
        match &self.target {
            RiscvDataAccessTarget::Memory { route, .. } => *route,
            RiscvDataAccessTarget::Mmio { .. } => {
                panic!("MMIO data access does not have a memory route")
            }
        }
    }

    fn memory_request(&self) -> Result<MemoryRequest, RiscvCpuError> {
        let line_layout = self.line_layout.expect("memory data access line layout");
        match &self.access {
            MemoryAccessKind::Load { .. } | MemoryAccessKind::LoadReserved { .. } => {
                MemoryRequest::read_shared(
                    self.request_id,
                    self.physical_address,
                    self.size,
                    line_layout,
                )
                .map_err(RiscvCpuError::Memory)
            }
            MemoryAccessKind::Store { value, .. } => MemoryRequest::write(
                self.request_id,
                self.physical_address,
                self.size,
                store_bytes(*value, self.size),
                ByteMask::full(self.size).map_err(RiscvCpuError::Memory)?,
                line_layout,
            )
            .map_err(RiscvCpuError::Memory),
            MemoryAccessKind::StoreConditional { value, .. } => MemoryRequest::atomic(
                self.request_id,
                self.physical_address,
                self.size,
                store_bytes(*value, self.size),
                ByteMask::full(self.size).map_err(RiscvCpuError::Memory)?,
                line_layout,
            )
            .map_err(RiscvCpuError::Memory),
            MemoryAccessKind::AtomicMemory { op, value, .. } => MemoryRequest::atomic_with_op(
                self.request_id,
                self.physical_address,
                self.size,
                match op {
                    AtomicMemoryOp::Swap => MemoryAtomicOp::Swap,
                    AtomicMemoryOp::Add => MemoryAtomicOp::Add,
                    AtomicMemoryOp::Xor => MemoryAtomicOp::Xor,
                    AtomicMemoryOp::Or => MemoryAtomicOp::Or,
                    AtomicMemoryOp::And => MemoryAtomicOp::And,
                    AtomicMemoryOp::MinSigned => MemoryAtomicOp::MinSigned,
                    AtomicMemoryOp::MaxSigned => MemoryAtomicOp::MaxSigned,
                    AtomicMemoryOp::MinUnsigned => MemoryAtomicOp::MinUnsigned,
                    AtomicMemoryOp::MaxUnsigned => MemoryAtomicOp::MaxUnsigned,
                },
                store_bytes(*value, self.size),
                ByteMask::full(self.size).map_err(RiscvCpuError::Memory)?,
                line_layout,
            )
            .map_err(RiscvCpuError::Memory),
        }
    }

    fn mmio_request(&self) -> Result<MmioRequest, RiscvCpuError> {
        mmio_request(
            self.request_id,
            &self.access,
            self.size,
            self.physical_address,
        )
    }

    fn clone_without_layout(&self) -> IssuedDataAccess {
        IssuedDataAccess {
            partition: self.partition,
            target: self.target.clone(),
            request: self.request_id,
            fetch_request: self.fetch_request,
            access: self.access.clone(),
            size: self.size,
            physical_address: self.physical_address,
        }
    }

    fn record(&self, tick: Tick) -> RiscvDataAccessRecord {
        self.clone_without_layout().record(tick)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct IssuedDataAccess {
    partition: PartitionId,
    target: RiscvDataAccessTarget,
    request: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    size: AccessSize,
    physical_address: Address,
}

impl IssuedDataAccess {
    fn record(&self, tick: Tick) -> RiscvDataAccessRecord {
        RiscvDataAccessRecord::new(
            tick,
            self.partition,
            self.target.clone(),
            self.request,
            self.fetch_request,
            self.access.clone(),
            self.size,
            self.physical_address,
        )
    }
}

enum PreparedDataParallelAccess {
    Transaction {
        issue: OutstandingDataAccess,
        transaction: ParallelMemoryTransaction,
    },
    ConditionalFailed {
        issue: OutstandingDataAccess,
    },
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

fn record_load_completion(
    state: &mut RiscvCoreState,
    access: &IssuedDataAccess,
    data: Option<&[u8]>,
    missing_data: &'static str,
) {
    match &access.access {
        MemoryAccessKind::Load {
            rd, width, signed, ..
        } => {
            let value = load_response_value(data.expect(missing_data), *width, *signed);
            state.hart.write(*rd, value);
        }
        MemoryAccessKind::LoadReserved { rd, width, .. } => {
            let value = load_response_value(data.expect(missing_data), *width, false);
            state.hart.write(*rd, value);
            state.reservation = Some(RiscvLoadReservation::new(
                access.physical_address,
                access.size,
            ));
        }
        MemoryAccessKind::StoreConditional { rd, .. } => {
            state.hart.write(*rd, 0);
            state.reservation = None;
        }
        MemoryAccessKind::AtomicMemory { rd, width, .. } => {
            let value = load_response_value(data.expect(missing_data), *width, false);
            state.hart.write(*rd, value);
        }
        MemoryAccessKind::Store { .. } => {}
    }
}

fn access_width(access: &MemoryAccessKind) -> MemoryWidth {
    match access {
        MemoryAccessKind::Load { width, .. }
        | MemoryAccessKind::LoadReserved { width, .. }
        | MemoryAccessKind::StoreConditional { width, .. }
        | MemoryAccessKind::AtomicMemory { width, .. }
        | MemoryAccessKind::Store { width, .. } => *width,
    }
}

fn access_address(access: &MemoryAccessKind) -> u64 {
    match access {
        MemoryAccessKind::Load { address, .. }
        | MemoryAccessKind::LoadReserved { address, .. }
        | MemoryAccessKind::StoreConditional { address, .. }
        | MemoryAccessKind::AtomicMemory { address, .. }
        | MemoryAccessKind::Store { address, .. } => *address,
    }
}

fn memory_width_size(width: MemoryWidth) -> Result<AccessSize, RiscvCpuError> {
    let bytes = match width {
        MemoryWidth::Byte => 1,
        MemoryWidth::Halfword => 2,
        MemoryWidth::Word => 4,
        MemoryWidth::Doubleword => 8,
    };
    AccessSize::new(bytes).map_err(RiscvCpuError::Memory)
}

fn store_bytes(value: u64, size: AccessSize) -> Vec<u8> {
    value.to_le_bytes()[..size.bytes() as usize].to_vec()
}

fn mmio_request(
    request: MemoryRequestId,
    access: &MemoryAccessKind,
    size: AccessSize,
    address: Address,
) -> Result<MmioRequest, RiscvCpuError> {
    match access {
        MemoryAccessKind::Load { .. } | MemoryAccessKind::LoadReserved { .. } => {
            MmioRequest::read(mmio_request_id(request), address, size).map_err(RiscvCpuError::Mmio)
        }
        MemoryAccessKind::AtomicMemory { .. } => {
            Err(RiscvCpuError::UnsupportedMmioAtomic { request, address })
        }
        MemoryAccessKind::Store { value, .. }
        | MemoryAccessKind::StoreConditional { value, .. } => MmioRequest::write(
            mmio_request_id(request),
            address,
            store_bytes(*value, size),
            ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
        )
        .map_err(RiscvCpuError::Mmio),
    }
}

fn mmio_request_id(request: MemoryRequestId) -> MmioRequestId {
    MmioRequestId::new(request.sequence())
}

fn load_response_value(data: &[u8], width: MemoryWidth, signed: bool) -> u64 {
    let raw = data.iter().enumerate().fold(0u64, |value, (shift, byte)| {
        value | (u64::from(*byte) << (shift * 8))
    });
    if !signed || width == MemoryWidth::Doubleword {
        return raw;
    }

    let bits = data.len() as u32 * 8;
    let sign_bit = 1u64 << (bits - 1);
    if raw & sign_bit == 0 {
        raw
    } else {
        raw | (!0u64 << bits)
    }
}

pub fn is_fetch_request(request: &MemoryRequest) -> bool {
    request.operation() == MemoryOperation::InstructionFetch
}
