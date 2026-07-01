use super::*;

#[test]
fn checkpoint_payload_restores_live_fetch_ahead_branch_speculation() {
    let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8004));
    assert_eq!(
        decision
            .branch_speculation()
            .map(|speculation| { (speculation.sequence(), speculation.pc()) }),
        Some((0, Address::new(0x8000)))
    );

    record_fetch_ahead_speculation(&core, &decision).unwrap();
    let captured = core.branch_predictor_checkpoint_payload();
    assert_eq!(
        captured.active_branch_kinds(),
        &[(0, BranchTargetKind::DirectConditional)]
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.branch_speculations.len(), 1);
        assert_eq!(
            state.branch_speculation_kinds.get(&0),
            Some(&BranchTargetKind::DirectConditional)
        );
        assert_eq!(state.branch_target_predictions.len(), 1);
        assert_eq!(state.branch_predictor.pending_speculation_count(), 1);
        state.discard_branch_speculations();
        assert!(state.branch_speculations.is_empty());
        assert!(state.branch_speculation_kinds.is_empty());
        assert!(state.branch_target_predictions.is_empty());
        assert!(state.branch_predictor.pending_speculations().is_empty());
    }

    core.restore_branch_predictor_checkpoint_payload(captured)
        .unwrap();
    assert_eq!(
        core.state
            .lock()
            .expect("riscv core lock")
            .branch_target_predictions
            .len(),
        1
    );
    let restored_branch_kind = {
        let state = core.state.lock().expect("riscv core lock");
        state.branch_speculation_kinds.get(&0).copied()
    };
    assert_eq!(
        restored_branch_kind,
        Some(BranchTargetKind::DirectConditional)
    );

    assert!(core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
    core.execute_next_completed_fetch().unwrap().unwrap();
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.is_empty());
    assert!(state.branch_target_predictions.is_empty());
    assert!(state.branch_predictor.pending_speculations().is_empty());
    assert_eq!(state.branch_speculation_summary.btb_mispredictions(), 1);
    assert_eq!(
        state
            .branch_speculation_summary
            .btb_mispredict_due_to_btb_miss()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(
        state
            .branch_speculation_summary
            .mispredict_due_to_predictor()
            .value(BranchTargetKind::DirectConditional),
        0
    );
    assert_eq!(
        state
            .branch_speculation_summary
            .mispredict_due_to_predictor()
            .total(),
        0
    );
    assert_eq!(
        state
            .branch_speculation_summary
            .predicted_taken_btb_misses(),
        0
    );
}

#[test]
fn checkpoint_payload_restores_live_return_address_stack_speculation() {
    let call = j_type(12, 1).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(call);
    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
    assert_eq!(
        decision
            .branch_speculation()
            .map(|speculation| { (speculation.sequence(), speculation.pc()) }),
        Some((0, Address::new(0x8000)))
    );

    record_fetch_ahead_speculation(&core, &decision).unwrap();
    let captured = core.branch_predictor_checkpoint_payload();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.return_address_stack.stack_entries(),
            &[Address::new(0x8004)]
        );
        assert_eq!(state.return_address_stack.pending_operation_count(), 1);
        assert_eq!(state.return_address_stack_operations.len(), 1);
        state.discard_branch_speculations();
        assert!(state.return_address_stack.stack_entries().is_empty());
        assert_eq!(state.return_address_stack.pending_operation_count(), 0);
        assert!(state.return_address_stack_operations.is_empty());
    }

    core.restore_branch_predictor_checkpoint_payload(captured)
        .unwrap();
    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.return_address_stack.stack_entries(),
            &[Address::new(0x8004)]
        );
        assert_eq!(state.return_address_stack.pending_operation_count(), 1);
        assert_eq!(state.return_address_stack_operations.len(), 1);
    }

    assert!(core
        .can_retire_completed_fetch_while_fetch_pending()
        .unwrap());
    core.execute_next_completed_fetch().unwrap().unwrap();
    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.is_empty());
    assert!(state.branch_target_predictions.is_empty());
    assert_eq!(
        state.return_address_stack.stack_entries(),
        &[Address::new(0x8004)]
    );
    assert_eq!(state.return_address_stack.pending_operation_count(), 0);
    assert!(state.return_address_stack_operations.is_empty());
}

#[test]
fn checkpoint_restored_basic_predictor_target_steers_with_cold_btb() {
    let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .branch_predictor
            .update(Address::new(0x8000), true, Some(Address::new(0x8008)));
    }
    let captured = core.branch_predictor_checkpoint_payload();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.branch_target_buffer.invalidate();
    }

    core.restore_branch_predictor_checkpoint_payload(captured)
        .unwrap();
    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
    assert_eq!(
        decision.branch_speculation().map(|speculation| {
            (
                speculation.sequence(),
                speculation.pc(),
                speculation.predicted_taken(),
                speculation.target(),
            )
        }),
        Some((0, Address::new(0x8000), true, Some(Address::new(0x8008))))
    );
    let btb = core.branch_target_buffer_snapshot();
    assert_eq!(btb.lookup_count(), 1);
    assert_eq!(btb.hit_count(), 0);
}

#[test]
fn checkpoint_restore_ignores_polluted_btb_target() {
    let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .branch_predictor
            .update(Address::new(0x8000), true, Some(Address::new(0x8008)));
        state.branch_target_buffer.update(
            Address::new(0x8000),
            Address::new(0x8008),
            BranchTargetKind::DirectConditional,
        );
    }
    let captured = core.branch_predictor_checkpoint_payload();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.branch_target_buffer.update(
            Address::new(0x8000),
            Address::new(0x8010),
            BranchTargetKind::DirectConditional,
        );
    }

    core.restore_branch_predictor_checkpoint_payload(captured)
        .unwrap();
    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
    assert_eq!(
        decision.branch_speculation().map(|speculation| {
            (
                speculation.sequence(),
                speculation.pc(),
                speculation.predicted_taken(),
                speculation.target(),
            )
        }),
        Some((0, Address::new(0x8000), true, Some(Address::new(0x8008))))
    );
    let btb = core.branch_target_buffer_snapshot();
    assert_eq!(btb.lookup_count(), 1);
    assert_eq!(btb.hit_count(), 1);
}

#[test]
fn checkpoint_restore_rejects_bad_btb_shape_without_partial_state_change() {
    let branch = b_type(8, 0, 0, 0).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    record_fetch_ahead_speculation(&core, &decision).unwrap();
    let original_predictor = core.branch_predictor_snapshot();
    let original_btb = core.branch_target_buffer_snapshot();
    let original_speculations = {
        let state = core.state.lock().expect("riscv core lock");
        state.branch_speculations.clone()
    };
    let mut alternate_predictor = BranchPredictor::new(
        BranchPredictorConfig::new(DEFAULT_RISCV_BRANCH_PREDICTOR_ENTRIES)
            .expect("default RISC-V branch predictor entries are valid"),
    );
    alternate_predictor.update(Address::new(0x9000), true, Some(Address::new(0x9008)));
    let incompatible_btb =
        BranchTargetBuffer::new(BranchTargetBufferConfig::new(8, 2).unwrap()).snapshot();
    let payload = BranchPredictorCheckpointPayload::from_snapshots(
        alternate_predictor.snapshot(),
        incompatible_btb,
        [],
    )
    .unwrap();

    let error = core
        .restore_branch_predictor_checkpoint_payload(payload)
        .unwrap_err();

    assert!(matches!(
        error,
        crate::BranchPredictorError::InvalidBranchTargetBufferCheckpoint { .. }
    ));
    assert_eq!(core.branch_predictor_snapshot(), original_predictor);
    assert_eq!(core.branch_target_buffer_snapshot(), original_btb);
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations, original_speculations);
}

#[test]
fn retired_fetch_gate_repairs_stale_oldest_branch_speculation() {
    let mut state = RiscvCoreState::new(0x1186a, 0);
    let stale = state
        .branch_predictor
        .predict_speculative(Address::new(0x1000));
    state.branch_speculations.insert(1, stale.id());
    state.executed_fetches.insert(request(1));

    assert!(can_retire_completed_fetch_with_branch_speculations(
        &mut state,
        &[completed(2, 0x1186a)]
    )
    .unwrap());
    assert!(state.branch_speculations.is_empty());
    assert!(state.branch_predictor.pending_speculations().is_empty());
}

#[test]
fn retired_fetch_gate_discards_stale_selected_gshare_speculation() {
    let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
    let mut state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
        1
    );
    state.executed_fetches.insert(request(0));

    assert!(can_retire_completed_fetch_with_branch_speculations(
        &mut state,
        &[completed(1, 0x8000)]
    )
    .unwrap());

    assert!(state.branch_speculations.is_empty());
    assert!(state.selected_branch_speculations.is_empty());
    assert!(state.branch_predictor.pending_speculations().is_empty());
    assert_eq!(
        state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
        0
    );
}
