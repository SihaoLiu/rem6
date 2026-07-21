use super::*;

impl O3RuntimeState {
    pub(in crate::o3_runtime) fn pending_data_address_candidate_metadata(
        &self,
        sequence: u64,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<(O3RenameMapEntry, Register, u64)> {
        let pending = self.pending_data_addresses.first()?;
        (pending.materialized.is_none()
            && pending.sequence == sequence
            && pending.fetch.pc() == pc
            && pending.decoded.instruction() == instruction
            && pending.consumed_requests == consumed_requests)
            .then_some((
                pending.destination,
                pending.producer_register,
                pending.producer_sequence,
            ))
    }

    pub(super) fn pending_data_address_producer_ready_tick(&self, sequence: u64) -> Option<u64> {
        let pending = self.pending_data_addresses.first()?;
        (pending.materialized.is_none()
            && pending.producer_sequence == sequence
            && pending.requested_wake_tick.is_some())
        .then_some(pending.requested_wake_tick?.saturating_sub(1))
    }

    pub(in crate::o3_runtime) fn pending_data_address_committed_producer_ready_tick(
        &self,
        sequence: u64,
        source: Register,
    ) -> Option<u64> {
        let pending = self.pending_data_addresses.first()?;
        (pending.producer_register == source)
            .then(|| self.pending_data_address_producer_ready_tick(sequence))?
    }

    pub(super) fn pending_data_address_request_sequence(
        &self,
        request: &O3LiveIssueRequest,
    ) -> Option<u64> {
        let pending = self.pending_data_addresses.first()?;
        (pending.materialized.is_none()
            && pending.fetch.pc() == request.pc()
            && pending.decoded == request.decoded()
            && pending.consumed_requests == request.consumed_requests())
        .then_some(pending.sequence)
    }

    pub(super) fn pending_data_address_sequence_for_replay(&self, sequence: u64) -> Option<u64> {
        self.pending_data_addresses
            .first()
            .filter(|pending| pending.sequence == sequence)
            .map(O3PendingDataAddress::sequence)
    }

    pub(super) fn pending_data_address_has_producer_sequence(&self, sequence: u64) -> bool {
        self.pending_data_addresses
            .first()
            .is_some_and(|pending| pending.producer_sequence == sequence)
    }

    pub(crate) fn pending_data_address_wakeup_seed(
        &self,
    ) -> Option<(MemoryRequestId, Vec<Address>)> {
        let pending = self.pending_data_addresses.first()?;
        let younger_pcs = self
            .snapshot
            .reorder_buffer
            .iter()
            .filter(|entry| {
                entry.sequence() >= pending.sequence
                    && self
                        .live_data_access_younger_sequences
                        .contains(&entry.sequence())
            })
            .copied()
            .map(O3ReorderBufferEntry::pc)
            .collect::<Vec<_>>();
        (younger_pcs.len() == self.live_data_access_younger_sequences.len())
            .then_some((pending.producer_fetch, younger_pcs))
    }

    pub(crate) fn pending_data_address_wake_tick(&self) -> Option<u64> {
        self.pending_data_addresses
            .first()
            .filter(|pending| pending.materialized.is_none())
            .and_then(|pending| pending.requested_wake_tick)
    }

    pub(super) fn pending_data_address_selected_issue_tick_for_reservation(&self) -> Option<u64> {
        self.pending_data_addresses
            .first()
            .and_then(|pending| pending.selected_issue_tick)
    }

    pub(super) fn record_pending_data_address_resource_blocked(
        &mut self,
        sequence: u64,
        actual_tick: u64,
    ) {
        let Some((producer_sequence, producer_register)) = self
            .pending_data_addresses
            .first()
            .filter(|pending| pending.sequence == sequence && pending.materialized.is_none())
            .map(|pending| (pending.producer_sequence, pending.producer_register))
        else {
            return;
        };
        let producer_ready = self
            .live_issue_source_value(producer_sequence, producer_register)
            .map(|(_, ready_tick)| ready_tick)
            .or_else(|| self.pending_data_address_producer_ready_tick(producer_sequence))
            .is_some_and(|ready_tick| ready_tick <= actual_tick);
        let Some(next_tick) = actual_tick.checked_add(1).filter(|_| producer_ready) else {
            return;
        };
        if let Some(pending) = self.pending_data_addresses.first_mut() {
            if pending.sequence == sequence && pending.materialized.is_none() {
                pending.requested_wake_tick = Some(next_tick);
            }
        }
    }

    pub(super) fn pending_data_address_producer_sequence(&self) -> Option<u64> {
        self.pending_data_addresses
            .first()
            .map(|pending| pending.producer_sequence)
    }

    pub(crate) fn pending_data_address_head_reservation(
        &self,
    ) -> Option<O3LiveIssueHeadReservation> {
        self.pending_data_address_producer_sequence()
            .map(|sequence| O3LiveIssueHeadReservation::memory(sequence, 0))
    }
}
