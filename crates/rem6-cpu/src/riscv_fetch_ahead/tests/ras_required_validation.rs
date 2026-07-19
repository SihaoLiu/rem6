use super::*;

#[test]
fn ras_required_apply_fails_closed_after_lineage_changes() {
    let load = i_type(0, 2, 0x2, 6, 0x03);
    let call = j_type(8, 1);
    let return_jump = i_type(0, 1, 0x0, 0, 0x67);
    let core = core_with_completed_fetches([
        (0, 0x8000, load.to_le_bytes().to_vec()),
        (1, 0x8004, call.to_le_bytes().to_vec()),
        (2, 0x800c, return_jump.to_le_bytes().to_vec()),
    ]);
    core.set_detailed_live_retire_gate_enabled(true);
    core.set_o3_scalar_memory_depth(4);
    core.set_branch_lookahead(2);

    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = core.next_fetch_ahead_before_retire().unwrap();
    assert_eq!(return_decision.pc(), Address::new(0x8008));
    assert!(matches!(
        return_decision
            .branch_speculation()
            .unwrap()
            .target_authority(),
        PredictedControlTargetAuthority::RasRequired { .. }
    ));
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap();

    core.state
        .lock()
        .expect("riscv core lock")
        .return_address_stack
        .push_speculative(Address::new(0x9000));
    core.record_prepared_fetch_ahead_speculation(prepared);

    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 1);
    assert!(!state.branch_speculations.contains_key(&2));
    assert!(!state.return_address_stack_operations.contains_key(&2));
    assert_eq!(state.return_address_stack.pending_operation_count(), 2);
}
