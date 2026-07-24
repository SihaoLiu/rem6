use std::collections::{BTreeMap, BTreeSet};

use super::*;

#[derive(Clone)]
struct O3LiveIssueRollback {
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

impl O3LiveIssueRollback {
    fn capture(runtime: &O3RuntimeState) -> Self {
        Self {
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

    fn restore(self, runtime: &mut O3RuntimeState) {
        runtime.snapshot.pending_state = self.pending_state;
        runtime.snapshot.reorder_buffer = self.reorder_buffer;
        runtime.live_speculative_executions = self.live_speculative_executions;
        runtime.pending_data_addresses = self.pending_data_addresses;
        runtime.writeback_calendar = self.writeback_calendar;
        runtime.live_writeback_counted_sequences = self.live_writeback_counted_sequences;
        runtime.finalized_writeback_port_stats = self.finalized_writeback_port_stats;
        runtime.live_staged_fetch_identities = self.live_staged_fetch_identities;
        runtime.stats = self.stats;
        runtime.live_issue = self.live_issue;
    }
}

enum O3RecordedLiveIssue {
    Pending {
        sequence: u64,
        pc: Address,
        issue_tick: u64,
    },
    FixedFu {
        sequence: u64,
        pc: Address,
        issue_class: O3LiveIssueTraceClass,
        issue_tick: u64,
        raw_ready_tick: u64,
        admitted_writeback_tick: u64,
    },
}

pub(in crate::o3_runtime) struct O3LiveIssueTransaction;

impl O3LiveIssueTransaction {
    pub(in crate::o3_runtime) fn record(
        runtime: &mut O3RuntimeState,
        prepared: Vec<O3PreparedLiveIssue>,
    ) -> Result<O3LiveIssueBatchOutcome, O3RuntimeError> {
        let rollback = O3LiveIssueRollback::capture(runtime);
        if !runtime.live_issue.begin_transaction() {
            return Err(O3RuntimeError::LiveIssueTransactionAlreadyActive);
        }
        let result = record_prepared_batch_in_place(runtime, prepared);
        match result {
            Ok(O3LiveIssueBatchOutcome::Recorded) => {
                runtime.live_issue.end_transaction();
                Ok(O3LiveIssueBatchOutcome::Recorded)
            }
            Ok(O3LiveIssueBatchOutcome::ReplayPending(sequence)) => {
                rollback.restore(runtime);
                runtime.discard_pending_data_address_from(sequence);
                Ok(O3LiveIssueBatchOutcome::ReplayPending(sequence))
            }
            Err(error) => {
                rollback.restore(runtime);
                Err(error)
            }
        }
    }
}

fn record_prepared_batch_in_place(
    runtime: &mut O3RuntimeState,
    prepared: Vec<O3PreparedLiveIssue>,
) -> Result<O3LiveIssueBatchOutcome, O3RuntimeError> {
    let ready = prepared
        .iter()
        .map(O3PreparedLiveIssue::fixed_fu_writeback_ready)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    let mut reservations = runtime
        .reserve_writeback_completions(ready)?
        .into_iter()
        .map(|reservation| (reservation.sequence(), reservation))
        .collect::<BTreeMap<_, _>>();
    let mut recorded_rows = Vec::with_capacity(prepared.len());

    for row in prepared {
        let sequence = row.candidate.sequence();
        let issue_tick = row.candidate.issue_tick(row.issue_tick);
        let recorded = if row.candidate.is_pending_data_address() {
            let recorded = runtime
                .record_pending_data_address_materialization_without_issue_removal(
                    &row.candidate,
                    &row.consumed_requests,
                    row.issue_tick,
                    row.execution,
                )?;
            if recorded {
                recorded_rows.push(O3RecordedLiveIssue::Pending {
                    sequence,
                    pc: row.candidate.pc(),
                    issue_tick,
                });
            }
            recorded
        } else {
            let reservation = reservations.remove(&sequence);
            let raw_ready_tick = issue_tick
                .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                    row.execution.instruction(),
                ))
                .ok_or(O3RuntimeError::WritebackTickOverflow { tick: issue_tick })?;
            let admitted_writeback_tick = reservation
                .map(O3WritebackReservation::admitted_tick)
                .unwrap_or(raw_ready_tick);
            let Some(issue_class) = live_issue_trace_class(row.candidate.instruction()) else {
                return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence });
            };
            let recorded = runtime.record_live_speculative_execution_with_reservation(
                &row.candidate,
                &row.consumed_requests,
                row.issue_tick,
                row.execution,
                reservation,
            )?;
            if recorded {
                recorded_rows.push(O3RecordedLiveIssue::FixedFu {
                    sequence,
                    pc: row.candidate.pc(),
                    issue_class,
                    issue_tick,
                    raw_ready_tick,
                    admitted_writeback_tick,
                });
            }
            recorded
        };
        if !recorded {
            if runtime
                .pending_data_address_sequence_for_replay(sequence)
                .is_some()
            {
                return Ok(O3LiveIssueBatchOutcome::ReplayPending(sequence));
            }
            return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence });
        }
    }

    debug_assert!(reservations.is_empty());
    for recorded in recorded_rows {
        let (sequence, removed) = match recorded {
            O3RecordedLiveIssue::Pending {
                sequence,
                pc,
                issue_tick,
            } => (
                sequence,
                runtime.live_issue.remove_exact_at(
                    sequence,
                    O3LiveIssueTraceAction::Selected,
                    pc,
                    O3LiveIssueTraceClass::MemoryAgu,
                    issue_tick,
                ),
            ),
            O3RecordedLiveIssue::FixedFu {
                sequence,
                pc,
                issue_class,
                issue_tick,
                raw_ready_tick,
                admitted_writeback_tick,
            } => (
                sequence,
                runtime.live_issue.remove_selected_at(
                    sequence,
                    pc,
                    issue_class,
                    issue_tick,
                    raw_ready_tick,
                    admitted_writeback_tick,
                ),
            ),
        };
        if !removed {
            return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence });
        }
    }
    Ok(O3LiveIssueBatchOutcome::Recorded)
}
