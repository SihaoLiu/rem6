use rem6_isa_riscv::RiscvInstruction;
use rem6_memory::Address;

use crate::{CpuFetchEventKind, RiscvCore, RiscvCpuError};

use super::{
    can_retire_completed_fetch_with_branch_speculations, completed_fetch_window, detailed_o3,
    fetch_ahead_decision, hart_has_enabled_pending_interrupt, next_fetch_ahead_candidate,
    preview_selected_branch_speculation, PreparedRiscvFetchAheadSpeculation,
    RiscvFetchAheadDecision,
};

impl RiscvCore {
    pub(crate) fn next_fetch_ahead_before_retire(&self) -> Option<RiscvFetchAheadDecision> {
        self.next_fetch_ahead_before_retire_with_translation(
            detailed_o3::TranslatedMemoryFetchAhead::Disabled,
        )
    }

    pub(crate) fn next_cached_translated_memory_fetch_ahead_before_retire(
        &self,
    ) -> Option<RiscvFetchAheadDecision> {
        self.next_fetch_ahead_before_retire_with_translation(
            detailed_o3::TranslatedMemoryFetchAhead::CachedMemory,
        )
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
        if state.o3_runtime.has_ready_live_scalar_memory_event() {
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
        if completed.is_empty() {
            return None;
        }
        completed.sort_by_key(|event| event.request_id().sequence());

        let fetch = match detailed_o3::additional_fetch_candidate(
            &state,
            &fetch_events,
            &completed,
            translated,
        ) {
            detailed_o3::DetailedFetchAheadCandidate::Ready(pc) => {
                return Some(RiscvFetchAheadDecision::straight_line(pc));
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
        if detailed_o3::scalar_memory_has_younger_fetch(
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
        let Some(speculation) = decision.branch_speculation() else {
            return Ok(None);
        };
        let fetch_events = self.core.fetch_events();
        let state = self.state.lock().expect("riscv core lock");
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
        Ok(Some(PreparedRiscvFetchAheadSpeculation {
            speculation: speculation.clone(),
            selected,
        }))
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
        if detailed_o3::scalar_memory_waits_for_younger_fetch(&state, &fetch_events, translated) {
            return Ok(false);
        }

        can_retire_completed_fetch_with_branch_speculations(&mut state, &fetch_events)
    }
}
