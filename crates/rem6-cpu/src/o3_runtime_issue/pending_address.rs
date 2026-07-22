use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3PendingDataAddressWakeSeed {
    fetch_predecessor_request: MemoryRequestId,
    head_reservation: O3LiveIssueHeadReservation,
    younger_pcs: Vec<Address>,
}

impl O3PendingDataAddressWakeSeed {
    pub(crate) const fn fetch_predecessor_request(&self) -> MemoryRequestId {
        self.fetch_predecessor_request
    }

    pub(crate) const fn head_reservation(&self) -> O3LiveIssueHeadReservation {
        self.head_reservation
    }

    pub(crate) fn younger_pcs(&self) -> &[Address] {
        &self.younger_pcs
    }
}

impl O3RuntimeState {
    pub(in crate::o3_runtime) fn pending_data_address_candidate_metadata(
        &self,
        sequence: u64,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<(O3RenameMapEntry, Register, u64)> {
        let pending = self.pending_data_addresses.find_sequence(sequence)?;
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
        self.pending_data_addresses
            .iter()
            .filter(|pending| {
                pending.materialized.is_none() && pending.producer_sequence == sequence
            })
            .filter_map(|pending| pending.requested_wake_tick)
            .min()
            .map(|tick| tick.saturating_sub(1))
    }

    pub(in crate::o3_runtime) fn pending_data_address_committed_producer_ready_tick(
        &self,
        sequence: u64,
        source: Register,
    ) -> Option<u64> {
        self.pending_data_addresses
            .iter()
            .filter(|pending| {
                pending.materialized.is_none()
                    && pending.producer_sequence == sequence
                    && pending.producer_register == source
            })
            .filter_map(|pending| pending.requested_wake_tick)
            .min()
            .map(|tick| tick.saturating_sub(1))
    }

    pub(super) fn pending_data_address_request_sequence(
        &self,
        request: &O3LiveIssueRequest,
    ) -> Option<u64> {
        self.pending_data_addresses
            .iter()
            .find(|pending| {
                pending.materialized.is_none()
                    && pending.fetch.pc() == request.pc()
                    && pending.decoded == request.decoded()
                    && pending.consumed_requests == request.consumed_requests()
            })
            .map(O3PendingDataAddress::sequence)
    }

    pub(super) fn pending_data_address_sequence_for_replay(&self, sequence: u64) -> Option<u64> {
        self.pending_data_addresses
            .find_sequence(sequence)
            .map(O3PendingDataAddress::sequence)
    }

    pub(super) fn pending_data_address_has_producer_sequence(&self, sequence: u64) -> bool {
        self.pending_data_addresses
            .iter()
            .any(|pending| pending.producer_sequence == sequence)
    }

    pub(crate) fn pending_data_address_wake_seed(&self) -> Option<O3PendingDataAddressWakeSeed> {
        let pending = self
            .pending_data_addresses
            .iter()
            .find(|pending| pending.materialized.is_none())?;
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
        let expected_rows = self
            .live_data_access_younger_sequences
            .iter()
            .filter(|sequence| **sequence >= pending.sequence)
            .count();
        (younger_pcs.len() == expected_rows).then_some(O3PendingDataAddressWakeSeed {
            fetch_predecessor_request: pending.fetch_predecessor_request,
            head_reservation: O3LiveIssueHeadReservation::memory(pending.producer_sequence, 0),
            younger_pcs,
        })
    }

    #[cfg(test)]
    pub(crate) fn pending_data_address_wakeup_seed(
        &self,
    ) -> Option<(MemoryRequestId, Vec<Address>)> {
        let seed = self.pending_data_address_wake_seed()?;
        Some((seed.fetch_predecessor_request, seed.younger_pcs))
    }

    pub(crate) fn pending_data_address_wake_tick(&self) -> Option<u64> {
        self.pending_data_addresses
            .iter()
            .filter(|pending| pending.materialized.is_none())
            .filter_map(|pending| pending.requested_wake_tick)
            .min()
    }

    #[cfg(test)]
    pub(crate) fn set_pending_data_address_resource_blocked_wake_for_test(
        &mut self,
        sequence: u64,
        wake_tick: u64,
    ) {
        let pending = self
            .pending_data_addresses
            .find_sequence_mut(sequence)
            .expect("pending data-address row");
        assert!(pending.materialized.is_none());
        pending.requested_wake_tick = Some(wake_tick);
    }

    pub(super) fn pending_data_address_selected_issue_tick_for_reservation(
        &self,
        tick: u64,
    ) -> bool {
        self.pending_data_addresses
            .iter()
            .any(|pending| pending.selected_issue_tick == Some(tick))
    }

    pub(super) fn record_pending_data_address_resource_blocked(
        &mut self,
        sequence: u64,
        actual_tick: u64,
    ) {
        let Some((producer_sequence, producer_register)) = self
            .pending_data_addresses
            .find_sequence(sequence)
            .filter(|pending| pending.materialized.is_none())
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
        let mut updated = O3PendingDataAddresses::default();
        for mut pending in self.pending_data_addresses.iter().cloned() {
            if pending.sequence == sequence && pending.materialized.is_none() {
                pending.requested_wake_tick = Some(next_tick);
            }
            assert!(updated.try_push(pending));
        }
        self.pending_data_addresses = updated;
    }
}
