use super::o3_runtime_issue::calendar::O3LiveIssueCalendar;
use super::*;

impl O3RuntimeState {
    pub(crate) fn memory_result_head_identity(
        &self,
    ) -> Option<(
        MemoryRequestId,
        MemoryRequestId,
        u64,
        u64,
        &MemoryAccessKind,
    )> {
        let head = self.sole_memory_result_head()?;
        Some((
            head.fetch_request,
            head.data_request,
            head.issue_tick,
            head.sequence,
            head.execution.execution().memory_access()?,
        ))
    }

    pub(crate) fn matches_exact_memory_result_head(
        &self,
        fetch_request: MemoryRequestId,
        data_request: MemoryRequestId,
        issue_tick: u64,
        o3_sequence: u64,
        access: &MemoryAccessKind,
    ) -> bool {
        self.sole_memory_result_head().is_some_and(|head| {
            head.fetch_request == fetch_request
                && head.data_request == data_request
                && head.issue_tick == issue_tick
                && head.sequence == o3_sequence
                && head.execution.execution().memory_access() == Some(access)
        })
    }

    pub(crate) fn discard_live_staged_suffix_for_fetch_identity(
        &mut self,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        let Some(sequence) =
            self.live_staged_sequence_for_fetch_identity(pc, instruction, consumed_requests)
        else {
            return false;
        };
        self.discard_live_staged_window_from(sequence);
        true
    }

    pub(crate) fn next_memory_result_issue_tick(&self, earliest_tick: u64) -> Option<u64> {
        let head = self.sole_memory_result_head()?;
        if !self.can_consider_memory_result_younger() {
            return None;
        }
        let reservation = O3LiveIssueHeadReservation::memory(head.sequence, head.issue_tick);
        O3LiveIssueCalendar::capture(self, reservation).next_memory_slot_at_or_after(earliest_tick)
    }

    fn sole_memory_result_head(&self) -> Option<&O3LiveDataAccess> {
        let [head] = self.live_data_accesses.as_slice() else {
            return None;
        };
        (head.outcome == O3LiveDataAccessOutcome::Resident
            && head.younger_window_policy == O3DataAccessWindowPolicy::MemoryResultWindow)
            .then_some(head)
    }
}
