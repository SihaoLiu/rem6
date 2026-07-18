use super::*;

fn detailed_control_core(branch_rs1: u8, with_descendant: bool) -> RiscvCore {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let branch = b_type(8, branch_rs1, 2, 0x0);
    let mut fetches = vec![
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, branch.to_le_bytes().to_vec()),
    ];
    if with_descendant {
        let mul = r_type(0x01, 7, 8, 0x0, 9, 0x33);
        fetches.push((2, 0x8008, mul.to_le_bytes().to_vec()));
    }
    let core = core_with_completed_fetches(fetches);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core
}

fn detailed_linked_control_core(
    fetches: impl IntoIterator<Item = (u64, u64, Vec<u8>)>,
) -> RiscvCore {
    let core = core_with_completed_fetches(fetches);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core
}

pub(super) fn live_same_link_core(with_target_fetch: bool) -> (RiscvCore, RiscvCpuExecutionEvent) {
    let load_raw = i_type(0, 18, 0x6, 12, 0x03);
    let producer_raw = i_type(0, 11, 0x0, 1, 0x13);
    let call_raw = i_type(0, 1, 0x0, 1, 0x67);
    let descendant_raw = i_type(0, 1, 0x0, 13, 0x13);
    let producer = RiscvInstruction::decode(producer_raw).unwrap();
    let call = RiscvInstruction::decode(call_raw).unwrap();
    let mut fetches = vec![
        (0, 0x8000, load_raw.to_le_bytes().to_vec()),
        (1, 0x8004, producer_raw.to_le_bytes().to_vec()),
        (2, 0x8008, call_raw.to_le_bytes().to_vec()),
    ];
    if with_target_fetch {
        fetches.push((3, 0x9000, descendant_raw.to_le_bytes().to_vec()));
    }
    let core = detailed_linked_control_core(fetches);
    core.set_branch_lookahead(1);
    let load = scalar_load_execution_event(0x8000, 0, 12, 18, 0x100);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state
            .o3_runtime
            .stage_live_data_access_issue_for_test(&load, request(20), 31));
        assert_eq!(
            state.o3_runtime.stage_live_data_access_younger_window(
                request(0),
                [
                    (Address::new(0x8004), producer),
                    (Address::new(0x8008), call),
                ],
            ),
            2
        );
        let producer_candidate = state
            .o3_runtime
            .live_speculative_issue_candidate(Address::new(0x8004), producer)
            .unwrap();
        assert!(state
            .o3_runtime
            .record_live_speculative_execution(
                producer_candidate,
                &[request(1)],
                20,
                RiscvExecutionRecord::new(
                    producer,
                    0x8004,
                    0x8008,
                    vec![rem6_isa_riscv::RegisterWrite::new(
                        Register::new(1).unwrap(),
                        0x9000,
                    )],
                    None,
                ),
            )
            .unwrap());
        let call_candidate = state
            .o3_runtime
            .live_speculative_issue_candidate(Address::new(0x8008), call)
            .unwrap();
        assert!(state
            .o3_runtime
            .record_live_speculative_execution(
                call_candidate,
                &[request(2)],
                21,
                RiscvExecutionRecord::new(
                    call,
                    0x8008,
                    0x9000,
                    vec![rem6_isa_riscv::RegisterWrite::new(
                        Register::new(1).unwrap(),
                        0x800c,
                    )],
                    None,
                ),
            )
            .unwrap());
    }
    (core, load)
}

fn recorded_same_window_coroutine_core() -> RiscvCore {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let coroutine = i_type(0, 1, 0x0, 5, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, coroutine.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let coroutine_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(coroutine_decision.pc(), Address::new(0x8008));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&coroutine_decision)
            .unwrap(),
    );

    core
}

#[test]
fn detailed_live_same_link_control_uses_runtime_forwarded_target() {
    let (core, _) = live_same_link_core(false);

    assert_eq!(core.requested_o3_writeback_wake_tick(19), Some(20));
    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link decision");
    assert_eq!(decision.pc(), Address::new(0x9000));
    assert_eq!(
        decision.branch_speculation().unwrap().target(),
        Some(Address::new(0x9000))
    );
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&decision).unwrap(),
    );
    let mut state = core.state.lock().expect("riscv core lock");
    let consumer_sequence = state
        .o3_runtime
        .producer_forwarded_control_target()
        .unwrap()
        .consumer_sequence();
    assert!(state
        .o3_runtime
        .has_recorded_producer_forwarded_control_target(consumer_sequence));
    let forwarded = state
        .o3_runtime
        .producer_forwarded_control_target()
        .unwrap();
    let authority = PredictedControlTargetAuthority::ProducerForwarded(forwarded);
    assert_eq!(
        recorded_predicted_pc(&state, request(2), Address::new(0x800c), authority),
        RecordedPredictedPc::Ready(Address::new(0x9000))
    );
    assert_eq!(
        recorded_predicted_pc(&state, request(99), Address::new(0x800c), authority),
        RecordedPredictedPc::Invalid
    );
    assert_eq!(
        recorded_predicted_pc(&state, request(2), Address::new(0x8010), authority),
        RecordedPredictedPc::Invalid
    );
    let descendant = RiscvInstruction::Addi {
        rd: Register::new(13).unwrap(),
        rs1: Register::new(1).unwrap(),
        imm: Immediate::new(0),
    };
    assert!(state
        .o3_runtime
        .append_producer_forwarded_control_descendant(
            forwarded,
            Address::new(0x9000),
            descendant,
            &[request(3)],
        )
        .is_some());
    assert_eq!(
        recorded_predicted_pc(&state, request(2), Address::new(0x800c), authority),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn recorded_producer_forwarded_target_rejects_unmarked_same_target_speculation() {
    let (core, _) = live_same_link_core(false);

    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link decision");
    let mut ordinary = decision.clone();
    ordinary
        .branch_speculation
        .as_mut()
        .expect("same-link decision has speculation")
        .producer_forwarded_control_target = None;
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&ordinary).unwrap(),
    );

    let state = core.state.lock().expect("riscv core lock");
    let forwarded = state
        .o3_runtime
        .producer_forwarded_control_target()
        .expect("resident same-link authority");
    assert!(!state
        .o3_runtime
        .has_recorded_producer_forwarded_control_target(forwarded.consumer_sequence()));
    assert_eq!(
        recorded_predicted_pc(
            &state,
            forwarded.fetch_request(),
            forwarded.sequential_pc(),
            PredictedControlTargetAuthority::ProducerForwarded(forwarded),
        ),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn producer_forwarded_speculation_apply_fails_closed_after_authority_invalidation() {
    let (core, mut load) = live_same_link_core(false);
    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link decision");
    let consumer_sequence = decision
        .branch_speculation()
        .and_then(|speculation| speculation.producer_forwarded_control_target)
        .expect("producer-forwarded authority")
        .consumer_sequence();
    let prepared = core.prepare_fetch_ahead_speculation(&decision).unwrap();

    load.set_data_access_event_kind(crate::RiscvDataAccessEventKind::Completed);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state
            .o3_runtime
            .complete_live_data_access_response(&load, request(20), 40, 9, Some(&[1, 0, 0, 0]))
            .unwrap());
        assert!(state
            .o3_runtime
            .producer_forwarded_control_target()
            .is_none());
    }

    core.record_prepared_fetch_ahead_speculation(prepared);

    let state = core.state.lock().expect("riscv core lock");
    assert!(!state
        .o3_runtime
        .has_recorded_producer_forwarded_control_target(consumer_sequence));
    assert!(state.branch_speculations.is_empty());
    assert!(state.branch_speculation_kinds.is_empty());
    assert!(state.branch_target_predictions.is_empty());
    assert!(state.return_address_stack_operations.is_empty());
    assert!(state.selected_branch_speculations.is_empty());
}

#[test]
fn completed_target_appends_before_ready_live_load_retirement() {
    let (core, mut load) = live_same_link_core(true);
    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link decision");
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&decision).unwrap(),
    );

    load.set_data_access_event_kind(crate::RiscvDataAccessEventKind::Completed);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        assert!(state
            .o3_runtime
            .complete_live_data_access_response(&load, request(20), 40, 9, Some(&[1, 0, 0, 0]))
            .unwrap());
        assert!(state.o3_runtime.has_ready_live_data_access_event());
        assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 3);
    }
    let config = RiscvCore::default_in_order_pipeline_snapshot()
        .config()
        .clone();
    core.restore_in_order_pipeline_snapshot(InOrderPipelineSnapshot::with_cycle(
        config,
        0,
        [
            InOrderPipelineInstruction::new(99, InOrderPipelineStage::Execute)
                .with_execute_wait(2, 1),
        ],
    ))
    .unwrap();
    assert!(
        core.detailed_o3_window_prefers_fetch_ahead(),
        "recorded completed authority must override an unrelated draining execute-wait"
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.o3_runtime.has_ready_live_data_access_event());
    assert_eq!(state.o3_runtime.snapshot().reorder_buffer().len(), 4);
    assert!(state
        .o3_runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .any(|entry| entry.pc() == Address::new(0x9000)));
}

#[test]
fn completed_producer_forwarded_authority_closes_when_load_event_is_taken() {
    let (core, mut load) = live_same_link_core(false);
    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link decision");
    let forwarded = decision
        .branch_speculation()
        .and_then(|speculation| speculation.producer_forwarded_control_target)
        .expect("producer-forwarded authority");
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&decision).unwrap(),
    );

    load.set_data_access_event_kind(crate::RiscvDataAccessEventKind::Completed);
    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .complete_live_data_access_response(&load, request(20), 40, 9, Some(&[1, 0, 0, 0]))
        .unwrap());
    assert_eq!(
        state
            .o3_runtime
            .retained_producer_forwarded_control_target(),
        Some(forwarded)
    );
    assert!(state
        .o3_runtime
        .take_ready_live_data_access_event(u64::MAX)
        .is_some());
    assert!(state
        .o3_runtime
        .retained_producer_forwarded_control_target()
        .is_none());
}

#[test]
fn failed_load_response_closes_recorded_producer_forwarded_authority() {
    let (core, mut load) = live_same_link_core(false);
    let decision = core
        .next_fetch_ahead_before_retire()
        .expect("runtime-forwarded same-link decision");
    let consumer_sequence = decision
        .branch_speculation()
        .and_then(|speculation| speculation.producer_forwarded_control_target)
        .expect("producer-forwarded authority")
        .consumer_sequence();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&decision).unwrap(),
    );

    load.set_data_access_event_kind(crate::RiscvDataAccessEventKind::Failed);
    let mut state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .complete_live_data_access_response(&load, request(20), 40, 9, None)
        .unwrap());
    assert!(state
        .o3_runtime
        .retained_producer_forwarded_control_target()
        .is_none());
    assert!(!state
        .o3_runtime
        .has_recorded_producer_forwarded_control_target(consumer_sequence));
}

fn recorded_same_window_coroutine_target_authority() -> PredictedControlTargetAuthority {
    let call = RiscvInstruction::decode(j_type(8, 1)).unwrap();
    let coroutine = RiscvInstruction::decode(i_type(0, 1, 0x0, 5, 0x67)).unwrap();
    let mut window =
        crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(6).unwrap()],
            1,
            4,
        )
        .unwrap();
    let call_classification = window.classify_sequenced_younger(call, 1);
    assert_eq!(
        call_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    let coroutine_classification = window.classify_sequenced_younger(coroutine, 2);
    assert_eq!(
        coroutine_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    predicted_control_target_authority(
        coroutine,
        Address::new(0x8010),
        coroutine_classification,
        &[(1, Address::new(0x8008)), (2, Address::new(0x8010))],
    )
    .unwrap()
}

fn recorded_same_window_coroutine_pc(core: &RiscvCore) -> RecordedPredictedPc {
    let state = core.state.lock().expect("riscv core lock");
    recorded_predicted_pc(
        &state,
        request(2),
        Address::new(0x8010),
        recorded_same_window_coroutine_target_authority(),
    )
}

fn unconsumed_same_window_coroutine_round_trip_core() -> RiscvCore {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let coroutine = i_type(0, 1, 0x0, 5, 0x67);
    let return_jump = i_type(0, 5, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, coroutine.to_le_bytes().to_vec()),
        (3, 0x8008, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(3);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let coroutine_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(coroutine_decision.pc(), Address::new(0x8008));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&coroutine_decision)
            .unwrap(),
    );

    core
}

fn recorded_same_window_coroutine_round_trip_core() -> RiscvCore {
    let core = unconsumed_same_window_coroutine_round_trip_core();
    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(return_decision.pc(), Address::new(0x8010));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );
    core
}

fn recorded_same_window_round_trip_target_authority() -> PredictedControlTargetAuthority {
    let call = RiscvInstruction::decode(j_type(8, 1)).unwrap();
    let coroutine = RiscvInstruction::decode(i_type(0, 1, 0x0, 5, 0x67)).unwrap();
    let return_jump = RiscvInstruction::decode(i_type(0, 5, 0x0, 0, 0x67)).unwrap();
    let mut window =
        crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(6).unwrap()],
            1,
            4,
        )
        .unwrap();
    let call_classification = window.classify_sequenced_younger(call, 1);
    assert_eq!(
        call_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    let coroutine_classification = window.classify_sequenced_younger(coroutine, 2);
    assert_eq!(
        coroutine_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(coroutine_classification.ras_push_sequence(), Some(1));
    let return_classification = window.classify_sequenced_younger(return_jump, 3);
    assert_eq!(
        return_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(return_classification.ras_push_sequence(), Some(2));
    predicted_control_target_authority(
        return_jump,
        Address::new(0x800c),
        return_classification,
        &[(1, Address::new(0x8008)), (2, Address::new(0x8010))],
    )
    .unwrap()
}

fn recorded_same_window_round_trip_pc(core: &RiscvCore) -> RecordedPredictedPc {
    let state = core.state.lock().expect("riscv core lock");
    recorded_predicted_pc(
        &state,
        request(3),
        Address::new(0x800c),
        recorded_same_window_round_trip_target_authority(),
    )
}

fn recorded_second_linked_coroutine_core() -> RiscvCore {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let first_coroutine = i_type(0, 1, 0x0, 5, 0x67);
    let second_coroutine = i_type(0, 5, 0x0, 1, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, first_coroutine.to_le_bytes().to_vec()),
        (3, 0x8008, second_coroutine.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(3);
    for expected_pc in [0x800c, 0x8008, 0x8010] {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(decision.pc(), Address::new(expected_pc));
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }
    core
}

fn second_linked_coroutine_target_authority() -> PredictedControlTargetAuthority {
    let call = RiscvInstruction::decode(j_type(8, 1)).unwrap();
    let first_coroutine = RiscvInstruction::decode(i_type(0, 1, 0x0, 5, 0x67)).unwrap();
    let second_coroutine = RiscvInstruction::decode(i_type(0, 5, 0x0, 1, 0x67)).unwrap();
    let mut window =
        crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(6).unwrap()],
            1,
            4,
        )
        .unwrap();
    assert_eq!(
        window.classify_sequenced_younger(call, 1).decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    assert_eq!(
        window
            .classify_sequenced_younger(first_coroutine, 2)
            .decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    let second_classification = window.classify_sequenced_younger(second_coroutine, 3);
    assert_eq!(
        second_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(second_classification.ras_push_sequence(), Some(2));
    predicted_control_target_authority(
        second_coroutine,
        Address::new(0x800c),
        second_classification,
        &[
            (1, Address::new(0x8008)),
            (2, Address::new(0x8010)),
            (3, Address::new(0x800c)),
        ],
    )
    .unwrap()
}

fn recorded_second_linked_coroutine_pc(core: &RiscvCore) -> RecordedPredictedPc {
    let state = core.state.lock().expect("riscv core lock");
    recorded_predicted_pc(
        &state,
        request(3),
        Address::new(0x800c),
        second_linked_coroutine_target_authority(),
    )
}

fn detailed_nested_control_core(split_inner: bool) -> RiscvCore {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let outer = b_type(12, 1, 2, 0x0);
    let inner = b_type(8, 3, 4, 0x0).to_le_bytes();
    let mul = r_type(0x01, 7, 8, 0x0, 9, 0x33);
    let mut fetches = vec![
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, outer.to_le_bytes().to_vec()),
    ];
    if split_inner {
        fetches.push((2, 0x8008, inner[..2].to_vec()));
        fetches.push((3, 0x800a, inner[2..].to_vec()));
        fetches.push((4, 0x800c, mul.to_le_bytes().to_vec()));
    } else {
        fetches.push((2, 0x8008, inner.to_vec()));
        fetches.push((3, 0x800c, mul.to_le_bytes().to_vec()));
    }
    let core = core_with_completed_fetches(fetches);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(2);
    core.set_o3_scalar_memory_depth(4);
    core
}

fn detailed_three_deep_control_core() -> RiscvCore {
    detailed_three_deep_control_core_with_split(false)
}

fn detailed_three_deep_split_control_core() -> RiscvCore {
    detailed_three_deep_control_core_with_split(true)
}

fn detailed_three_deep_control_core_with_split(split_third: bool) -> RiscvCore {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let outer = b_type(12, 1, 2, 0x1);
    let middle = b_type(8, 3, 4, 0x4);
    let inner = b_type(8, 6, 7, 0x7).to_le_bytes();
    let mut fetches = vec![
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, outer.to_le_bytes().to_vec()),
        (2, 0x8008, middle.to_le_bytes().to_vec()),
    ];
    if split_third {
        fetches.push((3, 0x800c, inner[..2].to_vec()));
        fetches.push((4, 0x800e, inner[2..].to_vec()));
    } else {
        fetches.push((3, 0x800c, inner.to_vec()));
    }
    let core = core_with_completed_fetches(fetches);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_branch_lookahead(3);
    core.set_o3_scalar_memory_depth(4);
    core
}

#[test]
fn detailed_scalar_window_returns_existing_branch_prediction_decision() {
    let core = detailed_control_core(1, false);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
    let speculation = decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x8004));
    assert!(!speculation.predicted_taken());
}

#[test]
fn detailed_scalar_window_direct_call_follows_target_and_pushes_ras() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
    ]);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x800c));
    let speculation = decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x8004));
    assert_eq!(speculation.target(), Some(Address::new(0x800c)));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&decision).unwrap(),
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.return_address_stack.stack_entries(),
        &[Address::new(0x8008)]
    );
    assert_eq!(state.return_address_stack.pending_operation_count(), 1);
    assert_eq!(state.return_address_stack_operations.len(), 1);
}

#[test]
fn detailed_scalar_window_forwards_call_ras_to_same_window_return() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let fallthrough = i_type(1, 0, 0x0, 7, 0x13);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
        (3, 0x8008, fallthrough.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(return_decision.pc(), Address::new(0x8008));
    let speculation = return_decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x800c));
    assert_eq!(speculation.target(), Some(Address::new(0x8008)));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );

    let state = core.state.lock().expect("riscv core lock");
    assert!(state.return_address_stack.stack_entries().is_empty());
    assert_eq!(state.return_address_stack.pending_operation_count(), 2);
    assert_eq!(state.return_address_stack_operations.len(), 2);
    assert_eq!(
        state
            .branch_speculation_summary
            .target_provider()
            .value(BranchTargetProvider::RAS),
        1
    );
}

#[test]
fn detailed_scalar_window_forwards_call_ras_to_same_window_coroutine() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let coroutine = i_type(0, 1, 0x0, 5, 0x67);
    let descendant = i_type(1, 0, 0x0, 7, 0x13);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, coroutine.to_le_bytes().to_vec()),
        (3, 0x8008, descendant.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let coroutine_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(coroutine_decision.pc(), Address::new(0x8008));
    let speculation = coroutine_decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x800c));
    assert_eq!(speculation.target(), Some(Address::new(0x8008)));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&coroutine_decision)
            .unwrap(),
    );

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        state.return_address_stack.stack_entries(),
        &[Address::new(0x8010)]
    );
    assert_eq!(state.return_address_stack.pending_operation_count(), 2);
    let coroutine_operation = &state.return_address_stack.pending_operations()[1];
    assert_eq!(
        coroutine_operation.kind(),
        crate::ReturnAddressStackOperationKind::PopThenPush
    );
    assert_eq!(
        coroutine_operation.pushed_address(),
        Some(Address::new(0x8010))
    );
}

#[test]
fn detailed_control_target_authority_rejects_non_predicted_decision() {
    let instruction = RiscvInstruction::decode(i_type(1, 0, 0x0, 7, 0x13)).unwrap();
    let mut window =
        crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(6).unwrap()],
            1,
            4,
        )
        .unwrap();
    let classification = window.classify_sequenced_younger(instruction, 1);
    assert_eq!(
        classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitContinue
    );

    assert_eq!(
        predicted_control_target_authority(
            instruction,
            Address::new(0x8004),
            classification,
            &[(1, Address::new(0x8004))],
        ),
        None
    );
}

#[test]
fn detailed_recorded_coroutine_accepts_exact_pop_then_push() {
    let core = recorded_same_window_coroutine_core();

    assert_eq!(
        recorded_same_window_coroutine_pc(&core),
        RecordedPredictedPc::Ready(Address::new(0x8008))
    );

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
}

#[test]
fn detailed_unconsumed_coroutine_round_trip_opens_exact_replacement_pop() {
    let core = unconsumed_same_window_coroutine_round_trip_core();

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8010));
    let speculation = decision.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x8008));
    assert_eq!(speculation.target(), Some(Address::new(0x8010)));
}

#[test]
fn detailed_recorded_coroutine_round_trip_accepts_exact_replacement_pop() {
    let core = recorded_same_window_coroutine_round_trip_core();

    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Ready(Address::new(0x8010))
    );
}

#[test]
fn detailed_unconsumed_coroutine_round_trip_rejects_newer_ras_operation() {
    let core = unconsumed_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_unconsumed_coroutine_round_trip_opens_linked_consumer() {
    let core = unconsumed_same_window_coroutine_round_trip_core();
    let state = core.state.lock().expect("riscv core lock");

    assert_eq!(
        detailed_o3::unconsumed_ras_required_target(
            &state,
            2,
            Address::new(0x8010),
            detailed_o3::RequiredRasConsumer::PopThenPush {
                pushed_address: Address::new(0x8014),
            },
        ),
        Some(Address::new(0x8010))
    );
}

#[test]
fn detailed_full_ras_call_producer_opens_exact_return() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let entries = state.return_address_stack.config().entries();
        for index in 0..entries {
            let operation = state
                .return_address_stack
                .push_speculative(Address::new(0x9000 + index as u64 * 4));
            state
                .return_address_stack
                .commit_operation(operation.id())
                .unwrap();
        }
        assert_eq!(state.return_address_stack.depth(), entries);
        assert_eq!(state.return_address_stack.pending_operation_count(), 0);
    }

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(return_decision.pc(), Address::new(0x8008));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );

    let call = RiscvInstruction::decode(call).unwrap();
    let return_jump = RiscvInstruction::decode(return_jump).unwrap();
    let mut window =
        crate::riscv_o3_window_policy::RiscvScalarIntegerLiveWindow::from_scalar_memory_prefix(
            [Register::new(6).unwrap()],
            1,
            4,
        )
        .unwrap();
    let call_classification = window.classify_sequenced_younger(call, 1);
    assert_eq!(
        call_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
    );
    let return_classification = window.classify_sequenced_younger(return_jump, 2);
    assert_eq!(
        return_classification.decision(),
        crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
    );
    assert_eq!(return_classification.ras_push_sequence(), Some(1));
    let target_authority = predicted_control_target_authority(
        return_jump,
        Address::new(0x8010),
        return_classification,
        &[(1, Address::new(0x8008))],
    )
    .unwrap();
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(
        recorded_predicted_pc(&state, request(2), Address::new(0x8010), target_authority),
        RecordedPredictedPc::Ready(Address::new(0x8008))
    );
}

#[test]
fn detailed_recorded_second_linked_coroutine_consumes_replacement_push() {
    let core = recorded_second_linked_coroutine_core();
    let target_authority = second_linked_coroutine_target_authority();
    assert_eq!(
        target_authority,
        PredictedControlTargetAuthority::RasRequired {
            push_sequence: 2,
            pushed_address: Address::new(0x8010),
            consumer: detailed_o3::RequiredRasConsumer::PopThenPush {
                pushed_address: Address::new(0x800c),
            },
        }
    );
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.return_address_stack.pending_operation_count(), 3);
    assert_eq!(state.return_address_stack.top(), Some(Address::new(0x800c)));
    assert_eq!(
        recorded_predicted_pc(&state, request(3), Address::new(0x800c), target_authority,),
        RecordedPredictedPc::Ready(Address::new(0x8010))
    );
}

#[test]
fn detailed_recorded_second_linked_coroutine_rejects_wrong_consumer_push() {
    let core = recorded_second_linked_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let consumer_id = *state.return_address_stack_operations.get(&3).unwrap();
        let snapshot = state.return_address_stack.snapshot();
        let pending_operations = snapshot
            .pending_operations()
            .iter()
            .map(|operation| {
                if operation.id() != consumer_id {
                    return operation.clone();
                }
                crate::ReturnAddressStackOperation::from_checkpoint_parts(
                    operation.id(),
                    operation.kind(),
                    Some(Address::new(0x9000)),
                    operation.predicted_return(),
                    operation.stack_before().to_vec(),
                    operation.stack_after().to_vec(),
                )
            })
            .collect();
        let malformed_snapshot = crate::ReturnAddressStackSnapshot::from_checkpoint_parts(
            snapshot.config().clone(),
            snapshot.stack_entries().to_vec(),
            snapshot.next_operation(),
            pending_operations,
        );
        state
            .return_address_stack
            .restore(&malformed_snapshot)
            .unwrap();
    }

    assert_eq!(
        recorded_second_linked_coroutine_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_plain_push_producer() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(3).unwrap();
        state.squash_return_address_stack_speculation(2).unwrap();
        let producer = state
            .return_address_stack
            .push_speculative(Address::new(0x8010));
        state
            .return_address_stack_operations
            .insert(2, producer.id());
        let consumer = state.return_address_stack.pop_speculative();
        state
            .return_address_stack_operations
            .insert(3, consumer.id());
    }

    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_wrong_replacement_address() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let producer_id = *state.return_address_stack_operations.get(&2).unwrap();
        let snapshot = state.return_address_stack.snapshot();
        let pending_operations = snapshot
            .pending_operations()
            .iter()
            .map(|operation| {
                if operation.id() != producer_id {
                    return operation.clone();
                }
                crate::ReturnAddressStackOperation::from_checkpoint_parts(
                    operation.id(),
                    operation.kind(),
                    Some(Address::new(0x9000)),
                    operation.predicted_return(),
                    operation.stack_before().to_vec(),
                    operation.stack_after().to_vec(),
                )
            })
            .collect();
        let malformed_snapshot = crate::ReturnAddressStackSnapshot::from_checkpoint_parts(
            snapshot.config().clone(),
            snapshot.stack_entries().to_vec(),
            snapshot.next_operation(),
            pending_operations,
        );
        state
            .return_address_stack
            .restore(&malformed_snapshot)
            .unwrap();
    }

    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_intervening_ras_operation() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(3).unwrap();
        let producer_id = *state.return_address_stack_operations.get(&2).unwrap();
        state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
        state.return_address_stack.pop_speculative();
        let consumer = state.return_address_stack.pop_speculative();
        let producer_stack_after = state
            .return_address_stack
            .pending_operations()
            .iter()
            .find(|operation| operation.id() == producer_id)
            .unwrap()
            .stack_after();
        assert_eq!(producer_stack_after, consumer.stack_before());
        assert_eq!(consumer.predicted_return(), Some(Address::new(0x8010)));
        state
            .return_address_stack_operations
            .insert(3, consumer.id());
    }

    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_stale_producer_stack() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let producer_id = *state.return_address_stack_operations.get(&2).unwrap();
        let snapshot = state.return_address_stack.snapshot();
        let pending_operations = snapshot
            .pending_operations()
            .iter()
            .map(|operation| {
                if operation.id() != producer_id {
                    return operation.clone();
                }
                crate::ReturnAddressStackOperation::from_checkpoint_parts(
                    operation.id(),
                    operation.kind(),
                    operation.pushed_address(),
                    operation.predicted_return(),
                    vec![Address::new(0x9000), Address::new(0x8008)],
                    operation.stack_after().to_vec(),
                )
            })
            .collect();
        let malformed_snapshot = crate::ReturnAddressStackSnapshot::from_checkpoint_parts(
            snapshot.config().clone(),
            snapshot.stack_entries().to_vec(),
            snapshot.next_operation(),
            pending_operations,
        );
        state
            .return_address_stack
            .restore(&malformed_snapshot)
            .unwrap();
    }

    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_round_trip_rejects_over_capacity_producer_stack() {
    let core = recorded_same_window_coroutine_round_trip_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let producer_id = *state.return_address_stack_operations.get(&2).unwrap();
        let consumer_id = *state.return_address_stack_operations.get(&3).unwrap();
        let snapshot = state.return_address_stack.snapshot();
        let entries = snapshot.config().entries();
        let mut producer_stack_before = (0..entries)
            .map(|index| Address::new(0x9000 + index as u64 * 4))
            .collect::<Vec<_>>();
        producer_stack_before.push(Address::new(0x8008));
        let mut producer_stack_after = producer_stack_before.clone();
        assert_eq!(producer_stack_after.pop(), Some(Address::new(0x8008)));
        producer_stack_after.remove(0);
        producer_stack_after.push(Address::new(0x8010));
        let mut consumer_stack_after = producer_stack_after.clone();
        assert_eq!(consumer_stack_after.pop(), Some(Address::new(0x8010)));
        let pending_operations = snapshot
            .pending_operations()
            .iter()
            .map(|operation| {
                if operation.id() == producer_id {
                    return crate::ReturnAddressStackOperation::from_checkpoint_parts(
                        operation.id(),
                        operation.kind(),
                        operation.pushed_address(),
                        operation.predicted_return(),
                        producer_stack_before.clone(),
                        producer_stack_after.clone(),
                    );
                }
                if operation.id() == consumer_id {
                    return crate::ReturnAddressStackOperation::from_checkpoint_parts(
                        operation.id(),
                        operation.kind(),
                        operation.pushed_address(),
                        operation.predicted_return(),
                        producer_stack_after.clone(),
                        consumer_stack_after.clone(),
                    );
                }
                operation.clone()
            })
            .collect();
        let malformed_snapshot = crate::ReturnAddressStackSnapshot::from_checkpoint_parts(
            snapshot.config().clone(),
            consumer_stack_after,
            snapshot.next_operation(),
            pending_operations,
        );
        state
            .return_address_stack
            .restore(&malformed_snapshot)
            .unwrap();
    }

    assert_eq!(
        recorded_same_window_round_trip_pc(&core),
        RecordedPredictedPc::Invalid
    );
}

#[test]
fn detailed_recorded_coroutine_rejects_wrong_replacement_address() {
    let core = recorded_same_window_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(2).unwrap();
        let replacement = state
            .return_address_stack
            .pop_then_push_speculative(Address::new(0x9000));
        state
            .return_address_stack_operations
            .insert(2, replacement.id());
    }

    assert_eq!(
        recorded_same_window_coroutine_pc(&core),
        RecordedPredictedPc::Invalid
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_recorded_coroutine_rejects_plain_pop_consumer() {
    let core = recorded_same_window_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(2).unwrap();
        let replacement = state.return_address_stack.pop_speculative();
        state
            .return_address_stack_operations
            .insert(2, replacement.id());
    }

    assert_eq!(
        recorded_same_window_coroutine_pc(&core),
        RecordedPredictedPc::Invalid
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_recorded_coroutine_rejects_extra_consumer_post_stack_entry() {
    let core = recorded_same_window_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let consumer_id = *state.return_address_stack_operations.get(&2).unwrap();
        let snapshot = state.return_address_stack.snapshot();
        let pending_operations = snapshot
            .pending_operations()
            .iter()
            .map(|operation| {
                if operation.id() != consumer_id {
                    return operation.clone();
                }
                crate::ReturnAddressStackOperation::from_checkpoint_parts(
                    operation.id(),
                    operation.kind(),
                    operation.pushed_address(),
                    operation.predicted_return(),
                    operation.stack_before().to_vec(),
                    vec![Address::new(0x9000), Address::new(0x8010)],
                )
            })
            .collect();
        let malformed_snapshot = crate::ReturnAddressStackSnapshot::from_checkpoint_parts(
            snapshot.config().clone(),
            snapshot.stack_entries().to_vec(),
            snapshot.next_operation(),
            pending_operations,
        );
        state
            .return_address_stack
            .restore(&malformed_snapshot)
            .unwrap();
    }

    assert_eq!(
        recorded_same_window_coroutine_pc(&core),
        RecordedPredictedPc::Invalid
    );
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_recorded_coroutine_rejects_intervening_stale_ras_operations() {
    let core = recorded_same_window_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(2).unwrap();
        state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
        state.return_address_stack.pop_speculative();
        let exact_consumer = state
            .return_address_stack
            .pop_then_push_speculative(Address::new(0x8010));
        state
            .return_address_stack_operations
            .insert(2, exact_consumer.id());
    }

    assert_eq!(
        recorded_same_window_coroutine_pc(&core),
        RecordedPredictedPc::Invalid
    );
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_recorded_coroutine_accepts_younger_ras_operation_after_exact_consumer() {
    let core = recorded_same_window_coroutine_core();
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
    }

    assert_eq!(
        recorded_same_window_coroutine_pc(&core),
        RecordedPredictedPc::Ready(Address::new(0x8008))
    );
}

#[test]
fn detailed_invalid_recorded_coroutine_does_not_retry_as_fresh_prediction() {
    let core = recorded_same_window_coroutine_core();
    core.set_branch_lookahead(3);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(2).unwrap();
        assert!(state.return_address_stack_operations.contains_key(&1));
        assert!(!state.return_address_stack_operations.contains_key(&2));
        assert_eq!(state.return_address_stack.top(), Some(Address::new(0x8008)));
    }

    assert_eq!(
        recorded_same_window_coroutine_pc(&core),
        RecordedPredictedPc::Invalid
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_same_window_return_consumes_call_ras_provenance() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
        (3, 0x8008, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(3);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let outer = state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
        state
            .return_address_stack
            .commit_operation(outer.id())
            .unwrap();
    }

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let first_return = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(first_return.pc(), Address::new(0x8008));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&first_return).unwrap(),
    );

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.return_address_stack.stack_entries(),
            &[Address::new(0x9000)]
        );
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_same_window_return_requires_latest_call_link_owner() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call_x1 = j_type(8, 1);
    let call_x5 = j_type(8, 5);
    let return_x1 = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call_x1.to_le_bytes().to_vec()),
        (2, 0x800c, call_x5.to_le_bytes().to_vec()),
        (3, 0x8014, return_x1.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(3);

    let first_call = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(first_call.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&first_call).unwrap(),
    );

    let second_call = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(second_call.pc(), Address::new(0x8014));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&second_call).unwrap(),
    );

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.return_address_stack.stack_entries(),
            &[Address::new(0x8008), Address::new(0x8010)]
        );
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_ordinary_return_consumes_pending_call_ras_owner() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call_x1 = j_type(8, 1);
    let return_x5 = i_type(0, 5, 0x0, 0, 0x67);
    let return_x1 = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call_x1.to_le_bytes().to_vec()),
        (2, 0x800c, return_x5.to_le_bytes().to_vec()),
        (3, 0x8008, return_x1.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(3);
    {
        let mut state = core.state.lock().expect("riscv core lock");
        let outer = state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
        state
            .return_address_stack
            .commit_operation(outer.id())
            .unwrap();
    }

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(call_decision.pc(), Address::new(0x800c));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );

    let middle_return = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(middle_return.pc(), Address::new(0x8008));
    let speculation = middle_return.branch_speculation().unwrap();
    assert_eq!(speculation.pc(), Address::new(0x800c));
    assert_eq!(speculation.target(), Some(Address::new(0x8008)));
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&middle_return)
            .unwrap(),
    );

    {
        let state = core.state.lock().expect("riscv core lock");
        assert_eq!(
            state.return_address_stack.stack_entries(),
            &[Address::new(0x9000)]
        );
    }
    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_same_window_return_does_not_fall_back_without_ras() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.discard_return_address_stack_speculations();
        state.hart.write(Register::new(1).unwrap(), 0x9000);
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_same_window_return_does_not_use_stale_ras_top() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_same_window_return_rejects_wrong_call_push_address() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.squash_return_address_stack_speculation(1).unwrap();
        let wrong_push = state
            .return_address_stack
            .push_speculative(Address::new(0x9000));
        state
            .return_address_stack_operations
            .insert(1, wrong_push.id());
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_recorded_same_window_return_requires_live_ras_lineage() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state.discard_return_address_stack_speculations();
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_invalid_recorded_return_does_not_retry_as_fresh_prediction() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = detailed_linked_control_core([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_branch_lookahead(3);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    let return_sequence = return_decision.branch_speculation().unwrap().sequence();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );
    {
        let mut state = core.state.lock().expect("riscv core lock");
        state
            .squash_return_address_stack_speculation(return_sequence)
            .unwrap();
        assert!(state.return_address_stack_operations.contains_key(&1));
        assert!(!state
            .return_address_stack_operations
            .contains_key(&return_sequence));
        assert_eq!(state.return_address_stack.top(), Some(Address::new(0x8008)));
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_split_control_keys_prediction_to_prefix_request() {
    let load = i_type(0, 2, 0x2, 5, 0x03);
    let branch = b_type(8, 1, 2, 0x0).to_le_bytes();
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, branch[..2].to_vec()),
        (2, 0x8006, branch[2..].to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);

    let decision = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(decision.pc(), Address::new(0x8008));
    assert_eq!(decision.branch_speculation().unwrap().sequence(), 1);
}

#[test]
fn detailed_scalar_window_follows_recorded_not_taken_path() {
    let core = detailed_control_core(1, true);
    let branch = core.next_fetch_ahead_before_retire().unwrap();
    let prepared = core.prepare_fetch_ahead_speculation(&branch).unwrap();
    core.record_prepared_fetch_ahead_speculation(prepared);

    let descendant = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(descendant.pc(), Address::new(0x800c));
    assert!(descendant.branch_speculation().is_none());
}

#[test]
fn detailed_scalar_window_follows_two_recorded_control_paths() {
    let core = detailed_nested_control_core(false);

    let outer = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(
        outer.branch_speculation().unwrap().pc(),
        Address::new(0x8004)
    );
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&outer).unwrap(),
    );

    let inner = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(
        inner.branch_speculation().unwrap().pc(),
        Address::new(0x8008)
    );
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&inner).unwrap(),
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_scalar_window_follows_three_recorded_control_paths() {
    let core = detailed_three_deep_control_core();

    for expected_pc in [0x8004, 0x8008, 0x800c] {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(
            decision.branch_speculation().unwrap().pc(),
            Address::new(expected_pc)
        );
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_three_deep_control_respects_branch_lookahead_two() {
    let core = detailed_three_deep_control_core();
    core.set_branch_lookahead(2);

    for expected_pc in [0x8004, 0x8008] {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        assert_eq!(
            decision.branch_speculation().unwrap().pc(),
            Address::new(expected_pc)
        );
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_nested_control_respects_branch_lookahead_one() {
    let core = detailed_nested_control_core(false);
    core.set_branch_lookahead(1);
    core.set_o3_scalar_memory_depth(4);

    let outer = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(
        outer.branch_speculation().unwrap().pc(),
        Address::new(0x8004)
    );
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&outer).unwrap(),
    );

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}

#[test]
fn detailed_split_inner_control_keys_prediction_to_prefix_request() {
    let core = detailed_nested_control_core(true);
    let outer = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&outer).unwrap(),
    );

    let inner = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(inner.branch_speculation().unwrap().sequence(), 2);
    assert_eq!(inner.pc(), Address::new(0x800c));
}

#[test]
fn detailed_split_third_control_keys_prediction_to_prefix_request() {
    let core = detailed_three_deep_split_control_core();
    for _ in 0..2 {
        let decision = core.next_fetch_ahead_before_retire().unwrap();
        core.record_prepared_fetch_ahead_speculation(
            core.prepare_fetch_ahead_speculation(&decision).unwrap(),
        );
    }

    let inner = core.next_fetch_ahead_before_retire().unwrap();

    assert_eq!(inner.branch_speculation().unwrap().sequence(), 3);
    assert_eq!(inner.pc(), Address::new(0x8010));
}

#[test]
fn dependent_terminal_branch_does_not_open_descendant_fetch() {
    let core = detailed_control_core(5, true);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
