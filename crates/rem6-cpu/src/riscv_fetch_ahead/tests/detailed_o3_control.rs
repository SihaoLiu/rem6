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
fn dependent_terminal_branch_does_not_open_descendant_fetch() {
    let core = detailed_control_core(5, true);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
