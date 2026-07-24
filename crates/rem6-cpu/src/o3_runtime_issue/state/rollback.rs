use super::*;

pub(in crate::o3_runtime) struct O3LiveIssueStateRollback {
    resident_sequences: O3LiveIssueResidentSequences,
    requested_service_tick: Option<u64>,
    active_tick: Option<O3LiveIssueActiveTick>,
    transaction_active: bool,
    mutation_generation: u64,
    last_service_generation: Option<(u64, u64)>,
    telemetry: O3LiveIssueTelemetry,
    trace_records_len: usize,
}

impl O3LiveIssueState {
    pub(in crate::o3_runtime) fn begin_transaction(&mut self) -> bool {
        let started = !self.transaction_active;
        self.transaction_active = true;
        started
    }

    pub(in crate::o3_runtime) fn end_transaction(&mut self) {
        self.transaction_active = false;
    }

    pub(in crate::o3_runtime) const fn transaction_active(&self) -> bool {
        self.transaction_active
    }

    #[cfg(test)]
    pub(in crate::o3_runtime) fn is_quiescent(&self) -> bool {
        self.resident_sequences.is_empty()
            && self.requested_service_tick.is_none()
            && !self.transaction_active
    }

    pub(in crate::o3_runtime) fn capture_rollback(&self) -> O3LiveIssueStateRollback {
        O3LiveIssueStateRollback {
            resident_sequences: self.resident_sequences.clone(),
            requested_service_tick: self.requested_service_tick,
            active_tick: self.active_tick.clone(),
            transaction_active: self.transaction_active,
            mutation_generation: self.mutation_generation,
            last_service_generation: self.last_service_generation,
            telemetry: self.telemetry,
            trace_records_len: self.trace_records.len(),
        }
    }

    pub(in crate::o3_runtime) fn restore_rollback(&mut self, rollback: O3LiveIssueStateRollback) {
        self.resident_sequences = rollback.resident_sequences;
        self.requested_service_tick = rollback.requested_service_tick;
        self.active_tick = rollback.active_tick;
        self.transaction_active = rollback.transaction_active;
        self.mutation_generation = rollback.mutation_generation;
        self.last_service_generation = rollback.last_service_generation;
        self.telemetry = rollback.telemetry;
        self.trace_records.truncate(rollback.trace_records_len);
    }
}

#[cfg(test)]
#[path = "rollback_tests.rs"]
mod tests;
