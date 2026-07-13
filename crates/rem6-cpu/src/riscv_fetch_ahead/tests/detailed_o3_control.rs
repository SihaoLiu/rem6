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
fn dependent_terminal_branch_does_not_open_descendant_fetch() {
    let core = detailed_control_core(5, true);

    assert_eq!(core.next_fetch_ahead_before_retire(), None);
}
