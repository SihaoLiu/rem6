use super::super::*;

#[test]
fn live_issue_transaction_state_rollback_preserves_preexisting_histories() {
    let mut state = O3LiveIssueState::default();
    for sequence in 0..512 {
        assert!(state.enqueue_at(
            sequence,
            Address::new(0x8000 + sequence * 4),
            O3LiveIssueTraceClass::ScalarInteger,
            sequence,
        ));
        assert!(state.remove_exact_at(
            sequence,
            O3LiveIssueTraceAction::Retired,
            Address::new(0x8000 + sequence * 4),
            O3LiveIssueTraceClass::ScalarInteger,
            sequence,
        ));
    }
    let resident = 10_000;
    assert!(state.enqueue_at(
        resident,
        Address::new(0xa000),
        O3LiveIssueTraceClass::IntegerMulDiv,
        600,
    ));
    assert!(state.begin_service_at(600));
    state.request_service_at(700);
    state.observe_sequences(700, &[], &[resident], &[], 1);
    state.seal_current_decision();
    state.trace_records.reserve(8);
    let before = state.clone();
    let trace_records_ptr = state.trace_records.as_ptr();
    let trace_records_capacity = state.trace_records.capacity();

    let rollback = state.capture_rollback();
    assert!(state.begin_transaction());
    assert!(state.enqueue_at(
        resident + 1,
        Address::new(0xa004),
        O3LiveIssueTraceClass::ScalarInteger,
        650,
    ));
    assert!(state.remove_selected_at(
        resident,
        Address::new(0xa000),
        O3LiveIssueTraceClass::IntegerMulDiv,
        700,
        703,
        704,
    ));
    state.restore_rollback(rollback);

    assert_eq!(state, before);
    assert_eq!(state.trace_records.as_ptr(), trace_records_ptr);
    assert_eq!(state.trace_records.capacity(), trace_records_capacity);
}
