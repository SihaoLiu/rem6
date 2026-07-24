use super::decision::{O3LiveIssueActiveTick, O3LiveIssueDecisionDelta};
use super::decision_projection::O3LiveIssueDecisionProjection;
use super::*;

impl O3LiveIssueState {
    pub(in crate::o3_runtime) fn observe_sequences(
        &mut self,
        tick: u64,
        issued: &[u64],
        resource_blocked: &[u64],
        dependency_blocked: &[u64],
        max_rows_at_tick: usize,
    ) {
        self.begin_active_decision_at(tick);
        self.active_tick
            .as_mut()
            .expect("active issue tick")
            .observe(
                issued,
                resource_blocked,
                dependency_blocked,
                max_rows_at_tick,
            );
    }

    pub(in crate::o3_runtime) fn seal_decision_before(&mut self, tick: u64) {
        if self
            .active_tick
            .as_ref()
            .is_some_and(|active| active.tick() < tick)
        {
            self.seal_current_decision();
        }
    }

    pub(in crate::o3_runtime) fn seal_current_decision(&mut self) {
        if let Some(active) = self.active_tick.take() {
            self.decision_window.retain(active);
        }
    }

    pub(in crate::o3_runtime) const fn projected_decision(
        &self,
    ) -> Option<O3LiveIssueDecisionDelta> {
        match &self.active_tick {
            Some(active) => active.projected_delta(),
            None => None,
        }
    }

    pub(in crate::o3_runtime) const fn projected_decisions(&self) -> O3LiveIssueDecisionProjection {
        let projection = self.decision_window.projection();
        match self.projected_decision() {
            Some(delta) => projection.with_delta(delta),
            None => projection,
        }
    }

    pub(in crate::o3_runtime) fn enter_scheduler_at(
        &mut self,
        earliest_tick: u64,
    ) -> O3LiveIssueDecisionProjection {
        assert!(
            self.scheduler_entry_tick
                .is_none_or(|previous| earliest_tick >= previous),
            "live issue scheduler entry tick regressed"
        );
        self.seal_current_decision();
        self.scheduler_entry_tick = Some(earliest_tick);
        self.decision_window.finalize_before(earliest_tick)
    }

    pub(super) fn begin_active_decision_at(&mut self, tick: u64) {
        if self
            .active_tick
            .as_ref()
            .is_some_and(|active| active.tick() != tick)
        {
            self.seal_current_decision();
        }
        if self.active_tick.is_none() {
            self.active_tick = self
                .decision_window
                .take(tick)
                .or_else(|| Some(O3LiveIssueActiveTick::at(tick, true)));
        }
    }

    pub(super) fn reset_live_issue_decision_baselines(&mut self) {
        if let Some(active) = self.active_tick.as_mut() {
            active.reset_baseline();
        }
        self.decision_window.reset_baselines();
    }

    pub(super) fn remove_active_blocked_sequence_at_or_after(&mut self, tick: u64, sequence: u64) {
        if let Some(active) = self
            .active_tick
            .as_mut()
            .filter(|active| active.tick() >= tick)
        {
            active.remove_blocked(sequence);
        }
    }

    pub(super) fn clear_active_blocked_sequences(&mut self) -> bool {
        let active_changed = self
            .active_tick
            .as_mut()
            .is_some_and(O3LiveIssueActiveTick::clear_blocked);
        let retained_changed = self.decision_window.clear_blocked();
        active_changed || retained_changed
    }

    pub(super) fn retain_active_blocked_sequences_before(&mut self, boundary: u64) {
        if let Some(active) = self.active_tick.as_mut() {
            active.retain_blocked_before(boundary);
        }
        self.decision_window.retain_blocked_before(boundary);
    }

    pub(in crate::o3_runtime) fn remove_durable_blocked_sequences_at_or_after(
        &mut self,
        tick: u64,
        sequences: &[u64],
    ) {
        for sequence in sequences {
            self.remove_active_blocked_sequence_at_or_after(tick, *sequence);
        }
        self.decision_window
            .remove_blocked_at_or_after(tick, sequences);
    }
}
