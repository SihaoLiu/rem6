use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};

use rem6_isa_riscv::{
    FloatRegister, MemoryAccessKind, Register, RiscvCounterSnapshot, RiscvHartState, RiscvPmaError,
    RiscvPmaRange, RiscvPmaTable, RiscvPmpConfig, RiscvPmpError, RiscvPmpSnapshot, RiscvPmpTable,
    RiscvPrivilegeMode, RiscvTrap, RiscvVectorConfig, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
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
mod bimode_predictor_checkpoint;
mod branch_predictor;
mod branch_predictor_checkpoint;
mod cpu_cluster;
mod cpu_core_debug;
mod cpu_identity;
mod data_config;
mod error;
mod fetch_config;
mod fetch_event;
mod gshare_predictor;
mod gshare_predictor_checkpoint;
mod htm_transaction;
mod in_order_pipeline;
mod indirect_target_predictor;
mod loop_predictor;
mod ltage_predictor;
mod multiperspective_perceptron;
mod multiperspective_perceptron_checkpoint;
mod multiperspective_perceptron_snapshot;
mod o3_dependency;
mod o3_pipeline;
mod o3_runtime;
mod outstanding_fetch;
mod parallel_flow;
mod public_api;
mod return_address_stack;
mod riscv_activity;
mod riscv_bimode_checkpoint;
mod riscv_branch_kind;
mod riscv_branch_speculation;
mod riscv_checker;
mod riscv_cluster;
mod riscv_cluster_drive;
mod riscv_cluster_error;
mod riscv_cluster_htm;
mod riscv_cluster_run;
mod riscv_cluster_run_loop;
mod riscv_cluster_scheduler;
mod riscv_data_access;
mod riscv_data_issue;
mod riscv_defaults;
mod riscv_drive;
mod riscv_execute;
mod riscv_execution_event;
mod riscv_fetch;
mod riscv_fetch_ahead;
#[cfg(test)]
mod riscv_fetch_ahead_tage_sc_l_tests;
mod riscv_gshare_checkpoint;
mod riscv_hart_run_state;
mod riscv_htm;
mod riscv_in_order_config;
mod riscv_multiperspective_perceptron_checkpoint;
mod riscv_reservation;
mod riscv_sc_progress;
mod riscv_selected_branch_speculation;
mod riscv_sv39_memory_walker;
mod riscv_tage_sc_l_checkpoint;
mod riscv_tournament_checkpoint;
mod riscv_translation;
mod riscv_translation_state;
mod riscv_trap_completion;
mod statistical_corrector;
mod tage_predictor;
mod tage_sc_l_predictor;
mod tage_sc_l_predictor_checkpoint;
mod topology;
mod tournament_predictor;
mod tournament_predictor_checkpoint;
mod translation;

pub(crate) use outstanding_fetch::{IssuedFetch, OutstandingFetch};
pub(crate) use riscv_defaults::*;
pub(crate) use riscv_selected_branch_speculation::RiscvSelectedBranchSpeculation;

pub use public_api::*;
pub use riscv_defaults::*;

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

    pub fn fetch_history(&self) -> Vec<CpuFetchEvent> {
        self.state.lock().expect("cpu core lock").history.clone()
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
        let event = CpuFetchEvent::issued(CpuFetchRecord::new(
            issue.tick,
            issue.partition,
            issue.route,
            issue.endpoint,
            issue.request_id,
            issue.pc,
            issue.size,
        ));
        state.events.push(event.clone());
        state.history.push(event);
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
                let event = CpuFetchEvent::completed(
                    fetch.record(
                        delivery.tick(),
                        delivery.route(),
                        delivery.endpoint().clone(),
                    ),
                    data,
                );
                state.events.push(event.clone());
                state.history.push(event);
            }
            ResponseStatus::Retry | ResponseStatus::StoreConditionalFailed => {
                let event = CpuFetchEvent::retry(fetch.record(
                    delivery.tick(),
                    delivery.route(),
                    delivery.endpoint().clone(),
                ));
                state.events.push(event.clone());
                state.history.push(event);
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
        let event = CpuFetchEvent::failed(fetch.record(tick, route, endpoint));
        state.events.push(event.clone());
        state.history.push(event);
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
    history: Vec<CpuFetchEvent>,
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
            history: Vec::new(),
        }
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

    pub fn read_vector_register(
        &self,
        register: VectorRegister,
    ) -> [u8; RISCV_VECTOR_REGISTER_BYTES] {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .read_vector(register)
    }

    pub fn counter_snapshot(&self) -> RiscvCounterSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .counter_snapshot()
    }

    pub fn restore_counter_snapshot(&self, snapshot: &RiscvCounterSnapshot) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.restore_counter_snapshot(snapshot);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn write_vector_register(
        &self,
        register: VectorRegister,
        value: [u8; RISCV_VECTOR_REGISTER_BYTES],
    ) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.write_vector(register, value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn vector_config(&self) -> RiscvVectorConfig {
        self.state
            .lock()
            .expect("riscv core lock")
            .hart
            .vector_config()
    }

    pub fn set_vector_config(&self, vector_config: RiscvVectorConfig) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_vector_config(vector_config);
        riscv_checker::sync_checker_hart(&mut state);
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
        InOrderPipelineState::new(riscv_in_order_config::default_riscv_in_order_pipeline_config())
            .snapshot()
    }

    pub fn reset_in_order_pipeline_config(&self, config: InOrderPipelineConfig) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.in_order_pipeline = InOrderPipelineState::new(config);
        state.in_order_pipeline_cycle_records.clear();
    }

    pub fn restore_in_order_pipeline_snapshot(
        &self,
        snapshot: InOrderPipelineSnapshot,
    ) -> Result<(), InOrderPipelineError> {
        let restored = InOrderPipelineState::restore(snapshot)?;
        let restored_cycle = restored.snapshot().cycle();
        let mut state = self.state.lock().expect("riscv core lock");
        state.in_order_pipeline = restored;
        state
            .in_order_pipeline_cycle_records
            .retain(|record| record.cycle() < restored_cycle);
        Ok(())
    }

    pub(crate) fn sync_in_order_fetch_state(&self) -> Result<(), RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        riscv_execute::sync_in_order_fetch_state(&mut state, &fetch_events)
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
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.write(register, value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn write_float_register(&self, register: FloatRegister, value: u64) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.write_float(register, value);
        riscv_checker::sync_checker_hart(&mut state);
    }

    pub fn redirect_pc(&self, pc: Address) {
        let mut state = self.state.lock().expect("riscv core lock");
        state.hart.set_pc(pc.get());
        state.pending_fetch_prefix = None;
        state.discard_branch_speculations();
        riscv_checker::sync_checker_hart(&mut state);
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

    pub fn data_access_event_count(&self) -> usize {
        self.state
            .lock()
            .expect("riscv core lock")
            .data_events
            .len()
    }

    pub fn data_access_events_from(&self, cursor: usize) -> Vec<RiscvDataAccessEvent> {
        let state = self.state.lock().expect("riscv core lock");
        state.data_events.get(cursor..).unwrap_or_default().to_vec()
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

    pub fn branch_target_buffer_snapshot(&self) -> BranchTargetBufferSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .branch_target_buffer
            .snapshot()
    }

    pub fn branch_predictor_checkpoint_payload(&self) -> BranchPredictorCheckpointPayload {
        let state = self.state.lock().expect("riscv core lock");
        BranchPredictorCheckpointPayload::from_snapshots_with_branch_target_predictions_and_return_address_stack(
            state.branch_predictor.snapshot(),
            state.branch_target_buffer.snapshot(),
            state
                .branch_speculations
                .iter()
                .map(|(sequence, id)| (*sequence, *id)),
            state
                .branch_target_predictions
                .iter()
                .map(|(sequence, prediction)| (*sequence, *prediction)),
            state.return_address_stack.snapshot(),
            state
                .return_address_stack_operations
                .iter()
                .map(|(sequence, operation)| (*sequence, *operation)),
        )
        .expect("captured RISC-V branch predictor checkpoint is internally consistent")
    }

    pub fn default_branch_predictor_checkpoint_payload() -> BranchPredictorCheckpointPayload {
        BranchPredictorCheckpointPayload::from_snapshots(
            BranchPredictor::new(
                BranchPredictorConfig::new(DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES)
                    .expect("default RISC-V branch predictor entries are valid"),
            )
            .snapshot(),
            BranchTargetBuffer::new(
                BranchTargetBufferConfig::new(
                    DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES,
                    DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY,
                )
                .expect("default RISC-V branch target buffer config is valid"),
            )
            .snapshot(),
            [],
        )
        .expect("default RISC-V branch predictor checkpoint is valid")
    }

    pub fn restore_branch_predictor_checkpoint_payload(
        &self,
        payload: BranchPredictorCheckpointPayload,
    ) -> Result<(), BranchPredictorError> {
        let (
            snapshot,
            branch_target_buffer,
            return_address_stack,
            active_speculations,
            active_branch_target_predictions,
            active_return_address_stack_operations,
        ) = payload.into_parts_with_branch_target_predictions();
        let mut state = self.state.lock().expect("riscv core lock");
        let mut restored_branch_predictor = state.branch_predictor.clone();
        restored_branch_predictor.restore(&snapshot)?;
        let mut restored_branch_target_buffer = state.branch_target_buffer.clone();
        restored_branch_target_buffer
            .restore(&branch_target_buffer)
            .map_err(|error| BranchPredictorError::InvalidBranchTargetBufferCheckpoint { error })?;
        let mut restored_return_address_stack = state.return_address_stack.clone();
        restored_return_address_stack
            .restore(&return_address_stack)
            .map_err(|error| BranchPredictorError::InvalidReturnAddressStackCheckpoint { error })?;
        state
            .rollback_all_selected_branch_speculations()
            .expect("selected branch speculation rollback is internally consistent");
        state.branch_predictor = restored_branch_predictor;
        state.branch_target_buffer = restored_branch_target_buffer;
        state.return_address_stack = restored_return_address_stack;
        state.branch_speculations.clear();
        state.branch_speculations.extend(active_speculations);
        state.branch_target_predictions.clear();
        state
            .branch_target_predictions
            .extend(active_branch_target_predictions);
        state.return_address_stack_operations.clear();
        state
            .return_address_stack_operations
            .extend(active_return_address_stack_operations);
        Ok(())
    }

    pub fn validate_branch_predictor_checkpoint_payload(
        &self,
        payload: &BranchPredictorCheckpointPayload,
    ) -> Result<(), BranchPredictorError> {
        let state = self.state.lock().expect("riscv core lock");
        let mut branch_predictor = state.branch_predictor.clone();
        branch_predictor.restore(payload.snapshot())?;
        let mut branch_target_buffer = state.branch_target_buffer.clone();
        branch_target_buffer
            .restore(payload.branch_target_buffer_snapshot())
            .map_err(|error| BranchPredictorError::InvalidBranchTargetBufferCheckpoint { error })?;
        let mut return_address_stack = state.return_address_stack.clone();
        return_address_stack
            .restore(payload.return_address_stack_snapshot())
            .map_err(|error| BranchPredictorError::InvalidReturnAddressStackCheckpoint { error })
    }

    pub fn gshare_branch_predictor_snapshot(&self) -> GShareBranchPredictorSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .gshare_branch_predictor
            .snapshot()
    }

    pub fn bimode_branch_predictor_snapshot(&self) -> BiModeBranchPredictorSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .bimode_branch_predictor
            .snapshot()
    }

    pub fn tournament_branch_predictor_snapshot(&self) -> TournamentBranchPredictorSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .tournament_branch_predictor
            .snapshot()
    }

    pub fn multiperspective_perceptron_snapshot(&self) -> MultiperspectivePerceptronSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .multiperspective_perceptron
            .snapshot()
    }

    pub fn selected_multiperspective_perceptron_rollback_count(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .selected_multiperspective_perceptron_rollbacks
    }

    pub fn tage_sc_l_branch_predictor_snapshot(&self) -> TageScLBranchPredictorSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .tage_sc_l_branch_predictor
            .snapshot()
    }

    pub fn selected_tage_sc_l_branch_predictor_rollback_count(&self) -> u64 {
        self.state
            .lock()
            .expect("riscv core lock")
            .selected_tage_sc_l_branch_predictor_rollbacks
    }

    pub fn in_order_pipeline_snapshot(&self) -> InOrderPipelineSnapshot {
        self.state
            .lock()
            .expect("riscv core lock")
            .in_order_pipeline
            .snapshot()
    }

    pub fn in_order_pipeline_cycle_records(&self) -> Vec<InOrderPipelineCycleRecord> {
        self.state
            .lock()
            .expect("riscv core lock")
            .in_order_pipeline_cycle_records
            .clone()
    }

    pub fn branch_speculation_summary(&self) -> RiscvBranchSpeculationSummary {
        self.state
            .lock()
            .expect("riscv core lock")
            .branch_speculation_summary
    }

    pub fn set_branch_lookahead(&self, lookahead: usize) {
        self.state.lock().expect("riscv core lock").branch_lookahead = lookahead.max(1);
    }

    pub fn set_branch_predictor_kind(&self, kind: RiscvBranchPredictorKind) {
        self.state
            .lock()
            .expect("riscv core lock")
            .branch_predictor_kind = kind;
    }

    pub(crate) fn record_in_order_fetch_wait_stall_cycle(
        &self,
    ) -> Result<InOrderPipelineCycleRecord, RiscvCpuError> {
        let mut state = self.state.lock().expect("riscv core lock");
        let record = state
            .in_order_pipeline
            .try_record_resource_stall_cycle_with_cause(InOrderPipelineStallCause::FetchWait)
            .map_err(RiscvCpuError::InOrderPipeline)?;
        state.in_order_pipeline_cycle_records.push(record.clone());
        Ok(record)
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
    checker: Option<riscv_checker::RiscvCheckerCpu>,
    branch_predictor: BranchPredictor,
    branch_target_buffer: BranchTargetBuffer,
    return_address_stack: ReturnAddressStack,
    branch_speculations: BTreeMap<u64, BranchSpeculationId>,
    return_address_stack_operations: BTreeMap<u64, ReturnAddressStackOperationId>,
    selected_branch_speculations: BTreeMap<u64, RiscvSelectedBranchSpeculation>,
    selected_tage_sc_l_branch_predictor_rollbacks: u64,
    selected_multiperspective_perceptron_rollbacks: u64,
    branch_target_predictions: BTreeMap<u64, BranchTargetPrediction>,
    branch_speculation_summary: RiscvBranchSpeculationSummary,
    branch_lookahead: usize,
    branch_predictor_kind: RiscvBranchPredictorKind,
    gshare_branch_predictor: GShareBranchPredictor,
    bimode_branch_predictor: BiModeBranchPredictor,
    tournament_branch_predictor: TournamentBranchPredictor,
    tage_sc_l_branch_predictor: TageScLBranchPredictor,
    multiperspective_perceptron: MultiperspectivePerceptron,
    o3_runtime: o3_runtime::O3RuntimeState,
    in_order_pipeline: InOrderPipelineState,
    in_order_pipeline_cycle_records: Vec<InOrderPipelineCycleRecord>,
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
            checker: None,
            branch_predictor: BranchPredictor::new(
                BranchPredictorConfig::new(DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES)
                    .expect("default RISC-V branch predictor entries are valid"),
            ),
            branch_target_buffer: BranchTargetBuffer::new(
                BranchTargetBufferConfig::new(
                    DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ENTRIES,
                    DEFAULT_RISCV_BRANCH_TARGET_BUFFER_ASSOCIATIVITY,
                )
                .expect("default RISC-V branch target buffer config is valid"),
            ),
            return_address_stack: ReturnAddressStack::new(
                ReturnAddressStackConfig::new(DEFAULT_RISCV_RETURN_ADDRESS_STACK_ENTRIES)
                    .expect("default RISC-V return-address stack config is valid"),
            ),
            branch_speculations: BTreeMap::new(),
            return_address_stack_operations: BTreeMap::new(),
            selected_branch_speculations: BTreeMap::new(),
            selected_tage_sc_l_branch_predictor_rollbacks: 0,
            selected_multiperspective_perceptron_rollbacks: 0,
            branch_target_predictions: BTreeMap::new(),
            branch_speculation_summary: RiscvBranchSpeculationSummary::default(),
            branch_lookahead: 1,
            branch_predictor_kind: RiscvBranchPredictorKind::default(),
            gshare_branch_predictor: GShareBranchPredictor::new(
                GShareBranchPredictorConfig::new(1, DEFAULT_RISCV_GSHARE_BRANCH_PREDICTOR_ENTRIES)
                    .expect("default RISC-V gshare branch predictor config is valid"),
            ),
            bimode_branch_predictor: BiModeBranchPredictor::new(
                BiModeBranchPredictorConfig::new(
                    1,
                    DEFAULT_RISCV_BIMODE_CHOICE_ENTRIES,
                    DEFAULT_RISCV_BIMODE_GLOBAL_ENTRIES,
                )
                .expect("default RISC-V bimode branch predictor config is valid"),
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
            tage_sc_l_branch_predictor: default_riscv_tage_sc_l_branch_predictor(),
            multiperspective_perceptron: default_riscv_multiperspective_perceptron(),
            o3_runtime: o3_runtime::O3RuntimeState::default(),
            in_order_pipeline: InOrderPipelineState::new(
                riscv_in_order_config::default_riscv_in_order_pipeline_config(),
            ),
            in_order_pipeline_cycle_records: Vec::new(),
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
        self.rollback_all_selected_branch_speculations()
            .expect("selected branch speculation rollback is internally consistent");
        self.discard_return_address_stack_speculations();
        self.branch_predictor.discard_all_speculations();
        self.branch_speculations.clear();
        self.branch_target_predictions.clear();
    }

    fn discard_return_address_stack_speculations(&mut self) {
        while let Some(operation_id) = self
            .return_address_stack
            .pending_operations()
            .first()
            .map(|operation| operation.id())
        {
            self.return_address_stack
                .squash_from(operation_id)
                .expect("pending RAS operation is known");
        }
        self.return_address_stack_operations.clear();
    }

    fn commit_return_address_stack_speculation(
        &mut self,
        sequence: u64,
    ) -> Result<(), RiscvCpuError> {
        let Some(operation_id) = self.return_address_stack_operations.remove(&sequence) else {
            return Ok(());
        };
        self.return_address_stack
            .commit_operation(operation_id)
            .map_err(RiscvCpuError::ReturnAddressStack)?;
        Ok(())
    }

    fn squash_return_address_stack_speculation(
        &mut self,
        sequence: u64,
    ) -> Result<(), RiscvCpuError> {
        let Some(operation_id) = self.return_address_stack_operations.remove(&sequence) else {
            return Ok(());
        };
        let repair = self
            .return_address_stack
            .squash_from(operation_id)
            .map_err(RiscvCpuError::ReturnAddressStack)?;
        let removed = repair
            .removed_youngers()
            .iter()
            .map(|operation| operation.id())
            .collect::<BTreeSet<_>>();
        self.return_address_stack_operations
            .retain(|_, operation| !removed.contains(operation));
        Ok(())
    }

    fn squash_inactive_return_address_stack_speculations(
        &mut self,
        active_sequences: &BTreeSet<u64>,
    ) -> Result<(), RiscvCpuError> {
        let inactive = self
            .return_address_stack_operations
            .keys()
            .filter(|sequence| !active_sequences.contains(sequence))
            .copied()
            .collect::<Vec<_>>();
        for sequence in inactive {
            self.squash_return_address_stack_speculation(sequence)?;
        }
        Ok(())
    }
}

pub fn is_fetch_request(request: &MemoryRequest) -> bool {
    request.operation() == MemoryOperation::InstructionFetch
}
