use super::*;

#[derive(Clone, Copy, Debug)]
pub(crate) struct O3ProducerForwardedScalarDescendant {
    parent: O3ProducerForwardedControlTarget,
    fetch_request: MemoryRequestId,
    last_fetch_request: MemoryRequestId,
    pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
    sequence: u64,
}

impl PartialEq for O3ProducerForwardedScalarDescendant {
    fn eq(&self, other: &Self) -> bool {
        self.parent.same_control_identity(other.parent)
            && self.fetch_request == other.fetch_request
            && self.last_fetch_request == other.last_fetch_request
            && self.pc == other.pc
            && self.sequential_pc == other.sequential_pc
            && self.instruction == other.instruction
            && self.sequence == other.sequence
    }
}

impl Eq for O3ProducerForwardedScalarDescendant {}

impl O3ProducerForwardedControlTarget {
    pub(crate) fn fetched_scalar_descendant(
        self,
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<O3ProducerForwardedScalarDescendant> {
        let (destination, sources) = o3_predicted_scalar_descendant_operands(instruction)?;
        if destination.is_zero()
            || destination == self.source()
            || !sources.contains(&self.source())
            || !valid_live_speculative_fetch_identity(consumed_requests)
        {
            return None;
        }
        let fetch_request = *consumed_requests.first()?;
        Some(O3ProducerForwardedScalarDescendant {
            parent: self,
            fetch_request,
            last_fetch_request: *consumed_requests.last()?,
            pc: self.target(),
            sequential_pc: Address::new(
                self.target()
                    .get()
                    .wrapping_add(u64::from(instruction_bytes)),
            ),
            instruction,
            sequence: fetch_request.sequence(),
        })
    }
}

impl O3ProducerForwardedScalarDescendant {
    pub(crate) const fn parent(self) -> O3ProducerForwardedControlTarget {
        self.parent
    }

    #[cfg(test)]
    pub(crate) const fn fetch_request(self) -> MemoryRequestId {
        self.fetch_request
    }

    pub(crate) const fn last_fetch_request(self) -> MemoryRequestId {
        self.last_fetch_request
    }

    #[cfg(test)]
    pub(crate) const fn pc(self) -> Address {
        self.pc
    }

    pub(crate) const fn sequential_pc(self) -> Address {
        self.sequential_pc
    }

    #[cfg(test)]
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) fn retained_return_descendant(
        self,
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        if o3_exact_link_return_source(instruction) != Some(self.parent().source())
            || !valid_live_speculative_fetch_identity(consumed_requests)
        {
            return None;
        }
        let fetch_request = *consumed_requests.first()?;
        Some(O3ProducerForwardedReturnDescendant {
            parent: self.parent(),
            scalar_descendant: Some(self),
            fetch_request,
            last_fetch_request: *consumed_requests.last()?,
            pc: self.sequential_pc(),
            sequential_pc: Address::new(
                self.sequential_pc()
                    .get()
                    .wrapping_add(u64::from(instruction_bytes)),
            ),
            instruction,
            sequence: fetch_request.sequence(),
        })
    }
}

impl O3RuntimeState {
    fn producer_forwarded_same_link_scalar_descendant_for_sequences(
        &self,
        producer_sequence: u64,
        consumer_sequence: u64,
        scalar_sequence: u64,
    ) -> Option<O3ProducerForwardedScalarDescendant> {
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
        let recorded_parent = self
            .live_staged_fetch_identities
            .get(&consumer_sequence)?
            .producer_forwarded_same_link_target()?;
        if !recorded_parent.same_control_identity(parent)
            || self.live_control_dependencies.get(&scalar_sequence) != Some(&consumer_sequence)
            || !self
                .live_control_window_sequences
                .contains(&scalar_sequence)
        {
            return None;
        }
        let entry = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == scalar_sequence)?;
        let issued = self
            .live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == scalar_sequence)?;
        let (destination, sources) =
            o3_predicted_scalar_descendant_operands(issued.execution.instruction())?;
        if entry.pc() != parent.target()
            || destination.is_zero()
            || destination == parent.source()
            || !sources.contains(&parent.source())
            || entry.rename_destination()
                != Some((O3RegisterClass::Integer, u32::from(destination.index())))
        {
            return None;
        }
        if issued.producer_sequences.as_slice() != [consumer_sequence]
            || Address::new(issued.execution.pc()) != parent.target()
            || Address::new(issued.execution.next_pc())
                != Address::new(
                    issued
                        .execution
                        .pc()
                        .wrapping_add(u64::from(issued.execution.instruction_bytes())),
                )
        {
            return None;
        }
        if !self.live_staged_fetch_identity_matches(
            scalar_sequence,
            issued.execution.instruction(),
            &issued.consumed_requests,
        ) {
            return None;
        }
        Some(O3ProducerForwardedScalarDescendant {
            parent,
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
            sequence: scalar_sequence,
        })
    }

    pub(crate) fn producer_forwarded_same_link_scalar_descendant(
        &self,
    ) -> Option<O3ProducerForwardedScalarDescendant> {
        if self.live_data_access_younger_sequences.len() != 3 {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        self.producer_forwarded_same_link_scalar_descendant_for_sequences(
            sequences.next()?,
            sequences.next()?,
            sequences.next()?,
        )
    }

    pub(crate) fn producer_forwarded_scalar_return_issue_context(
        &self,
    ) -> Option<(
        O3ProducerForwardedScalarDescendant,
        O3LiveIssueHeadReservation,
        u64,
    )> {
        if !self.live_data_accesses.is_empty() {
            return None;
        }
        let descendant = self.producer_forwarded_same_link_scalar_descendant()?;
        let retirement_tick = self.last_live_commit_tick?;
        let producer = self
            .live_speculative_executions
            .iter()
            .find(|execution| execution.sequence == descendant.parent().producer_sequence())?;
        Some((
            descendant,
            O3LiveIssueHeadReservation::for_instruction(
                producer.sequence,
                producer.issue_tick,
                producer.execution.instruction(),
            ),
            retirement_tick,
        ))
    }

    pub(crate) fn append_producer_forwarded_scalar_return_descendant(
        &mut self,
        descendant: O3ProducerForwardedScalarDescendant,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<u64> {
        if self.producer_forwarded_scalar_return_issue_context()?.0 != descendant
            || pc != descendant.sequential_pc()
            || o3_exact_link_return_source(instruction) != Some(descendant.parent().source())
            || self.live_data_accesses.len() + self.live_data_access_younger_sequences.len()
                >= self.scalar_memory_window_limit
        {
            return None;
        }
        let sequence = self.stage_live_instruction(pc, instruction, 0)?;
        if !self.bind_live_staged_fetch_identity_at_sequence(
            sequence,
            instruction,
            consumed_requests,
        ) {
            self.discard_live_staged_window_from(sequence);
            return None;
        }
        let consumer_sequence = descendant.parent().consumer_sequence();
        self.live_control_dependencies
            .insert(sequence, consumer_sequence);
        self.live_control_window_sequences.insert(sequence);
        self.live_data_access_younger_sequences.insert(sequence);
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        Some(sequence)
    }

    pub(super) fn producer_forwarded_scalar_return_descendant(
        &self,
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        if self.live_data_access_younger_sequences.len() != 4 || !self.live_data_accesses.is_empty()
        {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        let scalar_descendant = self.producer_forwarded_same_link_scalar_descendant_for_sequences(
            sequences.next()?,
            sequences.next()?,
            sequences.next()?,
        )?;
        let return_sequence = sequences.next()?;
        let parent = scalar_descendant.parent();
        if self.live_control_dependencies.get(&return_sequence) != Some(&parent.consumer_sequence())
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
        let issued = self
            .live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == return_sequence)?;
        if entry.pc() != scalar_descendant.sequential_pc()
            || entry.destination().is_some()
            || entry.rename_destination().is_some()
            || issued.producer_sequences.as_slice() != [parent.consumer_sequence()]
            || o3_exact_link_return_source(issued.execution.instruction()) != Some(parent.source())
            || Address::new(issued.execution.pc()) != scalar_descendant.sequential_pc()
            || Address::new(issued.execution.next_pc()) != parent.sequential_pc()
            || !issued.execution.register_writes().is_empty()
            || !self.live_staged_fetch_identity_matches(
                return_sequence,
                issued.execution.instruction(),
                &issued.consumed_requests,
            )
        {
            return None;
        }
        Some(O3ProducerForwardedReturnDescendant {
            parent,
            scalar_descendant: Some(scalar_descendant),
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

    #[cfg(test)]
    pub(crate) fn retire_producer_forwarded_data_head_for_test(
        &mut self,
        retire_tick: u64,
    ) -> bool {
        if self.live_data_accesses.len() != 1
            || self
                .producer_forwarded_same_link_scalar_descendant()
                .is_none()
            || self
                .snapshot
                .reorder_buffer
                .first()
                .map(|entry| entry.sequence())
                != self.live_data_accesses.first().map(|head| head.sequence)
        {
            return false;
        }
        self.live_data_accesses.clear();
        self.snapshot.reorder_buffer.remove(0);
        self.last_live_commit_tick = Some(retire_tick);
        true
    }

    #[cfg(test)]
    pub(crate) fn producer_forwarded_scalar_return_issue_tick_for_test(&self) -> Option<u64> {
        let sequence = self
            .producer_forwarded_same_link_return_descendant()?
            .sequence();
        self.live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == sequence)
            .map(|issued| issued.issue_tick)
    }
}
