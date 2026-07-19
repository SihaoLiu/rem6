use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3ProducerForwardedControlIdentity {
    data_access_fetch_request: MemoryRequestId,
    fetch_request: MemoryRequestId,
    last_fetch_request: MemoryRequestId,
    pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
    consumer_sequence: u64,
    producer_sequence: u64,
    target_source: rem6_isa_riscv::Register,
    target: Address,
}

#[path = "o3_runtime_producer_forwarded_chain/value.rs"]
mod value;

pub(crate) use value::{
    O3ProducerForwardedControlTarget, O3ProducerForwardedReturnDescendant,
    O3ProducerForwardedScalarChain, O3ProducerForwardedScalarDescendant,
};

impl O3RuntimeState {
    pub(crate) fn producer_forwarded_control_target(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        self.producer_forwarded_control_target_with_completed(false)
    }

    pub(crate) fn retained_producer_forwarded_control_target(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        let forwarded = self.producer_forwarded_control_target_with_completed(true)?;
        let recorded = self
            .live_staged_fetch_identities
            .get(&forwarded.consumer_sequence())?
            .forwarded_control_target_identity()?;
        recorded
            .same_control_identity(forwarded)
            .then_some(forwarded)
    }

    fn producer_forwarded_control_target_with_completed(
        &self,
        allow_completed: bool,
    ) -> Option<O3ProducerForwardedControlTarget> {
        if self.live_data_access_younger_sequences.len() != 2 {
            return None;
        }
        let mut younger_sequences = self.live_data_access_younger_sequences.iter().copied();
        self.producer_forwarded_control_target_for_sequences(
            allow_completed,
            younger_sequences.next()?,
            younger_sequences.next()?,
        )
    }

    fn producer_forwarded_control_target_for_sequences(
        &self,
        allow_completed: bool,
        producer_sequence: u64,
        consumer_sequence: u64,
    ) -> Option<O3ProducerForwardedControlTarget> {
        if self.live_data_accesses.len() != 1 {
            return None;
        }
        let live_data_access = &self.live_data_accesses[0];
        let valid_outcome = match live_data_access.outcome {
            O3LiveDataAccessOutcome::Resident => true,
            O3LiveDataAccessOutcome::Completed => allow_completed,
            O3LiveDataAccessOutcome::Retried | O3LiveDataAccessOutcome::Failed => false,
        };
        if live_data_access.event_taken || !valid_outcome {
            return None;
        }
        self.producer_forwarded_control_target_from_rows(
            producer_sequence,
            consumer_sequence,
            live_data_access.fetch_request,
        )
    }

    fn producer_forwarded_control_target_from_rows(
        &self,
        producer_sequence: u64,
        consumer_sequence: u64,
        data_access_fetch_request: MemoryRequestId,
    ) -> Option<O3ProducerForwardedControlTarget> {
        let producer = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == producer_sequence)?;
        let consumer = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == consumer_sequence)?;
        if !self.is_live_control_window_sequence(consumer_sequence)
            || self
                .pending_control_sequence_for(consumer_sequence)
                .is_some()
        {
            return None;
        }
        let consumer_execution = self
            .live_speculative_executions
            .iter()
            .find(|execution| execution.sequence == consumer_sequence)?;
        if !self.live_staged_fetch_identity_matches(
            consumer_sequence,
            consumer_execution.execution.instruction(),
            &consumer_execution.consumed_requests,
        ) {
            return None;
        }
        let control = o3_live_control_operands(consumer_execution.execution.instruction())?;
        let indirect_target = control.indirect_target()?;
        let rs1 = indirect_target.source();
        let expected_consumer_destination = control
            .destination()
            .map(|destination| (O3RegisterClass::Integer, u32::from(destination.index())));
        if consumer_execution.producer_sequences.as_slice() != [producer_sequence]
            || producer.rename_destination()
                != Some((O3RegisterClass::Integer, u32::from(rs1.index())))
            || consumer.rename_destination() != expected_consumer_destination
        {
            return None;
        }
        let producer_execution = self
            .live_speculative_executions
            .iter()
            .find(|execution| execution.sequence == producer_sequence)?;
        if !self.live_staged_fetch_identity_matches(
            producer_sequence,
            producer_execution.execution.instruction(),
            &producer_execution.consumed_requests,
        ) || Address::new(producer_execution.execution.pc()) != producer.pc()
        {
            return None;
        }
        let value = producer_execution
            .execution
            .register_writes()
            .iter()
            .find(|write| write.register() == rs1)?
            .value();
        let target = value.checked_add_signed(indirect_target.offset())? & !1;
        if target != consumer_execution.execution.next_pc()
            || Address::new(consumer_execution.execution.pc()) != consumer.pc()
        {
            return None;
        }
        let fetch_request = *consumer_execution.consumed_requests.first()?;
        let last_fetch_request = *consumer_execution.consumed_requests.last()?;
        Some(O3ProducerForwardedControlTarget::new(
            O3ProducerForwardedControlIdentity {
                data_access_fetch_request,
                fetch_request,
                last_fetch_request,
                pc: consumer.pc(),
                sequential_pc: Address::new(
                    consumer_execution
                        .execution
                        .pc()
                        .wrapping_add(u64::from(consumer_execution.execution.instruction_bytes())),
                ),
                instruction: consumer_execution.execution.instruction(),
                consumer_sequence,
                producer_sequence,
                target_source: rs1,
                target: Address::new(target),
            },
            producer_execution.admitted_writeback_tick,
        ))
    }

    pub(crate) fn record_producer_forwarded_control_target(
        &mut self,
        forwarded: O3ProducerForwardedControlTarget,
        speculation: BranchSpeculationId,
    ) -> bool {
        let Some(current) = self.producer_forwarded_control_target() else {
            return false;
        };
        if !forwarded.same_control_identity(current) {
            return false;
        }
        let Some(identity) = self
            .live_staged_fetch_identities
            .get_mut(&forwarded.consumer_sequence())
        else {
            return false;
        };
        identity.producer_forwarded_control_target = Some(current);
        identity.producer_forwarded_control_speculation = Some(speculation);
        true
    }
    pub(crate) fn has_recorded_producer_forwarded_control_target(
        &self,
        consumer_sequence: u64,
    ) -> bool {
        self.live_staged_fetch_identities
            .get(&consumer_sequence)
            .and_then(O3LiveStagedFetchIdentity::forwarded_control_target_identity)
            .is_some()
    }
    fn recorded_producer_forwarded_control_target_after_head_retire_for_sequences(
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
            .forwarded_control_target_identity()?;
        let current = self.producer_forwarded_control_target_from_rows(
            producer_sequence,
            consumer_sequence,
            recorded.data_access_fetch_request(),
        )?;
        recorded.same_control_identity(current).then_some(current)
    }

    pub(crate) fn producer_forwarded_control_target_after_head_retire(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        if self.live_data_access_younger_sequences.len() != 2 {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        self.recorded_producer_forwarded_control_target_after_head_retire_for_sequences(
            sequences.next()?,
            sequences.next()?,
        )
    }

    fn producer_forwarded_parent_for_descendant_sequences(
        &self,
        producer_sequence: u64,
        consumer_sequence: u64,
    ) -> Option<O3ProducerForwardedControlTarget> {
        let parent = self
            .producer_forwarded_control_target_for_sequences(
                true,
                producer_sequence,
                consumer_sequence,
            )
            .or_else(|| {
                self.recorded_producer_forwarded_control_target_after_head_retire_for_sequences(
                    producer_sequence,
                    consumer_sequence,
                )
            })?;
        parent.link_destination()?;
        let recorded = self
            .live_staged_fetch_identities
            .get(&consumer_sequence)?
            .forwarded_control_target_identity()?;
        recorded.same_control_identity(parent).then_some(parent)
    }

    fn producer_forwarded_descendant_rows(
        &self,
        sequence: u64,
    ) -> Option<(&O3ReorderBufferEntry, &O3LiveSpeculativeExecution)> {
        let entry = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == sequence)?;
        let issued = self
            .live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == sequence)?;
        self.live_staged_fetch_identity_matches(
            sequence,
            issued.execution.instruction(),
            &issued.consumed_requests,
        )
        .then_some((entry, issued))
    }

    fn producer_forwarded_control_descendant_sequence(
        &self,
        parent: O3ProducerForwardedControlTarget,
        sequence: u64,
    ) -> bool {
        self.pending_control_sequence_for(sequence) == Some(parent.consumer_sequence())
            && self.is_live_control_window_sequence(sequence)
    }

    fn producer_forwarded_return_descendant_for_sequence(
        &self,
        scalar_chain: O3ProducerForwardedScalarChain,
        expected_pc: Address,
        return_sequence: u64,
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        let parent = scalar_chain.parent();
        if !self.producer_forwarded_control_descendant_sequence(parent, return_sequence) {
            return None;
        }
        let (entry, issued) = self.producer_forwarded_descendant_rows(return_sequence)?;
        let link = parent.link_destination()?;
        if entry.pc() != expected_pc
            || entry.destination().is_some()
            || entry.rename_destination().is_some()
            || issued.producer_sequences.as_slice() != [parent.consumer_sequence()]
            || o3_exact_link_return_source(issued.execution.instruction()) != Some(link)
            || Address::new(issued.execution.pc()) != expected_pc
            || Address::new(issued.execution.next_pc()) != parent.sequential_pc()
            || !issued.execution.register_writes().is_empty()
        {
            return None;
        }
        Some(O3ProducerForwardedReturnDescendant::new(
            scalar_chain,
            *issued.consumed_requests.first()?,
            *issued.consumed_requests.last()?,
            entry.pc(),
            Address::new(
                issued
                    .execution
                    .pc()
                    .wrapping_add(u64::from(issued.execution.instruction_bytes())),
            ),
            issued.execution.instruction(),
            return_sequence,
        ))
    }

    pub(crate) fn producer_forwarded_descendant_issue_context(
        &self,
    ) -> Option<(O3ProducerForwardedControlTarget, O3LiveIssueHeadReservation)> {
        if let Some(authority) = self.retained_producer_forwarded_control_target() {
            let head =
                self.live_data_access_head_reservation(authority.data_access_fetch_request())?;
            return Some((authority, head));
        }
        let authority = self.producer_forwarded_control_target_after_head_retire()?;
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

    pub(super) fn record_producer_forwarded_return_descendant(&mut self) -> bool {
        let Some(descendant) = self.producer_forwarded_return_descendant() else {
            return false;
        };
        let Some(identity) = self
            .live_staged_fetch_identities
            .get_mut(&descendant.sequence())
        else {
            return false;
        };
        identity.record_forwarded_return_identity(descendant);
        true
    }

    pub(crate) fn has_recorded_producer_forwarded_return_descendant(&self, sequence: u64) -> bool {
        self.live_staged_fetch_identities
            .get(&sequence)
            .and_then(O3LiveStagedFetchIdentity::forwarded_return_identity)
            .is_some()
    }

    pub(crate) fn producer_forwarded_return_descendant(
        &self,
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        self.direct_producer_forwarded_return_descendant()
            .or_else(|| self.producer_forwarded_scalar_return_descendant())
    }

    fn direct_producer_forwarded_return_descendant(
        &self,
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        if self.live_data_access_younger_sequences.len() != 3 {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        let parent = self.producer_forwarded_parent_for_descendant_sequences(
            sequences.next()?,
            sequences.next()?,
        )?;
        self.producer_forwarded_return_descendant_for_sequence(
            O3ProducerForwardedScalarChain::empty(parent),
            parent.target(),
            sequences.next()?,
        )
    }

    fn producer_forwarded_scalar_chain_for_sequences(
        &self,
        producer_sequence: u64,
        consumer_sequence: u64,
        scalar_sequence: u64,
    ) -> Option<O3ProducerForwardedScalarChain> {
        let parent = self.producer_forwarded_parent_for_descendant_sequences(
            producer_sequence,
            consumer_sequence,
        )?;
        if !self.producer_forwarded_control_descendant_sequence(parent, scalar_sequence) {
            return None;
        }
        let link = parent.link_destination()?;
        let (entry, issued) = self.producer_forwarded_descendant_rows(scalar_sequence)?;
        let (destination, sources) =
            o3_predicted_scalar_descendant_operands(issued.execution.instruction())?;
        if entry.pc() != parent.target()
            || destination.is_zero()
            || destination == link
            || !sources.contains(&link)
            || entry.rename_destination()
                != Some((O3RegisterClass::Integer, u32::from(destination.index())))
            || issued.producer_sequences.as_slice() != [consumer_sequence]
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
        Some(O3ProducerForwardedScalarChain::from_descendant(
            O3ProducerForwardedScalarDescendant::new(
                parent,
                *issued.consumed_requests.first()?,
                *issued.consumed_requests.last()?,
                entry.pc(),
                Address::new(
                    issued
                        .execution
                        .pc()
                        .wrapping_add(u64::from(issued.execution.instruction_bytes())),
                ),
                issued.execution.instruction(),
                scalar_sequence,
            ),
        ))
    }

    pub(crate) fn producer_forwarded_scalar_chain(&self) -> Option<O3ProducerForwardedScalarChain> {
        if self.live_data_access_younger_sequences.len() != 3 {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        self.producer_forwarded_scalar_chain_for_sequences(
            sequences.next()?,
            sequences.next()?,
            sequences.next()?,
        )
    }

    pub(crate) fn producer_forwarded_scalar_return_issue_context(
        &self,
    ) -> Option<(
        O3ProducerForwardedScalarChain,
        O3LiveIssueHeadReservation,
        u64,
    )> {
        if !self.live_data_accesses.is_empty() {
            return None;
        }
        let scalar_chain = self.producer_forwarded_scalar_chain()?;
        let retirement_tick = self.last_live_commit_tick?;
        let parent = scalar_chain.parent();
        let producer = self
            .live_speculative_executions
            .iter()
            .find(|execution| execution.sequence == parent.producer_sequence())?;
        Some((
            scalar_chain,
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
        scalar_chain: &O3ProducerForwardedScalarChain,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<u64> {
        let current = self.producer_forwarded_scalar_return_issue_context()?.0;
        let scalar = scalar_chain.last()?;
        if current != *scalar_chain
            || pc != scalar.sequential_pc()
            || o3_exact_link_return_source(instruction) != scalar_chain.parent().link_destination()
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
        let consumer_sequence = scalar_chain.parent().consumer_sequence();
        self.record_live_control_descendant(sequence, consumer_sequence);
        self.live_data_access_younger_sequences.insert(sequence);
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        Some(sequence)
    }

    fn producer_forwarded_scalar_return_descendant(
        &self,
    ) -> Option<O3ProducerForwardedReturnDescendant> {
        if self.live_data_access_younger_sequences.len() != 4 || !self.live_data_accesses.is_empty()
        {
            return None;
        }
        let mut sequences = self.live_data_access_younger_sequences.iter().copied();
        let scalar_chain = self.producer_forwarded_scalar_chain_for_sequences(
            sequences.next()?,
            sequences.next()?,
            sequences.next()?,
        )?;
        let scalar = scalar_chain.last()?;
        self.producer_forwarded_return_descendant_for_sequence(
            scalar_chain,
            scalar.sequential_pc(),
            sequences.next()?,
        )
    }

    #[cfg(test)]
    pub(crate) fn retire_producer_forwarded_data_head_for_test(
        &mut self,
        retire_tick: u64,
    ) -> bool {
        if self.live_data_accesses.len() != 1
            || self.producer_forwarded_scalar_chain().is_none()
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
        let sequence = self.producer_forwarded_return_descendant()?.sequence();
        self.live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == sequence)
            .map(|issued| issued.issue_tick)
    }

    #[cfg(test)]
    pub(crate) fn replace_producer_forwarded_chain_fetch_identity_for_test(
        &mut self,
        sequence: u64,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        let Some(issued) = self
            .live_speculative_executions
            .iter_mut()
            .find(|issued| issued.sequence == sequence)
        else {
            return false;
        };
        issued.consumed_requests = consumed_requests.to_vec();
        true
    }
}
