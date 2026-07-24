use std::collections::BTreeMap;

use super::decision::O3LiveIssueActiveTick;
use super::decision_projection::O3LiveIssueDecisionProjection;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct O3LiveIssueDecisionWindow {
    ticks: BTreeMap<u64, O3LiveIssueActiveTick>,
    projection: O3LiveIssueDecisionProjection,
}

impl O3LiveIssueDecisionWindow {
    pub(super) const fn projection(&self) -> O3LiveIssueDecisionProjection {
        self.projection
    }

    pub(super) fn retain(&mut self, decision: O3LiveIssueActiveTick) {
        let tick = decision.tick();
        assert!(
            self.ticks.insert(tick, decision).is_none(),
            "live issue decision tick already retained"
        );
        self.refresh_projection();
    }

    pub(super) fn take(&mut self, tick: u64) -> Option<O3LiveIssueActiveTick> {
        let decision = self.ticks.remove(&tick);
        self.refresh_projection();
        decision
    }

    pub(super) fn finalize_before(&mut self, earliest_tick: u64) -> O3LiveIssueDecisionProjection {
        let retained = self.ticks.split_off(&earliest_tick);
        let finalized = std::mem::replace(&mut self.ticks, retained);
        let projection = project(finalized.values());
        self.refresh_projection();
        projection
    }

    pub(super) fn reset_baselines(&mut self) {
        for decision in self.ticks.values_mut() {
            decision.reset_baseline();
        }
        self.refresh_projection();
    }

    pub(super) fn remove_blocked_at_or_after(&mut self, tick: u64, sequences: &[u64]) {
        for decision in self.ticks.range_mut(tick..).map(|(_, decision)| decision) {
            for sequence in sequences {
                decision.remove_blocked(*sequence);
            }
        }
        self.refresh_projection();
    }

    pub(super) fn clear_blocked(&mut self) -> bool {
        let changed = self.ticks.values_mut().fold(false, |changed, decision| {
            decision.clear_blocked() || changed
        });
        self.refresh_projection();
        changed
    }

    pub(super) fn retain_blocked_before(&mut self, boundary: u64) {
        for decision in self.ticks.values_mut() {
            decision.retain_blocked_before(boundary);
        }
        self.refresh_projection();
    }

    #[cfg(test)]
    pub(super) fn counted_ticks(&self) -> Vec<u64> {
        self.ticks
            .iter()
            .filter_map(|(tick, decision)| {
                decision
                    .projected_delta()
                    .is_some_and(|delta| delta.new_cycle)
                    .then_some(*tick)
            })
            .collect()
    }

    #[cfg(test)]
    pub(super) fn len(&self) -> usize {
        self.ticks.len()
    }

    fn refresh_projection(&mut self) {
        self.projection = project(self.ticks.values());
    }
}

fn project<'a>(
    decisions: impl Iterator<Item = &'a O3LiveIssueActiveTick>,
) -> O3LiveIssueDecisionProjection {
    decisions.fold(
        O3LiveIssueDecisionProjection::default(),
        |projection, decision| {
            decision
                .projected_delta()
                .map_or(projection, |delta| projection.with_delta(delta))
        },
    )
}

#[cfg(test)]
#[path = "decision_window_tests.rs"]
mod tests;
