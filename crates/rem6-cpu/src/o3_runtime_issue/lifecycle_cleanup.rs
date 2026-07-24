use super::*;

impl O3RuntimeState {
    fn live_issue_identity(&self, sequence: u64) -> Option<(Address, O3LiveIssueTraceClass)> {
        if let Some(pending) = self.pending_data_addresses.find_sequence(sequence) {
            return Some((pending.fetch.pc(), O3LiveIssueTraceClass::MemoryAgu));
        }
        let entry = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == sequence)?;
        if self
            .live_data_accesses
            .iter()
            .any(|live| live.sequence == sequence)
        {
            return Some((entry.pc(), O3LiveIssueTraceClass::MemoryAgu));
        }
        let packet = self.live_staged_issue_packet(sequence)?;
        let issue_class = queue::live_issue_trace_class(packet.instruction())?;
        Some((entry.pc(), issue_class))
    }

    fn live_issue_rows_from(&self, boundary: u64) -> Vec<(u64, Address, O3LiveIssueTraceClass)> {
        self.live_issue
            .resident_sequences()
            .iter()
            .copied()
            .filter(|sequence| *sequence >= boundary)
            .filter_map(|sequence| {
                self.live_issue_identity(sequence)
                    .map(|(pc, issue_class)| (sequence, pc, issue_class))
            })
            .collect()
    }

    pub(in crate::o3_runtime) fn discard_live_issue_exact_at(
        &mut self,
        sequence: u64,
        action: O3LiveIssueTraceAction,
        now: u64,
    ) {
        let Some((pc, issue_class)) = self.live_issue_identity(sequence) else {
            return;
        };
        if self
            .live_issue
            .resident_sequences()
            .binary_search(&sequence)
            .is_err()
        {
            return;
        }
        let has_survivors = self.live_issue.resident_sequences().len() > 1;
        self.prepare_live_issue_cleanup_wake(has_survivors, now);
        let removed = self
            .live_issue
            .remove_exact_at(sequence, action, pc, issue_class, now);
        debug_assert!(removed);
    }

    pub(in crate::o3_runtime) fn discard_live_issue_suffix_at(
        &mut self,
        boundary: u64,
        action: O3LiveIssueTraceAction,
        now: u64,
    ) {
        let first_removed = self
            .live_issue
            .resident_sequences()
            .partition_point(|sequence| *sequence < boundary);
        if first_removed == self.live_issue.resident_sequences().len() {
            return;
        }
        let rows = self.live_issue_rows_from(boundary);
        self.prepare_live_issue_cleanup_wake(first_removed != 0, now);
        let removed = self
            .live_issue
            .remove_suffix_at(boundary, action, &rows, now);
        debug_assert_ne!(removed, 0);
    }

    pub(in crate::o3_runtime) fn discard_pending_live_issue_suffix_at(
        &mut self,
        sequence: u64,
        now: Option<u64>,
    ) {
        if let Some(cleanup_tick) = now.or_else(|| self.live_issue_service_tick()) {
            self.discard_live_issue_suffix_at(
                sequence,
                O3LiveIssueTraceAction::Replayed,
                cleanup_tick,
            );
        }
    }

    fn prepare_live_issue_cleanup_wake(&mut self, has_survivors: bool, now: u64) {
        self.live_issue.clear_requested_service_tick();
        if has_survivors {
            self.live_issue.request_service_at(now);
        }
    }

    pub(crate) fn discard_all_live_issue_transient_state(&mut self) {
        let projection = self.live_issue.projected_decisions();
        self.stats.record_issue_decisions(
            projection.issue_cycles,
            projection.issued_rows,
            projection.resource_blocked_rows,
            projection.dependency_blocked_rows,
            projection.max_rows_at_tick,
        );
        self.live_issue.discard_all();
    }
}
