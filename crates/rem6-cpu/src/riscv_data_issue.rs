use rem6_isa_riscv::{AtomicMemoryOp, MemoryAccessKind};
use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionId, PartitionedScheduler, Tick,
};
use rem6_memory::{
    AccessSize, Address, AddressRange, ByteMask, CacheLineLayout, MemoryAtomicOp, MemoryRequest,
    MemoryRequestId, ResponseStatus,
};
use rem6_mmio::{MmioBus, MmioCompletion, MmioError, MmioRequest, MmioRequestId, MmioRoute};
use rem6_transport::{
    MemoryRouteId, MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery,
    ResponseDelivery, TargetOutcome, TransportError,
};

use crate::{
    o3_runtime::{
        is_deferred_o3_data_access, o3_memory_result_destination,
        o3_memory_result_window_destination, o3_memory_result_younger_buffered_effect_destination,
        O3DataAccessWindowPolicy, O3StoreLoadForwardingPlan,
    },
    riscv_checker,
    riscv_cross_line::supports_cross_line_data_access,
    riscv_data_access,
    riscv_data_completion::{apply_data_completion, RiscvDataCompletion},
    riscv_execute,
    riscv_fetch_ahead::{O3MemoryResultWindowRole, O3MemoryResultWindowRoute},
    riscv_fu_latency::riscv_data_completion_execute_wait_cycles,
    riscv_live_retire_window::{
        stage_o3_data_access_younger_window, wake_o3_data_access_younger_window,
    },
    CpuFetchEvent, InOrderPipelineCycleRecord, InOrderPipelineStage, InOrderPipelineStallCause,
    O3RuntimeError, RiscvCore, RiscvCoreState, RiscvCpuError, RiscvCpuExecutionEvent,
    RiscvDataAccessEvent, RiscvDataAccessEventKind, RiscvDataAccessRecord, RiscvDataAccessTarget,
    RiscvLoadReservation,
};

mod buffered_effect;
mod dependent_result_address;
mod forwarding;
mod handoff;
mod o3_callback;
mod prepared;
mod request_helpers;
mod store_conditional;

pub(crate) use buffered_effect::BufferedO3Effect;
use buffered_effect::{buffered_o3_effect_admission, PreparedDataAccess};
use dependent_result_address::PendingAddressPreSubmit;
use o3_callback::{
    cloned_data_access_event_with_kind, mark_data_access_event_kind, record_callback_error,
    record_o3_data_access_outcome,
};
pub(crate) use prepared::{PreparedDataIssueCleanup, PreparedDataParallelAccess};
pub(crate) use request_helpers::{
    access_address, access_size, fault_only_first_line_prefix, masked_vector_memory_request_span,
    vector_store_request_payload,
};
use request_helpers::{pma_access_kind, pma_alignment_checks, pmp_access_kind};

#[cfg(test)]
use crate::CpuId;

pub(super) enum BufferedO3EffectAdmission {
    NotBuffered,
    Buffered(MemoryRequestId),
    Blocked,
}

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
        self.data_issue_attempt(|| {
            let Some(prepared) = self.prepare_data_access(scheduler.now(), transport)? else {
                return Ok(None);
            };
            let issue = match prepared {
                PreparedDataAccess::BufferedEffect(buffered) => {
                    return self
                        .submit_buffered_o3_effect(scheduler, transport, trace, buffered, responder)
                        .map(Some);
                }
                PreparedDataAccess::New(issue) => issue,
            };
            if issue.has_forwarded_load_data() {
                return self
                    .schedule_forwarded_load_completion(scheduler, issue)
                    .map(Some);
            }
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
            match self.o3_buffered_effect_predecessor(&issue) {
                BufferedO3EffectAdmission::Buffered(predecessor) => {
                    return self.schedule_buffered_o3_effect(
                        scheduler,
                        issue,
                        request,
                        predecessor,
                    );
                }
                BufferedO3EffectAdmission::Blocked => {
                    self.clear_deferred_o3_live_data_access_execution();
                    return Ok(None);
                }
                BufferedO3EffectAdmission::NotBuffered => {}
            }

            let request_id = issue.request_id;
            let responder_core = self.clone();
            let core = self.clone();
            let event = transport
                .submit(
                    scheduler,
                    issue.memory_route(),
                    request,
                    trace,
                    move |delivery, context| {
                        if responder_core.owns_outstanding_data_request(request_id) {
                            responder(delivery, context)
                        } else {
                            TargetOutcome::NoResponse
                        }
                    },
                    move |delivery| core.record_data_response(delivery),
                )
                .map_err(RiscvCpuError::Transport)?;

            self.record_data_issue(issue);
            Ok(Some(event))
        })
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

        self.submit_prepared_data_parallel_access(scheduler, transport, prepared)
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
        self.data_issue_attempt(|| {
            let Some(prepared) = self.prepare_data_access(tick, transport)? else {
                return Ok(None);
            };
            let issue = match prepared {
                PreparedDataAccess::BufferedEffect(buffered) => {
                    return Ok(Some(
                        self.prepare_buffered_o3_effect_parallel(buffered, trace, responder),
                    ));
                }
                PreparedDataAccess::New(issue) => issue,
            };
            if issue.has_forwarded_load_data() {
                return Ok(Some(PreparedDataParallelAccess::forwarded(self, issue)));
            }
            if self.store_conditional_fails(&issue) {
                return Ok(Some(PreparedDataParallelAccess::conditional_failed(
                    self, issue,
                )));
            }
            let request = self.apply_pma_data_request_attributes(
                issue.fetch_request,
                issue.physical_address,
                issue.size,
                issue.memory_request()?,
            )?;
            match self.o3_buffered_effect_predecessor(&issue) {
                BufferedO3EffectAdmission::Buffered(predecessor) => {
                    return Ok(Some(PreparedDataParallelAccess::buffered_effect(
                        self,
                        issue,
                        request,
                        predecessor,
                    )));
                }
                BufferedO3EffectAdmission::Blocked => {
                    self.clear_deferred_o3_live_data_access_execution();
                    return Ok(None);
                }
                BufferedO3EffectAdmission::NotBuffered => {}
            }
            let request_id = issue.request_id;
            let responder_core = self.clone();
            let core = self.clone();
            let transaction = ParallelMemoryTransaction::new(
                issue.memory_route(),
                request,
                trace,
                move |delivery, context| {
                    if responder_core.owns_outstanding_data_request(request_id) {
                        responder(delivery, context)
                    } else {
                        TargetOutcome::NoResponse
                    }
                },
                move |delivery| core.record_data_response(delivery),
            );
            Ok(Some(PreparedDataParallelAccess::transaction(
                self,
                issue,
                transaction,
            )))
        })
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
        self.data_issue_attempt(|| {
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
                    if !core.owns_outstanding_data_request(request_id) {
                        return;
                    }
                    let delivery_core = core.clone();
                    let completion_core = core.clone();
                    bus.submit_parallel_guarded(
                        context,
                        request,
                        move |_| delivery_core.owns_outstanding_data_request(request_id),
                        move |completion| {
                            completion_core.record_mmio_completion(request_id, completion);
                        },
                    )
                    .expect("validated parallel MMIO data access submission");
                })
                .map_err(RiscvCpuError::Scheduler)?;

            self.record_data_issue(issue);
            Ok(Some(event))
        })
    }

    pub(crate) fn data_issue_attempt<T>(
        &self,
        attempt: impl FnOnce() -> Result<T, RiscvCpuError>,
    ) -> Result<T, RiscvCpuError> {
        let result = attempt();
        if result.is_err() {
            self.clear_deferred_o3_live_data_access_execution();
        }
        result
    }

    fn prepare_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<Option<PreparedDataAccess>, RiscvCpuError> {
        if let Some(buffered) = self.ready_buffered_o3_effect() {
            return Ok(Some(PreparedDataAccess::BufferedEffect(buffered)));
        }
        if let Some(fetch) = self.data_translation_page_map_required_fetch() {
            return Err(RiscvCpuError::DataTranslationPageMapRequired { fetch });
        }
        let Some((fetch_request, mut access)) = self.next_unissued_data_access() else {
            return Ok(None);
        };
        let prepared = (|| {
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
            let mut size = request_span.size;
            let mut request_byte_offset = request_span.byte_offset;
            let line_layout = data
                .line_layout_for_access(address, size)
                .map_err(RiscvCpuError::Memory)?;
            let line_offset = line_layout.line_offset(address);
            let mut data_access_validated = false;
            if line_offset + size.bytes() > line_layout.bytes() {
                let access_error =
                    match self.check_pmp_data_access(fetch_request, &access, size, address) {
                        Ok(()) => match self.check_pma_data_access(
                            fetch_request,
                            &access,
                            size,
                            address,
                            request_byte_offset,
                        ) {
                            Ok(()) => {
                                if supports_cross_line_data_access(
                                    &access,
                                    address,
                                    size,
                                    line_layout,
                                ) {
                                    data_access_validated = true;
                                    None
                                } else {
                                    Some(RiscvCpuError::DataAccessCrossesLine {
                                        address,
                                        size,
                                        line_size: line_layout.bytes(),
                                    })
                                }
                            }
                            Err(error) => Some(error),
                        },
                        Err(error) => Some(error),
                    };
                if let Some(error) = access_error {
                    if let Some(prefix) = fault_only_first_line_prefix(
                        &access,
                        address,
                        size,
                        request_byte_offset,
                        line_layout,
                    )? {
                        access = prefix.access;
                        size = prefix.size;
                        request_byte_offset = prefix.byte_offset;
                    } else {
                        return Err(error);
                    }
                }
            }
            if !data_access_validated {
                self.check_pmp_data_access(fetch_request, &access, size, address)?;
                self.check_pma_data_access(
                    fetch_request,
                    &access,
                    size,
                    address,
                    request_byte_offset,
                )?;
            }
            let store_load_forwarding_plan =
                self.scalar_load_forwarding_plan(fetch_request, &access);
            let forwarded_load_data = store_load_forwarding_plan
                .filter(|plan| !plan.is_partial())
                .map(O3StoreLoadForwardingPlan::data);
            let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());

            Ok(OutstandingDataAccess {
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
                request_byte_offset,
                line_layout: Some(line_layout),
                forwarded_load_data,
                store_load_forwarding_plan,
            })
        })();
        let issue = match prepared {
            Ok(issue) => issue,
            Err(error)
                if self.pending_address_preparation_failure_is_replay(fetch_request, &error) =>
            {
                self.replay_pending_address_before_submit(fetch_request);
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        match self.validate_pending_address_pre_submit(&issue) {
            PendingAddressPreSubmit::NotPending | PendingAddressPreSubmit::Ready => {
                Ok(Some(PreparedDataAccess::New(issue)))
            }
            PendingAddressPreSubmit::Replay => {
                self.replay_pending_address_before_submit(fetch_request);
                Ok(None)
            }
        }
    }

    fn prepare_mmio_data_access(
        &self,
        scheduler: &PartitionedScheduler,
        bus: &MmioBus,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        if self.has_outstanding_data_request() {
            return Ok(None);
        }
        if let Some(fetch) = self.data_translation_page_map_required_fetch() {
            return Err(RiscvCpuError::DataTranslationPageMapRequired { fetch });
        }
        let Some((fetch_request, access)) = self.next_unissued_data_access() else {
            return Ok(None);
        };
        if self.pending_address_owns_fetch(fetch_request) {
            self.replay_pending_address_before_submit(fetch_request);
            return Ok(None);
        }
        let base_size = access_size(&access)?;
        let base_address = Address::new(access_address(&access));
        let request_span = masked_vector_memory_request_span(&access, base_address, base_size)?;
        let size = request_span.size;
        let address = request_span.address;
        let request_byte_offset = request_span.byte_offset;
        self.check_pmp_data_access(fetch_request, &access, size, address)?;
        self.check_pma_data_access(fetch_request, &access, size, address, request_byte_offset)?;
        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());
        let Some(route) = self.mmio_route_for_access(
            bus,
            request_id,
            &access,
            size,
            address,
            request_byte_offset,
        )?
        else {
            return Ok(None);
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
            request_byte_offset,
            line_layout: None,
            forwarded_load_data: None,
            store_load_forwarding_plan: None,
        }))
    }

    pub(crate) fn next_unissued_data_access_targets_mmio(
        &self,
        bus: &MmioBus,
    ) -> Result<bool, RiscvCpuError> {
        if let Some(fetch) = self.data_translation_page_map_required_fetch() {
            return Err(RiscvCpuError::DataTranslationPageMapRequired { fetch });
        }
        let Some((_, access)) = self.next_unissued_data_access() else {
            return Ok(false);
        };
        let base_size = access_size(&access)?;
        let base_address = Address::new(access_address(&access));
        let request_span = masked_vector_memory_request_span(&access, base_address, base_size)?;
        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());
        Ok(self
            .mmio_route_for_access(
                bus,
                request_id,
                &access,
                request_span.size,
                request_span.address,
                request_span.byte_offset,
            )?
            .is_some())
    }

    fn mmio_route_for_access(
        &self,
        bus: &MmioBus,
        request_id: MemoryRequestId,
        access: &MemoryAccessKind,
        size: AccessSize,
        address: Address,
        request_byte_offset: usize,
    ) -> Result<Option<MmioRoute>, RiscvCpuError> {
        let request = mmio_request(request_id, access, size, address, request_byte_offset)?;
        match bus.route_for(self.core.partition(), &request) {
            Ok(route) => Ok(Some(route)),
            Err(MmioError::UnmappedAddress { .. }) => Ok(None),
            Err(error) => Err(RiscvCpuError::Mmio(error)),
        }
    }

    pub(crate) fn check_pmp_data_access(
        &self,
        fetch: MemoryRequestId,
        access: &MemoryAccessKind,
        size: AccessSize,
        physical_address: Address,
    ) -> Result<(), RiscvCpuError> {
        let kind = pmp_access_kind(access);
        let state = self.state.lock().expect("riscv core lock");
        let privilege = state.hart.data_sv39_access_context().privilege();
        state
            .pmp
            .check_access(physical_address.get(), size.bytes(), kind, privilege)
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

    fn record_data_issue_state(&self, issue: OutstandingDataAccess, emit_issued_event: bool) {
        assert!(
            self.try_record_data_issue_state(issue, emit_issued_event, None),
            "O3-owned data issue must own an available memory slot"
        );
    }

    fn record_buffered_o3_effect_issue_state(
        &self,
        issue: OutstandingDataAccess,
        request: MemoryRequest,
        predecessor: MemoryRequestId,
    ) -> bool {
        self.try_record_data_issue_state(issue, true, Some((request, predecessor)))
    }

    fn try_record_data_issue_state(
        &self,
        issue: OutstandingDataAccess,
        emit_issued_event: bool,
        buffered_effect: Option<(MemoryRequest, MemoryRequestId)>,
    ) -> bool {
        self.core.advance_sequence_past(issue.request_id);
        let fetch_events = if o3_memory_result_window_destination(&issue.access).is_some() {
            self.core.fetch_events()
        } else {
            Vec::new()
        };
        let mut state = self.state.lock().expect("riscv core lock");
        if let Some((_, predecessor)) = buffered_effect.as_ref() {
            if !matches!(
                buffered_o3_effect_admission(&state, &issue),
                BufferedO3EffectAdmission::Buffered(current) if current == *predecessor
            ) {
                return false;
            }
        }
        let detailed = state.live_retire_gate.detailed_policy_enabled();
        let o3_data_access = is_deferred_o3_data_access(Some(&issue.access))
            && (detailed
                || state
                    .o3_runtime
                    .owns_pending_live_data_access_retirement(issue.fetch_request));
        let provisional_terminal_result = state
            .pending_terminal_memory_result
            .as_ref()
            .is_some_and(|pending| pending.owns_fetch(issue.fetch_request));
        let eligible_scalar_load = o3_data_access
            && !provisional_terminal_result
            && matches!(&issue.access, MemoryAccessKind::Load { .. })
            && o3_memory_result_destination(&issue.access).is_some()
            && matches!(&issue.target, RiscvDataAccessTarget::Memory { .. })
            && (state.data_translation.is_none()
                || state
                    .translated_scalar_load_window_fetches
                    .contains(&issue.fetch_request))
            && matches!(
                state
                    .pma
                    .is_uncacheable(issue.physical_address.get(), issue.size.bytes(),),
                Ok(false)
            );
        let memory_result_route = match &issue.target {
            RiscvDataAccessTarget::Memory { .. } => O3MemoryResultWindowRoute::Memory,
            RiscvDataAccessTarget::Mmio { .. } => O3MemoryResultWindowRoute::Mmio,
        };
        let memory_result_shape = o3_memory_result_window_destination(&issue.access);
        let expected_memory_result_role = if state.o3_runtime.has_live_data_access()
            && o3_memory_result_younger_buffered_effect_destination(&issue.access).is_some()
        {
            O3MemoryResultWindowRole::YoungerBufferedEffect
        } else if state.o3_runtime.has_live_data_access() {
            O3MemoryResultWindowRole::YoungerRead
        } else {
            O3MemoryResultWindowRole::Head
        };
        let eligible_memory_result_window = o3_data_access
            && !provisional_terminal_result
            && state
                .memory_result_window_authorizations
                .get(&issue.fetch_request)
                .copied()
                .is_some_and(|authorization| {
                    authorization.role() == expected_memory_result_role
                        && authorization.matches_resolved_range(
                            memory_result_route,
                            issue.physical_address,
                            issue.size,
                        )
                        && memory_result_shape == Some(authorization.integer_destination())
                        && (memory_result_route == O3MemoryResultWindowRoute::Mmio
                            || matches!(
                                state.pma.is_uncacheable(
                                    issue.physical_address.get(),
                                    issue.size.bytes(),
                                ),
                                Ok(false)
                            ))
                        && (memory_result_route != O3MemoryResultWindowRoute::Mmio
                            || matches!(
                                &issue.access,
                                MemoryAccessKind::Load { rd, .. } if !rd.is_zero()
                            ))
                });
        let younger_window_policy = if eligible_memory_result_window {
            O3DataAccessWindowPolicy::MemoryResultWindow
        } else if eligible_scalar_load && state.data_translation.is_none() {
            O3DataAccessWindowPolicy::UntranslatedScalarMemoryPrefix
        } else if eligible_scalar_load {
            O3DataAccessWindowPolicy::ScalarMemoryPrefix
        } else {
            O3DataAccessWindowPolicy::None
        };
        let execution = state.data_access_execution(issue.fetch_request).cloned();
        let pending_consumed = if o3_data_access {
            let Some(execution) = execution.as_ref() else {
                return false;
            };
            let consumed = state.o3_runtime.bind_pending_data_address_issue(
                execution,
                issue.request_id,
                issue.physical_address,
                issue.tick,
            );
            if consumed.is_none()
                && !state.o3_runtime.stage_live_data_access_issue(
                    execution,
                    issue.request_id,
                    issue.tick,
                    younger_window_policy,
                )
            {
                return false;
            }
            consumed
        } else {
            None
        };
        let pending_bound = pending_consumed.is_some();
        if let Some(consumed) = pending_consumed {
            state
                .events
                .push(execution.as_ref().expect("validated O3 execution").clone());
            state.executed_fetches.extend(consumed);
        }
        state
            .translated_scalar_load_window_fetches
            .remove(&issue.fetch_request);
        state
            .memory_result_window_authorizations
            .remove(&issue.fetch_request);
        state.issued_data_for_fetches.insert(issue.fetch_request);
        state
            .outstanding_data
            .insert(issue.request_id, issue.clone_without_layout());
        if o3_data_access
            && !pending_bound
            && younger_window_policy != O3DataAccessWindowPolicy::None
        {
            stage_o3_data_access_younger_window(
                &mut state,
                execution.as_ref().expect("validated O3 execution"),
                issue.tick,
                &fetch_events,
            );
        }
        if emit_issued_event {
            state
                .data_events
                .push(RiscvDataAccessEvent::issued(issue.record(issue.tick)));
        }
        if let Some((request, predecessor)) = buffered_effect {
            let request_id = issue.request_id;
            let buffered = BufferedO3Effect {
                predecessor,
                issue,
                request,
            };
            let replaced = state.buffered_o3_effects.insert(request_id, buffered);
            assert!(replaced.is_none(), "buffered O3 effect request is unique");
        }
        true
    }

    fn o3_scalar_load_wakeup_fetch_events(
        &self,
        request_id: MemoryRequestId,
    ) -> Vec<CpuFetchEvent> {
        let should_snapshot = {
            let state = self.state.lock().expect("riscv core lock");
            state
                .outstanding_data
                .get(&request_id)
                .is_some_and(|access| matches!(access.access, MemoryAccessKind::Load { .. }))
                && state
                    .o3_runtime
                    .live_data_access_younger_wakeup_seed()
                    .is_some()
        };
        if should_snapshot {
            self.core.fetch_events()
        } else {
            Vec::new()
        }
    }

    pub(crate) fn record_data_response(&self, delivery: ResponseDelivery) {
        let request_id = delivery.response().request_id();
        let fetch_events = matches!(delivery.response().status(), ResponseStatus::Completed)
            .then(|| self.o3_scalar_load_wakeup_fetch_events(request_id))
            .unwrap_or_default();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_callback_error.is_some() {
            return;
        }
        let Some(access) = state.outstanding_data.get(&request_id).cloned() else {
            return;
        };

        match delivery.response().status() {
            ResponseStatus::Completed => {
                let mut data = delivery.response().data().map(ToOwned::to_owned);
                let forwarding_plan = access.store_load_forwarding_plan.filter(|plan| {
                    plan.is_partial()
                        && data
                            .as_mut()
                            .is_some_and(|data| plan.overlay_response_data(data))
                });
                let completion = access.completion(data.clone());
                let deferred_retirement = deferred_o3_live_data_access_retirement(&state, &access);
                let completed_event = if deferred_retirement {
                    cloned_data_access_event_with_kind(
                        &state,
                        &access,
                        RiscvDataAccessEventKind::Completed,
                    )
                } else {
                    None
                };
                if deferred_retirement {
                    if let Err(error) = record_o3_data_access_outcome(
                        &mut state,
                        &access,
                        completed_event,
                        delivery.tick(),
                        Some(completion.clone()),
                        forwarding_plan,
                    ) {
                        record_callback_error(&mut state, error);
                        return;
                    }
                    state.outstanding_data.remove(&request_id);
                    let completed_event = mark_data_access_event_kind(
                        &mut state,
                        &access,
                        RiscvDataAccessEventKind::Completed,
                    );
                    debug_assert!(completed_event.is_some());
                } else {
                    state.outstanding_data.remove(&request_id);
                    let completed_event = record_data_retire_cycle(
                        &mut state,
                        &access,
                        delivery.tick(),
                        RiscvDataAccessEventKind::Completed,
                    );
                    if let Err(error) = record_o3_data_access_outcome(
                        &mut state,
                        &access,
                        completed_event,
                        delivery.tick(),
                        Some(completion.clone()),
                        forwarding_plan,
                    ) {
                        record_callback_error(&mut state, error);
                        return;
                    }
                }
                if !deferred_o3_data_completion_publication(&state, &access) {
                    apply_data_completion(&mut state, self.id(), &completion, "load response data");
                    riscv_checker::sync_checker_hart(&mut state);
                }
                if matches!(access.access, MemoryAccessKind::Load { .. }) {
                    wake_o3_data_access_younger_window(&mut state, delivery.tick(), &fetch_events);
                }
                state.data_events.push(RiscvDataAccessEvent::completed(
                    access.record(delivery.tick()),
                    data,
                ));
            }
            ResponseStatus::Retry => {
                state.outstanding_data.remove(&request_id);
                let retry_event = mark_data_access_event_kind(
                    &mut state,
                    &access,
                    RiscvDataAccessEventKind::Retry,
                );
                if let Err(error) = record_o3_data_access_outcome(
                    &mut state,
                    &access,
                    retry_event,
                    delivery.tick(),
                    None,
                    None,
                ) {
                    record_callback_error(&mut state, error);
                    return;
                }
                state
                    .data_events
                    .push(RiscvDataAccessEvent::retry(access.record(delivery.tick())));
            }
            ResponseStatus::StoreConditionalFailed => {
                state.outstanding_data.remove(&request_id);
                if !matches!(&access.access, MemoryAccessKind::StoreConditional { .. }) {
                    debug_assert!(false, "store-conditional failure for non-SC access");
                    state
                        .o3_runtime
                        .discard_data_access_outcome(access.fetch_request);
                    state
                        .data_events
                        .push(RiscvDataAccessEvent::retry(access.record(delivery.tick())));
                    return;
                }
                self.record_store_conditional_failure_outcome(&mut state, access, delivery.tick());
            }
        }
    }

    pub fn record_data_failure(&self, request_id: MemoryRequestId, tick: Tick) {
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_callback_error.is_some() {
            return;
        }
        let Some(access) = state.outstanding_data.remove(&request_id) else {
            return;
        };
        let failed_event =
            mark_data_access_event_kind(&mut state, &access, RiscvDataAccessEventKind::Failed);
        if let Err(error) =
            record_o3_data_access_outcome(&mut state, &access, failed_event, tick, None, None)
        {
            record_callback_error(&mut state, error);
            return;
        }
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
        if state.pending_callback_error.is_some() {
            return;
        }
        let Some(access) = state.outstanding_data.get(&request_id).cloned() else {
            return;
        };

        match completion.response() {
            Ok(response) => {
                let data = response.data().map(ToOwned::to_owned);
                let data_completion = access.completion(data.clone());
                let deferred_retirement = deferred_o3_live_data_access_retirement(&state, &access);
                let completed_event = if deferred_retirement {
                    cloned_data_access_event_with_kind(
                        &state,
                        &access,
                        RiscvDataAccessEventKind::Completed,
                    )
                } else {
                    None
                };
                if deferred_retirement {
                    if let Err(error) = record_o3_data_access_outcome(
                        &mut state,
                        &access,
                        completed_event,
                        completion.tick(),
                        Some(data_completion.clone()),
                        None,
                    ) {
                        record_callback_error(&mut state, error);
                        return;
                    }
                    state.outstanding_data.remove(&request_id);
                    let completed_event = mark_data_access_event_kind(
                        &mut state,
                        &access,
                        RiscvDataAccessEventKind::Completed,
                    );
                    debug_assert!(completed_event.is_some());
                } else {
                    state.outstanding_data.remove(&request_id);
                    let completed_event = record_data_retire_cycle(
                        &mut state,
                        &access,
                        completion.tick(),
                        RiscvDataAccessEventKind::Completed,
                    );
                    if let Err(error) = record_o3_data_access_outcome(
                        &mut state,
                        &access,
                        completed_event,
                        completion.tick(),
                        Some(data_completion.clone()),
                        None,
                    ) {
                        record_callback_error(&mut state, error);
                        return;
                    }
                }
                if !deferred_o3_data_completion_publication(&state, &access) {
                    apply_data_completion(
                        &mut state,
                        self.id(),
                        &data_completion,
                        "MMIO load response data",
                    );
                    riscv_checker::sync_checker_hart(&mut state);
                }
                state.data_events.push(RiscvDataAccessEvent::completed(
                    access.record(completion.tick()),
                    data,
                ));
            }
            Err(_) => {
                state.outstanding_data.remove(&request_id);
                let retry_event = mark_data_access_event_kind(
                    &mut state,
                    &access,
                    RiscvDataAccessEventKind::Retry,
                );
                if let Err(error) = record_o3_data_access_outcome(
                    &mut state,
                    &access,
                    retry_event,
                    completion.tick(),
                    None,
                    None,
                ) {
                    record_callback_error(&mut state, error);
                    return;
                }
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
    kind: RiscvDataAccessEventKind,
) -> Option<RiscvCpuExecutionEvent> {
    record_data_retire_cycle_for_fetch(
        state,
        access.fetch_request,
        access.tick,
        completion_tick,
        kind,
    )
}

pub(crate) fn record_deferred_o3_data_retire_cycle(
    state: &mut RiscvCoreState,
    fetch_request: MemoryRequestId,
    issue_tick: Tick,
    completion_tick: Tick,
) -> Option<RiscvCpuExecutionEvent> {
    let kind = state
        .o3_runtime
        .ready_live_data_access_event_kind()
        .expect("completed O3 data access has a terminal event kind");
    record_data_retire_cycle_for_fetch(state, fetch_request, issue_tick, completion_tick, kind)
}

fn record_data_retire_cycle_for_fetch(
    state: &mut RiscvCoreState,
    fetch_request: MemoryRequestId,
    issue_tick: Tick,
    completion_tick: Tick,
    kind: RiscvDataAccessEventKind,
) -> Option<RiscvCpuExecutionEvent> {
    let Some(index) = state
        .events
        .iter()
        .position(|event| event.fetch().request_id() == fetch_request)
    else {
        debug_assert!(
            false,
            "completed data access must have a matching execution event"
        );
        return None;
    };
    state.events[index].set_data_access_event_kind(kind);
    let data_wait_cycles = completion_tick.saturating_sub(issue_tick);
    if state.events[index].in_order_pipeline_cycle().is_some()
        || !state.events[index].counts_as_retired_instruction()
    {
        return Some(state.events[index].clone());
    }
    let attributed_data_wait_cycles =
        retag_existing_fetch_wait_cycles_for_data_access(state, fetch_request, data_wait_cycles);
    let remaining_data_wait_cycles = data_wait_cycles.saturating_sub(attributed_data_wait_cycles);
    let execute_wait_cycles =
        riscv_data_completion_execute_wait_cycles(state.events[index].instruction());
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
        fetch_request.sequence(),
        None,
        &waits,
    )
    .expect("completed data access records one in-order retire cycle");
    state.events[index].set_in_order_pipeline_cycle(cycle);
    state.events[index].set_in_order_pipeline_data_wait_cycles(data_wait_cycles);
    Some(state.events[index].clone())
}

fn deferred_o3_live_data_access_retirement(
    state: &RiscvCoreState,
    access: &IssuedDataAccess,
) -> bool {
    state
        .o3_runtime
        .owns_pending_live_data_access_retirement(access.fetch_request)
}

fn deferred_o3_data_completion_publication(
    state: &RiscvCoreState,
    access: &IssuedDataAccess,
) -> bool {
    deferred_o3_live_data_access_retirement(state, access)
        && (o3_memory_result_destination(&access.access).is_some()
            || matches!(&access.access, MemoryAccessKind::StoreConditional { .. }))
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
    pub(crate) forwarded_load_data: Option<Vec<u8>>,
    pub(crate) store_load_forwarding_plan: Option<O3StoreLoadForwardingPlan>,
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
            store_load_forwarding_plan: self.store_load_forwarding_plan,
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
    store_load_forwarding_plan: Option<O3StoreLoadForwardingPlan>,
}

impl IssuedDataAccess {
    fn completion(&self, bytes: Option<Vec<u8>>) -> RiscvDataCompletion {
        RiscvDataCompletion::from_issued_response(
            self.fetch_request,
            self.access.clone(),
            self.physical_address,
            self.size,
            self.request_byte_offset,
            bytes,
        )
    }

    pub(crate) fn memory_range(&self) -> Option<AddressRange> {
        if !matches!(self.target, RiscvDataAccessTarget::Memory { .. }) {
            return None;
        }
        AddressRange::new(self.physical_address, self.size).ok()
    }

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

#[cfg(test)]
#[path = "riscv_data_issue_tests.rs"]
mod tests;

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
