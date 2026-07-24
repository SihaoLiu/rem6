use std::{
    collections::{BTreeMap, BTreeSet},
    ops::{Deref, DerefMut},
};

use rem6_memory::Address;

#[path = "state/rollback.rs"]
mod rollback;
pub(in crate::o3_runtime) use rollback::O3LiveIssueStateRollback;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveIssueBlockedKind {
    Resource,
    Dependency,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
struct O3LiveIssueActiveTick {
    tick: u64,
    issued_sequences: BTreeSet<u64>,
    blocked_sequences: BTreeMap<u64, O3LiveIssueBlockedKind>,
    baseline_issued_sequences: BTreeSet<u64>,
    baseline_blocked_sequences: BTreeMap<u64, O3LiveIssueBlockedKind>,
    max_rows_after_reset: usize,
    observed_after_reset: bool,
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
    compatibility_cycle_ticks: BTreeSet<u64>,
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

    pub(in crate::o3_runtime) fn remove_exact_at(
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
        self.remove_blocked_sequence(sequence);
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

    pub(in crate::o3_runtime) fn remove_selected_at(
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

    pub(in crate::o3_runtime) fn remove_suffix_at(
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

    pub(in crate::o3_runtime) fn discard_suffix(&mut self, boundary: u64) -> usize {
        self.take_suffix(boundary).len()
    }

    pub(in crate::o3_runtime) fn discard_all(&mut self) {
        let mut changed =
            !self.resident_sequences.is_empty() || self.requested_service_tick.is_some();
        self.resident_sequences.clear();
        self.requested_service_tick = None;
        if let Some(active) = self.active_tick.as_mut() {
            changed |= !active.blocked_sequences.is_empty()
                || !active.baseline_blocked_sequences.is_empty();
            active.blocked_sequences.clear();
            active.baseline_blocked_sequences.clear();
        }
        if changed {
            self.mark_mutated();
        }
        self.telemetry.current_occupancy = 0;
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
        if self
            .active_tick
            .as_ref()
            .is_none_or(|active| active.tick != tick)
        {
            self.active_tick = Some(O3LiveIssueActiveTick {
                tick,
                ..O3LiveIssueActiveTick::default()
            });
        }
        true
    }

    pub(in crate::o3_runtime) fn mark_mutated(&mut self) {
        self.mutation_generation = self.mutation_generation.wrapping_add(1);
    }

    pub(in crate::o3_runtime) const fn telemetry(&self) -> O3LiveIssueTelemetry {
        self.telemetry
    }

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
        self.compatibility_cycle_ticks.clear();
        if let Some(active) = self.active_tick.as_mut() {
            active.baseline_issued_sequences = active.issued_sequences.clone();
            active.baseline_blocked_sequences = active.blocked_sequences.clone();
            active.max_rows_after_reset = 0;
            active.observed_after_reset = false;
        }
    }

    pub(in crate::o3_runtime) fn begin_compatibility_cycle_at(&mut self, tick: u64) -> bool {
        self.compatibility_cycle_ticks.insert(tick)
    }

    fn remove_blocked_sequence(&mut self, sequence: u64) {
        if let Some(active) = self.active_tick.as_mut() {
            active.blocked_sequences.remove(&sequence);
            active.baseline_blocked_sequences.remove(&sequence);
        }
    }

    fn take_suffix(&mut self, boundary: u64) -> Vec<u64> {
        let first = self
            .resident_sequences
            .partition_point(|sequence| *sequence < boundary);
        if first == self.resident_sequences.len() {
            return Vec::new();
        }
        let removed = self.resident_sequences.split_off(first);
        if let Some(active) = self.active_tick.as_mut() {
            active
                .blocked_sequences
                .retain(|sequence, _| *sequence < boundary);
            active
                .baseline_blocked_sequences
                .retain(|sequence, _| *sequence < boundary);
        }
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
