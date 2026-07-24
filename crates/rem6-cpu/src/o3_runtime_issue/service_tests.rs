use super::super::o3_runtime_issue::O3LiveIssueServiceError;
use super::*;

#[test]
fn service_live_issue_queue_at_issues_only_the_requested_tick() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);
    let outcome = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 21)
        .unwrap();
    assert_eq!(outcome.issued_rows(), 1);
    assert_eq!(fixture.runtime.live_speculative_executions.len(), 1);
    assert!(fixture
        .runtime
        .live_speculative_executions
        .iter()
        .all(|row| row.issue_tick == 21));
    assert_eq!(fixture.runtime.live_issue.resident_sequences().len(), 2);
}

#[test]
fn service_live_issue_queue_at_retains_resource_blocked_rows_for_next_tick() {
    let mut fixture = ScalarIssueFixture::new(1, ScalarIssueCase::CrossResource);
    let outcome = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 21)
        .unwrap();
    assert_eq!(outcome.next_service_tick(), Some(22));
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(22));
    assert_eq!(fixture.runtime.live_issue.resident_sequences().len(), 2);
}

#[test]
fn service_live_issue_queue_at_requests_earliest_dependency_ready_tick() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::Dependent);
    let outcome = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 21)
        .unwrap();
    let producer_ready = fixture.execution_at(SECOND_PC).admitted_writeback_tick;
    assert_eq!(outcome.next_service_tick(), Some(producer_ready));
    assert_eq!(
        fixture.runtime.live_issue_service_tick(),
        Some(producer_ready)
    );
    assert!(fixture
        .runtime
        .live_issue
        .resident_sequences()
        .contains(&fixture.sequence(THIRD_PC)));
}

#[test]
fn service_live_issue_queue_at_allows_capacity_remaining_same_tick_reentry() {
    assert_eq!(
        crate::riscv_fu_latency::riscv_execute_wait_cycles(addi(14, 2, 1)),
        0,
    );
    let mut fixture = ScalarIssueFixture::new(4, ScalarIssueCase::SameTickAluDependency);
    let first = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    assert_eq!(first.issued_rows(), 2);
    assert_eq!(first.next_service_tick(), Some(20));
    assert_eq!(fixture.runtime.live_issue_service_tick(), Some(20));

    let second = fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    assert_eq!(second.issued_rows(), 1);
    assert!(fixture.runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn service_live_issue_queue_at_reports_private_no_wake_invariant() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::SameWindowLinkReturn);
    fixture.bind_row(1);
    let sequence = fixture.sequence(SECOND_PC);

    assert_eq!(
        fixture
            .runtime
            .service_live_issue_queue_at(&fixture.hart, 20),
        Err(O3LiveIssueServiceError::NoWake { sequence }),
    );
    assert_eq!(fixture.runtime.live_issue.resident_sequences(), &[sequence]);
    assert_eq!(fixture.runtime.stats().issue_cycles(), 1);
    assert_eq!(fixture.runtime.stats().dependency_blocked_row_cycles(), 1);
}

#[test]
fn schedule_live_speculative_issues_translates_no_wake_and_preserves_diagnostics() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::SameWindowLinkReturn);
    fixture.bind_row(1);
    let sequence = fixture.sequence(SECOND_PC);

    assert_eq!(
        fixture
            .runtime
            .schedule_live_speculative_issues(&fixture.hart, fixture.head, 20,),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence }),
    );
    assert_eq!(fixture.runtime.live_issue.resident_sequences(), &[sequence]);
    assert_eq!(fixture.runtime.live_issue_service_tick(), None);
}

#[test]
fn live_issue_stats_same_tick_reentry_projects_once() {
    let mut fixture = ScalarIssueFixture::new(4, ScalarIssueCase::SameTickAluDependency);
    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    let first_projection = fixture.runtime.stats();
    assert_eq!(fixture.runtime.stats(), first_projection);
    assert_eq!(first_projection.issue_cycles(), 1);
    assert_eq!(first_projection.issued_rows(), 2);

    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    let second_projection = fixture.runtime.stats();
    assert_eq!(second_projection.issue_cycles(), 1);
    assert_eq!(second_projection.issued_rows(), 3);
    assert_eq!(fixture.runtime.stats(), second_projection);

    fixture.runtime.seal_live_issue_decision_before(21);
    assert_eq!(fixture.runtime.stats(), second_projection);
}

#[test]
fn live_issue_stats_reset_rebases_unsealed_decision() {
    let mut fixture = ScalarIssueFixture::new(4, ScalarIssueCase::SameTickAluDependency);
    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    assert_eq!(fixture.runtime.stats().issued_rows(), 2);
    fixture.runtime.reset_stats();
    assert_eq!(fixture.runtime.stats().issued_rows(), 0);

    fixture.runtime.live_issue.mark_mutated();
    fixture.runtime.live_issue.request_service_at(20);
    fixture
        .runtime
        .service_live_issue_queue_at(&fixture.hart, 20)
        .unwrap();
    let post_reset = fixture.runtime.stats();
    assert_eq!(post_reset.issue_cycles(), 1);
    assert_eq!(post_reset.issued_rows(), 1);
    assert_eq!(post_reset.resource_blocked_row_cycles(), 0);
    assert_eq!(post_reset.dependency_blocked_row_cycles(), 0);
}
