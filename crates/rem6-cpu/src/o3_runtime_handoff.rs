use super::{O3LiveScalarMemoryOutcome, O3RuntimeState};
use crate::riscv_execution_mode_handoff::RiscvResidentScalarMemoryHandoff;

impl O3RuntimeState {
    pub(crate) fn resident_scalar_memory_handoff(
        &self,
    ) -> Option<(Vec<RiscvResidentScalarMemoryHandoff>, usize)> {
        if self.deferred_scalar_memory_execution.is_some() || self.live_scalar_memories.is_empty() {
            return None;
        }
        let rows = self
            .live_scalar_memories
            .iter()
            .map(|live| {
                (live.outcome == O3LiveScalarMemoryOutcome::Resident
                    && !live.event_taken
                    && live.response_tick.is_none()
                    && live.latency_ticks.is_none()
                    && live.commit_tick.is_none()
                    && live.load_data.is_none()
                    && !live.forwarded)
                    .then(|| RiscvResidentScalarMemoryHandoff {
                        fetch_request: live.fetch_request,
                        data_request: live.data_request,
                        issue_tick: live.issue_tick,
                        o3_sequence: live.sequence,
                        trace_sequence: self
                            .pending_data_accesses
                            .get(&live.fetch_request)
                            .and_then(|pending| pending.trace_sequence),
                    })
            })
            .collect::<Option<Vec<_>>>()?;
        Some((rows, self.live_scalar_memory_younger_sequences.len()))
    }
}
