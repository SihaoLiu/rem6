use super::*;

impl O3RuntimeState {
    fn assert_durable_live_issue_removal_boundary(&self) {
        assert!(
            !self.live_issue.transaction_active(),
            "durable live issue removal cannot run during a transaction"
        );
    }

    fn finish_durable_live_issue_removal_at(
        &mut self,
        sequence: u64,
        tick: u64,
        removed: bool,
    ) -> Result<(), O3RuntimeError> {
        if !removed {
            return Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence });
        }
        self.live_issue
            .remove_durable_blocked_sequences_at_or_after(tick, &[sequence]);
        Ok(())
    }

    pub(in crate::o3_runtime) fn remove_durable_live_issue_at(
        &mut self,
        sequence: u64,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
        tick: u64,
        writeback_ticks: Option<(u64, u64)>,
    ) -> Result<(), O3RuntimeError> {
        self.assert_durable_live_issue_removal_boundary();
        let removed = match writeback_ticks {
            Some((raw, admitted)) => {
                self.live_issue
                    .remove_selected_at(sequence, pc, issue_class, tick, raw, admitted)
            }
            None => self.live_issue.remove_exact_at(
                sequence,
                O3LiveIssueTraceAction::Selected,
                pc,
                issue_class,
                tick,
            ),
        };
        self.finish_durable_live_issue_removal_at(sequence, tick, removed)
    }

    pub(in crate::o3_runtime) fn complete_committed_live_issue_removals_at(
        &mut self,
        tick: u64,
        sequences: &[u64],
    ) {
        self.assert_durable_live_issue_removal_boundary();
        self.live_issue
            .remove_durable_blocked_sequences_at_or_after(tick, sequences);
    }
}
