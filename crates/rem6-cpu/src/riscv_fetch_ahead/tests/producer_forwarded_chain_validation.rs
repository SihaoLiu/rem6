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
    let core = super::producer_forwarded_return::live_same_link_return_core(2);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    let call_prepared = core
        .prepare_fetch_ahead_speculation(&call_decision)
        .unwrap()
        .expect("same-link call preparation");
    core.record_prepared_fetch_ahead_speculation(Some(call_prepared));
    let return_decision = core.next_pending_data_fetch_ahead(true).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap()
        .expect("same-link direct return preparation");

    {
        let mut state = core.state.lock().expect("riscv core lock");
        let sequence = state
            .o3_runtime
            .producer_forwarded_same_link_return_descendant()
            .unwrap()
            .sequence();
        assert!(state
            .o3_runtime
            .replace_same_link_chain_fetch_identity_for_test(sequence, &[request(99)]));
        assert_eq!(
            state
                .o3_runtime
                .producer_forwarded_same_link_return_descendant(),
            None
        );
    }

    core.record_prepared_fetch_ahead_speculation(Some(prepared));
    assert_prepared_return_was_rejected(&core);
}

#[test]
fn scalar_return_apply_fails_closed_after_fetch_identity_changes() {
    let core = super::producer_forwarded_scalar_return::scalar_return_core(2, true);
    super::producer_forwarded_scalar_return::record_call_and_scalar(&core);
    super::producer_forwarded_scalar_return::retire_data_head(&core, 30);
    let return_decision = core.next_pending_data_fetch_ahead(false).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap()
        .expect("same-link scalar return preparation");

    {
        let mut state = core.state.lock().expect("riscv core lock");
        let sequence = state
            .o3_runtime
            .producer_forwarded_same_link_return_descendant()
            .unwrap()
            .sequence();
        assert!(state
            .o3_runtime
            .replace_same_link_chain_fetch_identity_for_test(sequence, &[request(99)]));
        assert_eq!(
            state
                .o3_runtime
                .producer_forwarded_same_link_return_descendant(),
            None
        );
    }

    core.record_prepared_fetch_ahead_speculation(Some(prepared));
    assert_prepared_return_was_rejected(&core);
}
