use super::*;
use crate::o3_runtime::o3_runtime_issue::calendar::O3LiveIssueCalendar;
use crate::o3_runtime::o3_runtime_pending_address_tests::multiple::ready_two_pending_issue;

#[path = "service_tests/scheduler_request.rs"]
mod scheduler_request;

const REPLAY_SERVICE_TICK: u64 = 41;

fn live_issue_plan_shape_at(runtime: &O3RuntimeState, tick: u64) -> (usize, usize) {
    let queue =
        match O3LiveIssueQueue::materialize(runtime, runtime.live_issue.resident_sequences())
            .unwrap()
        {
            O3LiveIssueQueueCapture::Ready(queue) => queue,
            O3LiveIssueQueueCapture::ReplayPending(sequence) => {
                panic!("unexpected replay boundary {sequence}")
            }
        };
    let dependencies = O3LiveIssueDependencyTable::new(runtime, queue.entries()).unwrap();
    let plan = O3LiveIssueCalendar::capture(runtime)
        .plan_scoped_at(
            tick,
            dependencies.resolved_scopes_at(tick),
            queue
                .entries()
                .iter()
                .map(|entry| dependencies.scoped_instruction(entry)),
        )
        .unwrap();
    (plan.reserved_width(), plan.issued().len())
}

fn replay_survivor_fixture() -> (
    O3RuntimeState,
    RiscvHartState,
    O3LiveIssueHeadReservation,
    u64,
    u64,
) {
    let (mut runtime, hart, head) = ready_two_pending_issue(2, false);
    let sequences = runtime.pending_data_address_sequences_for_test();
    let [survivor, replay] = sequences.as_slice() else {
        panic!("expected two pending rows")
    };
    assert!(runtime.remove_live_staged_issue_identity_for_test(*replay));
    assert!(runtime
        .live_issue
        .resident_sequences()
        .starts_with(&[*survivor, *replay]));
    (runtime, hart, head, *survivor, *replay)
}

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
fn service_live_issue_queue_at_translates_no_wake_to_runtime_error() {
    let mut fixture = ScalarIssueFixture::new_unbound(2, ScalarIssueCase::SameWindowLinkReturn);
    fixture.bind_row(1);
    let sequence = fixture.sequence(SECOND_PC);

    assert_eq!(
        fixture
            .runtime
            .service_live_issue_queue_at(&fixture.hart, 20),
        Err(O3RuntimeError::InvalidLiveIssueQueueEntry { sequence }),
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
fn two_pending_replay_reclassifies_older_resident_and_preserves_compatibility_wake() {
    let (mut direct, hart, _, survivor, replay) = replay_survivor_fixture();
    let outcome = direct
        .service_live_issue_queue_at(&hart, REPLAY_SERVICE_TICK)
        .unwrap();
    assert_eq!(outcome.replay_boundary(), Some(replay));
    assert_eq!(outcome.issued_rows(), 0);
    assert_eq!(outcome.next_service_tick(), Some(REPLAY_SERVICE_TICK));
    assert_eq!(direct.live_issue.resident_sequences(), [survivor]);
    assert_eq!(direct.live_issue_service_tick(), Some(REPLAY_SERVICE_TICK));
    assert!(!direct
        .live_speculative_executions
        .iter()
        .any(|issued| issued.sequence == survivor));
    assert_eq!(direct.stats().issue_cycles(), 1);
    assert_eq!(direct.stats().issued_rows(), 0);

    let (mut compatibility, hart, head, survivor, replay) = replay_survivor_fixture();
    compatibility
        .schedule_live_speculative_issues(&hart, head, REPLAY_SERVICE_TICK)
        .unwrap();
    assert_eq!(compatibility.live_issue.resident_sequences(), [survivor]);
    assert_eq!(
        compatibility.live_issue_service_tick(),
        Some(REPLAY_SERVICE_TICK)
    );
    assert_eq!(
        compatibility.pending_data_address_sequences_for_test(),
        [survivor]
    );
    assert!(!compatibility
        .live_speculative_executions
        .iter()
        .any(|issued| issued.sequence == survivor || issued.sequence == replay));
}

#[test]
fn preplan_replay_with_empty_survivors_records_no_issue_decision() {
    let (mut runtime, hart, _, _, _) = replay_survivor_fixture();
    let replay = runtime.pending_data_address_sequences_for_test()[0];
    assert!(runtime.remove_live_staged_issue_identity_for_test(replay));

    let outcome = runtime
        .service_live_issue_queue_at(&hart, REPLAY_SERVICE_TICK)
        .unwrap();
    assert_eq!(outcome.replay_boundary(), Some(replay));
    assert!(runtime.live_issue.resident_sequences().is_empty());
    assert_eq!(runtime.stats().issue_cycles(), 0);
    assert_eq!(runtime.stats().issued_rows(), 0);
    assert_eq!(runtime.stats().max_rows_per_cycle(), 0);
}

#[test]
fn postplan_replay_with_empty_survivors_preserves_arbitration_max_rows() {
    let (mut runtime, hart, _) = ready_two_pending_issue(2, false);
    let replay = runtime.pending_data_address_sequences_for_test()[0];
    let (expected_reserved_width, selected_rows) =
        live_issue_plan_shape_at(&runtime, REPLAY_SERVICE_TICK);
    assert_eq!(expected_reserved_width, 0);
    assert_eq!(selected_rows, 2);
    let recorded_before = runtime.live_speculative_executions.len();
    runtime.corrupt_pending_data_address_lsq_bytes_for_test(4);

    let outcome = runtime
        .service_live_issue_queue_at(&hart, REPLAY_SERVICE_TICK)
        .unwrap();
    assert_eq!(outcome.replay_boundary(), Some(replay));
    assert!(runtime.live_issue.resident_sequences().is_empty());
    assert_eq!(runtime.live_speculative_executions.len(), recorded_before);
    assert_eq!(runtime.stats().issue_cycles(), 1);
    assert_eq!(runtime.stats().issued_rows(), 0);
    assert_eq!(
        runtime.stats().max_rows_per_cycle(),
        expected_reserved_width as u64,
    );
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
