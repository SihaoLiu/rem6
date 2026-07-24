use std::collections::BTreeMap;
use std::ops::{Deref, DerefMut};

use rem6_memory::Address;

#[path = "state/decision.rs"]
mod decision;
use decision::O3LiveIssueActiveTick;

#[path = "state/decision_projection.rs"]
mod decision_projection;

#[path = "state/decision_window.rs"]
mod decision_window;
use decision_window::O3LiveIssueDecisionWindow;

#[path = "state/decision_state.rs"]
mod decision_state;

#[path = "state/rollback.rs"]
mod rollback;
pub(in crate::o3_runtime) use rollback::O3LiveIssueStateRollback;

#[cfg(test)]
#[path = "state/test_support_tests.rs"]
mod test_support;

macro_rules! copy_getters {
    ($($name:ident -> $value:ty),+ $(,)?) => {
        $(pub const fn $name(self) -> $value { self.$name })+
    };
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct O3LiveIssueTelemetry {
    enqueued_rows: u64,
    service_turns: u64,
    wake_requests: u64,
    current_occupancy: u64,
    peak_occupancy: u64,
    scalar_integer_issued_rows: u64,
    integer_mul_div_issued_rows: u64,
    memory_agu_issued_rows: u64,
    control_issued_rows: u64,
}

impl O3LiveIssueTelemetry {
    copy_getters!(enqueued_rows -> u64, service_turns -> u64, wake_requests -> u64);
    copy_getters!(current_occupancy -> u64, peak_occupancy -> u64);
    copy_getters!(scalar_integer_issued_rows -> u64, integer_mul_div_issued_rows -> u64);
    copy_getters!(memory_agu_issued_rows -> u64, control_issued_rows -> u64);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LiveIssueTraceClass {
    ScalarInteger,
    IntegerMulDiv,
    MemoryAgu,
    Control,
}

impl O3LiveIssueTraceClass {
    pub const fn name(self) -> &'static str {
        match self {
            Self::ScalarInteger => "scalar_integer",
            Self::IntegerMulDiv => "integer_mul_div",
            Self::MemoryAgu => "memory_agu",
            Self::Control => "control",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum O3LiveIssueTraceAction {
    Queued,
    Selected,
    RetainedResource,
    RetainedDependency,
    Replayed,
    Squashed,
    Retired,
}

impl O3LiveIssueTraceAction {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Selected => "selected",
            Self::RetainedResource => "retained_resource",
            Self::RetainedDependency => "retained_dependency",
            Self::Replayed => "replayed",
            Self::Squashed => "squashed",
            Self::Retired => "retired",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct O3LiveIssueTraceRecord {
    sequence: u64,
    pc: Address,
    action: O3LiveIssueTraceAction,
    issue_class: O3LiveIssueTraceClass,
    service_tick: u64,
    next_wake_tick: Option<u64>,
    raw_writeback_tick: Option<u64>,
    admitted_writeback_tick: Option<u64>,
    cleanup_boundary: Option<u64>,
}

impl O3LiveIssueTraceRecord {
    copy_getters!(sequence -> u64, pc -> Address);
    copy_getters!(action -> O3LiveIssueTraceAction, issue_class -> O3LiveIssueTraceClass);
    copy_getters!(service_tick -> u64, next_wake_tick -> Option<u64>);
    copy_getters!(raw_writeback_tick -> Option<u64>);
    copy_getters!(admitted_writeback_tick -> Option<u64>, cleanup_boundary -> Option<u64>);
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct O3LiveIssueResidentSequences(Vec<u64>);

impl Deref for O3LiveIssueResidentSequences {
    type Target = Vec<u64>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for O3LiveIssueResidentSequences {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::o3_runtime) struct O3LiveIssueState {
    resident_sequences: O3LiveIssueResidentSequences,
    requested_service_tick: Option<u64>,
    decision_window: O3LiveIssueDecisionWindow,
    scheduler_entry_tick: Option<u64>,
    active_tick: Option<O3LiveIssueActiveTick>,
    transaction_active: bool,
    mutation_generation: u64,
    last_service_generation: Option<(u64, u64)>,
    telemetry: O3LiveIssueTelemetry,
    trace_records: Vec<O3LiveIssueTraceRecord>,
}

impl O3LiveIssueState {
    pub(in crate::o3_runtime) fn resident_sequences(&self) -> &[u64] {
        self.resident_sequences.as_slice()
    }

    pub(in crate::o3_runtime) fn enqueue_at(
        &mut self,
        sequence: u64,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
        tick: u64,
    ) -> bool {
        let index = match self.resident_sequences.binary_search(&sequence) {
            Ok(_) => return false,
            Err(index) => index,
        };
        self.resident_sequences.insert(index, sequence);
        self.mark_mutated();
        self.telemetry.enqueued_rows = self.telemetry.enqueued_rows.saturating_add(1);
        self.update_occupancy();
        self.request_service_at(tick);
        self.trace_records.push(O3LiveIssueTraceRecord {
            sequence,
            pc,
            action: O3LiveIssueTraceAction::Queued,
            issue_class,
            service_tick: tick,
            next_wake_tick: self.requested_service_tick,
            raw_writeback_tick: None,
            admitted_writeback_tick: None,
            cleanup_boundary: None,
        });
        true
    }

    pub(super) fn remove_exact_at(
        &mut self,
        sequence: u64,
        action: O3LiveIssueTraceAction,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
        tick: u64,
    ) -> bool {
        let Ok(index) = self.resident_sequences.binary_search(&sequence) else {
            return false;
        };
        self.resident_sequences.remove(index);
        self.remove_active_blocked_sequence_at_or_after(tick, sequence);
        self.mark_mutated();
        self.update_occupancy();
        if self.resident_sequences.is_empty() {
            self.clear_requested_service_tick();
        }
        self.record_selected_class(action, issue_class);
        self.trace_records.push(O3LiveIssueTraceRecord {
            sequence,
            pc,
            action,
            issue_class,
            service_tick: tick,
            next_wake_tick: self.requested_service_tick,
            raw_writeback_tick: None,
            admitted_writeback_tick: None,
            cleanup_boundary: None,
        });
        true
    }

    pub(super) fn remove_selected_at(
        &mut self,
        sequence: u64,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
        tick: u64,
        raw_writeback_tick: u64,
        admitted_writeback_tick: u64,
    ) -> bool {
        let action = O3LiveIssueTraceAction::Selected;
        let removed = self.remove_exact_at(sequence, action, pc, issue_class, tick);
        if removed {
            let record = self.trace_records.last_mut().expect("selected trace");
            record.raw_writeback_tick = Some(raw_writeback_tick);
            record.admitted_writeback_tick = Some(admitted_writeback_tick);
        }
        removed
    }

    pub(super) fn remove_suffix_at(
        &mut self,
        boundary: u64,
        action: O3LiveIssueTraceAction,
        rows: &[(u64, Address, O3LiveIssueTraceClass)],
        tick: u64,
    ) -> usize {
        let removed = self.take_suffix(boundary);
        let metadata = rows
            .iter()
            .copied()
            .map(|(sequence, pc, issue_class)| (sequence, (pc, issue_class)))
            .collect::<BTreeMap<_, _>>();
        let next_wake_tick = self.requested_service_tick;
        for sequence in removed.iter().copied() {
            if let Some((pc, issue_class)) = metadata.get(&sequence).copied() {
                self.record_selected_class(action, issue_class);
                self.trace_records.push(O3LiveIssueTraceRecord {
                    sequence,
                    pc,
                    action,
                    issue_class,
                    service_tick: tick,
                    next_wake_tick,
                    raw_writeback_tick: None,
                    admitted_writeback_tick: None,
                    cleanup_boundary: Some(boundary),
                });
            }
        }
        removed.len()
    }

    pub(in crate::o3_runtime) fn discard_all(&mut self) {
        let mut changed =
            !self.resident_sequences.is_empty() || self.requested_service_tick.is_some();
        self.resident_sequences.clear();
        self.requested_service_tick = None;
        self.decision_window = O3LiveIssueDecisionWindow::default();
        self.scheduler_entry_tick = None;
        self.active_tick = None;
        self.transaction_active = false;
        self.last_service_generation = None;
        changed |= self.clear_active_blocked_sequences();
        changed |=
            self.telemetry != O3LiveIssueTelemetry::default() || !self.trace_records.is_empty();
        if changed {
            self.mark_mutated();
        }
        self.telemetry = O3LiveIssueTelemetry::default();
        self.trace_records.clear();
    }

    pub(in crate::o3_runtime) fn request_service_at(&mut self, tick: u64) {
        let requested = self
            .requested_service_tick
            .map_or(tick, |current| current.min(tick));
        if self.requested_service_tick != Some(requested) {
            self.requested_service_tick = Some(requested);
            self.telemetry.wake_requests = self.telemetry.wake_requests.saturating_add(1);
        }
    }

    pub(in crate::o3_runtime) fn request_live_issue_after_writeback_change(&mut self, tick: u64) {
        if self.resident_sequences.is_empty() {
            return;
        }
        self.mark_mutated();
        if !self.transaction_active() {
            self.request_service_at(tick);
        }
    }

    pub(in crate::o3_runtime) const fn requested_service_tick(&self) -> Option<u64> {
        self.requested_service_tick
    }

    pub(in crate::o3_runtime) fn clear_requested_service_tick(&mut self) {
        self.requested_service_tick = None;
    }

    pub(in crate::o3_runtime) fn begin_service_at(&mut self, tick: u64) -> bool {
        let Some(requested) = self.requested_service_tick else {
            return false;
        };
        if requested > tick {
            return false;
        }
        self.requested_service_tick = None;
        let generation = (tick, self.mutation_generation);
        if self.last_service_generation == Some(generation) {
            return false;
        }
        self.last_service_generation = Some(generation);
        self.telemetry.service_turns = self.telemetry.service_turns.saturating_add(1);
        self.begin_active_decision_at(tick);
        true
    }

    pub(in crate::o3_runtime) fn mark_mutated(&mut self) {
        self.mutation_generation = self.mutation_generation.wrapping_add(1);
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) const fn telemetry(&self) -> O3LiveIssueTelemetry {
        self.telemetry
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn trace_records(&self) -> &[O3LiveIssueTraceRecord] {
        &self.trace_records
    }

    pub(in crate::o3_runtime) fn reset_stats_baseline(&mut self) {
        let occupancy = sequence_count(self.resident_sequences.len());
        self.telemetry = O3LiveIssueTelemetry {
            current_occupancy: occupancy,
            peak_occupancy: occupancy,
            ..O3LiveIssueTelemetry::default()
        };
        self.trace_records.clear();
        self.reset_live_issue_decision_baselines();
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn counted_cycle_ticks_for_test(&self) -> Vec<u64> {
        let mut ticks = self.decision_window.counted_ticks();
        if let Some(active) = self.active_tick.as_ref().filter(|active| {
            active
                .projected_delta()
                .is_some_and(|delta| delta.new_cycle)
        }) {
            ticks.push(active.tick());
            ticks.sort_unstable();
        }
        ticks
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn counted_cycle_tick_len_for_test(&self) -> usize {
        self.decision_window.len() + usize::from(self.active_tick.is_some())
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) const fn scheduler_entry_tick_for_test(&self) -> Option<u64> {
        self.scheduler_entry_tick
    }

    fn take_suffix(&mut self, boundary: u64) -> Vec<u64> {
        let first = self
            .resident_sequences
            .partition_point(|sequence| *sequence < boundary);
        if first == self.resident_sequences.len() {
            return Vec::new();
        }
        let removed = self.resident_sequences.split_off(first);
        self.retain_active_blocked_sequences_before(boundary);
        self.mark_mutated();
        self.update_occupancy();
        if self.resident_sequences.is_empty() {
            self.clear_requested_service_tick();
        }
        removed
    }

    fn update_occupancy(&mut self) {
        let occupancy = sequence_count(self.resident_sequences.len());
        self.telemetry.current_occupancy = occupancy;
        self.telemetry.peak_occupancy = self.telemetry.peak_occupancy.max(occupancy);
    }

    fn record_selected_class(
        &mut self,
        action: O3LiveIssueTraceAction,
        issue_class: O3LiveIssueTraceClass,
    ) {
        if action != O3LiveIssueTraceAction::Selected {
            return;
        }
        let counter = match issue_class {
            O3LiveIssueTraceClass::ScalarInteger => &mut self.telemetry.scalar_integer_issued_rows,
            O3LiveIssueTraceClass::IntegerMulDiv => &mut self.telemetry.integer_mul_div_issued_rows,
            O3LiveIssueTraceClass::MemoryAgu => &mut self.telemetry.memory_agu_issued_rows,
            O3LiveIssueTraceClass::Control => &mut self.telemetry.control_issued_rows,
        };
        *counter = counter.saturating_add(1);
    }
}

fn sequence_count(count: usize) -> u64 {
    u64::try_from(count).unwrap_or(u64::MAX)
}
