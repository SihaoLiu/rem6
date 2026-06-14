use rem6_isa_riscv::{
    AtomicMemoryOp, MemoryAccessKind, MemoryResponseWritebackTarget, MemoryWidth,
    RiscvPmaAccessKind, RiscvPmpAccessKind, RiscvPrivilegeMode,
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
    riscv_data_access, riscv_execute, CpuId, RiscvCore, RiscvCoreState, RiscvCpuError,
    RiscvDataAccessEvent, RiscvDataAccessRecord, RiscvDataAccessTarget, RiscvLoadReservation,
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

        let size = memory_width_size(access_width(&access))?;
        let address = Address::new(access_address(&access));
        self.check_pmp_data_access(fetch_request, &access, size, address)?;
        self.check_pma_data_access(fetch_request, &access, size, address)?;
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
        scheduler: &PartitionedScheduler,
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
        self.check_pmp_data_access(fetch_request, &access, size, address)?;
        self.check_pma_data_access(fetch_request, &access, size, address)?;
        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());
        let request = mmio_request(request_id, &access, size, address)?;
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
    ) -> Result<(), RiscvCpuError> {
        let kind = pma_access_kind(access);
        self.state
            .lock()
            .expect("riscv core lock")
            .pma
            .check_data_alignment(address.get(), size.bytes(), kind)
            .map_err(|error| RiscvCpuError::DataPmaAccess { fetch, error })
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
        record_data_retire_cycle(&mut state, &access);
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
                record_data_retire_cycle(&mut state, &access);
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
                record_data_retire_cycle(&mut state, &access);
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
                record_data_retire_cycle(&mut state, &access);
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

fn record_data_retire_cycle(state: &mut RiscvCoreState, access: &IssuedDataAccess) {
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
    let cycle = riscv_execute::record_retired_in_order_pipeline_cycle(
        state,
        access.fetch_request.sequence(),
    )
    .expect("completed data access records one in-order retire cycle");
    state.events[index].set_in_order_pipeline_cycle(cycle);
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
            MemoryAccessKind::Load { .. } | MemoryAccessKind::FloatLoad { .. } => {
                MemoryRequest::read_shared(
                    self.request_id,
                    self.physical_address,
                    self.size,
                    line_layout,
                )
                .map_err(RiscvCpuError::Memory)
            }
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
pub(crate) struct IssuedDataAccess {
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
        MemoryAccessKind::Store { .. } | MemoryAccessKind::FloatStore { .. } => {}
    }
}

pub(crate) fn access_width(access: &MemoryAccessKind) -> MemoryWidth {
    match access {
        MemoryAccessKind::Load { width, .. }
        | MemoryAccessKind::FloatLoad { width, .. }
        | MemoryAccessKind::LoadReserved { width, .. }
        | MemoryAccessKind::StoreConditional { width, .. }
        | MemoryAccessKind::AtomicMemory { width, .. }
        | MemoryAccessKind::Store { width, .. }
        | MemoryAccessKind::FloatStore { width, .. } => *width,
    }
}

fn pmp_access_kind(access: &MemoryAccessKind) -> RiscvPmpAccessKind {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::LoadReserved { .. } => RiscvPmpAccessKind::Read,
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::StoreConditional { .. }
        | MemoryAccessKind::AtomicMemory { .. } => RiscvPmpAccessKind::Write,
    }
}

fn pma_access_kind(access: &MemoryAccessKind) -> RiscvPmaAccessKind {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
        | MemoryAccessKind::LoadReserved { .. } => RiscvPmaAccessKind::Read,
        MemoryAccessKind::Store { .. }
        | MemoryAccessKind::FloatStore { .. }
        | MemoryAccessKind::StoreConditional { .. }
        | MemoryAccessKind::AtomicMemory { .. } => RiscvPmaAccessKind::Write,
    }
}

fn access_address(access: &MemoryAccessKind) -> u64 {
    match access {
        MemoryAccessKind::Load { address, .. }
        | MemoryAccessKind::FloatLoad { address, .. }
        | MemoryAccessKind::LoadReserved { address, .. }
        | MemoryAccessKind::StoreConditional { address, .. }
        | MemoryAccessKind::AtomicMemory { address, .. }
        | MemoryAccessKind::Store { address, .. }
        | MemoryAccessKind::FloatStore { address, .. } => *address,
    }
}

pub(crate) fn memory_width_size(width: MemoryWidth) -> Result<AccessSize, RiscvCpuError> {
    AccessSize::new(width.bytes() as u64).map_err(RiscvCpuError::Memory)
}

pub(crate) fn store_bytes(value: u64, size: AccessSize) -> Vec<u8> {
    value.to_le_bytes()[..size.bytes() as usize].to_vec()
}

pub(crate) fn mmio_request(
    request: MemoryRequestId,
    access: &MemoryAccessKind,
    size: AccessSize,
    address: Address,
) -> Result<MmioRequest, RiscvCpuError> {
    match access {
        MemoryAccessKind::Load { .. }
        | MemoryAccessKind::FloatLoad { .. }
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
    }
}

fn mmio_request_id(request: MemoryRequestId) -> MmioRequestId {
    MmioRequestId::new(request.sequence())
}
