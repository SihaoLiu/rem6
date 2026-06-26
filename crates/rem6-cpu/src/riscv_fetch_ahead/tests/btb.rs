use super::*;

#[test]
fn btb_misprediction_counts_taken_fetch_prediction_without_btb_target() {
    let branch = b_type(8, 0, 0, 0x0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let pc = Address::new(0x8000);
        for _ in 0..2 {
            let prediction = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
                .unwrap();
            state
                .gshare_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8008));
    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
    assert!(prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x8008));
    assert!(!prediction.mispredicted());

    let summary = core.branch_speculation_summary();
    assert_eq!(summary.repairs(), 0);
    assert_eq!(summary.btb_mispredictions(), 1);
    assert_eq!(summary.predicted_taken_btb_misses(), 1);
    assert_eq!(
        summary
            .lookup_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.lookup_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .committed_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.committed_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .mispredicted_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        0
    );
    assert_eq!(summary.mispredicted_branch_kinds().total(), 0);
    assert_eq!(
        summary
            .corrected_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        0
    );
    assert_eq!(summary.corrected_branch_kinds().total(), 0);
    assert_eq!(
        summary
            .target_wrong_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        0
    );
    assert_eq!(summary.target_wrong_branch_kinds().total(), 0);
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::DirectConditional),
        0
    );
    assert_eq!(
        summary
            .mispredict_due_to_predictor()
            .value(BranchTargetKind::DirectConditional),
        0
    );
    assert_eq!(summary.mispredict_due_to_predictor().total(), 0);
    assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 0);
}

#[test]
fn target_provider_counts_no_target_when_warm_btb_conditional_predicts_not_taken() {
    let branch = b_type(8, 0, 0, 0x1).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.branch_target_buffer.update(
            Address::new(0x8000),
            Address::new(0x8008),
            BranchTargetKind::DirectConditional,
        );
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8004));
    assert_eq!(
        decision
            .branch_speculation()
            .map(|speculation| (speculation.predicted_taken(), speculation.target())),
        Some((false, None))
    );
    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(!prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), None);
    assert!(!prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), None);
    assert!(!prediction.mispredicted());

    let summary = core.branch_speculation_summary();
    assert_eq!(
        summary
            .target_provider()
            .value(BranchTargetProvider::NoTarget),
        1
    );
    assert_eq!(
        summary.target_provider().value(BranchTargetProvider::BTB),
        0
    );
    assert_eq!(summary.target_provider().total(), 1);
    assert_eq!(summary.committed_branch_kinds().total(), 1);
    assert_eq!(summary.mispredicted_branch_kinds().total(), 0);
}

#[test]
fn target_provider_counts_no_target_when_gshare_uses_static_conditional_target() {
    let branch = b_type(8, 0, 0, 0x0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let pc = Address::new(0x8000);
        for _ in 0..2 {
            let prediction = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, pc)
                .unwrap();
            state
                .gshare_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        state.branch_target_buffer.update(
            pc,
            Address::new(0x8010),
            BranchTargetKind::DirectConditional,
        );
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8008));
    assert_eq!(
        decision
            .branch_speculation()
            .map(|speculation| (speculation.predicted_taken(), speculation.target())),
        Some((true, Some(Address::new(0x8008))))
    );
    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
    assert!(prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x8008));
    assert!(!prediction.mispredicted());

    let summary = core.branch_speculation_summary();
    assert_eq!(
        summary
            .target_provider()
            .value(BranchTargetProvider::NoTarget),
        1
    );
    assert_eq!(
        summary.target_provider().value(BranchTargetProvider::BTB),
        0
    );
    assert_eq!(summary.target_provider().total(), 1);
    assert_eq!(summary.committed_branch_kinds().total(), 1);
    assert_eq!(summary.mispredicted_branch_kinds().total(), 0);
}

#[test]
fn btb_misprediction_counts_taken_fetch_prediction_with_wrong_btb_target() {
    let branch = b_type(8, 0, 0, 0x0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let pc = Address::new(0x8000);
        state
            .branch_predictor
            .update(pc, true, Some(Address::new(0x8008)));
        state
            .branch_predictor
            .update(pc, true, Some(Address::new(0x8008)));
        state.branch_target_buffer.update(
            pc,
            Address::new(0x8010),
            BranchTargetKind::DirectConditional,
        );
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8010));
    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x8010));
    assert!(prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x8008));
    assert!(prediction.mispredicted());
    assert_eq!(prediction.repair_target_pc(), Some(0x8008));

    let summary = core.branch_speculation_summary();
    assert_eq!(summary.repairs(), 1);
    assert_eq!(summary.btb_mispredictions(), 1);
    assert_eq!(summary.predicted_taken_btb_misses(), 0);
    assert_eq!(
        summary
            .lookup_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.lookup_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .committed_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.committed_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .mispredicted_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.mispredicted_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .corrected_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.corrected_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .target_wrong_branch_kinds()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.target_wrong_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .target_provider()
            .value(BranchTargetProvider::NoTarget),
        0
    );
    assert_eq!(
        summary.target_provider().value(BranchTargetProvider::BTB),
        1
    );
    assert_eq!(summary.target_provider().total(), 1);
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::DirectConditional),
        0
    );
}

#[test]
fn btb_misprediction_counts_direct_jump_cold_btb_without_branch_type_lane() {
    let jump = j_type(12, 0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(jump);

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x800c));
    assert!(prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x800c));
    assert!(!prediction.mispredicted());

    let summary = core.branch_speculation_summary();
    assert_eq!(summary.repairs(), 0);
    assert_eq!(summary.btb_mispredictions(), 1);
    assert_eq!(summary.predicted_taken_btb_misses(), 1);
    assert_eq!(
        summary
            .lookup_branch_kinds()
            .value(BranchTargetKind::DirectUnconditional),
        1
    );
    assert_eq!(summary.lookup_branch_kinds().total(), 1);
    assert_eq!(
        summary
            .target_wrong_branch_kinds()
            .value(BranchTargetKind::DirectUnconditional),
        0
    );
    assert_eq!(summary.target_wrong_branch_kinds().total(), 0);
    assert_eq!(
        summary
            .target_provider()
            .value(BranchTargetProvider::NoTarget),
        1
    );
    assert_eq!(
        summary.target_provider().value(BranchTargetProvider::BTB),
        0
    );
    assert_eq!(summary.target_provider().total(), 1);
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::DirectUnconditional),
        0
    );
    assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 0);
}

#[test]
fn target_provider_counts_no_target_when_direct_jump_uses_static_target() {
    let jump = j_type(12, 0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(jump);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.branch_target_buffer.update(
            Address::new(0x8000),
            Address::new(0x8010),
            BranchTargetKind::DirectUnconditional,
        );
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    assert_eq!(
        decision
            .branch_speculation()
            .map(|speculation| (speculation.predicted_taken(), speculation.target())),
        Some((true, Some(Address::new(0x800c))))
    );
    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x800c));
    assert!(prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x800c));
    assert!(!prediction.mispredicted());

    let summary = core.branch_speculation_summary();
    assert_eq!(
        summary
            .target_provider()
            .value(BranchTargetProvider::NoTarget),
        1
    );
    assert_eq!(
        summary.target_provider().value(BranchTargetProvider::BTB),
        0
    );
    assert_eq!(summary.target_provider().total(), 1);
    assert_eq!(
        summary
            .lookup_branch_kinds()
            .value(BranchTargetKind::DirectUnconditional),
        1
    );
}

#[test]
fn btb_update_classifies_direct_link_jump_as_call_direct() {
    let jump = j_type(12, 1).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(jump);

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    record_fetch_ahead_speculation(&core, &decision).unwrap();
    core.execute_next_completed_fetch().unwrap().unwrap();

    assert_eq!(
        btb_entry_kind(&core, 0x8000),
        Some(BranchTargetKind::CallDirect)
    );
}

#[test]
fn btb_mispredict_due_to_btb_miss_counts_indirect_unconditional_target_change() {
    let jalr = i_type(0, 6, 0x0, 0, 0x67).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(jalr);
    let target_register = Register::new(6).unwrap();
    core.write_register(target_register, 0x800c);

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    assert_eq!(
        decision.branch_speculation().map(|speculation| {
            (
                speculation.sequence(),
                speculation.pc(),
                speculation.predicted_taken(),
                speculation.target(),
                speculation.branch_target_prediction(),
            )
        }),
        Some((
            0,
            Address::new(0x8000),
            true,
            Some(Address::new(0x800c)),
            Some(BranchTargetPrediction::new(false, None)),
        ))
    );
    record_fetch_ahead_speculation(&core, &decision).unwrap();
    core.write_register(target_register, 0x8010);

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x800c));
    assert!(prediction.resolved_taken());
    assert_eq!(prediction.resolved_target_pc(), Some(0x8010));
    assert!(prediction.mispredicted());
    assert_eq!(prediction.repair_target_pc(), Some(0x8010));

    let summary = core.branch_speculation_summary();
    assert_eq!(summary.repairs(), 1);
    assert_eq!(summary.btb_mispredictions(), 1);
    assert_eq!(summary.predicted_taken_btb_misses(), 1);
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::IndirectUnconditional),
        1
    );
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::DirectConditional),
        0
    );
    assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 1);
    assert_eq!(
        btb_entry_kind(&core, 0x8000),
        Some(BranchTargetKind::IndirectUnconditional)
    );
}

#[test]
fn btb_mispredict_due_to_btb_miss_counts_indirect_call_target_change() {
    let jalr = i_type(0, 6, 0x0, 1, 0x67).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(jalr);
    let target_register = Register::new(6).unwrap();
    core.write_register(target_register, 0x800c);

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    record_fetch_ahead_speculation(&core, &decision).unwrap();
    core.write_register(target_register, 0x8010);

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.mispredicted());
    assert_eq!(prediction.repair_target_pc(), Some(0x8010));

    let summary = core.branch_speculation_summary();
    assert_eq!(summary.repairs(), 1);
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::CallIndirect),
        1
    );
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::IndirectUnconditional),
        0
    );
    assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 1);
    assert_eq!(
        btb_entry_kind(&core, 0x8000),
        Some(BranchTargetKind::CallIndirect)
    );
}

#[test]
fn btb_mispredict_due_to_btb_miss_counts_return_target_change() {
    let jalr = i_type(0, 1, 0x0, 0, 0x67).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(jalr);
    let target_register = Register::new(1).unwrap();
    core.write_register(target_register, 0x800c);

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x800c));
    record_fetch_ahead_speculation(&core, &decision).unwrap();
    core.write_register(target_register, 0x8010);

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.mispredicted());
    assert_eq!(prediction.repair_target_pc(), Some(0x8010));

    let summary = core.branch_speculation_summary();
    assert_eq!(summary.repairs(), 1);
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::Return),
        1
    );
    assert_eq!(
        summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::IndirectUnconditional),
        0
    );
    assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 1);
    assert_eq!(
        btb_entry_kind(&core, 0x8000),
        Some(BranchTargetKind::Return)
    );
}
