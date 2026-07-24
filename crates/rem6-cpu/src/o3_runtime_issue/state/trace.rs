use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueTraceRow {
    sequence: u64,
    pc: Address,
    issue_class: O3LiveIssueTraceClass,
}

impl O3LiveIssueTraceRow {
    pub(in crate::o3_runtime) const fn new(
        sequence: u64,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
    ) -> Self {
        Self {
            sequence,
            pc,
            issue_class,
        }
    }
}

impl O3LiveIssueState {
    pub(in crate::o3_runtime) fn finalize_service_turn_trace(
        &mut self,
        selected_sequences: &[u64],
        resource_blocked: &[O3LiveIssueTraceRow],
        dependency_blocked: &[O3LiveIssueTraceRow],
        service_tick: u64,
        next_wake_tick: Option<u64>,
    ) {
        for &sequence in selected_sequences {
            let selected = self.trace_records.iter_mut().rev().find(|record| {
                record.sequence == sequence
                    && record.action == O3LiveIssueTraceAction::Selected
                    && record.service_tick == service_tick
            });
            debug_assert!(selected.is_some(), "selected row must own a trace record");
            if let Some(selected) = selected {
                selected.next_wake_tick = next_wake_tick;
            }
        }
        self.append_retained_trace(
            resource_blocked,
            O3LiveIssueTraceAction::RetainedResource,
            service_tick,
            next_wake_tick,
        );
        self.append_retained_trace(
            dependency_blocked,
            O3LiveIssueTraceAction::RetainedDependency,
            service_tick,
            next_wake_tick,
        );
    }

    fn append_retained_trace(
        &mut self,
        rows: &[O3LiveIssueTraceRow],
        action: O3LiveIssueTraceAction,
        service_tick: u64,
        next_wake_tick: Option<u64>,
    ) {
        self.trace_records
            .extend(rows.iter().map(|row| O3LiveIssueTraceRecord {
                sequence: row.sequence,
                pc: row.pc,
                action,
                issue_class: row.issue_class,
                service_tick,
                next_wake_tick,
                raw_writeback_tick: None,
                admitted_writeback_tick: None,
                cleanup_boundary: None,
            }));
    }
}
