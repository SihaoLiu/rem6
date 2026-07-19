use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProducerForwardedScalarContinuation {
    scalar_chain: crate::o3_runtime::O3ProducerForwardedScalarChain,
    ras_stack: Vec<Address>,
    next_ras_operation: ReturnAddressStackOperationId,
}

impl ProducerForwardedScalarContinuation {
    pub(crate) const fn parent(&self) -> crate::o3_runtime::O3ProducerForwardedControlTarget {
        self.scalar_chain.parent()
    }

    pub(crate) fn capture_parent(
        state: &RiscvCoreState,
        parent: crate::o3_runtime::O3ProducerForwardedControlTarget,
    ) -> Option<Self> {
        if parent.link_destination().is_none()
            || state.branch_lookahead < 2
            || detailed_o3::recorded_predicted_pc(
                state,
                parent.fetch_request(),
                parent.sequential_pc(),
                &detailed_o3::PredictedControlTargetAuthority::ProducerForwarded(parent),
            ) != detailed_o3::RecordedPredictedPc::Ready(parent.target())
            || detailed_o3::unconsumed_ras_required_target(
                state,
                parent.fetch_request().sequence(),
                parent.sequential_pc(),
                detailed_o3::RequiredRasConsumer::Pop,
            ) != Some(parent.sequential_pc())
        {
            return None;
        }
        Some(Self {
            scalar_chain: crate::o3_runtime::O3ProducerForwardedScalarChain::empty(parent),
            ras_stack: state.return_address_stack.stack_entries().to_vec(),
            next_ras_operation: state.return_address_stack.next_operation(),
        })
    }

    pub(crate) fn capture(
        state: &RiscvCoreState,
        scalar_chain: crate::o3_runtime::O3ProducerForwardedScalarChain,
    ) -> Option<Self> {
        if let Some(retained) = state.producer_forwarded_scalar_continuation.as_ref() {
            if retained
                .scalar_chain
                .matches_retained_candidate(&scalar_chain)
                && retained.matches_parent_ras(state)
                && state.branch_speculations.len() < state.branch_lookahead
            {
                let mut retained = retained.clone();
                retained.scalar_chain = scalar_chain;
                return Some(retained);
            }
        }
        if state.o3_runtime.producer_forwarded_scalar_chain() != Some(scalar_chain.clone()) {
            return None;
        }
        let mut continuation = Self::capture_parent(state, scalar_chain.parent())?;
        continuation.scalar_chain = scalar_chain;
        Some(continuation)
    }

    pub(super) fn matches_live(&self, state: &RiscvCoreState) -> bool {
        !self.scalar_chain.is_empty()
            && state.o3_runtime.producer_forwarded_scalar_chain() == Some(self.scalar_chain.clone())
            && self.matches_parent_ras(state)
    }

    pub(crate) fn matches_parent_ras(&self, state: &RiscvCoreState) -> bool {
        state.return_address_stack.stack_entries() == self.ras_stack
            && state.return_address_stack.next_operation() == self.next_ras_operation
            && state.return_address_stack.top() == Some(self.parent().sequential_pc())
    }

    pub(super) fn matches_committed_parent(&self, state: &RiscvCoreState) -> bool {
        let parent_sequence = self.parent_sequence();
        !self.scalar_chain.is_empty()
            && self.matches_parent_ras(state)
            && !state.branch_speculations.contains_key(&parent_sequence)
            && !state
                .return_address_stack_operations
                .contains_key(&parent_sequence)
    }

    pub(super) fn matches_retained_parent(&self, state: &RiscvCoreState) -> bool {
        !self.scalar_chain.is_empty()
            && self.matches_parent_ras(state)
            && state
                .producer_forwarded_scalar_continuation
                .as_ref()
                .is_some_and(|retained| {
                    retained.ras_stack == self.ras_stack
                        && retained.next_ras_operation == self.next_ras_operation
                        && retained
                            .scalar_chain
                            .matches_retained_candidate(&self.scalar_chain)
                })
    }

    pub(crate) fn retains_scalar_chain(
        &self,
        state: &RiscvCoreState,
        scalar_chain: &crate::o3_runtime::O3ProducerForwardedScalarChain,
    ) -> bool {
        self.matches_parent_ras(state) && self.matches_scalar_identity(scalar_chain)
    }

    pub(crate) fn matches_scalar_identity(
        &self,
        scalar_chain: &crate::o3_runtime::O3ProducerForwardedScalarChain,
    ) -> bool {
        self.scalar_chain.matches_retained_candidate(scalar_chain)
    }

    pub(crate) fn matches_return_identity(
        &self,
        descendant: &crate::o3_runtime::O3ProducerForwardedReturnDescendant,
    ) -> bool {
        let scalar_chain = descendant.scalar_chain();
        scalar_chain.last().is_some_and(|scalar| {
            descendant.parent() == self.parent()
                && descendant.pc() == scalar.sequential_pc()
                && self.scalar_chain.matches_retained_candidate(scalar_chain)
        })
    }

    pub(crate) fn retains_return_fetch(
        &self,
        state: &RiscvCoreState,
        pc: Address,
        instruction: RiscvInstruction,
        instruction_bytes: u8,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        let Some(descendant) = self.scalar_chain.retained_return_descendant(
            instruction,
            instruction_bytes,
            consumed_requests,
        ) else {
            return false;
        };
        pc == descendant.pc()
            && self.matches_return_identity(&descendant)
            && (self.matches_parent_ras(state)
                || self.recorded_return_target(
                    state,
                    &descendant,
                    descendant.fetch_request().sequence(),
                ) == Some(descendant.target()))
    }

    pub(crate) fn unconsumed_return_target(
        &self,
        state: &RiscvCoreState,
        fetch_pc: Address,
        instruction: RiscvInstruction,
        descendant: &crate::o3_runtime::O3ProducerForwardedReturnDescendant,
    ) -> Option<Address> {
        (self.matches_parent_ras(state)
            && self.matches_return_identity(descendant)
            && fetch_pc == descendant.pc()
            && instruction == descendant.instruction())
        .then_some(descendant.target())
    }

    pub(crate) fn recorded_return_target(
        &self,
        state: &RiscvCoreState,
        descendant: &crate::o3_runtime::O3ProducerForwardedReturnDescendant,
        return_sequence: u64,
    ) -> Option<Address> {
        if !self.matches_return_identity(descendant)
            || state.return_address_stack.next_operation().get()
                != self.next_ras_operation.get().checked_add(1)?
        {
            return None;
        }
        let operation_id = state
            .return_address_stack_operations
            .get(&return_sequence)?;
        let operation = state
            .return_address_stack
            .pending_operations()
            .iter()
            .find(|operation| operation.id() == *operation_id)?;
        if operation.kind() != ReturnAddressStackOperationKind::Pop
            || operation.stack_before() != self.ras_stack
            || operation.predicted_return() != Some(descendant.target())
            || operation.stack_after() != state.return_address_stack.stack_entries()
        {
            return None;
        }
        Some(descendant.target())
    }

    pub(crate) fn waits_for_fetch(
        &self,
        state: &RiscvCoreState,
        fetch_events: &[CpuFetchEvent],
    ) -> bool {
        self.scalar_chain.last().is_some_and(|descendant| {
            self.matches_parent_ras(state)
                && fetch_events.iter().any(|event| {
                    event.pc() == descendant.sequential_pc()
                        && event.request_id().agent() == descendant.last_fetch_request().agent()
                        && event.request_id().sequence()
                            > descendant.last_fetch_request().sequence()
                        && !state.executed_fetches.contains(&event.request_id())
                        && event.kind() == CpuFetchEventKind::Issued
                        && !super::fetch_request_has_response(fetch_events, event)
                })
        })
    }

    pub(crate) const fn parent_sequence(&self) -> u64 {
        self.parent().fetch_request().sequence()
    }
}
