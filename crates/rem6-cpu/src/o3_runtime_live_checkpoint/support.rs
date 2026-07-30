use super::*;

impl O3RuntimeState {
    pub(crate) fn checkpoint_history_is_terminal(&self) -> bool {
        !self.live_speculative_executions.is_empty()
            && self.live_issue_is_quiescent()
            && self.live_data_accesses.is_empty()
            && !self.has_pending_data_address()
            && self.live_retired_instructions.is_empty()
            && self.live_speculative_executions.iter().all(|execution| {
                self.snapshot
                    .reorder_buffer
                    .iter()
                    .all(|owner| owner.sequence() != execution.sequence)
            })
    }

    pub(crate) fn checkpoint_history_allows_finalization(&self) -> bool {
        if self.live_speculative_executions.is_empty() {
            !self.has_live_writeback_owner()
        } else {
            self.checkpoint_history_is_terminal()
        }
    }

    pub(crate) fn finalize_quiescent_checkpoint_history(&mut self) {
        debug_assert!(self.live_issue_is_quiescent());
        debug_assert!(self.checkpoint_history_allows_finalization());
        self.live_speculative_executions.clear();
        if self.live_data_accesses.is_empty() && !self.has_pending_data_address() {
            self.live_data_access_younger_sequences.clear();
        }
    }
}

pub(super) fn checkpoint_telemetry(value: O3LiveIssueTelemetry) -> RiscvO3LiveCheckpointTelemetry {
    RiscvO3LiveCheckpointTelemetry {
        enqueued_rows: value.enqueued_rows(),
        service_turns: value.service_turns(),
        wake_requests: value.wake_requests(),
        current_occupancy: value.current_occupancy(),
        peak_occupancy: value.peak_occupancy(),
        scalar_integer_issued_rows: value.scalar_integer_issued_rows(),
        integer_mul_div_issued_rows: value.integer_mul_div_issued_rows(),
        memory_agu_issued_rows: value.memory_agu_issued_rows(),
        control_issued_rows: value.control_issued_rows(),
        scalar_float_issued_rows: value.scalar_float_issued_rows(),
        vector_to_scalar_issued_rows: value.vector_to_scalar_issued_rows(),
    }
}

pub(super) fn restore_telemetry(value: RiscvO3LiveCheckpointTelemetry) -> O3LiveIssueTelemetry {
    O3LiveIssueTelemetry::from_checkpoint([
        value.enqueued_rows,
        value.service_turns,
        value.wake_requests,
        value.current_occupancy,
        value.peak_occupancy,
        value.scalar_integer_issued_rows,
        value.integer_mul_div_issued_rows,
        value.memory_agu_issued_rows,
        value.control_issued_rows,
        value.scalar_float_issued_rows,
        value.vector_to_scalar_issued_rows,
    ])
}

pub(super) fn validate_unique_values(
    field: &'static str,
    values: impl IntoIterator<Item = u64>,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for value in values {
        if !seen.insert(value) {
            return Err(Error::DuplicateValue { field, value });
        }
    }
    Ok(())
}

pub(super) fn validate_unique_requests(
    field: &'static str,
    values: impl IntoIterator<Item = MemoryRequestId>,
) -> Result<(), Error> {
    let mut seen = BTreeSet::new();
    for request in values {
        if !seen.insert(request) {
            return Err(Error::DuplicateValue {
                field,
                value: request.sequence(),
            });
        }
    }
    Ok(())
}
