use rem6_kernel::{
    ParallelSchedulerContext, PartitionEventId, PartitionedScheduler, SchedulerContext, Tick,
};
use rem6_memory::{
    Address, ByteMask, MemoryRequestId, TranslationFault, TranslationPageMap, TranslationRequestId,
    TranslationTlbStats,
};
use rem6_mmio::{MmioBus, MmioError};
use rem6_transport::{
    MemoryTrace, MemoryTransport, ParallelMemoryTransaction, RequestDelivery, TargetOutcome,
    TransportError,
};

use crate::riscv_data_issue::{
    access_width, memory_width_size, mmio_request, store_bytes, OutstandingDataAccess,
    PreparedDataParallelAccess,
};
use crate::{
    riscv_data_access, CpuDataConfig, CpuTranslationFrontend, CpuTranslationOutcome,
    CpuTranslationRequest, RiscvCore, RiscvCoreDriveAction, RiscvCoreState, RiscvCpuError,
    RiscvDataAccessTarget,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct PendingDataTranslation {
    request_id: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: rem6_isa_riscv::MemoryAccessKind,
    size: rem6_memory::AccessSize,
}

impl PendingDataTranslation {
    pub(crate) const fn fetch_request(&self) -> MemoryRequestId {
        self.fetch_request
    }
}

impl RiscvCoreState {
    pub(super) fn next_unissued_data_access(
        &self,
    ) -> Option<(MemoryRequestId, rem6_isa_riscv::MemoryAccessKind)> {
        self.events.iter().find_map(|event| {
            let fetch_request = event.fetch().request_id();
            if self.issued_data_for_fetches.contains(&fetch_request) {
                return None;
            }
            if self
                .pending_data_translations
                .values()
                .any(|pending| pending.fetch_request() == fetch_request)
            {
                return None;
            }
            if self.ready_translated_data.contains_key(&fetch_request) {
                return None;
            }
            event
                .execution()
                .memory_access()
                .map(|access| (fetch_request, access.clone()))
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TranslatedDataAccess {
    request_id: MemoryRequestId,
    fetch_request: MemoryRequestId,
    access: rem6_isa_riscv::MemoryAccessKind,
    size: rem6_memory::AccessSize,
    physical_address: Address,
}

impl RiscvCore {
    pub fn with_data_translation(
        core: crate::CpuCore,
        data: CpuDataConfig,
        data_translation: CpuTranslationFrontend,
    ) -> Self {
        let core = Self::with_data(core, data);
        core.state.lock().expect("riscv core lock").data_translation = Some(data_translation);
        core
    }

    pub fn data_translation_tlb_stats(&self) -> Option<TranslationTlbStats> {
        self.state
            .lock()
            .expect("riscv core lock")
            .data_translation
            .as_ref()
            .and_then(|frontend| frontend.tlb().map(|tlb| tlb.stats()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn drive_next_action_with_data_translation<F, D>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        fetch_trace: MemoryTrace,
        data_trace: MemoryTrace,
        page_map: &TranslationPageMap,
        fetch_responder: F,
        data_responder: D,
    ) -> Result<Option<RiscvCoreDriveAction>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
        D: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        if self.core.has_pending_fetch() || self.has_outstanding_data_request() {
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

        let had_unissued_data = self.has_unissued_data_access();
        if let Some(event) = self.issue_next_translated_data_access(
            scheduler,
            transport,
            data_trace,
            page_map,
            data_responder,
        )? {
            return Ok(Some(RiscvCoreDriveAction::DataAccessIssued { event }));
        }
        if had_unissued_data || self.has_pending_data_access() {
            return Ok(None);
        }

        let event = self
            .issue_next_fetch(scheduler, transport, fetch_trace, fetch_responder)
            .map_err(RiscvCpuError::Cpu)?;
        Ok(Some(RiscvCoreDriveAction::FetchIssued { event }))
    }

    pub fn issue_next_translated_data_access<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        page_map: &TranslationPageMap,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut SchedulerContext<'_>) -> TargetOutcome + Send + 'static,
    {
        let Some(issue) =
            self.prepare_next_translated_data_access(scheduler.now(), transport, page_map)?
        else {
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

    pub fn issue_next_translated_data_access_parallel<F>(
        &self,
        scheduler: &mut PartitionedScheduler,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        page_map: &TranslationPageMap,
        responder: F,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(prepared) = self.prepare_translated_data_parallel_access(
            scheduler.now(),
            transport,
            trace,
            page_map,
            responder,
        )?
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
                    .expect("single translated data transaction returns one event");

                self.record_data_issue(issue);
                Ok(Some(event))
            }
            PreparedDataParallelAccess::ConditionalFailed { issue } => self
                .schedule_store_conditional_failure_parallel(scheduler, issue)
                .map(Some),
        }
    }

    pub fn issue_next_translated_mmio_data_access_parallel(
        &self,
        scheduler: &mut PartitionedScheduler,
        bus: &MmioBus,
        page_map: &TranslationPageMap,
    ) -> Result<Option<PartitionEventId>, RiscvCpuError> {
        let Some(issue) =
            self.prepare_next_translated_mmio_data_access(scheduler, bus, page_map)?
        else {
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
                .expect("validated translated parallel MMIO data access submission");
            })
            .map_err(RiscvCpuError::Scheduler)?;

        self.record_data_issue(issue);
        Ok(Some(event))
    }

    pub(crate) fn prepare_translated_data_parallel_access<F>(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        trace: MemoryTrace,
        page_map: &TranslationPageMap,
        responder: F,
    ) -> Result<Option<PreparedDataParallelAccess>, RiscvCpuError>
    where
        F: FnOnce(RequestDelivery, &mut ParallelSchedulerContext<'_>) -> TargetOutcome
            + Send
            + 'static,
    {
        let Some(issue) = self.prepare_next_translated_data_access(tick, transport, page_map)?
        else {
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

    fn prepare_next_translated_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        page_map: &TranslationPageMap,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        self.complete_ready_data_translations_with_page_map(tick, page_map)?;
        let mut issue = self.prepare_ready_translated_data_access(tick, transport)?;
        if issue.is_none() && self.enqueue_next_data_translation(tick)? {
            self.complete_ready_data_translations_with_page_map(tick, page_map)?;
            issue = self.prepare_ready_translated_data_access(tick, transport)?;
        }

        Ok(issue)
    }

    fn prepare_next_translated_mmio_data_access(
        &self,
        scheduler: &PartitionedScheduler,
        bus: &MmioBus,
        page_map: &TranslationPageMap,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let tick = scheduler.now();
        self.complete_ready_data_translations_with_page_map(tick, page_map)?;
        let mut issue = self.prepare_ready_translated_mmio_data_access(scheduler, bus)?;
        if issue.is_none() && self.enqueue_next_data_translation(tick)? {
            self.complete_ready_data_translations_with_page_map(tick, page_map)?;
            issue = self.prepare_ready_translated_mmio_data_access(scheduler, bus)?;
        }

        Ok(issue)
    }

    fn enqueue_next_data_translation(&self, tick: Tick) -> Result<bool, RiscvCpuError> {
        let Some((fetch_request, access)) = self.next_unissued_data_access() else {
            return Ok(false);
        };
        let size = memory_width_size(access_width(&access))?;
        let data = self
            .state
            .lock()
            .expect("riscv core lock")
            .data
            .clone()
            .ok_or(RiscvCpuError::MissingDataConfig {
                fetch: fetch_request,
            })?;
        let request_id = MemoryRequestId::new(self.core.agent(), self.core.next_sequence());
        let translation_id = TranslationRequestId::new(self.core.agent(), request_id.sequence());
        let pending = PendingDataTranslation {
            request_id,
            fetch_request,
            access: access.clone(),
            size,
        };
        let request = cpu_translation_request(translation_id, request_id, &data, &access, size)?;

        let mut state = self.state.lock().expect("riscv core lock");
        let frontend =
            state
                .data_translation
                .as_mut()
                .ok_or(RiscvCpuError::MissingDataTranslationConfig {
                    fetch: fetch_request,
                })?;
        match frontend
            .enqueue_or_translate_cached(tick, request)
            .map_err(RiscvCpuError::DataTranslation)?
        {
            Some(outcome) => {
                let translated = translated_data_from_outcome(pending, outcome)?;
                state
                    .ready_translated_data
                    .insert(translated.fetch_request, translated);
            }
            None => {
                state
                    .pending_data_translations
                    .insert(translation_id, pending);
            }
        }

        Ok(true)
    }

    fn complete_ready_data_translations_with_page_map(
        &self,
        tick: Tick,
        page_map: &TranslationPageMap,
    ) -> Result<(), RiscvCpuError> {
        let mut state = self.state.lock().expect("riscv core lock");
        let Some(frontend) = state.data_translation.as_mut() else {
            return Ok(());
        };
        let outcomes = frontend
            .complete_ready_with_tlb_page_map(tick, page_map)
            .map_err(RiscvCpuError::DataTranslation)?;

        for outcome in outcomes {
            let translation_id = match &outcome {
                CpuTranslationOutcome::Mapped(mapped) => mapped.translation_id(),
                CpuTranslationOutcome::Fault(fault) => fault.translation_id(),
            };
            let pending = state
                .pending_data_translations
                .remove(&translation_id)
                .expect("ready data translation has matching RISC-V metadata");
            let translated = translated_data_from_outcome(pending, outcome)?;
            state
                .ready_translated_data
                .insert(translated.fetch_request, translated);
        }

        Ok(())
    }

    fn prepare_ready_translated_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let translated = {
            let mut state = self.state.lock().expect("riscv core lock");
            let Some(fetch_request) = ready_translated_fetch_request(&state) else {
                return Ok(None);
            };
            state
                .ready_translated_data
                .remove(&fetch_request)
                .expect("selected ready data translation exists")
        };

        self.prepare_translated_data_access(tick, transport, translated)
            .map(Some)
    }

    fn prepare_ready_translated_mmio_data_access(
        &self,
        scheduler: &PartitionedScheduler,
        bus: &MmioBus,
    ) -> Result<Option<OutstandingDataAccess>, RiscvCpuError> {
        let tick = scheduler.now();
        let translated = {
            let state = self.state.lock().expect("riscv core lock");
            let Some(fetch_request) = ready_translated_fetch_request(&state) else {
                return Ok(None);
            };
            state
                .ready_translated_data
                .get(&fetch_request)
                .expect("selected ready data translation exists")
                .clone()
        };

        let request = mmio_request(
            translated.request_id,
            &translated.access,
            translated.size,
            translated.physical_address,
        )?;
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
            tick,
            scheduler.min_remote_delay(),
            scheduler.partition_count(),
        )
        .map_err(|error| RiscvCpuError::Mmio(MmioError::Scheduler(error)))?;

        {
            let mut state = self.state.lock().expect("riscv core lock");
            state
                .ready_translated_data
                .remove(&translated.fetch_request)
                .expect("selected ready data translation exists");
        }

        Ok(Some(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Mmio { route },
            request_id: translated.request_id,
            fetch_request: translated.fetch_request,
            access: translated.access,
            size: translated.size,
            physical_address: translated.physical_address,
            line_layout: None,
        }))
    }

    fn prepare_translated_data_access(
        &self,
        tick: Tick,
        transport: &MemoryTransport,
        translated: TranslatedDataAccess,
    ) -> Result<OutstandingDataAccess, RiscvCpuError> {
        let data = self
            .state
            .lock()
            .expect("riscv core lock")
            .data
            .clone()
            .ok_or(RiscvCpuError::MissingDataConfig {
                fetch: translated.fetch_request,
            })?;
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

        let line_layout = data
            .line_layout_for_access(translated.physical_address, translated.size)
            .map_err(RiscvCpuError::Memory)?;
        let line_offset = line_layout.line_offset(translated.physical_address);
        if line_offset + translated.size.bytes() > line_layout.bytes() {
            return Err(RiscvCpuError::DataAccessCrossesLine {
                address: translated.physical_address,
                size: translated.size,
                line_size: line_layout.bytes(),
            });
        }

        Ok(OutstandingDataAccess {
            tick,
            partition: self.core.partition(),
            target: RiscvDataAccessTarget::Memory {
                route: data.route(),
                endpoint: data.endpoint().clone(),
            },
            request_id: translated.request_id,
            fetch_request: translated.fetch_request,
            access: translated.access,
            size: translated.size,
            physical_address: translated.physical_address,
            line_layout: Some(line_layout),
        })
    }
}

fn cpu_translation_request(
    translation_id: TranslationRequestId,
    memory_request_id: MemoryRequestId,
    data: &CpuDataConfig,
    access: &rem6_isa_riscv::MemoryAccessKind,
    size: rem6_memory::AccessSize,
) -> Result<CpuTranslationRequest, RiscvCpuError> {
    match access {
        rem6_isa_riscv::MemoryAccessKind::Load { address, .. }
        | rem6_isa_riscv::MemoryAccessKind::LoadReserved { address, .. } => {
            CpuTranslationRequest::load(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                Address::new(*address),
                size,
            )
        }
        rem6_isa_riscv::MemoryAccessKind::Store { address, value, .. } => {
            CpuTranslationRequest::store(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                Address::new(*address),
                size,
                store_bytes(*value, size),
                ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
            )
        }
        rem6_isa_riscv::MemoryAccessKind::StoreConditional { address, value, .. } => {
            CpuTranslationRequest::atomic(
                translation_id,
                memory_request_id,
                data.route(),
                data.endpoint().clone(),
                Address::new(*address),
                size,
                store_bytes(*value, size),
                ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
            )
        }
        rem6_isa_riscv::MemoryAccessKind::AtomicMemory {
            address, value, op, ..
        } => CpuTranslationRequest::atomic_with_op(
            translation_id,
            memory_request_id,
            data.route(),
            data.endpoint().clone(),
            Address::new(*address),
            size,
            match op {
                rem6_isa_riscv::AtomicMemoryOp::Swap => rem6_memory::MemoryAtomicOp::Swap,
                rem6_isa_riscv::AtomicMemoryOp::Add => rem6_memory::MemoryAtomicOp::Add,
                rem6_isa_riscv::AtomicMemoryOp::Xor => rem6_memory::MemoryAtomicOp::Xor,
                rem6_isa_riscv::AtomicMemoryOp::Or => rem6_memory::MemoryAtomicOp::Or,
                rem6_isa_riscv::AtomicMemoryOp::And => rem6_memory::MemoryAtomicOp::And,
                rem6_isa_riscv::AtomicMemoryOp::MinSigned => rem6_memory::MemoryAtomicOp::MinSigned,
                rem6_isa_riscv::AtomicMemoryOp::MaxSigned => rem6_memory::MemoryAtomicOp::MaxSigned,
                rem6_isa_riscv::AtomicMemoryOp::MinUnsigned => {
                    rem6_memory::MemoryAtomicOp::MinUnsigned
                }
                rem6_isa_riscv::AtomicMemoryOp::MaxUnsigned => {
                    rem6_memory::MemoryAtomicOp::MaxUnsigned
                }
            },
            store_bytes(*value, size),
            ByteMask::full(size).map_err(RiscvCpuError::Memory)?,
        ),
    }
    .map_err(RiscvCpuError::DataTranslation)
}

fn ready_translated_fetch_request(state: &RiscvCoreState) -> Option<MemoryRequestId> {
    state.events.iter().find_map(|event| {
        let fetch_request = event.fetch().request_id();
        if state.issued_data_for_fetches.contains(&fetch_request) {
            return None;
        }
        state
            .ready_translated_data
            .contains_key(&fetch_request)
            .then_some(fetch_request)
    })
}

fn translated_data_from_outcome(
    pending: PendingDataTranslation,
    outcome: CpuTranslationOutcome,
) -> Result<TranslatedDataAccess, RiscvCpuError> {
    match outcome {
        CpuTranslationOutcome::Mapped(mapped) => {
            debug_assert_eq!(mapped.memory_request_id(), pending.request_id);
            debug_assert_eq!(mapped.size(), pending.size);
            Ok(TranslatedDataAccess {
                request_id: mapped.memory_request_id(),
                fetch_request: pending.fetch_request,
                access: pending.access,
                size: mapped.size(),
                physical_address: mapped.physical_address(),
            })
        }
        CpuTranslationOutcome::Fault(fault) => Err(data_translation_fault(
            pending.fetch_request,
            fault.fault().clone(),
        )),
    }
}

fn data_translation_fault(fetch: MemoryRequestId, fault: TranslationFault) -> RiscvCpuError {
    RiscvCpuError::DataTranslationFault { fetch, fault }
}
