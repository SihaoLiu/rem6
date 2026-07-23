use super::o3_runtime_issue::calendar::O3LiveIssueCalendar;
use super::*;

impl O3RuntimeState {
    pub(crate) fn next_memory_result_issue_tick(&self, earliest_tick: u64) -> Option<u64> {
        let head = self.live_data_accesses.first()?;
        if self.live_data_accesses.len() != 1 || !self.can_consider_memory_result_younger() {
            return None;
        }
        let reservation = O3LiveIssueHeadReservation::memory(head.sequence, head.issue_tick);
        Some(
            O3LiveIssueCalendar::capture(self, reservation)
                .next_memory_slot_at_or_after(earliest_tick),
        )
    }
}
