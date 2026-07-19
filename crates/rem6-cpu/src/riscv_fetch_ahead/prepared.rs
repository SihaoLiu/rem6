use super::*;

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
        let target_authority = speculation.target_authority();
        let producer_forwarded_parent = match target_authority {
            detailed_o3::PredictedControlTargetAuthority::ProducerForwarded(parent) => {
                Some(*parent)
            }
            detailed_o3::PredictedControlTargetAuthority::Normal
            | detailed_o3::PredictedControlTargetAuthority::ProducerForwardedReturn(_)
            | detailed_o3::PredictedControlTargetAuthority::RasRequired { .. } => None,
        };
        if state
            .branch_speculations
            .contains_key(&speculation.sequence)
        {
            return;
        }
        match target_authority {
            detailed_o3::PredictedControlTargetAuthority::ProducerForwardedReturn(descendant) => {
                let live_descendant = state
                    .o3_runtime
                    .producer_forwarded_return_descendant()
                    .as_ref()
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
            detailed_o3::PredictedControlTargetAuthority::RasRequired {
                push_sequence,
                pushed_address,
                consumer,
            } if detailed_o3::unconsumed_ras_required_target(
                state,
                *push_sequence,
                *pushed_address,
                *consumer,
            ) != speculation.target =>
            {
                return
            }
            detailed_o3::PredictedControlTargetAuthority::Normal
            | detailed_o3::PredictedControlTargetAuthority::ProducerForwarded(_)
            | detailed_o3::PredictedControlTargetAuthority::RasRequired { .. } => {}
        }
        let prediction = state.branch_predictor.predict_speculative_with_prediction(
            speculation.pc,
            speculation.predicted_taken,
            speculation.target,
        );
        if let Some(forwarded) = producer_forwarded_parent {
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
