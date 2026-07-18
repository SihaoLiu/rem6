use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct O3ProducerForwardedReturnDescendant {
    pub(super) parent: O3ProducerForwardedControlTarget,
    pub(super) scalar_descendant: Option<O3ProducerForwardedScalarDescendant>,
    pub(super) fetch_request: MemoryRequestId,
    pub(super) last_fetch_request: MemoryRequestId,
    pub(super) pc: Address,
    pub(super) sequential_pc: Address,
    pub(super) instruction: RiscvInstruction,
    pub(super) sequence: u64,
}

impl PartialEq for O3ProducerForwardedReturnDescendant {
    fn eq(&self, other: &Self) -> bool {
        self.parent.same_control_identity(other.parent)
            && self.scalar_descendant == other.scalar_descendant
            && self.fetch_request == other.fetch_request
            && self.last_fetch_request == other.last_fetch_request
            && self.pc == other.pc
            && self.sequential_pc == other.sequential_pc
            && self.instruction == other.instruction
            && self.sequence == other.sequence
    }
}

impl Eq for O3ProducerForwardedReturnDescendant {}

impl O3ProducerForwardedReturnDescendant {
    pub(crate) const fn parent(self) -> O3ProducerForwardedControlTarget {
        self.parent
    }

    pub(crate) const fn scalar_descendant(self) -> Option<O3ProducerForwardedScalarDescendant> {
        self.scalar_descendant
    }

    pub(crate) const fn fetch_request(self) -> MemoryRequestId {
        self.fetch_request
    }

    pub(crate) const fn last_fetch_request(self) -> MemoryRequestId {
        self.last_fetch_request
    }

    pub(crate) const fn pc(self) -> Address {
        self.pc
    }

    pub(crate) const fn sequential_pc(self) -> Address {
        self.sequential_pc
    }

    pub(crate) const fn instruction(self) -> RiscvInstruction {
        self.instruction
    }

    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn target(self) -> Address {
        self.parent.sequential_pc()
    }
}

impl O3RuntimeState {
    pub(super) fn recorded_producer_forwarded_same_link_control_target_after_head_retire_for_sequences(
        &self,
        producer_sequence: u64,
        consumer_sequence: u64,
    ) -> Option<O3ProducerForwardedControlTarget> {
        if !self.live_data_accesses.is_empty() {
            return None;
        }
        let recorded = self
            .live_staged_fetch_identities
            .get(&consumer_sequence)?
            .producer_forwarded_same_link_target()?;
        let current = self.producer_forwarded_same_link_control_target_from_rows(
            producer_sequence,
            consumer_sequence,
            recorded.data_access_fetch_request(),
        )?;
        recorded.same_control_identity(current).then_some(current)
    }

    pub(crate) fn producer_forwarded_same_link_control_target_after_head_retire(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        if self.live_data_access_younger_sequences.len() != 2 {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        self.recorded_producer_forwarded_same_link_control_target_after_head_retire_for_sequences(
            sequences.next()?,
            sequences.next()?,
        )
    }

    pub(crate) fn producer_forwarded_descendant_issue_context(
        &self,
    ) -> Option<(O3ProducerForwardedControlTarget, O3LiveIssueHeadReservation)> {
        if let Some(authority) = self.retained_producer_forwarded_same_link_control_target() {
            let head =
                self.live_data_access_head_reservation(authority.data_access_fetch_request())?;
            return Some((authority, head));
        }
        let authority = self.producer_forwarded_same_link_control_target_after_head_retire()?;
        let producer = self
            .live_speculative_executions
            .iter()
            .find(|execution| execution.sequence == authority.producer_sequence())?;
        Some((
            authority,
            O3LiveIssueHeadReservation::for_instruction(
                producer.sequence,
                producer.issue_tick,
                producer.execution.instruction(),
            ),
        ))
    }

    pub(super) fn record_producer_forwarded_same_link_return_descendant(&mut self) -> bool {
        let Some(descendant) = self.producer_forwarded_same_link_return_descendant() else {
            return false;
        };
        let Some(identity) = self
            .live_staged_fetch_identities
            .get_mut(&descendant.sequence())
        else {
            return false;
        };
        identity.record_producer_forwarded_return_descendant(descendant);
        true
    }

    pub(crate) fn has_recorded_producer_forwarded_same_link_return_descendant(
        &self,
        sequence: u64,
    ) -> bool {
        self.live_staged_fetch_identities
            .get(&sequence)
            .and_then(O3LiveStagedFetchIdentity::producer_forwarded_return_descendant)
            .is_some()
    }

    pub(crate) fn producer_forwarded_same_link_return_descendant(
        &self,
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        self.direct_producer_forwarded_same_link_return_descendant()
            .or_else(|| self.producer_forwarded_scalar_return_descendant())
    }

    fn direct_producer_forwarded_same_link_return_descendant(
        &self,
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        if self.live_data_access_younger_sequences.len() != 3 {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        let producer_sequence = sequences.next()?;
        let consumer_sequence = sequences.next()?;
        let return_sequence = sequences.next()?;
        let parent = self
            .producer_forwarded_same_link_control_target_for_sequences(
                true,
                producer_sequence,
                consumer_sequence,
            )
            .or_else(|| {
                self.recorded_producer_forwarded_same_link_control_target_after_head_retire_for_sequences(
                    producer_sequence,
                    consumer_sequence,
                )
            })?;
        let recorded = self
            .live_staged_fetch_identities
            .get(&consumer_sequence)?
            .producer_forwarded_same_link_target()?;
        if !recorded.same_control_identity(parent)
            || self.live_control_dependencies.get(&return_sequence) != Some(&consumer_sequence)
            || !self
                .live_control_window_sequences
                .contains(&return_sequence)
        {
            return None;
        }
        let entry = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == return_sequence)?;
        if entry.pc() != parent.target()
            || entry.destination().is_some()
            || entry.rename_destination().is_some()
        {
            return None;
        }
        let issued = self
            .live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == return_sequence)?;
        if issued.producer_sequences.as_slice() != [consumer_sequence]
            || o3_exact_link_return_source(issued.execution.instruction()) != Some(parent.source())
            || Address::new(issued.execution.pc()) != parent.target()
            || Address::new(issued.execution.next_pc()) != parent.sequential_pc()
            || !issued.execution.register_writes().is_empty()
        {
            return None;
        }
        if !self.live_staged_fetch_identity_matches(
            return_sequence,
            issued.execution.instruction(),
            &issued.consumed_requests,
        ) {
            return None;
        }
        Some(O3ProducerForwardedReturnDescendant {
            parent,
            scalar_descendant: None,
            fetch_request: *issued.consumed_requests.first()?,
            last_fetch_request: *issued.consumed_requests.last()?,
            pc: entry.pc(),
            sequential_pc: Address::new(
                issued
                    .execution
                    .pc()
                    .wrapping_add(u64::from(issued.execution.instruction_bytes())),
            ),
            instruction: issued.execution.instruction(),
            sequence: return_sequence,
        })
    }
}
