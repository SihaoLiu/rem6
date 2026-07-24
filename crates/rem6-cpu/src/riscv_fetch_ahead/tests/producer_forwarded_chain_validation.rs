use super::*;

fn assert_prepared_return_was_rejected(core: &RiscvCore) {
    let state = core.state.lock().expect("riscv core lock");
    assert_eq!(state.branch_speculations.len(), 1);
    assert_eq!(state.branch_speculation_kinds.len(), 1);
    assert_eq!(state.return_address_stack.pending_operation_count(), 1);
    assert_eq!(state.return_address_stack_operations.len(), 1);
}

#[test]
fn direct_return_apply_fails_closed_after_fetch_identity_changes() {
    let core = super::producer_forwarded_return::live_return_core(2, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    let call_prepared = core
        .prepare_fetch_ahead_speculation(&call_decision)
        .unwrap()
        .expect("same-link call preparation");
    record_prepared_fetch_ahead_speculation_and_fire_o3_wakes(&core, Some(call_prepared));
    let return_decision = next_pending_data_fetch_ahead_after_o3_wake(&core, true).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap()
        .expect("same-link direct return preparation");

    {
        let mut state = core.state.lock().expect("riscv core lock");
        let sequence = state
            .o3_runtime
            .producer_forwarded_return_descendant()
            .unwrap()
            .sequence();
        assert!(state
            .o3_runtime
            .replace_producer_forwarded_chain_fetch_identity_for_test(sequence, &[request(99)]));
        assert_eq!(
            state.o3_runtime.producer_forwarded_return_descendant(),
            None
        );
    }

    core.record_prepared_fetch_ahead_speculation(Some(prepared));
    assert_prepared_return_was_rejected(&core);
}

#[test]
fn scalar_return_apply_fails_closed_after_fetch_identity_changes() {
    let core = super::producer_forwarded_scalar_return::scalar_return_core(2, true, 1, 1);
    super::producer_forwarded_scalar_return::record_call_and_scalar(&core);
    super::producer_forwarded_scalar_return::retire_data_head(&core, 30);
    let return_decision = next_pending_data_fetch_ahead_after_o3_wake(&core, false).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap()
        .expect("linked-call scalar return preparation");

    {
        let mut state = core.state.lock().expect("riscv core lock");
        let sequence = state
            .o3_runtime
            .producer_forwarded_return_descendant()
            .unwrap()
            .sequence();
        assert!(state
            .o3_runtime
            .replace_producer_forwarded_chain_fetch_identity_for_test(sequence, &[request(99)]));
        assert_eq!(
            state.o3_runtime.producer_forwarded_return_descendant(),
            None
        );
    }

    core.record_prepared_fetch_ahead_speculation(Some(prepared));
    assert_prepared_return_was_rejected(&core);
}
