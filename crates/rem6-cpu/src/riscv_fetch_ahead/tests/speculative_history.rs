use super::*;

#[test]
fn selected_gshare_fetch_ahead_uses_speculative_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let first_pc = Address::new(0x8000);
        for _ in 0..2 {
            let prediction = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, first_pc)
                .unwrap();
            state
                .gshare_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }

        let history_seed = state
            .gshare_branch_predictor
            .predict(RISCV_LOCAL_GSHARE_THREAD, Address::new(0x9000))
            .unwrap();
        state
            .gshare_branch_predictor
            .update_history(history_seed.history(), true)
            .unwrap();
        let second_pc = Address::new(0x8008);
        for _ in 0..2 {
            let prediction = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, second_pc)
                .unwrap();
            assert_eq!(prediction.global_history_before(), 1);
            state
                .gshare_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        state
            .gshare_branch_predictor
            .squash(history_seed.history())
            .unwrap();
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
    }

    let first = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(first.pc(), Address::new(0x8008));
    core.set_fetch_ahead_pc(first.pc());
    record_fetch_ahead_speculation(&core, &first).unwrap();

    assert_eq!(
        core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
}

#[test]
fn selected_gshare_fetch_ahead_uses_direct_jump_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let history_seed = state
            .gshare_branch_predictor
            .predict(RISCV_LOCAL_GSHARE_THREAD, Address::new(0x9000))
            .unwrap();
        state
            .gshare_branch_predictor
            .update_history(history_seed.history(), true)
            .unwrap();
        let second_pc = Address::new(0x8008);
        for _ in 0..2 {
            let prediction = state
                .gshare_branch_predictor
                .predict(RISCV_LOCAL_GSHARE_THREAD, second_pc)
                .unwrap();
            assert_eq!(prediction.global_history_before(), 1);
            state
                .gshare_branch_predictor
                .train(prediction.history(), true, false)
                .unwrap();
        }
        state
            .gshare_branch_predictor
            .squash(history_seed.history())
            .unwrap();
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
    }

    let first = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(first.pc(), Address::new(0x8008));
    core.set_fetch_ahead_pc(first.pc());
    record_fetch_ahead_speculation(&core, &first).unwrap();

    assert_eq!(
        core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
}

#[test]
fn selected_bimode_fetch_ahead_uses_speculative_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::BiMode);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        train_selected_bimode_taken(&mut state, Address::new(0x8000));

        let history_seed = state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, Address::new(0x9000))
            .unwrap();
        state
            .bimode_branch_predictor
            .update_history(history_seed.history(), true)
            .unwrap();
        let second_pc = Address::new(0x8008);
        train_selected_bimode_taken(&mut state, second_pc);
        let trained_second = state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, second_pc)
            .unwrap();
        assert_eq!(trained_second.global_history_before(), 1);
        assert!(trained_second.predicted_taken());
        state
            .bimode_branch_predictor
            .squash(history_seed.history())
            .unwrap();
        assert_eq!(
            state.bimode_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
    }

    let first = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(first.pc(), Address::new(0x8008));
    core.set_fetch_ahead_pc(first.pc());
    record_fetch_ahead_speculation(&core, &first).unwrap();

    assert_eq!(
        core.bimode_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
}

#[test]
fn selected_bimode_fetch_ahead_uses_direct_jump_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::BiMode);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let history_seed = state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, Address::new(0x9000))
            .unwrap();
        state
            .bimode_branch_predictor
            .update_history(history_seed.history(), true)
            .unwrap();
        let second_pc = Address::new(0x8008);
        train_selected_bimode_taken(&mut state, second_pc);
        let trained_second = state
            .bimode_branch_predictor
            .predict(RISCV_LOCAL_BIMODE_THREAD, second_pc)
            .unwrap();
        assert_eq!(trained_second.global_history_before(), 1);
        assert!(trained_second.predicted_taken());
        state
            .bimode_branch_predictor
            .squash(history_seed.history())
            .unwrap();
        assert_eq!(
            state.bimode_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
    }

    let first = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(first.pc(), Address::new(0x8008));
    core.set_fetch_ahead_pc(first.pc());
    record_fetch_ahead_speculation(&core, &first).unwrap();

    assert_eq!(
        core.bimode_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
}

#[test]
fn selected_tournament_fetch_ahead_uses_pending_local_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tournament_predictor(&mut state);
        let older_pc = Address::new(0x8000);
        let younger_pc = Address::new(0x8008);
        train_selected_tournament_local_history_one_taken(&mut state, younger_pc);
        let base_prediction = state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc)
            .unwrap();
        assert_eq!(base_prediction.local_history_before(), 0);
        assert!(!base_prediction.predicted_taken());
        let overlay_prediction = state
            .tournament_branch_predictor
            .predict_with_histories(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc, 0, 1)
            .unwrap();
        assert!(overlay_prediction.predicted_taken());
        insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
        let fetch_events = core.core.fetch_events();
        let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
        assert_eq!(
            selected_tournament_speculative_histories(&state, &completed_fetches, younger_pc),
            Some((1, 1))
        );
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[0],
            0
        );
    }

    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
        0
    );
    assert_eq!(
        state
            .tournament_branch_predictor
            .snapshot()
            .local_history_table()[0],
        0
    );
}

#[test]
fn selected_tournament_fetch_ahead_uses_pending_conditional_global_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, b_type(4, 0, 0, 0).to_le_bytes().to_vec()),
        (1, 0x8004, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tournament_predictor(&mut state);
        let older_pc = Address::new(0x8000);
        let younger_pc = Address::new(0x8004);
        train_selected_tournament_global_history_one_taken(&mut state, Address::new(0x9000));
        assert!(!state
            .tournament_branch_predictor
            .shares_local_history_entry(older_pc, younger_pc));
        let base_prediction = state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc)
            .unwrap();
        assert_eq!(base_prediction.global_history_before(), 0);
        assert_eq!(base_prediction.local_history_before(), 0);
        assert!(!base_prediction.predicted_taken());
        let overlay_prediction = state
            .tournament_branch_predictor
            .predict_with_histories(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc, 1, 0)
            .unwrap();
        assert!(overlay_prediction.predicted_taken());
        insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
        let fetch_events = core.core.fetch_events();
        let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
        assert_eq!(
            selected_tournament_speculative_histories(&state, &completed_fetches, younger_pc),
            Some((1, 0))
        );
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[1],
            0
        );
    }

    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x800c));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
        0
    );
    assert_eq!(
        state
            .tournament_branch_predictor
            .snapshot()
            .local_history_table()[1],
        0
    );
}

#[test]
fn selected_tournament_fetch_ahead_uses_direct_jump_global_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tournament_predictor(&mut state);
        let older_pc = Address::new(0x8000);
        let younger_pc = Address::new(0x8008);
        train_selected_tournament_global_history_one_taken(&mut state, Address::new(0x9000));
        let base_prediction = state
            .tournament_branch_predictor
            .predict(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc)
            .unwrap();
        assert_eq!(base_prediction.global_history_before(), 0);
        assert_eq!(base_prediction.local_history_before(), 0);
        assert!(!base_prediction.predicted_taken());
        let overlay_prediction = state
            .tournament_branch_predictor
            .predict_with_histories(RISCV_LOCAL_TOURNAMENT_THREAD, younger_pc, 1, 0)
            .unwrap();
        assert!(overlay_prediction.predicted_taken());
        insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
        let fetch_events = core.core.fetch_events();
        let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
        assert_eq!(
            selected_tournament_speculative_histories(&state, &completed_fetches, younger_pc),
            Some((1, 0))
        );
        assert_eq!(
            state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[0],
            0
        );
    }

    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
        0
    );
    assert_eq!(
        state
            .tournament_branch_predictor
            .snapshot()
            .local_history_table()[0],
        0
    );
}

#[test]
fn selected_multiperspective_fetch_ahead_uses_pending_local_history_for_younger_branch() {
    let core = core_with_completed_fetches([
        (0, 0x8000, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
        (1, 0x8008, b_type(8, 0, 0, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::MultiperspectivePerceptron);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        use_local_bias_multiperspective_perceptron(&mut state);
        let older_pc = Address::new(0x8000);
        let younger_pc = Address::new(0x8008);
        let base_prediction = state
            .multiperspective_perceptron
            .predict(
                RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD,
                younger_pc,
                true,
            )
            .unwrap();
        assert!(!base_prediction.predicted_taken());
        insert_pending_branch_speculation(&mut state, 0, older_pc, younger_pc);
        assert_eq!(
            state
                .multiperspective_perceptron
                .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
                .unwrap()
                .local_history_for(younger_pc),
            0
        );
    }

    let second = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(second.pc(), Address::new(0x8010));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state
            .multiperspective_perceptron
            .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
            .unwrap()
            .local_history_for(Address::new(0x8008)),
        0
    );
}
