use rem6_memory::MemoryRequestId;

use super::O3RuntimeState;

impl O3RuntimeState {
    pub(crate) fn has_pending_retirement_authority(&self) -> bool {
        self.has_pending_scalar_memory_retirement()
            || self
                .snapshot
                .reorder_buffer
                .iter()
                .any(|entry| entry.is_live_staged())
            || !self.live_retired_instructions.is_empty()
            || !self.live_speculative_executions.is_empty()
            || !self.live_scalar_memory_younger_sequences.is_empty()
    }

    pub(crate) fn owns_pending_retirement_authority(&self, fetch_request: MemoryRequestId) -> bool {
        self.owns_pending_scalar_memory_retirement(fetch_request)
            || self
                .live_retired_instructions
                .iter()
                .any(|instruction| instruction.request == fetch_request)
    }
}

impl crate::RiscvCore {
    pub fn has_pending_o3_runtime_retirement(&self) -> bool {
        self.with_o3_runtime(|runtime| runtime.has_pending_retirement_authority())
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
}
