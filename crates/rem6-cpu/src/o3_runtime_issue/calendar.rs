use std::collections::{BTreeMap, BTreeSet};

use crate::o3_pipeline::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueCapacity, O3IssueQueueId, O3ScopedIssuePlan,
    O3ScopedIssueScheduler, O3ScopedReadyInstruction,
};

use super::super::o3_runtime_control_window::live_issue_op_class;
use super::*;

const LIVE_ISSUE_QUEUE: O3IssueQueueId = O3IssueQueueId::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueCalendar {
    issue_width: usize,
    by_tick: BTreeMap<u64, O3LiveIssueReservations>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueCyclePlan {
    plan: O3ScopedIssuePlan,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueTickDecision {
    issued_rows: usize,
    resource_blocked_rows: usize,
    dependency_blocked_rows: usize,
    max_rows_at_tick: usize,
    observed: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3LiveIssueReservations {
    width: usize,
    int_alu: usize,
    int_mult: usize,
    branch: usize,
    memory: usize,
}

impl O3LiveIssueCalendar {
    pub(in crate::o3_runtime) fn capture(
        runtime: &O3RuntimeState,
        head: O3LiveIssueHeadReservation,
    ) -> Self {
        let mut calendar = Self {
            issue_width: runtime.issue_width,
            by_tick: BTreeMap::new(),
        };
        calendar.reserve(head.issue_tick, head.op_class);

        let pending_ticks = runtime
            .pending_data_addresses
            .iter()
            .filter_map(|pending| pending.selected_issue_tick)
            .collect::<BTreeSet<_>>();
        for tick in pending_ticks {
            calendar.reserve(tick, O3IssueOpClass::Memory);
        }

        for issued in runtime.live_speculative_executions.iter().filter(|issued| {
            issued.sequence != head.sequence
                && runtime
                    .snapshot
                    .reorder_buffer
                    .iter()
                    .any(|entry| entry.is_live_staged() && entry.sequence() == issued.sequence)
        }) {
            calendar.reserve(
                issued.issue_tick,
                live_issue_op_class(issued.execution.instruction()),
            );
        }
        calendar
    }

    pub(super) fn plan_at(
        &self,
        tick: u64,
        dependency_table: &O3LiveIssueDependencyTable,
        candidates: &[O3LiveIssueSchedulingCandidate],
    ) -> Result<O3LiveIssueCyclePlan, O3RuntimeError> {
        self.plan_scoped_at(
            tick,
            dependency_table.resolved_scopes_at(tick),
            candidates
                .iter()
                .map(|candidate| dependency_table.scoped_instruction(candidate)),
        )
    }

    pub(in crate::o3_runtime) fn plan_scoped_at<R, I>(
        &self,
        tick: u64,
        resolved_scopes: R,
        ready: I,
    ) -> Result<O3LiveIssueCyclePlan, O3RuntimeError>
    where
        R: IntoIterator<Item = O3DependencyScopeId>,
        I: IntoIterator<Item = O3ScopedReadyInstruction>,
    {
        let reservations = self.by_tick.get(&tick).copied().unwrap_or_default();
        let scheduler = O3ScopedIssueScheduler::new(
            self.issue_width,
            live_issue_capacities_after_reservations(self.issue_width, reservations),
        )
        .expect("configured live O3 issue width is nonzero");
        scheduler
            .try_plan_with_reserved_width(reservations.width, resolved_scopes, ready)
            .map(|plan| O3LiveIssueCyclePlan { plan })
            .map_err(|error| O3RuntimeError::InvalidLiveIssuePlan { error })
    }

    fn reserve(&mut self, tick: u64, op_class: O3IssueOpClass) {
        self.by_tick.entry(tick).or_default().reserve(op_class);
    }
}

impl O3LiveIssueCyclePlan {
    pub(in crate::o3_runtime) fn issued(&self) -> &[O3ScopedReadyInstruction] {
        self.plan.issued()
    }

    pub(in crate::o3_runtime) fn resource_blocked(&self) -> &[O3ScopedReadyInstruction] {
        self.plan.resource_blocked()
    }

    pub(in crate::o3_runtime) fn dependency_blocked(&self) -> &[O3ScopedReadyInstruction] {
        self.plan.dependency_blocked()
    }

    pub(in crate::o3_runtime) const fn reserved_width(&self) -> usize {
        self.plan.reserved_width()
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn issued_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.plan.issued_sequences()
    }
}

impl O3LiveIssueTickDecision {
    pub(in crate::o3_runtime) fn observe(
        &mut self,
        plan: &O3LiveIssueCyclePlan,
        issued_rows: usize,
    ) {
        debug_assert!(issued_rows <= plan.issued().len());
        self.issued_rows = self.issued_rows.saturating_add(issued_rows);
        self.resource_blocked_rows = plan.resource_blocked().len();
        self.dependency_blocked_rows = plan.dependency_blocked().len();
        self.max_rows_at_tick = self
            .max_rows_at_tick
            .max(plan.reserved_width().saturating_add(issued_rows));
        self.observed = true;
    }

    pub(in crate::o3_runtime) fn take(&mut self) -> Option<Self> {
        self.observed.then(|| std::mem::take(self))
    }

    pub(in crate::o3_runtime) const fn issued_rows(self) -> usize {
        self.issued_rows
    }

    pub(in crate::o3_runtime) const fn resource_blocked_rows(self) -> usize {
        self.resource_blocked_rows
    }

    pub(in crate::o3_runtime) const fn dependency_blocked_rows(self) -> usize {
        self.dependency_blocked_rows
    }

    pub(in crate::o3_runtime) const fn max_rows_at_tick(self) -> usize {
        self.max_rows_at_tick
    }
}

impl O3LiveIssueReservations {
    fn reserve(&mut self, op_class: O3IssueOpClass) {
        self.width = self.width.saturating_add(1);
        match op_class {
            O3IssueOpClass::IntAlu => self.int_alu = self.int_alu.saturating_add(1),
            O3IssueOpClass::IntMult => self.int_mult = self.int_mult.saturating_add(1),
            O3IssueOpClass::Branch => self.branch = self.branch.saturating_add(1),
            O3IssueOpClass::Memory => self.memory = self.memory.saturating_add(1),
            O3IssueOpClass::Float | O3IssueOpClass::System => {}
        }
    }
}

fn live_issue_capacities_after_reservations(
    issue_width: usize,
    reservations: O3LiveIssueReservations,
) -> Vec<O3IssueQueueCapacity> {
    [
        (
            O3IssueOpClass::IntAlu,
            issue_width.saturating_sub(reservations.int_alu),
        ),
        (
            O3IssueOpClass::IntMult,
            1_usize.saturating_sub(reservations.int_mult),
        ),
        (
            O3IssueOpClass::Branch,
            1_usize.saturating_sub(reservations.branch),
        ),
        (
            O3IssueOpClass::Memory,
            1_usize.saturating_sub(reservations.memory),
        ),
    ]
    .into_iter()
    .filter(|(_, slots)| *slots != 0)
    .map(|(op_class, slots)| {
        O3IssueQueueCapacity::new(LIVE_ISSUE_QUEUE, op_class, slots)
            .expect("live O3 issue capacities are nonzero")
    })
    .collect()
}
