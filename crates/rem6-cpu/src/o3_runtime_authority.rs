use rem6_memory::MemoryRequestId;

use super::O3RuntimeState;

impl O3RuntimeState {
    pub(crate) fn has_live_retirement_authority(&self) -> bool {
        self.has_pending_live_data_access_retirement()
            || self
                .snapshot
                .reorder_buffer
                .iter()
                .any(|entry| entry.is_live_staged())
            || !self.live_retired_instructions.is_empty()
            || !self.live_speculative_executions.is_empty()
            || !self.live_data_access_younger_sequences.is_empty()
            || !self.invalidated_live_staged_fetch_identities.is_empty()
    }

    pub(crate) fn has_pending_retirement_authority(&self) -> bool {
        self.has_live_retirement_authority() || self.has_unpublished_writeback_reservation()
    }

    pub(crate) fn has_live_writeback_owner(&self) -> bool {
        self.live_speculative_executions
            .iter()
            .any(|execution| execution.writeback_slot.is_some())
            || self.live_retired_instructions.iter().any(|instruction| {
                self.writeback_calendar
                    .reservation(instruction.sequence)
                    .is_some()
            })
            || self
                .live_data_accesses
                .iter()
                .any(|live| live.admitted_writeback_tick.is_some())
    }

    pub(crate) fn owns_pending_retirement_authority(&self, fetch_request: MemoryRequestId) -> bool {
        self.owns_pending_live_data_access_retirement(fetch_request)
            || self
                .live_retired_instructions
                .iter()
                .any(|instruction| instruction.request == fetch_request)
            || self
                .invalidated_live_staged_fetch_identities
                .values()
                .any(|identity| identity.owns_fetch_request(fetch_request))
    }
}

impl crate::RiscvCore {
    pub fn has_pending_o3_runtime_retirement(&self) -> bool {
        let state = self.state.lock().expect("riscv core lock");
        state.o3_runtime.has_pending_retirement_authority()
            || state.o3_writeback_wake.has_pending_checkpoint_authority()
    }

    pub fn owns_pending_o3_runtime_retirement(&self, fetch_request: MemoryRequestId) -> bool {
        self.with_o3_runtime(|runtime| runtime.owns_pending_retirement_authority(fetch_request))
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{Register, RiscvInstruction};
    use rem6_memory::Address;

    use super::*;
    use crate::o3_runtime::O3LiveWritebackReady;

    #[test]
    fn live_staged_window_owns_retirement_authority_until_discarded() {
        let mut runtime = O3RuntimeState::default();
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            RiscvInstruction::Div {
                rd: Register::new(3).unwrap(),
                rs1: Register::new(1).unwrap(),
                rs2: Register::new(2).unwrap(),
            },
            29,
            None,
        );

        assert!(runtime.has_pending_retirement_authority());

        runtime.discard_live_retire_window();

        assert!(!runtime.has_pending_retirement_authority());
    }

    #[test]
    fn published_writeback_reservation_does_not_own_retirement_authority() {
        let mut runtime = O3RuntimeState::default();
        runtime
            .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(7, 29)])
            .unwrap();

        assert!(runtime.has_pending_retirement_authority());

        runtime.finalize_writeback_publication(7);

        assert!(runtime.writeback_calendar.reservation(7).is_some());
        assert!(!runtime.has_pending_retirement_authority());
    }
}
