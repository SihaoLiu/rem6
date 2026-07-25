use super::*;

#[test]
fn live_issue_cleanup_retirement_removes_exact_row_before_metadata_finalization() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::CrossResource);
    fixture.bind_row_at(0, 20);
    fixture.bind_row_at(1, 21);
    let sequence = fixture.sequence(BRANCH_PC);
    let survivor = fixture.sequence(SECOND_PC);
    let instruction = fixture.rows[0].1;
    let mut hart = fixture.hart.clone();
    hart.set_pc(BRANCH_PC);
    let execution = hart.execute_decoded(decoded(instruction)).unwrap();

    fixture.runtime.retire_live_staged_instruction(
        &RiscvCpuExecutionEvent::new(fetch_event(BRANCH_PC, 11), instruction, execution),
        &[request(11)],
        30,
    );

    assert_eq!(fixture.runtime.live_issue.resident_sequences(), [survivor]);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(30));
    let retired = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .find(|record| {
            record.sequence() == sequence && record.action() == O3LiveIssueTraceAction::Retired
        })
        .expect("retired cleanup trace");
    assert_eq!(retired.pc(), Address::new(BRANCH_PC));
    assert_eq!(retired.issue_class(), O3LiveIssueTraceClass::Control);
    assert_eq!(retired.next_wake_tick(), Some(30));
    assert_eq!(retired.cleanup_boundary(), Some(sequence));
}

#[test]
fn live_issue_cleanup_deferred_retirement_squashes_younger_rows_before_metadata_finalization() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let load = scalar_load_event();
    let consumed_requests = [load.fetch().request_id()];
    let decoded_load =
        RiscvInstruction::decode_with_length(i_type(0, 10, 0b110, 12, 0x03)).unwrap();
    assert_eq!(decoded_load.instruction(), load.instruction());
    assert!(fixture.runtime.bind_live_staged_issue_packet(
        Address::new(LOAD_PC),
        decoded_load,
        &consumed_requests,
        20,
    ));
    let boundary = fixture.head.sequence();
    assert!(fixture.runtime.live_issue.enqueue_at(
        boundary,
        Address::new(LOAD_PC),
        O3LiveIssueTraceClass::MemoryAgu,
        20,
    ));
    let younger = [
        fixture.sequence(BRANCH_PC),
        fixture.sequence(SECOND_PC),
        fixture.sequence(THIRD_PC),
    ];
    assert_eq!(
        fixture.runtime.live_issue.resident_sequences(),
        [boundary, younger[0], younger[1], younger[2]]
    );

    fixture
        .runtime
        .retire_live_staged_instruction(&load, &consumed_requests, 30);

    assert_eq!(fixture.runtime.live_issue.resident_sequences(), []);
    assert_eq!(fixture.runtime.live_issue_service_tick(), None);
    let retired = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .find(|record| {
            record.sequence() == boundary && record.action() == O3LiveIssueTraceAction::Retired
        })
        .expect("deferred boundary retirement trace");
    assert_eq!(retired.pc(), Address::new(LOAD_PC));
    let squashed = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .filter(|record| record.action() == O3LiveIssueTraceAction::Squashed)
        .collect::<Vec<_>>();
    assert_eq!(
        squashed
            .iter()
            .map(|record| record.sequence())
            .collect::<Vec<_>>(),
        younger
    );
    assert!(squashed
        .iter()
        .all(|record| record.cleanup_boundary() == boundary.checked_add(1)));
}

#[test]
fn live_issue_cleanup_squash_removes_boundary_and_younger_with_rearmed_wake() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let first = fixture.sequence(BRANCH_PC);
    let boundary = fixture.sequence(SECOND_PC);
    let youngest = fixture.sequence(THIRD_PC);

    fixture
        .runtime
        .discard_live_staged_window_from_at(boundary, 30);

    assert_eq!(fixture.runtime.live_issue.resident_sequences(), [first]);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(30));
    let squashed = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .filter(|event| event.action() == O3LiveIssueTraceAction::Squashed)
        .collect::<Vec<_>>();
    assert_eq!(
        squashed
            .iter()
            .map(|event| event.sequence())
            .collect::<Vec<_>>(),
        vec![boundary, youngest]
    );
    assert!(squashed
        .iter()
        .all(|event| event.cleanup_boundary() == Some(boundary)));
    assert!(squashed
        .iter()
        .all(|event| event.next_wake_tick() == Some(30)));
}

#[test]
fn live_issue_branch_cleanup_records_branch_sequence_as_boundary() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let branch = fixture.sequence(BRANCH_PC);
    let descendants = [fixture.sequence(SECOND_PC), fixture.sequence(THIRD_PC)];

    fixture
        .runtime
        .discard_live_control_descendants_from_at(branch, 30);

    assert_eq!(fixture.runtime.live_issue.resident_sequences(), [branch]);
    let squashed = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .filter(|event| event.action() == O3LiveIssueTraceAction::Squashed)
        .collect::<Vec<_>>();
    assert_eq!(
        squashed
            .iter()
            .map(|event| event.sequence())
            .collect::<Vec<_>>(),
        descendants,
    );
    assert!(squashed
        .iter()
        .all(|event| event.cleanup_boundary() == Some(branch)));
}

#[test]
fn live_issue_pending_cleanup_does_not_replay_an_already_squashed_row() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let branch = fixture.sequence(BRANCH_PC);
    let first_descendant = fixture.sequence(SECOND_PC);

    fixture
        .runtime
        .discard_live_control_descendants_from_at(branch, 30);
    fixture
        .runtime
        .discard_pending_live_issue_suffix_at(first_descendant, Some(30));

    let terminal = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .filter(|event| event.sequence() == first_descendant)
        .filter(|event| {
            matches!(
                event.action(),
                O3LiveIssueTraceAction::Squashed | O3LiveIssueTraceAction::Replayed
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal.len(), 1);
    assert_eq!(terminal[0].action(), O3LiveIssueTraceAction::Squashed);
    assert_eq!(terminal[0].cleanup_boundary(), Some(branch));
}

#[test]
fn live_issue_cleanup_pending_replay_removes_exact_suffix_with_replayed_trace() {
    let mut runtime = O3RuntimeState::default();
    let sequence = super::super::queue::stage_queue_pending_row(&mut runtime);
    let older = sequence - 1;
    assert!(runtime.live_issue.enqueue_at(
        older,
        Address::new(BRANCH_PC - 4),
        O3LiveIssueTraceClass::ScalarInteger,
        20,
    ));

    runtime.discard_pending_data_address_from(sequence);

    assert_eq!(runtime.live_issue.resident_sequences(), [older]);
    assert!(runtime.pending_data_addresses.is_empty());
    let replayed = runtime
        .live_issue_trace_records()
        .iter()
        .find(|event| {
            event.sequence() == sequence && event.action() == O3LiveIssueTraceAction::Replayed
        })
        .expect("pending replay cleanup trace");
    assert_eq!(replayed.cleanup_boundary(), Some(sequence));
}

#[test]
fn live_issue_cleanup_records_selected_pending_replay_boundary() {
    let mut runtime = O3RuntimeState::default();
    let sequence = super::super::queue::stage_queue_pending_row(&mut runtime);
    let pc = runtime
        .pending_data_addresses
        .find_sequence(sequence)
        .expect("pending replay boundary")
        .fetch
        .pc();
    runtime
        .remove_durable_live_issue_at(
            sequence,
            pc,
            O3LiveIssueTraceClass::MemoryAgu,
            20,
            Some((20, 20)),
        )
        .unwrap();

    runtime.discard_pending_data_address_at_internal(sequence, Some(21));

    let replayed = runtime
        .live_issue_trace_records()
        .iter()
        .find(|event| {
            event.sequence() == sequence && event.action() == O3LiveIssueTraceAction::Replayed
        })
        .expect("selected pending replay boundary trace");
    assert_eq!(replayed.cleanup_boundary(), Some(sequence));
    assert!(!runtime.live_issue_trace_records().iter().any(|event| {
        event.sequence() == sequence && event.action() == O3LiveIssueTraceAction::Squashed
    }));
}

#[test]
fn live_issue_nonresident_cleanup_survives_stats_reset() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let branch = fixture.sequence(BRANCH_PC);
    let selected = fixture.sequence(SECOND_PC);
    fixture
        .runtime
        .remove_durable_live_issue_at(
            selected,
            Address::new(SECOND_PC),
            O3LiveIssueTraceClass::IntegerMulDiv,
            20,
            Some((20, 20)),
        )
        .unwrap();

    fixture.runtime.reset_stats();
    fixture
        .runtime
        .discard_live_control_descendants_from_at(branch, 30);

    let squashed = fixture
        .runtime
        .live_issue_trace_records()
        .iter()
        .find(|event| {
            event.sequence() == selected && event.action() == O3LiveIssueTraceAction::Squashed
        })
        .expect("post-reset nonresident cleanup trace");
    assert_eq!(squashed.cleanup_boundary(), Some(branch));
}

#[test]
fn live_issue_cleanup_is_idempotent_without_duplicate_trace() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let first = fixture.sequence(BRANCH_PC);
    let boundary = fixture.sequence(SECOND_PC);

    fixture
        .runtime
        .discard_live_staged_window_from_at(boundary, 30);
    let squashed_after_first = squash_trace_count(&fixture.runtime);
    fixture
        .runtime
        .discard_live_staged_window_from_at(boundary, 31);

    assert_eq!(fixture.runtime.live_issue.resident_sequences(), [first]);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(30));
    assert_eq!(squashed_after_first, 2);
    assert_eq!(squash_trace_count(&fixture.runtime), squashed_after_first);
}

#[test]
fn live_issue_full_discard_clears_transient_state_and_preserves_projected_stats() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let issued = fixture.sequence(BRANCH_PC);
    let resource_blocked = fixture.sequence(SECOND_PC);
    let dependency_blocked = fixture.sequence(THIRD_PC);
    let runtime = &mut fixture.runtime;
    runtime.live_issue.observe_sequences(
        31,
        &[issued],
        &[resource_blocked],
        &[dependency_blocked],
        3,
    );
    assert!(!runtime.live_issue_trace_records().is_empty());
    assert_eq!(runtime.stats().issue_cycles(), 1);
    assert_eq!(runtime.stats().issued_rows(), 1);
    assert_eq!(runtime.stats().resource_blocked_row_cycles(), 1);
    assert_eq!(runtime.stats().dependency_blocked_row_cycles(), 1);
    let projected = runtime.stats();

    runtime.discard_live_staged_instructions();

    assert_eq!(runtime.stats().issue_cycles(), projected.issue_cycles());
    assert_eq!(runtime.stats().issued_rows(), projected.issued_rows());
    assert_eq!(
        runtime.stats().resource_blocked_row_cycles(),
        projected.resource_blocked_row_cycles()
    );
    assert_eq!(
        runtime.stats().dependency_blocked_row_cycles(),
        projected.dependency_blocked_row_cycles()
    );
    assert_eq!(
        runtime.stats().max_rows_per_cycle(),
        projected.max_rows_per_cycle()
    );
    assert!(runtime.live_issue.resident_sequences().is_empty());
    assert_eq!(runtime.live_issue_service_tick(), None);
    assert_eq!(
        runtime.live_issue_telemetry(),
        O3LiveIssueTelemetry::default()
    );
    assert!(runtime.live_issue_trace_records().is_empty());
    assert!(runtime.live_issue.projected_decision().is_none());
}

#[test]
fn o3_runtime_restore_clears_live_issue_membership_telemetry_and_wake_from_drained_checkpoint() {
    let mut runtime = O3RuntimeState::default();
    runtime.live_issue.observe_sequences(20, &[1], &[], &[], 1);
    runtime.seal_live_issue_decision();
    let projected = runtime.stats();
    let encoded = runtime.checkpoint_payload().encode();
    let payload = O3RuntimeCheckpointPayload::decode(&encoded).unwrap();

    assert!(runtime.live_issue.enqueue_at(
        1,
        Address::new(0x8000),
        O3LiveIssueTraceClass::IntegerMulDiv,
        21,
    ));

    runtime.restore_checkpoint_payload(payload).unwrap();

    assert_eq!(runtime.stats(), projected);
    assert_eq!(
        runtime.live_issue_telemetry(),
        O3LiveIssueTelemetry::default()
    );
    assert!(runtime.live_issue_trace_records().is_empty());
    assert!(runtime.live_issue_is_quiescent());
}

fn squash_trace_count(runtime: &O3RuntimeState) -> usize {
    runtime
        .live_issue_trace_records()
        .iter()
        .filter(|event| event.action() == O3LiveIssueTraceAction::Squashed)
        .count()
}
