use rem6_kernel::{PartitionEventId, PartitionedScheduler, SchedulerContext, Tick};
use rem6_memory::{
    Address, ByteMask, MemoryRequestId, TranslationFault, TranslationPageMap, TranslationRequestId,
    TranslationTlbStats,
};
use rem6_transport::{
    MemoryTrace, MemoryTransport, RequestDelivery, TargetOutcome, TransportError,
};

use crate::{
    access_width, memory_width_size, store_bytes, CpuDataConfig, CpuTranslationFrontend,
    CpuTranslationOutcome, CpuTranslationRequest, RiscvCore, RiscvCpuError, RiscvDataAccessTarget,
};

use super::OutstandingDataAccess;

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
        self.complete_ready_data_translations_with_page_map(scheduler.now(), page_map)?;
        let mut issue = self.prepare_ready_translated_data_access(scheduler.now(), transport)?;
        if issue.is_none() && self.enqueue_next_data_translation(scheduler.now())? {
            self.complete_ready_data_translations_with_page_map(scheduler.now(), page_map)?;
            issue = self.prepare_ready_translated_data_access(scheduler.now(), transport)?;
        }

        let Some(issue) = issue else {
            return Ok(None);
        };
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
            let Some(fetch_request) = state.events.iter().find_map(|event| {
                let fetch_request = event.fetch().request_id();
                if state.issued_data_for_fetches.contains(&fetch_request) {
                    return None;
                }
                state
                    .ready_translated_data
                    .contains_key(&fetch_request)
                    .then_some(fetch_request)
            }) else {
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
        rem6_isa_riscv::MemoryAccessKind::Load { address, .. } => CpuTranslationRequest::load(
            translation_id,
            memory_request_id,
            data.route(),
            data.endpoint().clone(),
            Address::new(*address),
            size,
        ),
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
    }
    .map_err(RiscvCpuError::DataTranslation)
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
