use rem6_isa_riscv::{MemoryAccessKind, RiscvInstruction};
use rem6_memory::Address;
use rem6_mmio::{MmioBus, MmioError, MmioRequest, MmioRequestId};

use crate::{riscv_data_issue::mmio_request, CpuFetchEventKind, RiscvCore, RiscvCpuError};

use super::{
    can_retire_completed_fetch_with_branch_speculations, completed_fetch_window, detailed_o3,
    fetch_ahead_decision, hart_has_enabled_pending_interrupt, next_fetch_ahead_candidate,
    preview_selected_branch_speculation, PreparedRiscvFetchAheadSpeculation,
    ProducerForwardedScalarContinuation, RiscvFetchAheadDecision,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DataAccessResultHeadRoute {
    Memory,
    Mmio,
    Blocked,
}

impl RiscvCore {
    pub(crate) fn next_fetch_ahead_before_retire(&self) -> Option<RiscvFetchAheadDecision> {
        self.next_fetch_ahead_before_retire_with_translation(
            detailed_o3::TranslatedMemoryFetchAhead::Disabled,
        )
    }

    pub(crate) fn next_producer_forwarded_fetch_ahead_before_retire(
        &self,
    ) -> Option<RiscvFetchAheadDecision> {
        producer_forwarded_control_decision(self.next_fetch_ahead_before_retire())
    }

    pub(crate) fn next_pending_data_fetch_ahead(
        &self,
        pending_data_blocks_new_work: bool,
    ) -> Option<RiscvFetchAheadDecision> {
        if pending_data_blocks_new_work {
            (!self.has_pending_fetch())
                .then(|| self.next_producer_forwarded_fetch_ahead_before_retire())
                .flatten()
        } else {
            self.next_fetch_ahead_before_retire()
        }
    }

    pub(crate) fn next_cached_translated_memory_fetch_ahead_before_retire(
        &self,
    ) -> Option<RiscvFetchAheadDecision> {
        self.next_fetch_ahead_before_retire_with_translation(
            detailed_o3::TranslatedMemoryFetchAhead::CachedMemory,
        )
    }

    pub(crate) fn next_mmio_aware_fetch_ahead_before_retire(
        &self,
        bus: &MmioBus,
    ) -> Option<RiscvFetchAheadDecision> {
        let translated = match self.data_access_result_head_route(bus) {
            DataAccessResultHeadRoute::Memory => {
                detailed_o3::TranslatedMemoryFetchAhead::CachedMemory
            }
            DataAccessResultHeadRoute::Mmio => detailed_o3::TranslatedMemoryFetchAhead::Mmio,
            DataAccessResultHeadRoute::Blocked => detailed_o3::TranslatedMemoryFetchAhead::Blocked,
        };
        self.next_fetch_ahead_before_retire_with_translation(translated)
    }

    pub(crate) fn next_mmio_aware_producer_forwarded_fetch_ahead_before_retire(
        &self,
        bus: &MmioBus,
    ) -> Option<RiscvFetchAheadDecision> {
        producer_forwarded_control_decision(self.next_mmio_aware_fetch_ahead_before_retire(bus))
    }

    pub(crate) fn next_pending_data_mmio_fetch_ahead(
        &self,
        bus: &MmioBus,
        pending_data_blocks_new_work: bool,
    ) -> Option<RiscvFetchAheadDecision> {
        if pending_data_blocks_new_work {
            (!self.has_pending_fetch())
                .then(|| self.next_mmio_aware_producer_forwarded_fetch_ahead_before_retire(bus))
                .flatten()
        } else {
            self.next_mmio_aware_fetch_ahead_before_retire(bus)
        }
    }

    fn next_fetch_ahead_before_retire_with_translation(
        &self,
        translated: detailed_o3::TranslatedMemoryFetchAhead,
    ) -> Option<RiscvFetchAheadDecision> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some() || state.pending_fetch_prefix.is_some() {
            return None;
        }
        if hart_has_enabled_pending_interrupt(&state.hart) {
            return None;
        }

        let mut completed = fetch_events
            .iter()
            .filter(|event| {
                event.kind() == CpuFetchEventKind::Completed
                    && !state.executed_fetches.contains(&event.request_id())
            })
            .collect::<Vec<_>>();
        completed.sort_by_key(|event| event.request_id().sequence());
        let staged_producer_forwarded_descendant =
            crate::riscv_live_retire_window::stage_o3_producer_forwarded_control_descendant(
                &mut state,
                &fetch_events,
            );
        crate::riscv_live_retire_window::stage_o3_producer_forwarded_scalar_return_descendant(
            &mut state,
            &fetch_events,
        );
        let has_producer_forwarded_return = state
            .o3_runtime
            .producer_forwarded_return_descendant()
            .is_some();
        let has_producer_forwarded_scalar = state
            .o3_runtime
            .producer_forwarded_scalar_descendant()
            .is_some();
        if staged_producer_forwarded_descendant
            && !has_producer_forwarded_return
            && !has_producer_forwarded_scalar
        {
            return None;
        }
        if state.o3_runtime.has_ready_live_data_access_event()
            && !has_producer_forwarded_return
            && !has_producer_forwarded_scalar
        {
            return None;
        }

        let fetch = match detailed_o3::additional_fetch_candidate(
            &state,
            &fetch_events,
            &completed,
            translated,
        ) {
            detailed_o3::DetailedFetchAheadCandidate::Ready(pc) => {
                return Some(RiscvFetchAheadDecision::straight_line(pc));
            }
            detailed_o3::DetailedFetchAheadCandidate::ReadyProducerForwardedScalar {
                pc,
                descendant,
            } => {
                return Some(
                    RiscvFetchAheadDecision::straight_line(pc)
                        .with_producer_forwarded_scalar_continuation(descendant),
                );
            }
            detailed_o3::DetailedFetchAheadCandidate::ReadyPredictedControl {
                request,
                pc,
                sequential_pc,
                instruction,
                target_authority,
            } => {
                return fetch_ahead_decision(
                    &mut state,
                    &completed,
                    request,
                    pc,
                    sequential_pc,
                    instruction,
                    target_authority,
                    translated,
                );
            }
            detailed_o3::DetailedFetchAheadCandidate::ReadyCachedTranslatedLoad {
                pc,
                fetch_request,
            } => {
                state
                    .cached_translated_scalar_load_window_fetches
                    .insert(fetch_request);
                return Some(RiscvFetchAheadDecision::straight_line(pc));
            }
            detailed_o3::DetailedFetchAheadCandidate::Blocked => return None,
            detailed_o3::DetailedFetchAheadCandidate::NotApplicable => {
                if completed.len() >= completed_fetch_window(&state) {
                    return None;
                }
                next_fetch_ahead_candidate(&state, &completed)?
            }
        };
        let data = fetch.data()?;
        let raw = match data {
            [a, b, c, d] => u32::from_le_bytes([*a, *b, *c, *d]),
            _ => return None,
        };
        let Ok(decoded) = RiscvInstruction::decode_with_length(raw) else {
            return None;
        };
        let sequential_pc = Address::new(fetch.pc().get().wrapping_add(u64::from(decoded.bytes())));
        if detailed_o3::data_access_has_younger_fetch(
            &state,
            &fetch_events,
            fetch.request_id(),
            sequential_pc,
            decoded.instruction(),
            translated,
        ) {
            return None;
        }

        fetch_ahead_decision(
            &mut state,
            &completed,
            fetch.request_id(),
            fetch.pc(),
            sequential_pc,
            decoded.instruction(),
            detailed_o3::PredictedControlTargetAuthority::Normal,
            translated,
        )
    }

    pub(crate) fn set_fetch_ahead_pc(&self, pc: Address) {
        self.core.set_pc(pc);
    }

    pub(crate) fn prepare_fetch_ahead_speculation(
        &self,
        decision: &RiscvFetchAheadDecision,
    ) -> Result<Option<PreparedRiscvFetchAheadSpeculation>, RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let state = self.state.lock().expect("riscv core lock");
        if let Some(descendant) = decision.producer_forwarded_scalar_continuation {
            return Ok(
                ProducerForwardedScalarContinuation::capture(&state, descendant)
                    .map(PreparedRiscvFetchAheadSpeculation::scalar),
            );
        }
        let Some(speculation) = decision.branch_speculation() else {
            return Ok(None);
        };
        if state
            .branch_speculations
            .contains_key(&speculation.sequence)
        {
            return Ok(None);
        }
        let selected = speculation
            .selected_speculation
            .as_ref()
            .map(|selected| {
                preview_selected_branch_speculation(
                    &state,
                    &fetch_events,
                    speculation.sequence,
                    selected,
                )
            })
            .transpose()?;
        Ok(Some(PreparedRiscvFetchAheadSpeculation::branch(
            speculation.clone(),
            selected,
        )))
    }

    pub(crate) fn record_prepared_fetch_ahead_speculation(
        &self,
        prepared: Option<PreparedRiscvFetchAheadSpeculation>,
    ) {
        let Some(prepared) = prepared else {
            return;
        };
        let mut state = self.state.lock().expect("riscv core lock");
        prepared.apply(&mut state);
    }

    pub(crate) fn can_retire_completed_fetch_while_fetch_pending(
        &self,
    ) -> Result<bool, RiscvCpuError> {
        self.can_retire_completed_fetch_while_fetch_pending_with_translation(
            detailed_o3::TranslatedMemoryFetchAhead::Disabled,
        )
    }

    pub(crate) fn can_retire_completed_fetch_while_cached_translated_memory_fetch_pending(
        &self,
    ) -> Result<bool, RiscvCpuError> {
        self.can_retire_completed_fetch_while_fetch_pending_with_translation(
            detailed_o3::TranslatedMemoryFetchAhead::CachedMemory,
        )
    }

    pub(crate) fn can_retire_completed_fetch_while_mmio_aware_fetch_pending(
        &self,
        bus: &MmioBus,
    ) -> Result<bool, RiscvCpuError> {
        let translated = match self.data_access_result_head_route(bus) {
            DataAccessResultHeadRoute::Memory => {
                detailed_o3::TranslatedMemoryFetchAhead::CachedMemory
            }
            DataAccessResultHeadRoute::Mmio => detailed_o3::TranslatedMemoryFetchAhead::Mmio,
            DataAccessResultHeadRoute::Blocked => detailed_o3::TranslatedMemoryFetchAhead::Blocked,
        };
        self.can_retire_completed_fetch_while_fetch_pending_with_translation(translated)
    }

    fn data_access_result_head_route(&self, bus: &MmioBus) -> DataAccessResultHeadRoute {
        let fetch_events = self.core.fetch_events();
        let state = self.state.lock().expect("riscv core lock");
        let (fetch_request, access, range, request_byte_offset) =
            match detailed_o3::data_access_result_head_physical_probe(&state, &fetch_events) {
                detailed_o3::DataAccessResultHeadPhysicalProbe::Memory => {
                    return DataAccessResultHeadRoute::Memory;
                }
                detailed_o3::DataAccessResultHeadPhysicalProbe::Ready {
                    fetch_request,
                    access,
                    range,
                    request_byte_offset,
                } => (fetch_request, access, range, request_byte_offset),
                detailed_o3::DataAccessResultHeadPhysicalProbe::Blocked => {
                    return DataAccessResultHeadRoute::Blocked;
                }
            };
        let probe = if matches!(access, MemoryAccessKind::AtomicMemory { .. }) {
            MmioRequest::read(
                MmioRequestId::new(fetch_request.sequence()),
                range.start(),
                range.size(),
            )
            .map_err(RiscvCpuError::Mmio)
        } else {
            mmio_request(
                fetch_request,
                &access,
                range.size(),
                range.start(),
                request_byte_offset,
            )
        };
        let Ok(probe) = probe else {
            return DataAccessResultHeadRoute::Blocked;
        };
        match bus.route_for(self.partition(), &probe) {
            Ok(_) if matches!(access, MemoryAccessKind::AtomicMemory { .. }) => {
                DataAccessResultHeadRoute::Blocked
            }
            Ok(_) => DataAccessResultHeadRoute::Mmio,
            Err(MmioError::UnmappedAddress { .. }) => DataAccessResultHeadRoute::Memory,
            Err(_) => DataAccessResultHeadRoute::Blocked,
        }
    }

    fn can_retire_completed_fetch_while_fetch_pending_with_translation(
        &self,
        translated: detailed_o3::TranslatedMemoryFetchAhead,
    ) -> Result<bool, RiscvCpuError> {
        let fetch_events = self.core.fetch_events();
        let mut state = self.state.lock().expect("riscv core lock");
        if state.pending_trap.is_some()
            || state.pending_fetch_prefix.is_some()
            || hart_has_enabled_pending_interrupt(&state.hart)
        {
            return Ok(false);
        }
        if detailed_o3::data_access_waits_for_younger_fetch(&state, &fetch_events, translated) {
            return Ok(false);
        }
        if state
            .producer_forwarded_scalar_continuation
            .as_ref()
            .is_some_and(|continuation| continuation.waits_for_fetch(&state, &fetch_events))
        {
            return Ok(false);
        }

        can_retire_completed_fetch_with_branch_speculations(&mut state, &fetch_events)
    }
}

fn producer_forwarded_control_decision(
    decision: Option<RiscvFetchAheadDecision>,
) -> Option<RiscvFetchAheadDecision> {
    decision.filter(|decision| {
        decision.producer_forwarded_scalar_continuation.is_some()
            || decision.branch_speculation().is_some_and(|speculation| {
                speculation.producer_forwarded_control_target.is_some()
                    || speculation.producer_forwarded_return_descendant.is_some()
            })
    })
}
