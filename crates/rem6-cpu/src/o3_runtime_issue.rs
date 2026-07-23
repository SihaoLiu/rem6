use rem6_isa_riscv::{RiscvExecutionRecord, RiscvHartState, RiscvInstruction};

use super::*;
use crate::o3_pipeline::{O3IssueOpClass, O3ScopedReadyInstruction};

#[path = "o3_runtime_issue/dependency.rs"]
mod dependency;

#[path = "o3_runtime_issue/calendar.rs"]
pub(in crate::o3_runtime) mod calendar;
#[path = "o3_runtime_issue/pending_address.rs"]
mod pending_address;
#[path = "o3_runtime_issue/queue.rs"]
pub(in crate::o3_runtime) mod queue;
#[path = "o3_runtime_issue/state.rs"]
mod state;
use calendar::{O3LiveIssueCalendar, O3LiveIssueTickDecision};
pub(crate) use dependency::O3LiveIssueDependencyTable;
use queue::{
    live_issue_op_class, O3LiveIssueQueue, O3LiveIssueQueueCapture, O3LiveSpeculativeIssueCandidate,
};
pub(in crate::o3_runtime) use state::O3LiveIssueState;
pub use state::{
    O3LiveIssueTelemetry, O3LiveIssueTraceAction, O3LiveIssueTraceClass, O3LiveIssueTraceRecord,
};

#[derive(Clone)]
pub(crate) struct O3PreparedLiveIssue {
    pub(crate) candidate: O3LiveSpeculativeIssueCandidate,
    pub(crate) consumed_requests: Vec<MemoryRequestId>,
    pub(crate) issue_tick: u64,
    pub(crate) execution: RiscvExecutionRecord,
}

enum O3PreparedLiveIssueBatch {
    Prepared(Vec<O3PreparedLiveIssue>),
    ReplayPending(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum O3LiveIssueBatchOutcome {
    Recorded,
    ReplayPending(u64),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueHeadReservation {
    sequence: u64,
    issue_tick: u64,
    op_class: O3IssueOpClass,
}

impl O3LiveIssueHeadReservation {
    pub(crate) fn for_instruction(
        sequence: u64,
        issue_tick: u64,
        instruction: RiscvInstruction,
    ) -> Self {
        Self {
            sequence,
            issue_tick,
            op_class: live_issue_op_class(instruction),
        }
    }

    pub(super) const fn memory(sequence: u64, issue_tick: u64) -> Self {
        Self {
            sequence,
            issue_tick,
            op_class: O3IssueOpClass::Memory,
        }
    }

    pub(in crate::o3_runtime) const fn sequence(self) -> u64 {
        self.sequence
    }
}

impl O3RuntimeState {
    pub(in crate::o3_runtime) fn enqueue_bound_live_issue_sequence_at(
        &mut self,
        sequence: u64,
        tick: u64,
    ) -> bool {
        let Some(rob) = self
            .snapshot
            .reorder_buffer
            .iter()
            .copied()
            .find(|entry| entry.is_live_staged() && entry.sequence() == sequence)
        else {
            return false;
        };
        if self
            .live_speculative_executions
            .iter()
            .any(|issued| issued.sequence == sequence)
        {
            return true;
        }
        let pending = self.pending_data_addresses.find_sequence(sequence);
        if pending.is_some_and(|row| row.materialized.is_some()) {
            return true;
        }
        let Some(packet) = self.live_staged_issue_packet(sequence) else {
            return false;
        };
        let issue_class = if pending.is_some() {
            Some(O3LiveIssueTraceClass::MemoryAgu)
        } else {
            queue::live_issue_trace_class(packet.instruction())
        };
        let Some(issue_class) = issue_class else {
            return true;
        };
        self.live_issue
            .enqueue_at(sequence, rob.pc(), issue_class, tick);
        true
    }

    pub(crate) fn live_data_access_head_reservation(
        &self,
        fetch_request: MemoryRequestId,
    ) -> Option<O3LiveIssueHeadReservation> {
        self.live_data_accesses
            .iter()
            .find(|live| live.fetch_request == fetch_request)
            .map(|live| O3LiveIssueHeadReservation::memory(live.sequence, live.issue_tick))
    }

    pub(crate) fn record_live_issue_head_execution(
        &mut self,
        head: O3LiveIssueHeadReservation,
        consumed_requests: &[MemoryRequestId],
        execution: RiscvExecutionRecord,
    ) -> Result<bool, O3RuntimeError> {
        if !self
            .live_staged_issue_packet(head.sequence())
            .is_some_and(|packet| packet.matches_execution(&execution, consumed_requests))
        {
            return Ok(false);
        }
        if let Some(recorded) = self
            .live_speculative_executions
            .iter()
            .find(|recorded| recorded.sequence == head.sequence())
        {
            return Ok(recorded.consumed_requests == consumed_requests
                && recorded.issue_tick == head.issue_tick
                && recorded.execution == execution);
        }
        let Some(entry) = self
            .snapshot
            .reorder_buffer
            .iter()
            .copied()
            .find(|entry| entry.is_live_staged() && entry.sequence() == head.sequence())
        else {
            return Ok(false);
        };
        if entry.pc() != Address::new(execution.pc())
            || live_issue_op_class(execution.instruction()) != head.op_class
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
            || staged_rename_entry(entry).is_some_and(|destination| {
                !execution_writes_rename_destination(&execution, destination)
            })
        {
            return Ok(false);
        }
        let raw_ready_tick = head
            .issue_tick
            .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                execution.instruction(),
            ))
            .ok_or(O3RuntimeError::WritebackTickOverflow {
                tick: head.issue_tick,
            })?;
        let consumes_writeback_slot = staged_rename_entry(entry).is_some();
        let issue_class = live_issue_trace_class(execution.instruction());
        let (admitted_writeback_tick, writeback_slot) = self.reserve_fixed_fu_writeback(
            head.sequence(),
            raw_ready_tick,
            consumes_writeback_slot,
        )?;
        if let Some(entry) = self
            .snapshot
            .reorder_buffer
            .iter_mut()
            .find(|entry| entry.is_live_staged() && entry.sequence() == head.sequence())
        {
            entry.mark_ready_at(admitted_writeback_tick);
        }
        self.live_speculative_executions
            .push(O3LiveSpeculativeExecution {
                consumed_requests: consumed_requests.to_vec(),
                sequence: head.sequence(),
                producer_sequences: Vec::new(),
                issue_tick: head.issue_tick,
                raw_ready_tick,
                admitted_writeback_tick,
                writeback_slot,
                execution,
            });
        self.live_speculative_executions
            .sort_by_key(|recorded| recorded.sequence);
        if let Some(issue_class) = issue_class {
            self.live_issue.remove_selected_at(
                head.sequence(),
                entry.pc(),
                issue_class,
                head.issue_tick,
                raw_ready_tick,
                admitted_writeback_tick,
            );
        }
        Ok(true)
    }

    pub(crate) fn record_live_issue_batch(
        &mut self,
        prepared: Vec<O3PreparedLiveIssue>,
    ) -> Result<O3LiveIssueBatchOutcome, O3RuntimeError> {
        let mut staged = self.clone();
        for row in prepared {
            let sequence = row.candidate.sequence();
            let recorded = if row.candidate.is_pending_data_address() {
                staged.record_pending_data_address_materialization(
                    row.candidate,
                    &row.consumed_requests,
                    row.issue_tick,
                    row.execution,
                )?
            } else {
                staged.record_live_speculative_execution(
                    row.candidate,
                    &row.consumed_requests,
                    row.issue_tick,
                    row.execution,
                )?
            };
            if !recorded {
                if staged
                    .pending_data_address_sequence_for_replay(sequence)
                    .is_some()
                {
                    staged.discard_pending_data_address_from(sequence);
                    *self = staged;
                    return Ok(O3LiveIssueBatchOutcome::ReplayPending(sequence));
                }
                return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable { sequence });
            }
        }
        *self = staged;
        Ok(O3LiveIssueBatchOutcome::Recorded)
    }

    pub(crate) fn schedule_live_speculative_issues(
        &mut self,
        hart: &RiscvHartState,
        head: O3LiveIssueHeadReservation,
        earliest_tick: u64,
    ) -> Result<(), O3RuntimeError> {
        if !self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.is_live_staged() && entry.sequence() == head.sequence())
            && !self.pending_data_address_has_producer_sequence(head.sequence())
        {
            return Ok(());
        }
        let mut tick = earliest_tick;
        let mut tick_decision = O3LiveIssueTickDecision::default();
        loop {
            let queue = match O3LiveIssueQueue::capture(self, head)? {
                O3LiveIssueQueueCapture::Ready(queue) => queue,
                O3LiveIssueQueueCapture::ReplayPending(sequence) => {
                    let mut staged = self.clone();
                    staged.discard_pending_data_address_from(sequence);
                    *self = staged;
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    break;
                }
            };
            if queue.entries().is_empty() {
                self.flush_live_issue_decision(tick, &mut tick_decision);
                break;
            }

            let dependency_table = O3LiveIssueDependencyTable::new(self, queue.entries())?;
            let calendar = O3LiveIssueCalendar::capture(self, head);
            let plan = calendar.plan_at(tick, &dependency_table, queue.entries())?;
            let issued_rows = plan.issued().len();
            if issued_rows != 0 {
                let prepared = self.prepare_live_issue_batch(hart, &queue, plan.issued(), tick)?;
                let outcome = match prepared {
                    O3PreparedLiveIssueBatch::Prepared(prepared) => {
                        self.record_live_issue_batch(prepared)?
                    }
                    O3PreparedLiveIssueBatch::ReplayPending(sequence) => {
                        let mut staged = self.clone();
                        staged.discard_pending_data_address_from(sequence);
                        *self = staged;
                        O3LiveIssueBatchOutcome::ReplayPending(sequence)
                    }
                };
                if matches!(outcome, O3LiveIssueBatchOutcome::ReplayPending(_)) {
                    tick_decision.observe(&plan, 0);
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    break;
                }
            }
            tick_decision.observe(&plan, issued_rows);

            let blocked_pending = plan.resource_blocked().iter().find_map(|blocked| {
                self.pending_data_address_sequence_for_replay(blocked.sequence())
            });
            if let Some(sequence) = blocked_pending {
                self.record_pending_data_address_resource_blocked(sequence, tick);
                self.flush_live_issue_decision(tick, &mut tick_decision);
                break;
            } else if !plan.resource_blocked().is_empty() {
                let next_tick = tick.saturating_add(1);
                self.flush_live_issue_decision(tick, &mut tick_decision);
                if next_tick == tick {
                    break;
                }
                tick = next_tick;
            } else if !plan.dependency_blocked().is_empty() {
                if issued_rows != 0 {
                    continue;
                }
                let next_tick =
                    dependency_table.earliest_resolution_after(tick, plan.dependency_blocked());
                let Some(next_tick) = next_tick.filter(|next_tick| *next_tick > tick) else {
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    break;
                };
                if queue
                    .entries()
                    .iter()
                    .any(|entry| entry.scheduling().is_pending_data_address())
                    && next_tick > earliest_tick
                {
                    self.flush_live_issue_decision(tick, &mut tick_decision);
                    break;
                }
                self.flush_live_issue_decision(tick, &mut tick_decision);
                tick = next_tick;
            } else if issued_rows == 0 {
                self.flush_live_issue_decision(tick, &mut tick_decision);
                break;
            }
        }
        Ok(())
    }

    fn prepare_live_issue_batch(
        &self,
        hart: &RiscvHartState,
        queue: &O3LiveIssueQueue,
        issued: &[O3ScopedReadyInstruction],
        issue_tick: u64,
    ) -> Result<O3PreparedLiveIssueBatch, O3RuntimeError> {
        let mut selected = Vec::with_capacity(issued.len());
        for issued in issued {
            let Some(entry) = queue.entry(issued.sequence()) else {
                return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
                    sequence: issued.sequence(),
                });
            };
            selected.push(entry);
        }
        selected.sort_by_key(|entry| {
            (
                !entry.scheduling().is_pending_data_address(),
                entry.sequence(),
            )
        });

        let mut prepared = Vec::with_capacity(selected.len());
        for entry in selected {
            let packet = entry.packet();
            let Some(candidate) =
                self.materialize_live_speculative_issue_candidate(entry.scheduling())
            else {
                return if entry.scheduling().is_pending_data_address() {
                    Ok(O3PreparedLiveIssueBatch::ReplayPending(entry.sequence()))
                } else {
                    Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
                        sequence: entry.sequence(),
                    })
                };
            };
            let mut speculative_hart = hart.clone();
            for write in candidate.forwarded_register_writes() {
                speculative_hart.write(write.register(), write.value());
            }
            speculative_hart.set_pc(entry.scheduling().pc().get());
            let execution = match speculative_hart.execute_decoded(packet.decoded()) {
                Ok(execution) => execution,
                Err(_) if entry.scheduling().is_pending_data_address() => {
                    return Ok(O3PreparedLiveIssueBatch::ReplayPending(entry.sequence()));
                }
                Err(_) => {
                    return Err(O3RuntimeError::SelectedIssueCandidateNotExecutable {
                        sequence: entry.sequence(),
                    });
                }
            };
            prepared.push(O3PreparedLiveIssue {
                candidate,
                consumed_requests: packet.consumed_requests().to_vec(),
                issue_tick,
                execution,
            });
        }
        Ok(O3PreparedLiveIssueBatch::Prepared(prepared))
    }

    fn record_live_issue_decision(
        &mut self,
        tick: u64,
        issued_rows: usize,
        resource_blocked_rows: usize,
        dependency_blocked_rows: usize,
        total_rows_at_tick: usize,
    ) {
        let new_cycle = self.live_issue.begin_compatibility_cycle_at(tick);
        self.stats.record_issue_cycle(
            new_cycle,
            issued_rows,
            resource_blocked_rows,
            dependency_blocked_rows,
            total_rows_at_tick,
        );
    }

    fn flush_live_issue_decision(&mut self, tick: u64, decision: &mut O3LiveIssueTickDecision) {
        let Some(decision) = decision.take() else {
            return;
        };
        self.record_live_issue_decision(
            tick,
            decision.issued_rows(),
            decision.resource_blocked_rows(),
            decision.dependency_blocked_rows(),
            decision.max_rows_at_tick(),
        );
    }
}
