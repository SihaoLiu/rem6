use super::*;

fn dependency_lookahead_fixture() -> ScalarIssueFixture {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::SameTickAluDependency);
    let producer_sequence = fixture.sequence(BRANCH_PC);
    let producer_instruction = fixture.rows[0].1;
    let mut producer_hart = fixture.hart.clone();
    producer_hart.set_pc(BRANCH_PC);
    let execution = producer_hart
        .execute_decoded(decoded(producer_instruction))
        .unwrap();
    fixture
        .runtime
        .live_speculative_executions
        .push(O3LiveSpeculativeExecution {
            consumed_requests: Vec::new(),
            sequence: producer_sequence,
            producer_sequences: Vec::new(),
            issue_tick: 28,
            raw_ready_tick: 30,
            admitted_writeback_tick: 30,
            writeback_slot: None,
            execution,
        });
    fixture.bind_row(1);
    fixture
}

#[test]
fn scheduler_facing_same_tick_wake_preserves_future_request_and_observations() {
    let mut fixture = dependency_lookahead_fixture();
    let first = fixture
        .runtime
        .service_live_issue_scheduler_at(&fixture.hart, 20)
        .unwrap();
    assert_eq!(first.next_service_tick(), Some(30));
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(30));
    let stats = fixture.runtime.stats();
    let telemetry = fixture.runtime.live_issue_telemetry();

    let second = fixture
        .runtime
        .service_live_issue_scheduler_at(&fixture.hart, 20)
        .unwrap();

    assert_eq!(second.issued_rows(), 0);
    assert_eq!(second.next_service_tick(), None);
    assert_eq!(second.replay_boundary(), None);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(30));
    assert_eq!(fixture.runtime.stats(), stats);
    assert_eq!(fixture.runtime.live_issue_telemetry(), telemetry);
}

#[test]
fn scheduler_facing_early_wake_advances_frontier_without_servicing_future_request() {
    let mut fixture = dependency_lookahead_fixture();
    fixture
        .runtime
        .service_live_issue_scheduler_at(&fixture.hart, 20)
        .unwrap();
    let stats = fixture.runtime.stats();
    let telemetry = fixture.runtime.live_issue_telemetry();

    let unrelated = fixture
        .runtime
        .service_live_issue_scheduler_at(&fixture.hart, 25)
        .unwrap();

    assert_eq!(unrelated.issued_rows(), 0);
    assert_eq!(unrelated.next_service_tick(), None);
    assert_eq!(unrelated.replay_boundary(), None);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(30));
    assert_eq!(fixture.runtime.stats(), stats);
    assert_eq!(fixture.runtime.live_issue_telemetry(), telemetry);
    assert_eq!(
        fixture.runtime.live_issue.scheduler_entry_tick_for_test(),
        Some(25)
    );
}

#[test]
fn compatibility_seeds_and_services_earliest_tick() {
    let mut fixture = dependency_lookahead_fixture();
    fixture.runtime.live_issue.clear_requested_service_tick();
    let telemetry = fixture.runtime.live_issue_telemetry();

    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20)
        .unwrap();

    assert_eq!(fixture.issue_tick(SECOND_PC), 30);
    assert_eq!(fixture.runtime.live_issue_service_tick(), None);
    assert_eq!(fixture.runtime.stats().issue_cycles(), 2);
    assert_eq!(fixture.runtime.stats().issued_rows(), 1);
    assert_eq!(fixture.runtime.stats().dependency_blocked_row_cycles(), 1);
    assert_eq!(
        fixture.runtime.live_issue_telemetry().service_turns(),
        telemetry.service_turns() + 2
    );
    assert_eq!(
        fixture.runtime.live_issue_telemetry().wake_requests(),
        telemetry.wake_requests() + 2
    );
}

#[test]
fn compatibility_scheduler_entry_issues_independent_work_before_retained_lookahead() {
    let mut fixture = dependency_lookahead_fixture();
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20)
        .unwrap();
    assert_eq!(fixture.issue_tick(SECOND_PC), 30);
    let before = fixture.runtime.stats();
    assert_eq!(before.issue_cycles(), 2);
    assert_eq!(before.issued_rows(), 1);
    assert_eq!(
        fixture.runtime.live_issue.counted_cycle_ticks_for_test(),
        [20, 30],
    );

    fixture.bind_row_at(2, 21);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(21));
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 21)
        .unwrap();
    assert_eq!(fixture.issue_tick(THIRD_PC), 21);
    let after = fixture.runtime.stats();
    assert_eq!(after.issue_cycles(), before.issue_cycles() + 1);
    assert_eq!(after.issued_rows(), before.issued_rows() + 1);
    assert_eq!(
        fixture.runtime.live_issue.counted_cycle_ticks_for_test(),
        [21, 30],
    );
    assert_eq!(
        fixture.runtime.live_issue.scheduler_entry_tick_for_test(),
        Some(21),
    );
}

#[test]
fn scheduler_entry_seals_future_active_decision_before_earlier_tick() {
    let mut runtime = O3RuntimeState::default();
    runtime.live_issue.observe_sequences(30, &[30], &[], &[], 1);
    assert_eq!(runtime.stats().issue_cycles(), 1);
    assert_eq!(runtime.stats().issued_rows(), 1);

    runtime.enter_live_issue_scheduler_at(21);
    assert_eq!(runtime.stats().issue_cycles(), 1);
    assert_eq!(runtime.stats().issued_rows(), 1);
    assert_eq!(runtime.live_issue_service_tick(), None);
    assert_eq!(runtime.live_issue.counted_cycle_ticks_for_test(), [30]);

    runtime.live_issue.observe_sequences(21, &[21], &[], &[], 1);
    runtime.seal_live_issue_decision();
    runtime.live_issue.observe_sequences(30, &[31], &[], &[], 1);
    let revisited = runtime.stats();
    assert_eq!(revisited.issue_cycles(), 2);
    assert_eq!(revisited.issued_rows(), 3);
}

#[test]
fn scheduler_facing_service_finalizes_prior_decisions_without_pruning_lookahead() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);
    fixture
        .runtime
        .live_issue
        .observe_sequences(18, &[], &[900], &[], 1);
    fixture.runtime.seal_live_issue_decision();
    fixture
        .runtime
        .live_issue
        .observe_sequences(30, &[], &[], &[901], 1);
    fixture.runtime.seal_live_issue_decision();
    assert_eq!(
        fixture.runtime.live_issue.counted_cycle_ticks_for_test(),
        [18, 30]
    );

    let outcome = fixture
        .runtime
        .service_live_issue_scheduler_at(&fixture.hart, 21)
        .unwrap();

    assert_eq!(outcome.issued_rows(), 1);
    assert_eq!(
        fixture.runtime.live_issue.scheduler_entry_tick_for_test(),
        Some(21)
    );
    assert_eq!(
        fixture.runtime.live_issue.counted_cycle_ticks_for_test(),
        [21, 30]
    );
    assert_eq!(fixture.runtime.stats().resource_blocked_row_cycles(), 3);
    assert_eq!(fixture.runtime.stats().dependency_blocked_row_cycles(), 1);
}

#[test]
fn stats_reset_clears_cycle_evidence_without_delaying_issue_timing() {
    let mut fixture = dependency_lookahead_fixture();
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20)
        .unwrap();
    assert_eq!(fixture.issue_tick(SECOND_PC), 30);

    fixture.runtime.reset_stats();
    assert_eq!(fixture.runtime.stats().issue_cycles(), 0);
    assert!(fixture
        .runtime
        .live_issue
        .counted_cycle_ticks_for_test()
        .is_empty());

    fixture.bind_row_at(2, 21);
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(21));
    fixture
        .runtime
        .schedule_live_speculative_issues(&fixture.hart, fixture.head, 21)
        .unwrap();
    assert_eq!(fixture.issue_tick(THIRD_PC), 21);
    let after = fixture.runtime.stats();
    assert_eq!(after.issue_cycles(), 1);
    assert_eq!(after.issued_rows(), 1);
}
