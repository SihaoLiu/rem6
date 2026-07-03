use rem6_isa_riscv::{
    AtomicMemoryOp, MemoryAccessKind, MemoryResponseWritebackTarget, MemoryWidth, RiscvHartState,
    RiscvPrivilegeMode, VectorRegister, RISCV_VECTOR_REGISTER_BYTES,
};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, Tick,
};
use rem6_memory::{
    AccessSize, Address, ByteMask, CacheLineLayout, MemoryAtomicOp, MemoryRequest, MemoryRequestId,
    ResponseStatus,
};
use rem6_mmio::{MmioBus, MmioCompletion, MmioError, MmioRequest, MmioRequestId};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome, TransportError,
};

use crate::{
    riscv_checker, riscv_data_access, riscv_execute, CpuId, InOrderPipelineCycleRecord,
    InOrderPipelineStage, InOrderPipelineStallCause, RiscvCore, RiscvCoreState, RiscvCpuError,
    RiscvDataAccessEvent, RiscvDataAccessRecord, RiscvDataAccessTarget, RiscvLoadReservation,
};

mod request_helpers;

pub(crate) use request_helpers::{
    access_address, access_size, masked_vector_memory_request_span, vector_store_request_payload,
};
use request_helpers::{
    normalized_masked_indexed_load_data, normalized_masked_load_data,
    normalized_masked_strided_load_data, pma_access_kind, pma_alignment_checks, pmp_access_kind,
};

impl RiscvCore {
    pub fn issue_next_data_access<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut rem6_kernel::SchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(issue) = self.prepare_data_access(scheduler.now(), transport)? else {
            return Ok(None);
        };
        if self.store_conditional_fails(&issue) {
            return self
                .schedule_store_conditional_failure(scheduler, issue)
                .map(Some);
        }
        let request = self.apply_pma_data_request_attributes(
            issue.fetch_request,
            issue.physical_address,
            issue.size,
            issue.memory_request()?,
        )?;

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

    pub(crate) fn prepare_data_parallel_access<F>(
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
        let request = self.apply_pma_data_request_attributes(
            issue.fetch_request,
            issue.physical_address,
            issue.size,
            issue.memory_request()?,
        )?;
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

    pub(crate) fn record_prepared_data_issue(&self, issue: OutstandingDataAccess) {
        self.record_data_issue(issue);
    }

    pub(crate) fn schedule_prepared_store_conditional_failure_parallel(
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
        let Some(issue) = self.prepare_mmio_data_access(scheduler, bus)? else {
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

        let base_size = access_size(&access)?;
        let base_address = Address::new(access_address(&access));
        let request_span = masked_vector_memory_request_span(&access, base_address, base_size)?;
        let address = request_span.address;
        let size = request_span.size;
        self.check_pmp_data_access(fetch_request, &access, size, address)?;
        self.check_pma_data_access(
            fetch_request,
            &access,
            size,
            address,
            request_span.byte_offset,
        )?;
        let line_layout = data
            .line_layout_for_access(address, size)
            .map_err(RiscvCpuError::Memory)?;
        let line_offset = line_layout.line_offset(address);
        if line_offset + size.bytes() > line_layout.bytes()
            && !supports_cross_line_data_access(&access, address, size, line_layout)
        {
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
            request_byte_offset: request_span.byte_offset,
            line_layout: Some(line_layout),
        }))
    }

    fn prepare_mmio_data_access(
        &self,
        scheduler: &PartitionedScheduler,
        bus: &MmioBus,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        if let Some(fetch) = self.data_translation_page_map_required_fetch() {
            return Err(RiscvCpuError::DataTranslationPageMapRequired { fetch });
        }
        let Some((fetch_request, access)) = self.next_unissued_data_access() else {
            return Ok(None);
        };
        let size = access_size(&access)?;
        let address = Address::new(access_address(&access));
        self.check_pmp_data_access(fetch_request, &access, size, address)?;
        self.check_pma_data_access(fetch_request, &access, size, address, 0)?;
        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());
        let request = mmio_request(request_id, &access, size, address, 0)?;
        let route = match bus.route_for(self.core.partition(), &request) {
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
        riscv_data_access::validate_parallel_mmio_route(
            route,
            scheduler.now(),
            scheduler.min_remote_delay(),
            scheduler.partition_count(),
        )
        .map_err(|error| RiscvCpuError::Mmio(MmioError::Scheduler(error)))?;

        Ok(Some(OutstandingDataAccess {
            tick: scheduler.now(),
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Mmio { route },
            request_id,
            fetch_request,
            access,
            size,
            physical_address: address,
            request_byte_offset: 0,
            line_layout: None,
        }))
    }

    pub(crate) fn check_pmp_data_access(
        &self,
        fetch: MemoryRequestId,
        access: &MemoryAccessKind,
        size: AccessSize,
        physical_address: Address,
    ) -> Result<(), RiscvCpuError> {
        let kind = pmp_access_kind(access);
        self.state
            .lock()
            .expect("riscv core lock")
            .pmp
            .check_access(
                physical_address.get(),
                size.bytes(),
                kind,
                RiscvPrivilegeMode::Machine,
            )
            .map_err(|error| RiscvCpuError::DataPmpAccess { fetch, error })
    }

    pub(crate) fn check_pma_data_access(
        &self,
        fetch: MemoryRequestId,
        access: &MemoryAccessKind,
        size: AccessSize,
        address: Address,
        request_byte_offset: usize,
    ) -> Result<(), RiscvCpuError> {
        let kind = pma_access_kind(access);
        let checks = pma_alignment_checks(access, address, size, request_byte_offset)?;
        let state = self.state.lock().expect("riscv core lock");
        for check in checks {
            state
                .pma
                .check_data_alignment(check.address.get(), check.size.bytes(), kind)
                .map_err(|error| RiscvCpuError::DataPmaAccess { fetch, error })?;
        }
        Ok(())
    }

    pub(crate) fn apply_pma_data_request_attributes(
        &self,
        fetch: MemoryRequestId,
        address: Address,
        size: AccessSize,
        request: MemoryRequest,
    ) -> Result<MemoryRequest, RiscvCpuError> {
        let is_uncacheable = self
            .state
            .lock()
            .expect("riscv core lock")
            .pma
            .is_uncacheable(address.get(), size.bytes())
            .map_err(|error| RiscvCpuError::DataPmaAccess { fetch, error })?;
        if is_uncacheable {
            Ok(request.with_uncacheable_strict_order())
        } else {
            Ok(request)
        }
    }

    pub(crate) fn record_data_issue(&self, issue: OutstandingDataAccess) {
        self.record_data_issue_state(issue, true);
    }

    fn record_local_store_conditional_failure_issue(&self, issue: OutstandingDataAccess) {
        self.record_data_issue_state(issue, false);
    }

    fn record_data_issue_state(&self, issue: OutstandingDataAccess, emit_issued_event: bool) {
        self.core.advance_sequence_past(issue.request_id);
        let mut state = self.state.lock().expect("riscv core lock");
        state.issued_data_for_fetches.insert(issue.fetch_request);
        state
            .outstanding_data
            .insert(issue.request_id, issue.clone_without_layout());
        if emit_issued_event {
            state
                .data_events
                .push(RiscvDataAccessEvent::issued(issue.record(issue.tick)));
        }
    }

    pub(crate) fn schedule_store_conditional_failure(
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
        self.record_local_store_conditional_failure_issue(issue);
        Ok(event)
    }

    pub(crate) fn schedule_store_conditional_failure_parallel(
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
        self.record_local_store_conditional_failure_issue(issue);
        Ok(event)
    }

    pub(crate) fn store_conditional_fails(&self, issue: &OutstandingDataAccess) -> bool {
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
            debug_assert!(false, "store-conditional failure for non-SC access");
            return;
        };
        state.hart.write(*rd, 1);
        state.reservation = None;
        state
            .sc_progress
            .record_failure(self.id(), tick, access.physical_address, access.size);
        riscv_checker::sync_checker_hart(&mut state);
        record_data_retire_cycle(&mut state, &access, tick);
        state
            .data_events
            .push(RiscvDataAccessEvent::conditional_failed(
                access.record(tick),
            ));
    }

    pub(crate) fn record_data_response(&self, delivery: ResponseDelivery) {
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
                record_load_completion(
                    &mut state,
                    self.id(),
                    &access,
                    data.as_deref(),
                    "load response data",
                );
                riscv_checker::sync_checker_hart(&mut state);
                record_data_retire_cycle(&mut state, &access, delivery.tick());
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
            ResponseStatus::StoreConditionalFailed => {
                let MemoryAccessKind::StoreConditional { rd, .. } = &access.access else {
                    debug_assert!(false, "store-conditional failure for non-SC access");
                    state
                        .data_events
                        .push(RiscvDataAccessEvent::retry(access.record(delivery.tick())));
                    return;
                };
                state.hart.write(*rd, 1);
                state.reservation = None;
                state.sc_progress.record_failure(
                    self.id(),
                    delivery.tick(),
                    access.physical_address,
                    access.size,
                );
                riscv_checker::sync_checker_hart(&mut state);
                record_data_retire_cycle(&mut state, &access, delivery.tick());
                state
                    .data_events
                    .push(RiscvDataAccessEvent::conditional_failed(
                        access.record(delivery.tick()),
                    ));
            }
        }
    }

    pub fn record_data_failure(&self, request_id: MemoryRequestId, tick: Tick) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(access) = state.outstanding_data.remove(&request_id) else {
            return;
        };
        state
            .data_events
            .push(RiscvDataAccessEvent::failed(access.record(tick)));
    }

    pub(crate) fn record_mmio_completion(
        &self,
        request_id: MemoryRequestId,
        completion: MmioCompletion,
    ) {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(access) = state.outstanding_data.remove(&request_id) else {
            return;
        };

        match completion.response() {
            Ok(response) => {
                let data = response.data().map(ToOwned::to_owned);
                record_load_completion(
                    &mut state,
                    self.id(),
                    &access,
                    data.as_deref(),
                    "MMIO load response data",
                );
                riscv_checker::sync_checker_hart(&mut state);
                record_data_retire_cycle(&mut state, &access, completion.tick());
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

fn record_data_retire_cycle(
    state: &mut RiscvCoreState,
    access: &IssuedDataAccess,
    completion_tick: Tick,
) {
    let Some(index) = state
        .events
        .iter()
        .position(|event| event.fetch().request_id() == access.fetch_request)
    else {
        debug_assert!(
            false,
            "completed data access must have a matching execution event"
        );
        return;
    };
    if state.events[index].in_order_pipeline_cycle().is_some()
        || !state.events[index].counts_as_retired_instruction()
    {
        return;
    }
    let data_wait_cycles = completion_tick.saturating_sub(access.tick);
    let attributed_data_wait_cycles = retag_existing_fetch_wait_cycles_for_data_access(
        state,
        access.fetch_request,
        data_wait_cycles,
    );
    let remaining_data_wait_cycles = data_wait_cycles.saturating_sub(attributed_data_wait_cycles);
    let execute_wait_cycles =
        riscv_execute::in_order_execute_wait_cycles(state.events[index].instruction());
    let mut waits = Vec::with_capacity(2);
    if execute_wait_cycles > 0 {
        waits.push((execute_wait_cycles, InOrderPipelineStallCause::ExecuteWait));
    }
    if remaining_data_wait_cycles > 0 {
        waits.push((
            remaining_data_wait_cycles,
            InOrderPipelineStallCause::DataWait,
        ));
    }
    let cycle = riscv_execute::record_retired_in_order_pipeline_cycle_after_waits_with_causes(
        state,
        access.fetch_request.sequence(),
        None,
        &waits,
    )
    .expect("completed data access records one in-order retire cycle");
    state.events[index].set_in_order_pipeline_cycle(cycle);
    state.events[index].set_in_order_pipeline_data_wait_cycles(data_wait_cycles);
}

fn retag_existing_fetch_wait_cycles_for_data_access(
    state: &mut RiscvCoreState,
    fetch_request: MemoryRequestId,
    mut remaining_data_wait_cycles: u64,
) -> u64 {
    let sequence = fetch_request.sequence();
    let mut retagged = 0u64;
    for record in state.in_order_pipeline_cycle_records.iter_mut().rev() {
        if remaining_data_wait_cycles == 0 {
            break;
        }
        if record.stall_cause() != Some(InOrderPipelineStallCause::FetchWait)
            || !cycle_record_blocks_memory_wait_sequence(record, sequence)
        {
            continue;
        }
        record.set_stall_cause(Some(InOrderPipelineStallCause::DataWait));
        let cycles = record.stall_cycle_count();
        retagged = retagged.saturating_add(cycles);
        remaining_data_wait_cycles = remaining_data_wait_cycles.saturating_sub(cycles);
    }
    retagged
}

fn cycle_record_blocks_memory_wait_sequence(
    record: &InOrderPipelineCycleRecord,
    sequence: u64,
) -> bool {
    record
        .plan()
        .resource_blocked()
        .iter()
        .chain(record.plan().ordering_blocked())
        .any(|instruction| {
            instruction.sequence() == sequence
                && matches!(
                    instruction.stage(),
                    InOrderPipelineStage::Execute | InOrderPipelineStage::Commit
                )
        })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct OutstandingDataAccess {
    pub(crate) tick: Tick,
    pub(crate) partition: PartitionId,
    pub(crate) target: RiscvDataAccessTarget,
    pub(crate) request_id: MemoryRequestId,
    pub(crate) fetch_request: MemoryRequestId,
    pub(crate) access: MemoryAccessKind,
    pub(crate) size: AccessSize,
    pub(crate) physical_address: Address,
    pub(crate) request_byte_offset: usize,
    pub(crate) line_layout: Option<CacheLineLayout>,
}

impl OutstandingDataAccess {
    pub(crate) fn memory_route(&self) -> MemoryRouteId {
        let RiscvDataAccessTarget::Memory { route, .. } = &self.target else {
            unreachable!("memory data access target");
        };
        *route
    }

    pub(crate) fn memory_request(&self) -> Result<MemoryRequest, RiscvCpuError> {
        let line_layout = self.line_layout.expect("memory data access line layout");
        let request = match &self.access {
            MemoryAccessKind::Load { .. }
            | MemoryAccessKind::FloatLoad { .. }
            | MemoryAccessKind::VectorLoadUnitStride { .. }
            | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
            | MemoryAccessKind::VectorLoadStrided { .. }
            | MemoryAccessKind::VectorLoadIndexed { .. } => MemoryRequest::read_shared(
                self.request_id,
                self.physical_address,
                self.size,
                line_layout,
            )
            .map_err(RiscvCpuError::Memory),
            MemoryAccessKind::LoadReserved { .. } => MemoryRequest::load_locked(
                self.request_id,
                self.physical_address,
                self.size,
                line_layout,
            )
            .map_err(RiscvCpuError::Memory),
            MemoryAccessKind::Store { value, .. } | MemoryAccessKind::FloatStore { value, .. } => {
                MemoryRequest::write(
                    self.request_id,
                    self.physical_address,
                    self.size,
                    store_bytes(*value, self.size),
                    ByteMask::full(self.size).map_err(RiscvCpuError::Memory)?,
                    line_layout,
                )
                .map_err(RiscvCpuError::Memory)
            }
            MemoryAccessKind::VectorStoreUnitStride {
                data, byte_mask, ..
            } => {
                let (data, byte_mask) = vector_store_request_payload(
                    self.size,
                    self.request_byte_offset,
                    data,
                    byte_mask.as_deref(),
                )?;
                MemoryRequest::write(
                    self.request_id,
                    self.physical_address,
                    self.size,
                    data,
                    store_byte_mask(self.size, byte_mask.as_deref())?,
                    line_layout,
                )
                .map_err(RiscvCpuError::Memory)
            }
            MemoryAccessKind::VectorStoreSegmentUnitStride {
                data, byte_mask, ..
            } => {
                let (data, byte_mask) = vector_store_request_payload(
                    self.size,
                    self.request_byte_offset,
                    data,
                    byte_mask.as_deref(),
                )?;
                MemoryRequest::write(
                    self.request_id,
                    self.physical_address,
                    self.size,
                    data,
                    store_byte_mask(self.size, byte_mask.as_deref())?,
                    line_layout,
                )
                .map_err(RiscvCpuError::Memory)
            }
            MemoryAccessKind::VectorStoreStrided {
                data, byte_mask, ..
            } => {
                let (data, byte_mask) = vector_store_request_payload(
                    self.size,
                    self.request_byte_offset,
                    data,
                    Some(byte_mask.as_slice()),
                )?;
                MemoryRequest::write(
                    self.request_id,
                    self.physical_address,
                    self.size,
                    data,
                    store_byte_mask(self.size, byte_mask.as_deref())?,
                    line_layout,
                )
                .map_err(RiscvCpuError::Memory)
            }
            MemoryAccessKind::VectorStoreIndexed {
                data, byte_mask, ..
            } => {
                let (data, byte_mask) = vector_store_request_payload(
                    self.size,
                    self.request_byte_offset,
                    data,
                    Some(byte_mask.as_slice()),
                )?;
                MemoryRequest::write(
                    self.request_id,
                    self.physical_address,
                    self.size,
                    data,
                    store_byte_mask(self.size, byte_mask.as_deref())?,
                    line_layout,
                )
                .map_err(RiscvCpuError::Memory)
            }
            MemoryAccessKind::StoreConditional { value, .. } => MemoryRequest::store_conditional(
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
        }?;
        Ok(request.with_ordering(riscv_data_access::memory_request_ordering(&self.access)))
    }

    pub(crate) fn mmio_request(&self) -> Result<MmioRequest, RiscvCpuError> {
        mmio_request(
            self.request_id,
            &self.access,
            self.size,
            self.physical_address,
            self.request_byte_offset,
        )
    }

    fn clone_without_layout(&self) -> IssuedDataAccess {
        IssuedDataAccess {
            tick: self.tick,
            partition: self.partition,
            target: self.target.clone(),
            request: self.request_id,
            fetch_request: self.fetch_request,
            access: self.access.clone(),
            size: self.size,
            physical_address: self.physical_address,
            request_byte_offset: self.request_byte_offset,
        }
    }

    fn record(&self, tick: Tick) -> RiscvDataAccessRecord {
        self.clone_without_layout().record(tick)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct IssuedDataAccess {
    tick: Tick,
    partition: PartitionId,
    target: RiscvDataAccessTarget,
    request: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: MemoryAccessKind,
    size: AccessSize,
    physical_address: Address,
    request_byte_offset: usize,
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
        .with_request_byte_offset(self.request_byte_offset)
    }
}

#[allow(clippy::large_enum_variant)]
pub(crate) enum PreparedDataParallelAccess {
    Transaction {
        issue: OutstandingDataAccess,
        transaction: ParallelMemoryTransaction,
    },
    ConditionalFailed {
        issue: OutstandingDataAccess,
    },
}

fn record_load_completion(
    state: &mut RiscvCoreState,
    cpu: CpuId,
    access: &IssuedDataAccess,
    data: Option<&[u8]>,
    missing_data: &'static str,
) {
    match &access.access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::AtomicMemory { .. } => {
            let writeback = access
                .access
                .read_response_writeback(data.expect(missing_data))
                .expect("read response payload width")
                .expect("read response writeback");
            match writeback.target() {
                MemoryResponseWritebackTarget::Integer(register) => {
                    state.hart.write(register, writeback.value());
                }
                MemoryResponseWritebackTarget::Float(register) => {
                    state.hart.write_float(register, writeback.value());
                }
            }
        }
        MemoryAccessKind::LoadReserved { .. } => {
            let writeback = access
                .access
                .read_response_writeback(data.expect(missing_data))
                .expect("read response payload width")
                .expect("read response writeback");
            state
                .hart
                .write(writeback.expect_integer_register(), writeback.value());
            state.reservation = Some(RiscvLoadReservation::new(
                access.physical_address,
                access.size,
            ));
        }
        MemoryAccessKind::StoreConditional { rd, .. } => {
            state.hart.write(*rd, 0);
            state.reservation = None;
            state.sc_progress.record_success(cpu);
        }
        MemoryAccessKind::VectorLoadUnitStride {
            vd,
            byte_len,
            byte_mask,
            group_registers,
            ..
        } => {
            let data = data.expect(missing_data);
            let data = normalized_masked_load_data(*byte_len, byte_mask.as_deref(), data);
            assert_eq!(*byte_len, data.len(), "vector load response payload width");
            let mut destination = read_vector_register_group(&state.hart, *vd, *group_registers);
            if let Some(byte_mask) = byte_mask {
                assert_eq!(
                    *byte_len,
                    byte_mask.len(),
                    "vector load byte mask payload width"
                );
                for (index, active) in byte_mask.iter().copied().enumerate() {
                    if active {
                        destination[index] = data[index];
                    }
                }
            } else {
                destination[..*byte_len].copy_from_slice(&data);
            }
            write_vector_register_group(&mut state.hart, *vd, *group_registers, &destination);
        }
        MemoryAccessKind::VectorLoadSegmentUnitStride {
            vd,
            fields,
            element_count,
            byte_len,
            byte_mask,
            group_registers,
            ..
        } => {
            let data = data.expect(missing_data);
            let data = normalized_masked_load_data(*byte_len, byte_mask.as_deref(), data);
            assert_eq!(*byte_len, data.len(), "segment vector load response width");
            if let Some(byte_mask) = byte_mask {
                assert_eq!(
                    *byte_len,
                    byte_mask.len(),
                    "segment vector load byte mask width"
                );
            }
            scatter_segment_load(
                &data,
                &mut state.hart,
                *vd,
                *fields,
                *element_count,
                byte_mask.as_deref(),
                *group_registers,
            );
        }
        MemoryAccessKind::VectorLoadStrided {
            vd,
            width,
            stride,
            element_count,
            span_len,
            byte_mask,
            group_registers,
            ..
        } => {
            let data = data.expect(missing_data);
            let data = normalized_masked_strided_load_data(
                *span_len,
                byte_mask.as_deref(),
                *stride,
                *element_count,
                width.bytes(),
                data,
            );
            assert_eq!(*span_len, data.len(), "strided vector load response width");
            let mut destination = read_vector_register_group(&state.hart, *vd, *group_registers);
            scatter_strided_load(
                &data,
                &mut destination,
                width.bytes(),
                *stride,
                *element_count,
                byte_mask.as_deref(),
            );
            write_vector_register_group(&mut state.hart, *vd, *group_registers, &destination);
        }
        MemoryAccessKind::VectorLoadIndexed {
            vd,
            width,
            offsets,
            span_len,
            byte_mask,
            group_registers,
            ..
        } => {
            let data = data.expect(missing_data);
            let data = normalized_masked_indexed_load_data(
                *span_len,
                byte_mask.as_deref(),
                offsets,
                width.bytes(),
                data,
            );
            assert_eq!(*span_len, data.len(), "indexed vector load response width");
            let mut destination = read_vector_register_group(&state.hart, *vd, *group_registers);
            scatter_indexed_load(
                &data,
                &mut destination,
                width.bytes(),
                offsets,
                byte_mask.as_deref(),
            );
            write_vector_register_group(&mut state.hart, *vd, *group_registers, &destination);
        }
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::VectorStoreUnitStride { .. }
        | MemoryAccessKind::VectorStoreSegmentUnitStride { .. }
        | MemoryAccessKind::VectorStoreStrided { .. }
        | MemoryAccessKind::VectorStoreIndexed { .. } => {}
    }
}

fn scatter_segment_load(
    data: &[u8],
    hart: &mut RiscvHartState,
    register: VectorRegister,
    fields: usize,
    element_count: usize,
    byte_mask: Option<&[bool]>,
    group_registers: usize,
) {
    debug_assert!(element_count > 0);
    let element_bytes = data.len() / fields / element_count;
    for field in 0..fields {
        let field_register = vector_register_at(register, field * group_registers);
        let mut destination = read_vector_register_group(hart, field_register, group_registers);
        for element_index in 0..element_count {
            let source_offset = (element_index * fields + field) * element_bytes;
            let active = byte_mask.map(|mask| mask[source_offset]).unwrap_or(true);
            if !active {
                continue;
            }
            let destination_offset = element_index * element_bytes;
            destination[destination_offset..destination_offset + element_bytes]
                .copy_from_slice(&data[source_offset..source_offset + element_bytes]);
        }
        write_vector_register_group(hart, field_register, group_registers, &destination);
    }
}

fn scatter_strided_load(
    data: &[u8],
    destination: &mut [u8],
    element_bytes: usize,
    stride: usize,
    element_count: usize,
    byte_mask: Option<&[bool]>,
) {
    for element_index in 0..element_count {
        let memory_offset = element_index * stride;
        let destination_offset = element_index * element_bytes;
        let active = byte_mask
            .map(|mask| mask[destination_offset])
            .unwrap_or(true);
        if !active {
            continue;
        }
        destination[destination_offset..destination_offset + element_bytes]
            .copy_from_slice(&data[memory_offset..memory_offset + element_bytes]);
    }
}

fn scatter_indexed_load(
    data: &[u8],
    destination: &mut [u8],
    element_bytes: usize,
    offsets: &[usize],
    byte_mask: Option<&[bool]>,
) {
    for (element_index, memory_offset) in offsets.iter().copied().enumerate() {
        let destination_offset = element_index * element_bytes;
        let active = byte_mask
            .map(|mask| mask[destination_offset])
            .unwrap_or(true);
        if !active {
            continue;
        }
        destination[destination_offset..destination_offset + element_bytes]
            .copy_from_slice(&data[memory_offset..memory_offset + element_bytes]);
    }
}

fn read_vector_register_group(
    hart: &RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
) -> Vec<u8> {
    let group_bytes = group_registers * RISCV_VECTOR_REGISTER_BYTES;
    let mut bytes = vec![0; group_bytes];
    for group_index in 0..group_registers {
        let vector = hart.read_vector(vector_register_at(register, group_index));
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES].copy_from_slice(&vector);
    }
    bytes
}

fn write_vector_register_group(
    hart: &mut RiscvHartState,
    register: VectorRegister,
    group_registers: usize,
    bytes: &[u8],
) {
    assert_eq!(
        bytes.len(),
        group_registers * RISCV_VECTOR_REGISTER_BYTES,
        "vector register group payload width"
    );
    for group_index in 0..group_registers {
        let offset = group_index * RISCV_VECTOR_REGISTER_BYTES;
        let mut vector = [0; RISCV_VECTOR_REGISTER_BYTES];
        vector.copy_from_slice(&bytes[offset..offset + RISCV_VECTOR_REGISTER_BYTES]);
        hart.write_vector(vector_register_at(register, group_index), vector);
    }
}

fn vector_register_at(register: VectorRegister, group_index: usize) -> VectorRegister {
    let index = usize::from(register.index()) + group_index;
    VectorRegister::new(index as u8).expect("validated vector register group")
}

pub(crate) fn supports_cross_line_data_access(
    access: &MemoryAccessKind,
    address: Address,
    size: AccessSize,
    line_layout: CacheLineLayout,
) -> bool {
    let line_bytes = line_layout.bytes();
    if line_layout.line_offset(address) != 0
        || size.bytes() <= line_bytes
        || !size.bytes().is_multiple_of(line_bytes)
    {
        return false;
    }

    match access {
        MemoryAccessKind::VectorLoadUnitStride {
            width,
            byte_len,
            group_registers,
            ..
        } => supported_full_register_group_vector_access(*width, *byte_len, *group_registers, size),
        MemoryAccessKind::VectorStoreUnitStride {
            width,
            data,
            group_registers,
            ..
        } => {
            supported_full_register_group_vector_access(*width, data.len(), *group_registers, size)
        }
        MemoryAccessKind::VectorLoadStrided {
            width,
            stride,
            element_count,
            span_len,
            group_registers,
            ..
        } => sparse_e64_strided_m1_vector_access(
            *width,
            *stride,
            *element_count,
            *span_len,
            *group_registers,
            size,
        ),
        MemoryAccessKind::VectorStoreStrided {
            width,
            stride,
            element_count,
            data,
            group_registers,
            ..
        } => sparse_e64_strided_m1_vector_access(
            *width,
            *stride,
            *element_count,
            data.len(),
            *group_registers,
            size,
        ),
        MemoryAccessKind::VectorLoadIndexed {
            width,
            index_width,
            offsets,
            span_len,
            group_registers,
            ..
        } => sparse_e64_indexed_m1_vector_access(
            *width,
            *index_width,
            offsets,
            *span_len,
            *group_registers,
            size,
        ),
        MemoryAccessKind::VectorStoreIndexed {
            width,
            index_width,
            offsets,
            data,
            group_registers,
            ..
        } => sparse_e64_indexed_m1_vector_access(
            *width,
            *index_width,
            offsets,
            data.len(),
            *group_registers,
            size,
        ),
        _ => false,
    }
}

fn supported_full_register_group_vector_access(
    width: MemoryWidth,
    byte_len: usize,
    group_registers: usize,
    size: AccessSize,
) -> bool {
    let Ok(size_bytes) = usize::try_from(size.bytes()) else {
        return false;
    };
    let full_group_bytes = group_registers * RISCV_VECTOR_REGISTER_BYTES;
    let supported_shape = (width == MemoryWidth::Halfword && group_registers == 2)
        || (width == MemoryWidth::Word && matches!(group_registers, 2 | 4 | 8));
    supported_shape && byte_len == full_group_bytes && size_bytes == byte_len
}

fn sparse_e64_strided_m1_vector_access(
    width: MemoryWidth,
    stride: usize,
    element_count: usize,
    byte_len: usize,
    group_registers: usize,
    size: AccessSize,
) -> bool {
    let Ok(size_bytes) = usize::try_from(size.bytes()) else {
        return false;
    };
    group_registers == 1
        && width == MemoryWidth::Doubleword
        && stride == 24
        && element_count == 2
        && byte_len == 32
        && size_bytes == byte_len
}

fn sparse_e64_indexed_m1_vector_access(
    width: MemoryWidth,
    index_width: MemoryWidth,
    offsets: &[usize],
    byte_len: usize,
    group_registers: usize,
    size: AccessSize,
) -> bool {
    let Ok(size_bytes) = usize::try_from(size.bytes()) else {
        return false;
    };
    group_registers == 1
        && width == MemoryWidth::Doubleword
        && matches!(
            index_width,
            MemoryWidth::Byte | MemoryWidth::Halfword | MemoryWidth::Word | MemoryWidth::Doubleword
        )
        && offsets == [0, 24]
        && byte_len == 32
        && size_bytes == byte_len
}

pub(crate) fn store_bytes(value: u64, size: AccessSize) -> Vec<u8> {
    value.to_le_bytes()[..size.bytes() as usize].to_vec()
}

pub(crate) fn store_byte_mask(
    size: AccessSize,
    byte_mask: Option<&[bool]>,
) -> Result<ByteMask, RiscvCpuError> {
    match byte_mask {
        Some(mask) => ByteMask::from_bits(mask.to_vec()).map_err(RiscvCpuError::Memory),
        None => ByteMask::full(size).map_err(RiscvCpuError::Memory),
    }
}

pub(crate) fn mmio_request(
    request: MemoryRequestId,
    access: &MemoryAccessKind,
    size: AccessSize,
    address: Address,
    request_byte_offset: usize,
) -> Result<MmioRequest, RiscvCpuError> {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::VectorLoadUnitStride { .. }
        | MemoryAccessKind::VectorLoadSegmentUnitStride { .. }
        | MemoryAccessKind::VectorLoadStrided { .. }
        | MemoryAccessKind::VectorLoadIndexed { .. }
        | MemoryAccessKind::LoadReserved { .. } => {
            MmioRequest::read(mmio_request_id(request), address, size).map_err(RiscvCpuError::Mmio)
        }
        MemoryAccessKind::AtomicMemory { .. } => {
            Err(RiscvCpuError::UnsupportedMmioAtomic { request, address })
        }
        MemoryAccessKind::Store { value, .. }
        | MemoryAccessKind::FloatStore { value, .. }
        | MemoryAccessKind::StoreConditional { value, .. } => MmioRequest::write(
            mmio_request_id(request),
            address,
            store_bytes(*value, size),
            ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
        )
        .map_err(RiscvCpuError::Mmio),
        MemoryAccessKind::VectorStoreUnitStride {
            data, byte_mask, ..
        } => {
            let (data, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                data,
                byte_mask.as_deref(),
            )?;
            MmioRequest::write(
                mmio_request_id(request),
                address,
                data,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
            .map_err(RiscvCpuError::Mmio)
        }
        MemoryAccessKind::VectorStoreSegmentUnitStride {
            data, byte_mask, ..
        } => {
            let (data, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                data,
                byte_mask.as_deref(),
            )?;
            MmioRequest::write(
                mmio_request_id(request),
                address,
                data,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
            .map_err(RiscvCpuError::Mmio)
        }
        MemoryAccessKind::VectorStoreStrided {
            data, byte_mask, ..
        } => {
            let (data, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                data,
                Some(byte_mask.as_slice()),
            )?;
            MmioRequest::write(
                mmio_request_id(request),
                address,
                data,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
            .map_err(RiscvCpuError::Mmio)
        }
        MemoryAccessKind::VectorStoreIndexed {
            data, byte_mask, ..
        } => {
            let (data, byte_mask) = vector_store_request_payload(
                size,
                request_byte_offset,
                data,
                Some(byte_mask.as_slice()),
            )?;
            MmioRequest::write(
                mmio_request_id(request),
                address,
                data,
                store_byte_mask(size, byte_mask.as_deref())?,
            )
            .map_err(RiscvCpuError::Mmio)
        }
    }
}

fn mmio_request_id(request: MemoryRequestId) -> MmioRequestId {
    MmioRequestId::new(request.sequence())
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{MemoryAccessKind, MemoryWidth, VectorRegister};
    use rem6_memory::{AccessSize, Address, CacheLineLayout};

    use super::supports_cross_line_data_access;

    #[test]
    fn cross_line_vector_access_accepts_aligned_two_line_full_lmul2_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 32, 2),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_e16_two_line_full_lmul2_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride_with_width(0x8000, MemoryWidth::Halfword, 32, 2),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_unit_stride_with_width(0x8000, MemoryWidth::Halfword, 32, 2),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_four_line_full_lmul4_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(64).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 64, 4),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_unit_stride(0x8000, 64, 4),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_eight_line_full_lmul8_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(128).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 128, 8),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_unit_stride(0x8000, 128, 8),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Doubleword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Doubleword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1_with_e32_indices() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Word, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Word, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1_with_e16_indices() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Halfword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Halfword, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_indexed_e64_m1_with_e8_indices() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_indexed_e64_m1(0x8000, MemoryWidth::Byte, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_indexed_e64_m1(0x8000, MemoryWidth::Byte, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_accepts_aligned_sparse_strided_e64_m1() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(supports_cross_line_data_access(
            &vector_load_strided_e64_m1(0x8000, 32),
            Address::new(0x8000),
            size,
            layout
        ));
        assert!(supports_cross_line_data_access(
            &vector_store_strided_e64_m1(0x8000, 32),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_unaligned_full_lmul2_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_unit_stride(0x8004, 32, 2),
            Address::new(0x8004),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_partial_lmul8_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(64).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 64, 8),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    #[test]
    fn cross_line_vector_access_rejects_partial_lmul4_group() {
        let layout = CacheLineLayout::new(16).unwrap();
        let size = AccessSize::new(32).unwrap();

        assert!(!supports_cross_line_data_access(
            &vector_load_unit_stride(0x8000, 32, 4),
            Address::new(0x8000),
            size,
            layout
        ));
    }

    fn vector_load_unit_stride(
        address: u64,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        vector_load_unit_stride_with_width(address, MemoryWidth::Word, byte_len, group_registers)
    }

    fn vector_load_unit_stride_with_width(
        address: u64,
        width: MemoryWidth,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadUnitStride {
            vd: VectorRegister::new(2).unwrap(),
            address,
            width,
            byte_len,
            byte_mask: None,
            group_registers,
        }
    }

    fn vector_store_unit_stride(
        address: u64,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        vector_store_unit_stride_with_width(address, MemoryWidth::Word, byte_len, group_registers)
    }

    fn vector_store_unit_stride_with_width(
        address: u64,
        width: MemoryWidth,
        byte_len: usize,
        group_registers: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreUnitStride {
            address,
            width,
            data: vec![0; byte_len],
            byte_mask: None,
            group_registers,
        }
    }

    fn vector_load_strided_e64_m1(address: u64, span_len: usize) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadStrided {
            vd: VectorRegister::new(1).unwrap(),
            address,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            span_len,
            byte_mask: None,
            group_registers: 1,
        }
    }

    fn vector_store_strided_e64_m1(address: u64, data_len: usize) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreStrided {
            address,
            width: MemoryWidth::Doubleword,
            stride: 24,
            element_count: 2,
            data: vec![0; data_len],
            byte_mask: vec![true; data_len],
            group_registers: 1,
        }
    }

    fn vector_load_indexed_e64_m1(
        address: u64,
        index_width: MemoryWidth,
        span_len: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorLoadIndexed {
            vd: VectorRegister::new(1).unwrap(),
            address,
            width: MemoryWidth::Doubleword,
            index_width,
            offsets: vec![0, 24],
            span_len,
            byte_mask: None,
            group_registers: 1,
        }
    }

    fn vector_store_indexed_e64_m1(
        address: u64,
        index_width: MemoryWidth,
        data_len: usize,
    ) -> MemoryAccessKind {
        MemoryAccessKind::VectorStoreIndexed {
            address,
            width: MemoryWidth::Doubleword,
            index_width,
            offsets: vec![0, 24],
            data: vec![0; data_len],
            byte_mask: vec![true; data_len],
            group_registers: 1,
        }
    }
}
