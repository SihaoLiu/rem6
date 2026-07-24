use std::collections::{BTreeMap, BTreeSet};

use super::super::o3_runtime_issue::calendar::O3LiveIssueCalendar;
use super::super::o3_runtime_issue::{
    O3LiveIssueBatchOutcome, O3LiveIssueTransactionError, O3PreparedLiveIssueBatch,
};
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

fn prepared_scalar_row(
    runtime: &O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
    issue_tick: u64,
    destination: Register,
    value: u64,
) -> O3PreparedLiveIssue {
    O3PreparedLiveIssue {
        candidate: runtime
            .live_speculative_issue_candidate(Address::new(pc), instruction)
            .expect("scalar candidate is available"),
        consumed_requests: vec![request(request_sequence)],
        issue_tick,
        execution: rem6_isa_riscv::RiscvExecutionRecord::new(
            instruction,
            pc,
            pc + 4,
            vec![RegisterWrite::new(destination, value)],
            None,
        ),
    }
}

fn bind_scalar_row(
    runtime: &mut O3RuntimeState,
    pc: u64,
    instruction: RiscvInstruction,
    request_sequence: u64,
) {
    assert!(runtime.bind_live_staged_issue_packet(
        Address::new(pc),
        decoded(instruction),
        &[request(request_sequence)],
        0,
    ));
}

struct WritebackReplanRollbackFixture {
    runtime: O3RuntimeState,
    prepared: Vec<O3PreparedLiveIssue>,
    child_sequence: u64,
    rejected_sequence: u64,
}

fn writeback_replan_rollback_fixture() -> WritebackReplanRollbackFixture {
    const PRODUCER_PC: u64 = 0x9000;
    const CHILD_PC: u64 = 0x9004;
    const FIRST_PC: u64 = 0x9008;
    const SECOND_PC: u64 = 0x900c;
    const REJECTED_PC: u64 = 0x9010;

    let mut runtime = O3RuntimeState::default();
    assert!(runtime.set_writeback_width(1));
    runtime.set_scalar_memory_window_limit(4);
    let producer = addi(6, 0, 1);
    let child = addi(7, 6, 1);
    let rows = [addi(20, 0, 1), addi(21, 0, 2), addi(22, 0, 3)];
    let load = scalar_load_event();
    assert!(runtime.stage_live_data_access_issue_for_test(&load, request(20), 31));
    assert_eq!(
        runtime.stage_live_data_access_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(PRODUCER_PC), producer),
                (Address::new(CHILD_PC), child),
            ],
        ),
        2
    );
    runtime
        .stage_live_retire_window(
            Address::new(FIRST_PC),
            rows[0],
            0,
            [
                (Address::new(SECOND_PC), rows[1]),
                (Address::new(REJECTED_PC), rows[2]),
            ],
        )
        .unwrap();
    let producer_sequence = runtime
        .snapshot()
        .reorder_buffer()
        .iter()
        .find(|entry| entry.pc() == Address::new(PRODUCER_PC))
        .unwrap()
        .sequence();
    for (pc, instruction, request_sequence) in [
        (PRODUCER_PC, producer, 50),
        (CHILD_PC, child, 51),
        (FIRST_PC, rows[0], 60),
        (SECOND_PC, rows[1], 61),
        (REJECTED_PC, rows[2], 62),
    ] {
        bind_scalar_row(&mut runtime, pc, instruction, request_sequence);
    }
    let producer_row = prepared_scalar_row(&runtime, PRODUCER_PC, producer, 50, 50, reg(6), 1);
    assert!(runtime
        .record_live_speculative_execution(
            producer_row.candidate,
            &producer_row.consumed_requests,
            producer_row.issue_tick,
            producer_row.execution,
        )
        .unwrap());
    let child_row = prepared_scalar_row(&runtime, CHILD_PC, child, 51, 50, reg(7), 2);
    let child_sequence = child_row.candidate.sequence();
    assert_eq!(
        child_row.candidate.producer_sequences(),
        [producer_sequence]
    );
    assert!(runtime
        .record_live_speculative_execution(
            child_row.candidate,
            &child_row.consumed_requests,
            child_row.issue_tick,
            child_row.execution,
        )
        .unwrap());

    let mut prepared = vec![
        prepared_scalar_row(&runtime, FIRST_PC, rows[0], 60, 49, reg(20), 1),
        prepared_scalar_row(&runtime, SECOND_PC, rows[1], 61, 49, reg(21), 2),
        prepared_scalar_row(&runtime, REJECTED_PC, rows[2], 62, 49, reg(22), 3),
    ];
    let rejected_sequence = prepared[2].candidate.sequence();
    prepared[2].execution = rem6_isa_riscv::RiscvExecutionRecord::new(
        rows[2],
        REJECTED_PC,
        REJECTED_PC + 4,
        Vec::new(),
        None,
    );
    WritebackReplanRollbackFixture {
        runtime,
        prepared,
        child_sequence,
        rejected_sequence,
    }
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
        Err(O3LiveIssueTransactionError::Runtime(
            O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence }
        ))
            if sequence == rejected
    ));
    assert_eq!(touched(&fixture.runtime), before);
}

#[test]
fn live_issue_transaction_writeback_replan_rollback_restores_ports_and_descendants() {
    let probe = writeback_replan_rollback_fixture();
    let ready = probe
        .prepared
        .iter()
        .map(|row| O3LiveWritebackReady::fixed_fu(row.candidate.sequence(), raw_ready_tick(row)))
        .collect::<Vec<_>>();
    let pre_replan_trace_records = probe.runtime.live_issue.trace_records().len();
    let mut probe_runtime = probe.runtime;
    probe_runtime.reserve_writeback_completions(ready).unwrap();
    assert!(probe_runtime
        .live_speculative_executions
        .iter()
        .all(|execution| execution.sequence != probe.child_sequence));
    assert!(probe_runtime
        .live_issue
        .resident_sequences()
        .contains(&probe.child_sequence));
    assert!(probe_runtime.live_issue.trace_records().len() > pre_replan_trace_records);

    let fixture = writeback_replan_rollback_fixture();
    let mut runtime = fixture.runtime;
    let before = touched(&runtime);
    assert!(matches!(
        runtime.record_live_issue_batch(fixture.prepared),
        Err(O3LiveIssueTransactionError::Runtime(
            O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence }
        ))
            if sequence == fixture.rejected_sequence
    ));
    assert_eq!(touched(&runtime), before);
    assert!(runtime
        .live_speculative_executions
        .iter()
        .any(|execution| execution.sequence == fixture.child_sequence));
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
        Err(O3LiveIssueTransactionError::AlreadyActive),
    );
    assert_eq!(touched(&fixture.runtime), before);
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

#[test]
fn live_issue_transaction_reserved_recording_rejects_wrong_sequence_reservation() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    let row = prepared_fixed_fu_row(&fixture, 21);
    let wrong_sequence = row.candidate.sequence().checked_add(1000).unwrap();
    let reservation = fixture
        .runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(
            wrong_sequence,
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

#[test]
fn live_issue_transaction_reserved_recording_rejects_stale_replanned_reservation() {
    let mut fixture = ScalarIssueFixture::new(2, ScalarIssueCase::CrossResource);
    assert!(fixture.runtime.set_writeback_width(1));
    let row = prepared_fixed_fu_row(&fixture, 21);
    let sequence = row.candidate.sequence();
    let raw_ready_tick = raw_ready_tick(&row);
    let stale = fixture
        .runtime
        .reserve_writeback_completions([O3LiveWritebackReady::fixed_fu(sequence, raw_ready_tick)])
        .unwrap()[0];
    fixture
        .runtime
        .reserve_writeback_completions([O3LiveWritebackReady::memory_result(0, raw_ready_tick)])
        .unwrap();
    let current = fixture.runtime.writeback_reservation(sequence).unwrap();
    assert_ne!(current, stale);
    assert!(stale.matches_fixed_fu(sequence, raw_ready_tick));

    assert!(!fixture
        .runtime
        .record_live_speculative_execution_with_reservation(
            &row.candidate,
            &row.consumed_requests,
            row.issue_tick,
            row.execution,
            Some(stale),
        )
        .unwrap());
    assert!(fixture.runtime.live_speculative_executions.is_empty());
}
