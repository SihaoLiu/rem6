use super::*;

type O3LiveIssueCleanupRow = (u64, Address, O3LiveIssueTraceClass);

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
        let issue_class = if self
            .live_data_accesses
            .iter()
            .any(|live| live.sequence == sequence)
        {
            O3LiveIssueTraceClass::MemoryAgu
        } else {
            queue::live_issue_trace_class(self.live_staged_issue_packet(sequence)?.instruction())?
        };
        Some((entry.pc(), issue_class))
    }

    fn live_issue_rows_from(&self, boundary: u64) -> Vec<O3LiveIssueCleanupRow> {
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

    fn live_staged_issue_rows_from(&self, boundary: u64) -> Vec<O3LiveIssueCleanupRow> {
        self.snapshot
            .reorder_buffer
            .iter()
            .filter(|entry| entry.is_live_staged() && entry.sequence() >= boundary)
            .filter_map(|entry| {
                self.live_issue_identity(entry.sequence())
                    .map(|(pc, issue_class)| (entry.sequence(), pc, issue_class))
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
        if !self.live_issue.has_resident_sequence(sequence) {
            return;
        }
        let has_survivors = self.live_issue.resident_sequences().len() > 1;
        self.live_issue.prepare_cleanup_wake(has_survivors, now);
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
        self.discard_live_issue_suffix_with_cleanup_boundary_at(boundary, boundary, action, now);
    }

    fn discard_live_issue_suffix_with_cleanup_boundary_at(
        &mut self,
        removal_boundary: u64,
        cleanup_boundary: u64,
        action: O3LiveIssueTraceAction,
        now: u64,
    ) {
        let first_removed = self
            .live_issue
            .resident_sequences()
            .partition_point(|sequence| *sequence < removal_boundary);
        if first_removed == self.live_issue.resident_sequences().len() {
            return;
        }
        let rows = self.live_issue_rows_from(removal_boundary);
        self.live_issue
            .prepare_cleanup_wake(first_removed != 0, now);
        let removed = self.live_issue.remove_suffix_at(
            removal_boundary,
            cleanup_boundary,
            action,
            &rows,
            now,
        );
        debug_assert_ne!(removed, 0);
    }

    pub(in crate::o3_runtime) fn discard_live_staged_issue_suffix_with_cleanup_boundary_at(
        &mut self,
        removal_boundary: u64,
        cleanup_boundary: u64,
        action: O3LiveIssueTraceAction,
        now: u64,
    ) {
        let rows = self.live_staged_issue_rows_from(removal_boundary);
        self.discard_live_issue_suffix_with_cleanup_boundary_at(
            removal_boundary,
            cleanup_boundary,
            action,
            now,
        );
        for (sequence, pc, issue_class) in rows {
            self.live_issue.record_nonresident_cleanup_at(
                sequence,
                cleanup_boundary,
                action,
                pc,
                issue_class,
                now,
            );
        }
    }

    pub(in crate::o3_runtime) fn discard_pending_live_issue_suffix_at(
        &mut self,
        sequence: u64,
        now: Option<u64>,
    ) {
        let Some(cleanup_tick) = now.or_else(|| self.live_issue_service_tick()) else {
            return;
        };
        let nonresident = !self.live_issue.has_resident_sequence(sequence);
        if nonresident
            && self.live_issue.trace_records().iter().any(|record| {
                record.sequence() == sequence && record.action() == O3LiveIssueTraceAction::Squashed
            })
        {
            return;
        }
        if nonresident {
            if let Some((pc, issue_class)) = self.live_issue_identity(sequence) {
                self.live_issue.record_nonresident_cleanup_at(
                    sequence,
                    sequence,
                    O3LiveIssueTraceAction::Replayed,
                    pc,
                    issue_class,
                    cleanup_tick,
                );
            }
        }
        self.discard_live_issue_suffix_at(sequence, O3LiveIssueTraceAction::Replayed, cleanup_tick);
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
