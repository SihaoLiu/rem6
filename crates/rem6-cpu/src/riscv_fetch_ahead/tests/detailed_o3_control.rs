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
