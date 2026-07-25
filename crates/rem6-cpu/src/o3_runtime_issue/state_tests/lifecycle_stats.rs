use super::*;

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
