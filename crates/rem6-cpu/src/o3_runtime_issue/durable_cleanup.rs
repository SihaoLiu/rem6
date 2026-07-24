use super::*;

impl O3RuntimeState {
    pub(in crate::o3_runtime) fn complete_durable_live_issue_removal_at(
        &mut self,
        tick: u64,
        sequences: &[u64],
    ) {
        debug_assert!(!self.live_issue.transaction_active());
        self.live_issue
            .remove_durable_blocked_sequences_at_or_after(tick, sequences);
    }
}
