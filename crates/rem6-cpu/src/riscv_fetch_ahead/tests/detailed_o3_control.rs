use super::*;

#[path = "detailed_o3_control/linked_control.rs"]
mod linked_control;

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
    let producer_decoded = RiscvInstruction::decode_with_length(producer_raw).unwrap();
    let call_decoded = RiscvInstruction::decode_with_length(call_raw).unwrap();
    let producer = producer_decoded.instruction();
    let call = call_decoded.instruction();
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
        assert!(state.o3_runtime.bind_live_staged_issue_packet(
            Address::new(0x8004),
            producer_decoded,
            &[request(1)],
            20,
        ));
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
        assert!(state.o3_runtime.bind_live_staged_issue_packet(
            Address::new(0x8008),
            call_decoded,
            &[request(2)],
            21,
        ));
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
