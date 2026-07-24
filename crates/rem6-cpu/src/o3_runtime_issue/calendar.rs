use std::collections::BTreeMap;

use crate::o3_pipeline::{
    O3DependencyScopeId, O3IssueOpClass, O3IssueQueueCapacity, O3IssueQueueId, O3ScopedIssuePlan,
    O3ScopedIssueScheduler, O3ScopedReadyInstruction,
};

use super::queue::{live_issue_op_class, O3LiveIssueQueueEntry};
use super::*;

pub(in crate::o3_runtime) const LIVE_ISSUE_QUEUE: O3IssueQueueId = O3IssueQueueId::new(0);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueCalendar {
    issue_width: usize,
    memory_issue_width: usize,
    by_tick: BTreeMap<u64, O3LiveIssueReservations>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueCyclePlan {
    plan: O3ScopedIssuePlan,
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
    pub(in crate::o3_runtime) fn capture(runtime: &O3RuntimeState) -> Self {
        let mut calendar = Self {
            issue_width: runtime.issue_width,
            memory_issue_width: runtime.memory_issue_width(),
            by_tick: BTreeMap::new(),
        };

        for live in &runtime.live_data_accesses {
            calendar.reserve(live.issue_tick, O3IssueOpClass::Memory);
        }
        for pending in runtime.pending_data_addresses.iter() {
            if let Some(tick) = pending.selected_issue_tick {
                calendar.reserve(tick, O3IssueOpClass::Memory);
            }
        }
        for issued in &runtime.live_speculative_executions {
            calendar.reserve(
                issued.issue_tick,
                live_issue_op_class(issued.execution.instruction()),
            );
        }
        calendar
    }

    pub(in crate::o3_runtime) fn capture_with_head_for_admission(
        runtime: &O3RuntimeState,
        head: O3LiveIssueHeadReservation,
    ) -> Self {
        let mut calendar = Self::capture(runtime);
        let canonical_memory_head = head.op_class == O3IssueOpClass::Memory
            && runtime
                .live_data_accesses
                .iter()
                .any(|live| live.sequence == head.sequence() && live.issue_tick == head.issue_tick);
        if !canonical_memory_head {
            calendar.reserve(head.issue_tick, head.op_class);
        }
        calendar
    }

    pub(super) fn plan_at(
        &self,
        tick: u64,
        dependency_table: &O3LiveIssueDependencyTable,
        entries: &[O3LiveIssueQueueEntry],
    ) -> Result<O3LiveIssueCyclePlan, O3RuntimeError> {
        self.plan_scoped_at(
            tick,
            dependency_table.resolved_scopes_at(tick),
            entries
                .iter()
                .map(|entry| dependency_table.scoped_instruction(entry)),
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
            live_issue_capacities_after_reservations(
                self.issue_width,
                self.memory_issue_width,
                reservations,
            ),
        )
        .expect("configured live O3 issue width is nonzero");
        scheduler
            .try_plan_with_reserved_width(reservations.width, resolved_scopes, ready)
            .map(|plan| O3LiveIssueCyclePlan { plan })
            .map_err(|error| O3RuntimeError::InvalidLiveIssuePlan { error })
    }

    pub(in crate::o3_runtime) fn next_memory_slot_at_or_after(
        &self,
        earliest_tick: u64,
    ) -> Option<u64> {
        let mut tick = earliest_tick;
        loop {
            let reservations = self.by_tick.get(&tick).copied().unwrap_or_default();
            if reservations.width < self.issue_width
                && reservations.memory < self.memory_issue_width
            {
                return Some(tick);
            }
            tick = tick.checked_add(1)?;
        }
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
    memory_issue_width: usize,
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
            memory_issue_width.saturating_sub(reservations.memory),
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
