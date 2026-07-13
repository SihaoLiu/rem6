use super::*;

#[test]
fn fetch_ahead_accepts_compressed_straight_line_instruction() {
    let mut fetch_data = Vec::new();
    fetch_data.extend_from_slice(&0x0001_u16.to_le_bytes());
    fetch_data.extend_from_slice(&0x0000_0073_u32.to_le_bytes()[..2]);
    let core = core_with_completed_fetch(fetch_data);

    assert_eq!(
        core.next_fetch_ahead_before_retire()
            .map(|decision| decision.pc()),
        Some(Address::new(0x8002))
    );
}

#[test]
fn fetch_ahead_uses_direct_jal_target() {
    let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
    assert_eq!(
        decision.branch_speculation().map(|speculation| {
            (
                speculation.sequence(),
                speculation.pc(),
                speculation.predicted_taken(),
                speculation.target(),
            )
        }),
        Some((0, Address::new(0x8000), true, Some(Address::new(0x800c))))
    );
}

#[test]
fn selected_gshare_speculation_controls_retire_branch_prediction() {
    let branch = b_type(8, 0, 0, 0x1).to_le_bytes().to_vec();
    let core = core_with_completed_fetch(branch);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        train_selected_gshare_taken(&mut state, Address::new(0x8000));
    }

    let decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(decision.pc(), Address::new(0x8008));
    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let event = core.execute_next_completed_fetch().unwrap().unwrap();
    let basic_update = event.branch_update().unwrap();
    assert!(!basic_update.predicted_taken());

    let cycle = event.in_order_pipeline_cycle().unwrap();
    let prediction = cycle.branch_predictions().first().unwrap();
    assert!(prediction.predicted_taken());
    assert_eq!(prediction.predicted_target_pc(), Some(0x8008));
    assert!(!prediction.resolved_taken());
    assert_eq!(prediction.repair_target_pc(), Some(0x8004));
    let summary = core.branch_speculation_summary();
    assert_eq!(summary.repairs(), 1);
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
        1
    );
    assert_eq!(
        summary.target_provider().value(BranchTargetProvider::BTB),
        0
    );
    assert_eq!(summary.target_provider().total(), 1);
    assert_eq!(
        summary
            .mispredict_due_to_predictor()
            .value(BranchTargetKind::DirectConditional),
        1
    );
    assert_eq!(summary.mispredict_due_to_predictor().total(), 1);
    assert_eq!(summary.btb_mispredict_due_to_btb_miss().total(), 0);
}

#[test]
fn selected_gshare_direct_speculation_redirect_discards_live_history() {
    let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.selected_branch_speculations.len(), 1);
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            1
        );
    }

    core.redirect_pc(Address::new(0x9000));

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.is_empty());
    assert!(state.selected_branch_speculations.is_empty());
    assert_eq!(
        state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
        0
    );
}

#[test]
fn selected_tage_sc_l_direct_speculation_redirect_discards_live_history() {
    let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::TageScL);
    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.selected_branch_speculations.len(), 1);
        let snapshot = state.tage_sc_l_branch_predictor.snapshot();
        assert_eq!(snapshot.history_update_count(), 1);
        assert_eq!(
            snapshot.ltage().tage().threads()[0].global_history_value(),
            1
        );
    }

    core.redirect_pc(Address::new(0x9000));

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.is_empty());
    assert!(state.selected_branch_speculations.is_empty());
    let snapshot = state.tage_sc_l_branch_predictor.snapshot();
    assert_eq!(snapshot.history_update_count(), 0);
    assert_eq!(
        snapshot.ltage().tage().threads()[0].global_history_value(),
        0
    );
}

#[test]
fn selected_multiperspective_direct_speculation_redirect_discards_live_history() {
    let core = core_with_recorded_selected_direct_speculation(
        RiscvBranchPredictorKind::MultiperspectivePerceptron,
    );
    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(state.selected_branch_speculations.len(), 1);
        let thread = state
            .multiperspective_perceptron
            .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
            .unwrap();
        assert_eq!(thread.local_history_for(Address::new(0x8000)), 1);
    }

    core.redirect_pc(Address::new(0x9000));

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.is_empty());
    assert!(state.selected_branch_speculations.is_empty());
    let thread = state
        .multiperspective_perceptron
        .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
        .unwrap();
    assert_eq!(thread.local_history_for(Address::new(0x8000)), 0);
}

#[test]
fn selected_tage_sc_l_direct_speculation_retire_keeps_single_history_update() {
    let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::TageScL);

    core.execute_next_completed_fetch().unwrap().unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.selected_branch_speculations.is_empty());
    let snapshot = state.tage_sc_l_branch_predictor.snapshot();
    assert_eq!(snapshot.update_count(), 1);
    assert_eq!(snapshot.history_update_count(), 0);
    assert_eq!(
        snapshot.ltage().tage().threads()[0].global_history_value(),
        1
    );
}

#[test]
fn selected_multiperspective_direct_speculation_retire_keeps_single_history_update() {
    let core = core_with_recorded_selected_direct_speculation(
        RiscvBranchPredictorKind::MultiperspectivePerceptron,
    );

    core.execute_next_completed_fetch().unwrap().unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.selected_branch_speculations.is_empty());
    let thread = state
        .multiperspective_perceptron
        .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
        .unwrap();
    assert_eq!(thread.local_history_for(Address::new(0x8000)), 1);
}

#[test]
fn selected_record_failure_does_not_leave_generic_branch_speculation() {
    let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let prediction = state
            .gshare_branch_predictor
            .predict_unconditional(RISCV_LOCAL_GSHARE_THREAD, Address::new(0x9000))
            .unwrap();
        state
            .gshare_branch_predictor
            .update_history(prediction.history(), true)
            .unwrap();
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            1
        );
    }

    assert!(record_fetch_ahead_speculation(&core, &decision).is_err());

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.is_empty());
    assert!(state.selected_branch_speculations.is_empty());
    assert!(state.branch_predictor.pending_speculations().is_empty());
}

#[test]
fn prepared_fetch_issue_records_speculation_when_pipeline_admission_is_deferred() {
    let core = core_with_completed_fetch(j_type(12, 0).to_le_bytes().to_vec());
    let decision = core.next_fetch_ahead_before_retire().unwrap();
    let prepared = core.prepare_fetch_ahead_speculation(&decision).unwrap();
    let config = RiscvCore::default_in_order_pipeline_snapshot()
        .config()
        .clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        u64::MAX,
        [InOrderPipelineInstruction::new(
            99,
            InOrderPipelineStage::Fetch1,
        )],
    ))
    .unwrap();
    let issue = OutstandingFetch {
        tick: 4,
        partition: PartitionId::new(0),
        route: MemoryRouteId::new(0),
        endpoint: endpoint("cpu0.ifetch"),
        request_id: request(1),
        pc: decision.pc(),
        size: AccessSize::new(4).unwrap(),
        line_layout: layout(),
    };

    core.record_prepared_fetch_issue_with_prepared_fetch_ahead(issue, prepared)
        .unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.contains_key(&0));
    assert!(state.branch_predictor.pending_speculation_count() > 0);
    let snapshot = state.in_order_pipeline.snapshot();
    assert_eq!(snapshot.cycle(), u64::MAX);
    assert_eq!(
        snapshot.in_flight(),
        &[InOrderPipelineInstruction::new(
            0,
            InOrderPipelineStage::Fetch1
        )]
    );
    assert!(!state.in_order_pipeline.contains_sequence(1));
    assert!(state.in_order_pipeline_cycle_records.is_empty());
}

#[test]
fn selected_gshare_direct_speculation_after_restore_uses_pending_generic_history() {
    let core = core_with_completed_fetch(j_type(8, 0).to_le_bytes().to_vec());
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::GShare);
    let fetch_events = core.core.fetch_events();
    let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
    let selected = {
        let mut state = core.state.lock().expect("riscv core lock");
        insert_pending_branch_speculation(
            &mut state,
            0,
            Address::new(0x8000),
            Address::new(0x8008),
        );
        assert_eq!(state.branch_speculations.len(), 1);
        assert!(state.selected_branch_speculations.is_empty());
        assert_eq!(
            state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
            0
        );

        selected_direct_branch_speculation(
            &mut state,
            &completed_fetches,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            Address::new(0x8010),
        )
        .unwrap()
        .expect("selected direct speculation")
    };
    if let RiscvSelectedBranchSpeculation::GShare { prediction, .. } = &selected {
        assert_eq!(prediction.global_history_before(), 1);
    } else {
        panic!("expected selected GShare speculation");
    }
    let decision = RiscvFetchAheadDecision::branch(
        Address::new(0x8010),
        1,
        Address::new(0x8008),
        BranchTargetKind::DirectUnconditional,
        true,
        Some(Address::new(0x8010)),
        Some(selected),
        None,
        None,
        BranchTargetProvider::NoTarget,
    );

    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 2);
    assert_eq!(state.selected_branch_speculations.len(), 2);
    assert_eq!(
        state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
        3
    );
}

#[test]
fn selected_tage_sc_l_direct_speculation_after_restore_uses_pending_generic_history() {
    let core = core_with_completed_fetch(j_type(8, 0).to_le_bytes().to_vec());
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::TageScL);
    let fetch_events = core.core.fetch_events();
    let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
    let selected = {
        let mut state = core.state.lock().expect("riscv core lock");
        insert_pending_branch_speculation(
            &mut state,
            0,
            Address::new(0x8000),
            Address::new(0x8008),
        );
        assert_eq!(state.branch_speculations.len(), 1);
        assert!(state.selected_branch_speculations.is_empty());
        assert_eq!(
            state
                .tage_sc_l_branch_predictor
                .snapshot()
                .ltage()
                .tage()
                .threads()[0]
                .global_history_value(),
            0
        );

        selected_direct_branch_speculation(
            &mut state,
            &completed_fetches,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            Address::new(0x8010),
        )
        .unwrap()
        .expect("selected direct speculation")
    };
    if let RiscvSelectedBranchSpeculation::TageScL { prediction, .. } = &selected {
        assert_eq!(
            prediction
                .history()
                .ltage_history()
                .tage_history()
                .thread_before()
                .global_history_value(),
            1
        );
    } else {
        panic!("expected selected TAGE-SC-L speculation");
    }
    let decision = RiscvFetchAheadDecision::branch(
        Address::new(0x8010),
        1,
        Address::new(0x8008),
        BranchTargetKind::DirectUnconditional,
        true,
        Some(Address::new(0x8010)),
        Some(selected),
        None,
        None,
        BranchTargetProvider::NoTarget,
    );

    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 2);
    assert_eq!(state.selected_branch_speculations.len(), 2);
    assert_eq!(
        state
            .tage_sc_l_branch_predictor
            .snapshot()
            .ltage()
            .tage()
            .threads()[0]
            .global_history_value(),
        3
    );
}

#[test]
fn selected_tournament_direct_replay_after_restore_keeps_local_history_unchanged() {
    let core = core_with_completed_fetches([
        (0, 0x8000, j_type(8, 0).to_le_bytes().to_vec()),
        (1, 0x8008, j_type(8, 0).to_le_bytes().to_vec()),
    ]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
    let fetch_events = core.core.fetch_events();
    let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
    let selected = {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tournament_predictor(&mut state);
        insert_pending_branch_speculation(
            &mut state,
            0,
            Address::new(0x8000),
            Address::new(0x8008),
        );
        assert_eq!(state.branch_speculations.len(), 1);
        assert!(state.selected_branch_speculations.is_empty());
        assert_eq!(
            state
                .tournament_branch_predictor
                .snapshot()
                .local_history_table()[0],
            0
        );

        selected_direct_branch_speculation(
            &mut state,
            &completed_fetches,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            Address::new(0x8010),
        )
        .unwrap()
        .expect("selected direct speculation")
    };
    if let RiscvSelectedBranchSpeculation::Tournament { prediction, .. } = &selected {
        assert_eq!(prediction.global_history_before(), 1);
        assert!(!prediction.local_history_valid());
    } else {
        panic!("expected selected Tournament speculation");
    }
    let decision = RiscvFetchAheadDecision::branch(
        Address::new(0x8010),
        1,
        Address::new(0x8008),
        BranchTargetKind::DirectUnconditional,
        true,
        Some(Address::new(0x8010)),
        Some(selected),
        None,
        None,
        BranchTargetProvider::NoTarget,
    );

    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 2);
    assert_eq!(state.selected_branch_speculations.len(), 2);
    let replayed = state.selected_branch_speculations.get(&0).unwrap();
    let RiscvSelectedBranchSpeculation::Tournament { prediction, .. } = replayed else {
        panic!("expected replayed Tournament speculation");
    };
    assert!(!prediction.local_history_valid());
    assert_eq!(
        state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
        1
    );
    assert_eq!(
        state
            .tournament_branch_predictor
            .snapshot()
            .history_update_count(),
        2
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
fn selected_tournament_replay_uses_completed_event_after_issued_event() {
    let core = core_with_completed_fetches(std::iter::empty::<(u64, u64, Vec<u8>)>());
    {
        let mut core_state = core.core.state.lock().expect("cpu core lock");
        core_state
            .events
            .push(crate::CpuFetchEvent::issued(CpuFetchRecord::new(
                3,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                request(0),
                Address::new(0x8000),
                AccessSize::new(4).unwrap(),
            )));
        core_state.events.push(crate::CpuFetchEvent::completed(
            CpuFetchRecord::new(
                4,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                endpoint("cpu0.ifetch"),
                request(0),
                Address::new(0x8000),
                AccessSize::new(4).unwrap(),
            ),
            j_type(8, 0).to_le_bytes().to_vec(),
        ));
    }
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
    let fetch_events = core.core.fetch_events();
    let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
    let selected = {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tournament_predictor(&mut state);
        insert_pending_branch_speculation(
            &mut state,
            0,
            Address::new(0x8000),
            Address::new(0x8008),
        );
        selected_direct_branch_speculation(
            &mut state,
            &completed_fetches,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            Address::new(0x8010),
        )
        .unwrap()
        .expect("selected direct speculation")
    };
    let decision = RiscvFetchAheadDecision::branch(
        Address::new(0x8010),
        1,
        Address::new(0x8008),
        BranchTargetKind::DirectUnconditional,
        true,
        Some(Address::new(0x8010)),
        Some(selected),
        None,
        None,
        BranchTargetProvider::NoTarget,
    );

    record_fetch_ahead_speculation(&core, &decision).unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 2);
    assert_eq!(state.selected_branch_speculations.len(), 2);
    assert_eq!(
        state
            .tournament_branch_predictor
            .snapshot()
            .history_update_count(),
        2
    );
}

#[test]
fn selected_tournament_replay_requires_completed_instruction_metadata() {
    let core = core_with_completed_fetches([(1, 0x8008, j_type(8, 0).to_le_bytes().to_vec())]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
    let fetch_events = core.core.fetch_events();
    let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
    let selected = {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tournament_predictor(&mut state);
        insert_pending_branch_speculation(
            &mut state,
            0,
            Address::new(0x8000),
            Address::new(0x8008),
        );
        selected_direct_branch_speculation(
            &mut state,
            &completed_fetches,
            Address::new(0x8008),
            BranchTargetKind::DirectUnconditional,
            Address::new(0x8010),
        )
        .unwrap()
        .expect("selected direct speculation")
    };
    let decision = RiscvFetchAheadDecision::branch(
        Address::new(0x8010),
        1,
        Address::new(0x8008),
        BranchTargetKind::DirectUnconditional,
        true,
        Some(Address::new(0x8010)),
        Some(selected),
        None,
        None,
        BranchTargetProvider::NoTarget,
    );

    let error = record_fetch_ahead_speculation(&core, &decision).unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::MissingBranchSpeculationInstruction { sequence: 0 }
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 1);
    assert!(state.selected_branch_speculations.is_empty());
    assert_eq!(state.branch_predictor.pending_speculation_count(), 1);
    assert_eq!(
        state
            .tournament_branch_predictor
            .snapshot()
            .history_update_count(),
        0
    );
}

#[test]
fn selected_tournament_replay_failure_discards_partial_recording() {
    let core = core_with_completed_fetches([(0, 0x8000, j_type(8, 0).to_le_bytes().to_vec())]);
    core.set_branch_predictor_kind(RiscvBranchPredictorKind::Tournament);
    let fetch_events = core.core.fetch_events();
    let completed_fetches = fetch_events.iter().collect::<Vec<_>>();
    let selected = {
        let mut state = core.state.lock().expect("riscv core lock");
        use_small_tournament_predictor(&mut state);
        insert_pending_branch_speculation(
            &mut state,
            0,
            Address::new(0x8000),
            Address::new(0x8008),
        );
        insert_pending_branch_speculation(
            &mut state,
            1,
            Address::new(0x8008),
            Address::new(0x8010),
        );
        selected_direct_branch_speculation(
            &mut state,
            &completed_fetches,
            Address::new(0x8010),
            BranchTargetKind::DirectUnconditional,
            Address::new(0x8018),
        )
        .unwrap()
        .expect("selected direct speculation")
    };
    let decision = RiscvFetchAheadDecision::branch(
        Address::new(0x8018),
        2,
        Address::new(0x8010),
        BranchTargetKind::DirectUnconditional,
        true,
        Some(Address::new(0x8018)),
        Some(selected),
        None,
        None,
        BranchTargetProvider::NoTarget,
    );

    let error = record_fetch_ahead_speculation(&core, &decision).unwrap_err();

    assert!(matches!(
        error,
        RiscvCpuError::MissingBranchSpeculationInstruction { sequence: 1 }
    ));
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 2);
    assert!(state.selected_branch_speculations.is_empty());
    assert_eq!(state.branch_predictor.pending_speculation_count(), 2);
    assert_eq!(
        state.tournament_branch_predictor.snapshot().threads()[0].global_history(),
        0
    );
    assert_eq!(
        state
            .tournament_branch_predictor
            .snapshot()
            .history_update_count(),
        0
    );
}

#[test]
fn generic_branch_checkpoint_restore_rolls_back_selected_family_history() {
    let core = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
    assert_eq!(
        core.gshare_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );

    core.restore_branch_predictor_checkpoint_payload(
        RiscvCore::default_branch_predictor_checkpoint_payload(),
    )
    .unwrap();

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.branch_speculations.is_empty());
    assert!(state.selected_branch_speculations.is_empty());
    assert_eq!(
        state.gshare_branch_predictor.snapshot().threads()[0].global_history(),
        0
    );
}

#[test]
fn selected_family_checkpoint_payloads_use_committed_fetch_ahead_history() {
    let gshare = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::GShare);
    assert_eq!(
        gshare.gshare_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
    assert_eq!(
        gshare
            .gshare_branch_predictor_checkpoint_payload()
            .snapshot()
            .threads()[0]
            .global_history(),
        0
    );

    let bimode = core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::BiMode);
    assert_eq!(
        bimode.bimode_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
    assert_eq!(
        bimode
            .bimode_branch_predictor_checkpoint_payload()
            .snapshot()
            .threads()[0]
            .global_history(),
        0
    );

    let tournament =
        core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::Tournament);
    assert_eq!(
        tournament.tournament_branch_predictor_snapshot().threads()[0].global_history(),
        1
    );
    assert_eq!(
        tournament
            .tournament_branch_predictor_checkpoint_payload()
            .snapshot()
            .threads()[0]
            .global_history(),
        0
    );

    let tage_sc_l =
        core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::TageScL);
    assert_eq!(
        tage_sc_l
            .tage_sc_l_branch_predictor_snapshot()
            .ltage()
            .tage()
            .threads()[0]
            .global_history_value(),
        1
    );
    assert_eq!(
        tage_sc_l
            .tage_sc_l_branch_predictor_checkpoint_payload()
            .snapshot()
            .ltage()
            .tage()
            .threads()[0]
            .global_history_value(),
        0
    );

    let multiperspective = core_with_recorded_selected_direct_speculation(
        RiscvBranchPredictorKind::MultiperspectivePerceptron,
    );
    assert_eq!(
        multiperspective
            .multiperspective_perceptron_snapshot()
            .threads[0]
            .local_history_for(Address::new(0x8000)),
        1
    );
    assert_eq!(
        multiperspective
            .multiperspective_perceptron_checkpoint_payload()
            .snapshot()
            .threads[0]
            .local_history_for(Address::new(0x8000)),
        0
    );
}

#[test]
fn selected_family_checkpoint_restore_forgets_unreflected_selected_speculation() {
    for kind in [
        RiscvBranchPredictorKind::GShare,
        RiscvBranchPredictorKind::BiMode,
        RiscvBranchPredictorKind::Tournament,
    ] {
        let core = core_with_recorded_selected_direct_speculation(kind);
        {
            let state = core.state.lock().expect("riscv core lock");
            assert_eq!(selected_family_speculation_count(&state, kind), 1);
            assert_eq!(selected_family_global_history(&state, kind), 1);
        }

        restore_selected_family_checkpoint(&core, kind);

        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(selected_family_speculation_count(&state, kind), 0);
        assert_eq!(selected_family_global_history(&state, kind), 0);
    }

    let tage_sc_l =
        core_with_recorded_selected_direct_speculation(RiscvBranchPredictorKind::TageScL);
    {
        let state = tage_sc_l.state.lock().expect("riscv core lock");
        assert_eq!(
            selected_family_speculation_count(&state, RiscvBranchPredictorKind::TageScL),
            1
        );
        assert_eq!(
            state
                .tage_sc_l_branch_predictor
                .snapshot()
                .ltage()
                .tage()
                .threads()[0]
                .global_history_value(),
            1
        );
    }

    restore_selected_family_checkpoint(&tage_sc_l, RiscvBranchPredictorKind::TageScL);

    {
        let state = tage_sc_l.state.lock().expect("riscv core lock");
        assert_eq!(
            selected_family_speculation_count(&state, RiscvBranchPredictorKind::TageScL),
            0
        );
        assert_eq!(
            state
                .tage_sc_l_branch_predictor
                .snapshot()
                .ltage()
                .tage()
                .threads()[0]
                .global_history_value(),
            0
        );
    }

    let multiperspective = core_with_recorded_selected_direct_speculation(
        RiscvBranchPredictorKind::MultiperspectivePerceptron,
    );
    {
        let state = multiperspective.state.lock().expect("riscv core lock");
        assert_eq!(
            selected_family_speculation_count(
                &state,
                RiscvBranchPredictorKind::MultiperspectivePerceptron
            ),
            1
        );
        assert_eq!(
            state
                .multiperspective_perceptron
                .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
                .unwrap()
                .local_history_for(Address::new(0x8000)),
            1
        );
    }

    restore_selected_family_checkpoint(
        &multiperspective,
        RiscvBranchPredictorKind::MultiperspectivePerceptron,
    );

    let state = multiperspective.state.lock().expect("riscv core lock");
    assert_eq!(
        selected_family_speculation_count(
            &state,
            RiscvBranchPredictorKind::MultiperspectivePerceptron
        ),
        0
    );
    assert_eq!(
        state
            .multiperspective_perceptron
            .thread_snapshot(RISCV_LOCAL_MULTIPERSPECTIVE_PERCEPTRON_THREAD)
            .unwrap()
            .local_history_for(Address::new(0x8000)),
        0
    );
}
