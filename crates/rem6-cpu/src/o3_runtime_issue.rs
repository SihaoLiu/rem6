use rem6_isa_riscv::{RiscvExecutionRecord, RiscvInstruction};

use super::o3_runtime_writeback::{O3LiveWritebackReady, O3WritebackReservation};
use super::*;
use crate::o3_pipeline::O3IssueOpClass;

#[path = "o3_runtime_issue/dependency.rs"]
mod dependency;

#[path = "o3_runtime_issue/calendar.rs"]
pub(in crate::o3_runtime) mod calendar;
#[path = "o3_runtime_issue/pending_address.rs"]
mod pending_address;
#[path = "o3_runtime_issue/queue.rs"]
pub(in crate::o3_runtime) mod queue;
#[path = "o3_runtime_issue/service.rs"]
mod service;
#[path = "o3_runtime_issue/state.rs"]
mod state;
#[path = "o3_runtime_issue/transaction.rs"]
mod transaction;
pub(crate) use dependency::O3LiveIssueDependencyTable;
use queue::{live_issue_op_class, O3LiveSpeculativeIssueCandidate};
#[cfg(test)]
pub(in crate::o3_runtime) use service::O3LiveIssueServiceError;
pub(in crate::o3_runtime) use state::{O3LiveIssueState, O3LiveIssueStateRollback};
pub use state::{
    O3LiveIssueTelemetry, O3LiveIssueTraceAction, O3LiveIssueTraceClass, O3LiveIssueTraceRecord,
};
use transaction::O3LiveIssueTransaction;
pub(in crate::o3_runtime) use transaction::O3LiveIssueTransactionError;

#[derive(Clone)]
pub(crate) struct O3PreparedLiveIssue {
    pub(crate) candidate: O3LiveSpeculativeIssueCandidate,
    pub(crate) consumed_requests: Vec<MemoryRequestId>,
    pub(crate) issue_tick: u64,
    pub(crate) execution: RiscvExecutionRecord,
}

impl O3PreparedLiveIssue {
    fn fixed_fu_writeback_ready(&self) -> Result<Option<O3LiveWritebackReady>, O3RuntimeError> {
        if self.candidate.is_pending_data_address() || !self.candidate.consumes_writeback_slot() {
            return Ok(None);
        }
        let issue_tick = self.candidate.issue_tick(self.issue_tick);
        let raw_ready_tick = issue_tick
            .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                self.execution.instruction(),
            ))
            .ok_or(O3RuntimeError::WritebackTickOverflow { tick: issue_tick })?;
        Ok(Some(O3LiveWritebackReady::fixed_fu(
            self.candidate.sequence(),
            raw_ready_tick,
        )))
    }
}

pub(in crate::o3_runtime) enum O3PreparedLiveIssueBatch {
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
        let writeback_reservation = self.reserve_fixed_fu_writeback(
            head.sequence(),
            raw_ready_tick,
            consumes_writeback_slot,
        )?;
        let admitted_writeback_tick = writeback_reservation
            .map(O3WritebackReservation::admitted_tick)
            .unwrap_or(raw_ready_tick);
        let writeback_slot = writeback_reservation.map(O3WritebackReservation::slot);
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

    pub(in crate::o3_runtime) fn record_live_issue_batch(
        &mut self,
        prepared: Vec<O3PreparedLiveIssue>,
    ) -> Result<O3LiveIssueBatchOutcome, O3LiveIssueTransactionError> {
        O3LiveIssueTransaction::record(self, prepared)
    }
}
