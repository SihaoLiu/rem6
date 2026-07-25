use super::*;

#[test]
fn producer_forwarded_return_apply_records_retirement_trace_authority() {
    let core = live_return_core(2, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    record_prepared_fetch_ahead_speculation_and_fire_o3_wakes(
        &core,
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = next_pending_data_fetch_ahead_after_o3_wake(&core, true).unwrap();
    let prepared = core
        .prepare_fetch_ahead_speculation(&return_decision)
        .unwrap();
    let sequence = {
        let mut state = core.state.lock().expect("riscv core lock");
        let sequence = state
            .o3_runtime
            .producer_forwarded_return_descendant()
            .unwrap()
            .sequence();
        assert!(state
            .o3_runtime
            .clear_recorded_producer_forwarded_return_descendant_for_test(sequence));
        sequence
    };

    core.record_prepared_fetch_ahead_speculation(prepared);

    let state = core.state.lock().expect("riscv core lock");
    assert!(state
        .o3_runtime
        .has_recorded_producer_forwarded_return_descendant(sequence));
}

#[test]
fn committed_producer_forwarded_call_retains_direct_return_ras_authority() {
    let core = live_return_core(2, 1, 1);
    let call_decision = core.next_fetch_ahead_before_retire().unwrap();
    record_prepared_fetch_ahead_speculation_and_fire_o3_wakes(
        &core,
        core.prepare_fetch_ahead_speculation(&call_decision)
            .unwrap(),
    );
    let return_decision = next_pending_data_fetch_ahead_after_o3_wake(&core, true).unwrap();
    core.record_prepared_fetch_ahead_speculation(
        core.prepare_fetch_ahead_speculation(&return_decision)
            .unwrap(),
    );

    let return_instruction = RiscvInstruction::decode(i_type(0, 1, 0x0, 0, 0x67)).unwrap();
    let mut state = core.state.lock().expect("riscv core lock");
    state
        .commit_return_address_stack_speculation(2, true)
        .unwrap();
    let continuation = state
        .producer_forwarded_scalar_continuation
        .as_ref()
        .expect("producer-forwarded call RAS checkpoint");
    assert!(continuation.retains_return_fetch(
        &state,
        Address::new(0x9000),
        return_instruction,
        4,
        &[request(3)],
    ));
}
