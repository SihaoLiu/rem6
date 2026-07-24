use std::collections::{BTreeMap, BTreeSet};

use super::super::o3_runtime_issue::calendar::O3LiveIssueCalendar;
use super::super::o3_runtime_issue::{O3LiveIssueBatchOutcome, O3PreparedLiveIssueBatch};
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
struct TouchedIssueState {
    pending_state: O3PendingStateSnapshot,
    reorder_buffer: Vec<O3ReorderBufferEntry>,
    live_speculative_executions: Vec<O3LiveSpeculativeExecution>,
    pending_data_addresses: O3PendingDataAddresses,
    writeback_calendar: O3WritebackReservationCalendar,
    live_writeback_counted_sequences: BTreeSet<u64>,
    finalized_writeback_port_stats: O3FinalizedWritebackPortStats,
    live_staged_fetch_identities: BTreeMap<u64, O3LiveStagedFetchIdentity>,
    stats: O3RuntimeStats,
    live_issue: O3LiveIssueState,
}

fn touched(runtime: &O3RuntimeState) -> TouchedIssueState {
    TouchedIssueState {
        pending_state: runtime.snapshot.pending_state.clone(),
        reorder_buffer: runtime.snapshot.reorder_buffer.clone(),
        live_speculative_executions: runtime.live_speculative_executions.clone(),
        pending_data_addresses: runtime.pending_data_addresses.clone(),
        writeback_calendar: runtime.writeback_calendar.clone(),
        live_writeback_counted_sequences: runtime.live_writeback_counted_sequences.clone(),
        finalized_writeback_port_stats: runtime.finalized_writeback_port_stats.clone(),
        live_staged_fetch_identities: runtime.live_staged_fetch_identities.clone(),
        stats: runtime.stats,
        live_issue: runtime.live_issue.clone(),
    }
}

fn prepared_rows(fixture: &ScalarIssueFixture, tick: u64) -> Vec<O3PreparedLiveIssue> {
    let queue = fixture.queue();
    let dependencies = O3LiveIssueDependencyTable::new(&fixture.runtime, queue.entries()).unwrap();
    let plan = O3LiveIssueCalendar::capture(&fixture.runtime)
        .plan_scoped_at(
            tick,
            dependencies.resolved_scopes_at(tick),
            queue
                .entries()
                .iter()
                .map(|entry| dependencies.scoped_instruction(entry)),
        )
        .unwrap();
    match fixture
        .runtime
        .prepare_live_issue_batch(&fixture.hart, &queue, plan.issued(), tick)
        .unwrap()
    {
        O3PreparedLiveIssueBatch::Prepared(rows) => rows,
        O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
            panic!("unexpected pending replay at {sequence}")
        }
    }
}

fn prepared_fixed_fu_row(fixture: &ScalarIssueFixture, tick: u64) -> O3PreparedLiveIssue {
    prepared_rows(fixture, tick)
        .into_iter()
        .find(|row| row.candidate.consumes_writeback_slot())
        .expect("selected fixed-FU writeback row")
}

fn raw_ready_tick(row: &O3PreparedLiveIssue) -> u64 {
    let issue_tick = row.candidate.issue_tick(row.issue_tick);
    issue_tick
        .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
            row.execution.instruction(),
        ))
        .unwrap()
}

#[test]
fn live_issue_transaction_failure_records_no_partial_runtime_or_queue_state() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let prepared = prepared_rows(&fixture, 21);
    assert_eq!(prepared.len(), 2);
    let rejected = prepared[1].candidate.sequence();
    assert!(fixture
        .runtime
        .remove_live_staged_issue_identity_for_test(rejected));
    let before = touched(&fixture.runtime);

    assert!(matches!(
        fixture.runtime.record_live_issue_batch(prepared),
        Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence })
            if sequence == rejected
    ));
    assert_eq!(touched(&fixture.runtime), before);
}

#[test]
fn live_issue_transaction_writeback_replan_rollback_restores_ports_and_descendants() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    assert!(fixture.runtime.set_writeback_width(1));
    let prepared = prepared_rows(&fixture, 21);
    assert_eq!(prepared.len(), 2);
    let rejected = prepared[1].candidate.sequence();
    assert!(fixture
        .runtime
        .remove_live_staged_issue_identity_for_test(rejected));
    let calendar = fixture.runtime.writeback_calendar.clone();
    let pending_state = fixture.runtime.snapshot.pending_state.clone();
    let executions = fixture.runtime.live_speculative_executions.clone();

    assert!(fixture.runtime.record_live_issue_batch(prepared).is_err());
    assert_eq!(fixture.runtime.writeback_calendar, calendar);
    assert_eq!(fixture.runtime.snapshot.pending_state, pending_state);
    assert_eq!(fixture.runtime.live_speculative_executions, executions);
}

#[test]
fn live_issue_transaction_pending_replay_commits_only_exact_suffix_cleanup() {
    let mut runtime = O3RuntimeState::default();
    let sequence = super::queue::stage_queue_pending_row(&mut runtime);
    runtime.set_pending_data_address_resource_blocked_wake_for_test(sequence, 41);
    let mut hart = RiscvHartState::new(BRANCH_PC);
    hart.write(reg(12), 0x9100);
    let queue = super::queue::materialized_queue(&runtime);
    let dependencies = O3LiveIssueDependencyTable::new(&runtime, queue.entries()).unwrap();
    let plan = O3LiveIssueCalendar::capture(&runtime)
        .plan_scoped_at(
            40,
            dependencies.resolved_scopes_at(40),
            queue
                .entries()
                .iter()
                .map(|entry| dependencies.scoped_instruction(entry)),
        )
        .unwrap();
    let prepared = match runtime
        .prepare_live_issue_batch(&hart, &queue, plan.issued(), 40)
        .unwrap()
    {
        O3PreparedLiveIssueBatch::Prepared(rows) => rows,
        O3PreparedLiveIssueBatch::ReplayPending(replay) => {
            panic!("unexpected early replay at {replay}")
        }
    };
    assert!(runtime.remove_live_staged_issue_identity_for_test(sequence));
    let older_live_data = runtime.live_data_accesses.clone();

    assert_eq!(
        runtime.record_live_issue_batch(prepared).unwrap(),
        O3LiveIssueBatchOutcome::ReplayPending(sequence),
    );
    assert_eq!(runtime.live_data_accesses, older_live_data);
    assert!(runtime.pending_data_addresses.is_empty());
    assert!(runtime.live_issue.resident_sequences().is_empty());
}

#[test]
fn live_issue_transaction_commit_removes_only_durable_selected_sequences() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let prepared = prepared_rows(&fixture, 21);
    let committed = prepared
        .iter()
        .map(|row| row.candidate.sequence())
        .collect::<BTreeSet<_>>();
    assert_eq!(committed.len(), 2);

    assert_eq!(
        fixture.runtime.record_live_issue_batch(prepared).unwrap(),
        O3LiveIssueBatchOutcome::Recorded,
    );
    assert!(committed.iter().all(|sequence| {
        !fixture
            .runtime
            .live_issue
            .resident_sequences()
            .contains(sequence)
    }));
    assert_eq!(fixture.runtime.live_issue.resident_sequences().len(), 1);
}

#[test]
fn live_issue_transaction_rejects_an_already_active_transaction() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let prepared = prepared_rows(&fixture, 21);
    assert!(fixture.runtime.live_issue.begin_transaction());
    let before = touched(&fixture.runtime);

    assert_eq!(
        fixture.runtime.record_live_issue_batch(prepared),
        Err(O3RuntimeError::LiveIssueTransactionAlreadyActive),
    );
    assert_eq!(touched(&fixture.runtime), before);
}

#[test]
fn live_issue_transaction_active_error_display_is_stable() {
    assert_eq!(
        O3RuntimeError::LiveIssueTransactionAlreadyActive.to_string(),
        "O3 runtime live issue transaction is already active",
    );
}

#[test]
fn live_issue_transaction_reserved_recording_rejects_same_sequence_raw_ready_mismatch() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let row = prepared_fixed_fu_row(&fixture, 21);
    let sequence = row.candidate.sequence();
    let reservation = fixture
        .runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
            sequence,
            raw_ready_tick(&row) + 1,
        )])
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    assert!(!fixture
        .runtime
        .record_live_speculative_execution_with_reservation(
            &row.candidate,
            &row.consumed_requests,
            row.issue_tick,
            row.execution,
            Some(reservation),
        )
        .unwrap());
    assert!(fixture.runtime.live_speculative_executions.is_empty());
}

#[test]
fn live_issue_transaction_reserved_recording_rejects_same_sequence_source_mismatch() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let row = prepared_fixed_fu_row(&fixture, 21);
    let sequence = row.candidate.sequence();
    let reservation = fixture
        .runtime
        .reserve_writeback_completions([O3LiveWritebackReady::memory_result(
            sequence,
            raw_ready_tick(&row),
        )])
        .unwrap()
        .into_iter()
        .next()
        .unwrap();

    assert!(!fixture
        .runtime
        .record_live_speculative_execution_with_reservation(
            &row.candidate,
            &row.consumed_requests,
            row.issue_tick,
            row.execution,
            Some(reservation),
        )
        .unwrap());
    assert!(fixture.runtime.live_speculative_executions.is_empty());
}
