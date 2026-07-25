use super::*;

impl O3LiveIssueState {
    pub(in crate::o3_runtime::o3_runtime_issue) fn has_resident_sequence(
        &self,
        sequence: u64,
    ) -> bool {
        self.resident_sequences.binary_search(&sequence).is_ok()
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

    pub(in crate::o3_runtime::o3_runtime_issue) fn prepare_cleanup_wake(
        &mut self,
        has_survivors: bool,
        now: u64,
    ) {
        self.clear_requested_service_tick();
        if has_survivors {
            self.request_service_at(now);
        }
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
}
