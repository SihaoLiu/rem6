use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProducerForwardedScalarContinuation {
    parent: crate::o3_runtime::O3ProducerForwardedControlTarget,
    descendant: Option<crate::o3_runtime::O3ProducerForwardedScalarDescendant>,
    ras_stack: Vec<Address>,
    next_ras_operation: ReturnAddressStackOperationId,
}

impl ProducerForwardedScalarContinuation {
    pub(crate) const fn parent(&self) -> crate::o3_runtime::O3ProducerForwardedControlTarget {
        self.parent
    }

    pub(crate) fn capture_parent(
        state: &RiscvCoreState,
        parent: crate::o3_runtime::O3ProducerForwardedControlTarget,
    ) -> Option<Self> {
        if !parent.supports_same_link_descendants()
            || state.branch_lookahead < 2
            || detailed_o3::recorded_predicted_pc(
                state,
                parent.fetch_request(),
                parent.sequential_pc(),
                detailed_o3::PredictedControlTargetAuthority::ProducerForwarded(parent),
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
            parent,
            descendant: None,
            ras_stack: state.return_address_stack.stack_entries().to_vec(),
            next_ras_operation: state.return_address_stack.next_operation(),
        })
    }

    pub(crate) fn capture(
        state: &RiscvCoreState,
        descendant: crate::o3_runtime::O3ProducerForwardedScalarDescendant,
    ) -> Option<Self> {
        if let Some(retained) = state.producer_forwarded_scalar_continuation.as_ref() {
            if retained.parent == descendant.parent()
                && retained
                    .descendant
                    .is_none_or(|current| current == descendant)
                && retained.matches_parent_ras(state)
                && state.branch_speculations.len() < state.branch_lookahead
            {
                let mut retained = retained.clone();
                retained.descendant = Some(descendant);
                return Some(retained);
            }
        }
        if state
            .o3_runtime
            .producer_forwarded_same_link_scalar_descendant()
            != Some(descendant)
        {
            return None;
        }
        let parent = descendant.parent();
        let mut continuation = Self::capture_parent(state, parent)?;
        continuation.descendant = Some(descendant);
        Some(continuation)
    }

    fn matches_live(&self, state: &RiscvCoreState) -> bool {
        self.descendant.is_some_and(|descendant| {
            state
                .o3_runtime
                .producer_forwarded_same_link_scalar_descendant()
                == Some(descendant)
        }) && self.matches_parent_ras(state)
    }

    pub(crate) fn matches_parent_ras(&self, state: &RiscvCoreState) -> bool {
        state.return_address_stack.stack_entries() == self.ras_stack
            && state.return_address_stack.next_operation() == self.next_ras_operation
            && state.return_address_stack.top() == Some(self.parent.sequential_pc())
    }

    fn matches_committed_parent(&self, state: &RiscvCoreState) -> bool {
        let parent_sequence = self.parent_sequence();
        self.descendant.is_some()
            && self.matches_parent_ras(state)
            && !state.branch_speculations.contains_key(&parent_sequence)
            && !state
                .return_address_stack_operations
                .contains_key(&parent_sequence)
    }

    fn matches_retained_parent(&self, state: &RiscvCoreState) -> bool {
        let Some(descendant) = self.descendant else {
            return false;
        };
        self.matches_parent_ras(state)
            && state
                .producer_forwarded_scalar_continuation
                .as_ref()
                .is_some_and(|retained| {
                    retained.parent == self.parent
                        && retained.ras_stack == self.ras_stack
                        && retained.next_ras_operation == self.next_ras_operation
                        && retained
                            .descendant
                            .is_none_or(|current| current == descendant)
                })
    }

    pub(crate) fn retains_scalar(
        &self,
        state: &RiscvCoreState,
        descendant: crate::o3_runtime::O3ProducerForwardedScalarDescendant,
    ) -> bool {
        self.matches_parent_ras(state) && self.matches_scalar_identity(descendant)
    }

    pub(crate) fn matches_scalar_identity(
        &self,
        descendant: crate::o3_runtime::O3ProducerForwardedScalarDescendant,
    ) -> bool {
        descendant.parent() == self.parent
            && self.descendant.is_none_or(|current| current == descendant)
    }

    pub(crate) fn matches_return_identity(
        &self,
        descendant: crate::o3_runtime::O3ProducerForwardedReturnDescendant,
    ) -> bool {
        descendant.scalar_descendant().is_some_and(|scalar| {
            scalar.parent() == self.parent
                && descendant.parent() == self.parent
                && descendant.pc() == scalar.sequential_pc()
                && self.descendant.is_none_or(|current| current == scalar)
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
        let Some(scalar) = self.descendant else {
            return false;
        };
        let Some(descendant) =
            scalar.retained_return_descendant(instruction, instruction_bytes, consumed_requests)
        else {
            return false;
        };
        pc == descendant.pc()
            && self.matches_return_identity(descendant)
            && (self.matches_parent_ras(state)
                || self.recorded_return_target(
                    state,
                    descendant,
                    descendant.fetch_request().sequence(),
                ) == Some(descendant.target()))
    }

    pub(crate) fn unconsumed_return_target(
        &self,
        state: &RiscvCoreState,
        fetch_pc: Address,
        instruction: RiscvInstruction,
        descendant: crate::o3_runtime::O3ProducerForwardedReturnDescendant,
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
        descendant: crate::o3_runtime::O3ProducerForwardedReturnDescendant,
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
        self.descendant.is_some_and(|descendant| {
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
        self.parent.fetch_request().sequence()
    }
}

pub(crate) struct PreparedRiscvFetchAheadSpeculation {
    speculation: Option<RiscvFetchAheadSpeculation>,
    selected: Option<SelectedBranchRecordedState>,
    scalar_continuation: Option<ProducerForwardedScalarContinuation>,
}

impl PreparedRiscvFetchAheadSpeculation {
    pub(super) fn branch(
        speculation: RiscvFetchAheadSpeculation,
        selected: Option<SelectedBranchRecordedState>,
    ) -> Self {
        Self {
            speculation: Some(speculation),
            selected,
            scalar_continuation: None,
        }
    }

    pub(super) fn scalar(continuation: ProducerForwardedScalarContinuation) -> Self {
        Self {
            speculation: None,
            selected: None,
            scalar_continuation: Some(continuation),
        }
    }

    pub(super) fn apply(self, state: &mut RiscvCoreState) {
        let Self {
            speculation,
            selected,
            scalar_continuation,
        } = self;
        if let Some(continuation) = scalar_continuation {
            if !continuation.matches_live(state)
                && !continuation.matches_committed_parent(state)
                && !continuation.matches_retained_parent(state)
            {
                return;
            }
            state.producer_forwarded_scalar_continuation = Some(continuation);
        }
        let Some(speculation) = speculation else {
            return;
        };
        let producer_forwarded_parent = speculation.producer_forwarded_control_target;
        if state
            .branch_speculations
            .contains_key(&speculation.sequence)
        {
            return;
        }
        if let Some(descendant) = speculation.producer_forwarded_return_descendant {
            let live_descendant = state
                .o3_runtime
                .producer_forwarded_same_link_return_descendant()
                == Some(descendant);
            let retained_descendant = state
                .producer_forwarded_scalar_continuation
                .as_ref()
                .is_some_and(|continuation| continuation.matches_return_identity(descendant));
            if (!live_descendant && !retained_descendant)
                || speculation.sequence != descendant.fetch_request().sequence()
                || speculation.pc != descendant.pc()
                || detailed_o3::unconsumed_producer_forwarded_return_target(
                    state,
                    descendant.pc(),
                    descendant.instruction(),
                    descendant,
                ) != speculation.target
            {
                return;
            }
        }
        let prediction = state.branch_predictor.predict_speculative_with_prediction(
            speculation.pc,
            speculation.predicted_taken,
            speculation.target,
        );
        if let Some(forwarded) = speculation.producer_forwarded_control_target {
            if !state
                .o3_runtime
                .record_producer_forwarded_control_target(forwarded, prediction.id())
            {
                state
                    .branch_predictor
                    .discard_speculation(prediction.id())
                    .expect("new producer-forwarded speculation is pending");
                return;
            }
        }
        if let Some(selected) = selected {
            selected.apply(state);
        }
        state
            .branch_speculations
            .insert(speculation.sequence, prediction.id());
        state
            .branch_speculation_kinds
            .insert(speculation.sequence, speculation.branch_kind);
        if let Some(branch_target_prediction) = speculation.branch_target_prediction {
            state
                .branch_target_predictions
                .insert(speculation.sequence, branch_target_prediction);
        }
        if let Some(operation_id) = record_return_address_stack_speculation(state, &speculation) {
            state
                .return_address_stack_operations
                .insert(speculation.sequence, operation_id);
        }
        if let Some(parent) = producer_forwarded_parent {
            state.producer_forwarded_scalar_continuation =
                ProducerForwardedScalarContinuation::capture_parent(state, parent);
        }
        let pending = state.branch_speculations.len() as u64;
        state.branch_speculation_summary.record_prediction(
            speculation.branch_kind,
            speculation.target_provider,
            pending,
        );
    }
}
