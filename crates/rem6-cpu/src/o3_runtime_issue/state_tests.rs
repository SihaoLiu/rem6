use super::*;

#[test]
fn live_issue_state_enqueues_supported_bound_rows_once_and_orders_by_sequence() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::CrossResource);
    fixture.bind_row_at(2, 31);
    fixture.bind_row_at(0, 31);
    fixture.bind_row_at(1, 31);
    let expected = [BRANCH_PC, SECOND_PC, THIRD_PC].map(|pc| fixture.sequence(pc));
    assert_eq!(fixture.runtime.live_issue.resident_sequences(), expected);

    fixture.bind_row_at(1, 31);
    assert_eq!(fixture.runtime.live_issue.resident_sequences(), expected);
}

#[test]
fn live_issue_state_skips_bound_fp_vector_and_system_rows() {
    let mut runtime = O3RuntimeState::default();
    for (pc, raw, request_sequence) in [
        (BRANCH_PC, 0x0020_81d3, 11),
        (SECOND_PC, 0x0220_81d7, 12),
        (THIRD_PC, 0x0000_0073, 13),
    ] {
        let decoded = RiscvInstruction::decode_with_length(raw).unwrap();
        runtime
            .stage_live_instruction(Address::new(pc), decoded.instruction(), 0)
            .unwrap();
        assert!(runtime.bind_live_staged_issue_packet(
            Address::new(pc),
            decoded,
            &[request(request_sequence)],
            31,
        ));
    }
    assert!(runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn live_issue_state_requests_current_tick_on_admission() {
    let mut fixture = ScalarIssueFixture::new_unbound(1, ScalarIssueCase::CrossResource);
    fixture.bind_row_at(0, 31);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(31));
    assert_eq!(fixture.runtime.live_issue_telemetry().wake_requests(), 1);
    assert!(!fixture.runtime.live_issue_is_quiescent());
    let queued = fixture.runtime.live_issue_trace_records()[0];
    assert_eq!(queued.sequence(), fixture.sequence(BRANCH_PC));
    assert_eq!(queued.pc(), Address::new(BRANCH_PC));
    assert_eq!(queued.action(), O3LiveIssueTraceAction::Queued);
    assert_eq!(queued.action().name(), "queued");
    assert_eq!(queued.issue_class(), O3LiveIssueTraceClass::Control);
    assert_eq!(queued.issue_class().name(), "control");
    assert_eq!(queued.service_tick(), 31);
    assert_eq!(queued.next_wake_tick(), Some(31));
    assert_eq!(queued.raw_writeback_tick(), None);
    assert_eq!(queued.admitted_writeback_tick(), None);
    assert_eq!(queued.cleanup_boundary(), None);
}

#[test]
fn live_issue_full_discard_clears_persistent_state_without_trace() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(instruction),
        &[request(11)],
        31,
    ));
    assert_eq!(runtime.live_issue.resident_sequences().len(), 1);
    assert_eq!(runtime.live_issue_service_tick(), Some(31));
    let trace_records = runtime.live_issue_trace_records().to_vec();

    runtime.discard_live_staged_instructions();

    assert!(runtime.live_issue.resident_sequences().is_empty());
    assert_eq!(runtime.live_issue_telemetry().current_occupancy(), 0);
    assert_eq!(runtime.live_issue_service_tick(), None);
    assert!(runtime.live_issue_is_quiescent());
    assert_eq!(runtime.live_issue_trace_records(), trace_records);
}

#[test]
fn live_issue_timed_full_discard_clears_persistent_state_without_trace() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded(instruction),
        &[request(11)],
        31,
    ));
    assert_eq!(runtime.live_issue.resident_sequences().len(), 1);
    assert_eq!(runtime.live_issue_service_tick(), Some(31));
    let trace_records = runtime.live_issue_trace_records().to_vec();

    runtime.discard_live_staged_instructions_at(32);

    assert!(runtime.live_issue.resident_sequences().is_empty());
    assert_eq!(runtime.live_issue_telemetry().current_occupancy(), 0);
    assert_eq!(runtime.live_issue_service_tick(), None);
    assert!(runtime.live_issue_is_quiescent());
    assert_eq!(runtime.live_issue_trace_records(), trace_records);
}

#[test]
fn live_issue_state_removes_exact_and_suffix_rows_atomically() {
    let mut state = O3LiveIssueState::default();
    for sequence in 1..=4 {
        assert!(state.enqueue_at(
            sequence,
            Address::new(0x8000 + sequence * 4),
            O3LiveIssueTraceClass::ScalarInteger,
            31,
        ));
    }
    assert!(state.remove_exact_at(
        2,
        O3LiveIssueTraceAction::Retired,
        Address::new(0x8008),
        O3LiveIssueTraceClass::ScalarInteger,
        32,
    ));
    let suffix = [
        (
            3,
            Address::new(0x800c),
            O3LiveIssueTraceClass::ScalarInteger,
        ),
        (4, Address::new(0x8010), O3LiveIssueTraceClass::Control),
    ];
    assert_eq!(
        state.remove_suffix_at(3, O3LiveIssueTraceAction::Squashed, &suffix, 33,),
        2,
    );
    assert_eq!(state.resident_sequences(), [1]);
    assert_eq!(state.telemetry().current_occupancy(), 1);
}

#[test]
fn live_issue_state_stats_reset_preserves_membership_and_requested_wake() {
    let mut state = O3LiveIssueState::default();
    assert!(state.enqueue_at(
        7,
        Address::new(0x8020),
        O3LiveIssueTraceClass::IntegerMulDiv,
        41,
    ));
    state.reset_stats_baseline();
    assert_eq!(state.resident_sequences(), [7]);
    assert_eq!(state.requested_service_tick(), Some(41));
    assert_eq!(state.telemetry().enqueued_rows(), 0);
    assert_eq!(state.telemetry().wake_requests(), 0);
    assert_eq!(state.telemetry().current_occupancy(), 1);
    assert_eq!(state.telemetry().peak_occupancy(), 1);
    assert_eq!(state.telemetry().service_turns(), 0);
    assert_eq!(state.telemetry().scalar_integer_issued_rows(), 0);
    assert_eq!(state.telemetry().integer_mul_div_issued_rows(), 0);
    assert_eq!(state.telemetry().memory_agu_issued_rows(), 0);
    assert_eq!(state.telemetry().control_issued_rows(), 0);
}

#[test]
fn live_issue_state_service_accepts_due_generation_once() {
    let mut state = O3LiveIssueState::default();
    state.request_service_at(31);
    assert!(!state.begin_service_at(30));
    assert!(state.begin_service_at(31));
    assert_eq!(state.telemetry().service_turns(), 1);
    state.request_service_at(31);
    assert!(!state.begin_service_at(31));
    state.mark_mutated();
    state.request_service_at(31);
    assert!(state.begin_service_at(31));
    assert_eq!(state.telemetry().service_turns(), 2);
}

#[test]
fn live_issue_state_compatibility_cycles_remember_noncontiguous_ticks() {
    let mut state = O3LiveIssueState::default();
    assert!(state.begin_compatibility_cycle_at(20));
    assert!(state.begin_compatibility_cycle_at(22));
    assert!(!state.begin_compatibility_cycle_at(20));
    assert!(state.begin_compatibility_cycle_at(21));

    state.reset_stats_baseline();
    assert!(state.begin_compatibility_cycle_at(20));
}

#[test]
fn live_issue_head_binding_enqueues_then_durable_record_removes_exact_row() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let decoded = decoded(instruction);
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &[request(11)],
        20,
    ));
    assert_eq!(runtime.live_issue.resident_sequences(), [sequence]);

    let mut hart = RiscvHartState::new(BRANCH_PC);
    let execution = hart.execute_decoded(decoded).unwrap();
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);
    assert!(runtime
        .record_live_issue_head_execution(head, &[request(11)], execution)
        .unwrap());
    assert!(runtime.live_issue.resident_sequences().is_empty());
    assert_eq!(
        runtime.live_issue_telemetry().scalar_integer_issued_rows(),
        1,
    );

    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &[request(11)],
        20,
    ));
    assert!(runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn live_issue_head_selected_trace_records_writeback_ticks() {
    let mut runtime = O3RuntimeState::default();
    let instruction = addi(3, 0, 1);
    let sequence = runtime
        .stage_live_instruction(Address::new(BRANCH_PC), instruction, 0)
        .unwrap();
    let decoded = decoded(instruction);
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(BRANCH_PC),
        decoded,
        &[request(11)],
        20,
    ));
    let execution = RiscvHartState::new(BRANCH_PC)
        .execute_decoded(decoded)
        .unwrap();
    let head = O3LiveIssueHeadReservation::for_instruction(sequence, 20, instruction);
    assert!(runtime
        .record_live_issue_head_execution(head, &[request(11)], execution)
        .unwrap());

    let issued = runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == sequence)
        .unwrap();
    let selected = runtime
        .live_issue_trace_records()
        .iter()
        .find(|record| {
            record.sequence() == sequence && record.action() == O3LiveIssueTraceAction::Selected
        })
        .unwrap();
    assert_eq!(selected.raw_writeback_tick(), Some(issued.raw_ready_tick));
    assert_eq!(
        selected.admitted_writeback_tick(),
        Some(issued.admitted_writeback_tick),
    );
}

#[test]
fn live_issue_fixed_fu_selected_trace_records_writeback_ticks() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    fixture.schedule(20);
    let sequence = fixture.sequence(SECOND_PC);
    let issued = fixture
        .runtime
        .live_speculative_executions
        .iter()
        .find(|issued| issued.sequence == sequence)
        .unwrap();
    let selected = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .find(|record| {
            record.sequence() == sequence && record.action() == O3LiveIssueTraceAction::Selected
        })
        .unwrap();
    assert_eq!(selected.raw_writeback_tick(), Some(issued.raw_ready_tick));
    assert_eq!(
        selected.admitted_writeback_tick(),
        Some(issued.admitted_writeback_tick),
    );
}

#[test]
fn live_issue_active_transaction_is_nonquiescent() {
    let mut state = O3LiveIssueState::default();
    assert!(state.is_quiescent());
    assert!(state.begin_transaction());
    assert!(state.transaction_active());
    assert!(!state.is_quiescent());
    state.end_transaction();
    assert!(!state.transaction_active());
    assert!(state.is_quiescent());
}
