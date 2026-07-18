use super::*;

fn recorded_same_link_core() -> RiscvCore {
    let (core, _) = super::detailed_o3_control::live_same_link_core(false);
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&decision).unwrap(),
    );
    core
}

fn recorded_forwarded_pc(
    state: &RiscvCoreState,
    forwarded: crate::o3_runtime::O3ProducerForwardedControlTarget,
) -> RecordedPredictedPc {
    recorded_predicted_pc(
        state,
        forwarded.fetch_request(),
        forwarded.sequential_pc(),
        PredictedControlTargetAuthority::ProducerForwarded(forwarded),
    )
}

#[test]
fn producer_forwarded_control_requires_exact_speculation_kind_and_ras() {
    let core = recorded_same_link_core();
    let mut state = core.state.lock().expect("riscv core lock");
    let forwarded = state
        .o3_runtime
        .producer_forwarded_control_target()
        .unwrap();
    let sequence = forwarded.fetch_request().sequence();
    let original = *state.branch_speculations.get(&sequence).unwrap();
    let replacement = state.branch_predictor.predict_speculative_with_prediction(
        forwarded.pc(),
        true,
        Some(forwarded.target()),
    );
    state.branch_speculations.insert(sequence, replacement.id());
    assert_eq!(
        recorded_forwarded_pc(&state, forwarded),
        RecordedPredictedPc::Invalid
    );

    state.branch_speculations.insert(sequence, original);
    state
        .branch_speculation_kinds
        .insert(sequence, BranchTargetKind::IndirectUnconditional);
    assert_eq!(
        recorded_forwarded_pc(&state, forwarded),
        RecordedPredictedPc::Invalid
    );

    state
        .branch_speculation_kinds
        .insert(sequence, BranchTargetKind::CallIndirect);
    let ras = state
        .return_address_stack_operations
        .remove(&sequence)
        .unwrap();
    assert_eq!(
        recorded_forwarded_pc(&state, forwarded),
        RecordedPredictedPc::Invalid
    );
    state.return_address_stack_operations.insert(sequence, ras);
}

#[test]
fn discarded_producer_forwarded_control_clears_only_speculation_binding() {
    let core = recorded_same_link_core();
    let mut state = core.state.lock().expect("riscv core lock");
    let forwarded = state
        .o3_runtime
        .producer_forwarded_control_target()
        .unwrap();
    let sequence = forwarded.fetch_request().sequence();
    let speculation = *state.branch_speculations.get(&sequence).unwrap();
    assert!(state
        .o3_runtime
        .recorded_producer_forwarded_control_speculation_matches(sequence, speculation));

    super::super::discard_branch_speculation_mapping(&mut state, sequence).unwrap();

    assert!(!state
        .o3_runtime
        .recorded_producer_forwarded_control_speculation_matches(sequence, speculation));
    assert!(state
        .o3_runtime
        .has_recorded_producer_forwarded_control_target(forwarded.consumer_sequence()));
}
