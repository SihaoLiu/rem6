use super::*;

impl O3LiveIssueState {
    pub(in crate::o3_runtime) fn remove_exact_at_for_test(
        &mut self,
        sequence: u64,
        action: O3LiveIssueTraceAction,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
        tick: u64,
    ) -> bool {
        self.remove_exact_at(sequence, action, pc, issue_class, tick)
    }
}
