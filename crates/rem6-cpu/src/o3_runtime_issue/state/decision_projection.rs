use super::decision::O3LiveIssueDecisionDelta;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueDecisionProjection {
    pub(in crate::o3_runtime) issue_cycles: usize,
    pub(in crate::o3_runtime) issued_rows: usize,
    pub(in crate::o3_runtime) resource_blocked_rows: usize,
    pub(in crate::o3_runtime) dependency_blocked_rows: usize,
    pub(in crate::o3_runtime) max_rows_at_tick: usize,
}

impl O3LiveIssueDecisionProjection {
    pub(super) const fn with_delta(mut self, delta: O3LiveIssueDecisionDelta) -> Self {
        self.issue_cycles = self
            .issue_cycles
            .saturating_add(if delta.new_cycle { 1 } else { 0 });
        self.issued_rows = self.issued_rows.saturating_add(delta.issued_rows);
        self.resource_blocked_rows = self
            .resource_blocked_rows
            .saturating_add(delta.resource_blocked_rows);
        self.dependency_blocked_rows = self
            .dependency_blocked_rows
            .saturating_add(delta.dependency_blocked_rows);
        self.max_rows_at_tick = if self.max_rows_at_tick > delta.max_rows_at_tick {
            self.max_rows_at_tick
        } else {
            delta.max_rows_at_tick
        };
        self
    }
}
