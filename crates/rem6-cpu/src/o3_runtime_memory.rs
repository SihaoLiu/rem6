use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveScalarMemory {
    pub(super) fetch_request: MemoryRequestId,
    pub(super) data_request: MemoryRequestId,
    pub(super) execution: RiscvCpuExecutionEvent,
    pub(super) sequence: u64,
    pub(super) issue_tick: u64,
    pub(super) issue_rob_occupancy: usize,
    pub(super) issue_lsq_occupancy: usize,
    pub(super) response_tick: Option<u64>,
    pub(super) latency_ticks: Option<u64>,
    pub(super) commit_tick: Option<u64>,
    pub(super) load_data: Option<Vec<u8>>,
    pub(super) forwarded: bool,
    pub(super) outcome: O3LiveScalarMemoryOutcome,
    pub(super) event_taken: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum O3LiveScalarMemoryOutcome {
    Resident,
    Completed,
    Retried,
    Failed,
}

pub(super) fn is_deferred_o3_scalar_memory_access(access: Option<&MemoryAccessKind>) -> bool {
    matches!(
        access,
        Some(MemoryAccessKind::Load { .. } | MemoryAccessKind::Store { .. })
    )
}

pub(super) fn is_deferred_o3_scalar_memory_instruction(instruction: RiscvInstruction) -> bool {
    matches!(
        instruction,
        RiscvInstruction::Load { .. } | RiscvInstruction::Store { .. }
    )
}

pub(super) fn is_terminal_o3_scalar_memory_event(execution: &RiscvCpuExecutionEvent) -> bool {
    execution.is_scalar_memory_access()
        && matches!(
            execution.data_access_event_kind(),
            Some(
                RiscvDataAccessEventKind::Completed
                    | RiscvDataAccessEventKind::Retry
                    | RiscvDataAccessEventKind::Failed
            )
        )
}

impl O3RuntimeState {
    pub(crate) fn scalar_memory_lifecycle_is_quiescent(&self) -> bool {
        self.deferred_scalar_memory_execution.is_none()
            && self.live_scalar_memories.is_empty()
            && self.live_scalar_memory_younger_sequences.is_empty()
    }

    pub(crate) fn has_pending_scalar_memory_retirement(&self) -> bool {
        self.pending_scalar_memory_retirement_count() > 0
    }

    pub(crate) fn pending_scalar_memory_retirement_count(&self) -> usize {
        self.live_scalar_memories.len()
            + usize::from(self.deferred_scalar_memory_execution.is_some())
    }

    pub(crate) fn owns_pending_scalar_memory_retirement(
        &self,
        fetch_request: MemoryRequestId,
    ) -> bool {
        self.deferred_scalar_memory_execution == Some(fetch_request)
            || self
                .live_scalar_memories
                .iter()
                .any(|live| live.fetch_request == fetch_request)
    }

    pub(crate) fn has_live_scalar_memory(&self) -> bool {
        !self.live_scalar_memories.is_empty()
    }

    pub(crate) fn has_live_scalar_memory_window(&self) -> bool {
        !self.live_scalar_memories.is_empty()
            || !self.live_scalar_memory_younger_sequences.is_empty()
    }

    pub(crate) fn has_ready_live_scalar_memory_event(&self) -> bool {
        self.live_scalar_memories.first().is_some_and(|live| {
            live.outcome != O3LiveScalarMemoryOutcome::Resident && !live.event_taken
        })
    }

    pub(crate) fn ready_live_scalar_memory_event_kind(&self) -> Option<RiscvDataAccessEventKind> {
        let live = self.live_scalar_memories.first()?;
        if live.outcome == O3LiveScalarMemoryOutcome::Resident || live.event_taken {
            return None;
        }
        live.execution.data_access_event_kind()
    }

    pub(crate) fn ready_live_scalar_memory_completion_timing(
        &self,
    ) -> Option<(MemoryRequestId, u64, u64)> {
        let live = self.live_scalar_memories.first()?;
        if live.outcome != O3LiveScalarMemoryOutcome::Completed || live.event_taken {
            return None;
        }
        Some((
            live.fetch_request,
            live.issue_tick,
            live.response_tick
                .expect("completed live scalar memory has a response tick"),
        ))
    }

    pub(crate) fn replace_ready_live_scalar_memory_execution(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        let Some(live) = self.live_scalar_memories.first_mut() else {
            return false;
        };
        if live.fetch_request != execution.fetch().request_id()
            || live.outcome != O3LiveScalarMemoryOutcome::Completed
            || live.event_taken
        {
            return false;
        }
        live.execution = execution.clone();
        true
    }

    pub(crate) fn defer_scalar_memory_execution(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        if !execution.is_scalar_memory_access() {
            return false;
        }
        let fetch_request = execution.fetch().request_id();
        match self.deferred_scalar_memory_execution {
            Some(pending) => pending == fetch_request,
            None => {
                if !self.live_scalar_memories.is_empty() && !self.can_stage_scalar_memory(execution)
                {
                    return false;
                }
                self.deferred_scalar_memory_execution = Some(fetch_request);
                true
            }
        }
    }

    pub(crate) fn defer_scalar_memory_if_detailed(
        &mut self,
        detailed: bool,
        execution: &RiscvCpuExecutionEvent,
    ) -> bool {
        !detailed
            || !execution.is_scalar_memory_access()
            || self.defer_scalar_memory_execution(execution)
    }

    pub(crate) fn abort_deferred_scalar_memory_execution(
        &mut self,
        fetch_request: MemoryRequestId,
    ) -> bool {
        if self.deferred_scalar_memory_execution == Some(fetch_request) {
            self.deferred_scalar_memory_execution = None;
            true
        } else {
            false
        }
    }

    pub(crate) fn clear_deferred_scalar_memory_execution(&mut self) -> bool {
        self.deferred_scalar_memory_execution.take().is_some()
    }

    pub(crate) fn stage_live_scalar_memory_issue(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        issue_tick: u64,
    ) -> bool {
        if !self.has_scalar_memory_window_capacity() {
            return false;
        }
        let Some(access) = execution.execution().memory_access() else {
            return false;
        };
        if !is_deferred_o3_scalar_memory_access(Some(access)) {
            return false;
        }
        if !self.live_scalar_memories.is_empty() && !self.can_stage_scalar_memory(execution) {
            return false;
        }
        if self
            .deferred_scalar_memory_execution
            .is_some_and(|pending| pending != execution.fetch().request_id())
        {
            return false;
        }
        self.deferred_scalar_memory_execution = None;

        let sequence = self.allocate_sequence();
        let rename_destination = o3_memory_destination_registers(access).into_iter().next();
        let destination = rename_destination.map(|_| self.allocate_physical_register());
        self.snapshot.reorder_buffer.push(
            O3ReorderBufferEntry::new(
                sequence,
                Address::new(execution.execution().pc()),
                destination,
            )
            .with_live_staged_rename_destination(rename_destination),
        );
        self.snapshot
            .load_store_queue
            .extend(o3_lsq_entries(sequence, access));

        let issue_rob_occupancy = self.snapshot.reorder_buffer.len();
        let issue_lsq_occupancy = self.snapshot.load_store_queue.len();
        self.stats.observe_rob_occupancy(issue_rob_occupancy);
        self.stats.observe_lsq_occupancy(issue_lsq_occupancy);
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        self.live_scalar_memories.push(O3LiveScalarMemory {
            fetch_request: execution.fetch().request_id(),
            data_request,
            execution: execution.clone(),
            sequence,
            issue_tick,
            issue_rob_occupancy,
            issue_lsq_occupancy,
            response_tick: None,
            latency_ticks: None,
            commit_tick: None,
            load_data: None,
            forwarded: false,
            outcome: O3LiveScalarMemoryOutcome::Resident,
            event_taken: false,
        });
        true
    }

    pub(crate) fn complete_live_scalar_memory_response(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response_tick: u64,
        latency_ticks: u64,
        load_data: Option<&[u8]>,
    ) -> bool {
        self.complete_live_scalar_memory(
            execution,
            data_request,
            response_tick,
            latency_ticks,
            load_data,
            false,
        )
    }

    pub(crate) fn complete_live_scalar_memory_forwarding(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response_tick: u64,
        latency_ticks: u64,
        load_data: &[u8],
    ) -> bool {
        self.complete_live_scalar_memory(
            execution,
            data_request,
            response_tick,
            latency_ticks,
            Some(load_data),
            true,
        )
    }

    fn complete_live_scalar_memory(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        data_request: MemoryRequestId,
        response_tick: u64,
        latency_ticks: u64,
        load_data: Option<&[u8]>,
        forwarded: bool,
    ) -> bool {
        let Some(index) = self.live_scalar_memories.iter().position(|live| {
            live.data_request == data_request
                && live.fetch_request == execution.fetch().request_id()
                && live.outcome == O3LiveScalarMemoryOutcome::Resident
        }) else {
            return false;
        };
        let sequence = self.live_scalar_memories[index].sequence;

        let outcome = match execution.data_access_event_kind() {
            Some(RiscvDataAccessEventKind::Completed) => {
                let Some(rob_index) = self
                    .snapshot
                    .reorder_buffer
                    .iter()
                    .position(|entry| entry.sequence() == sequence)
                else {
                    return false;
                };
                let Some(lsq_index) = self
                    .snapshot
                    .load_store_queue
                    .iter()
                    .position(|entry| entry.sequence() == sequence)
                else {
                    return false;
                };
                self.snapshot.reorder_buffer[rob_index].mark_ready_at(response_tick);
                self.snapshot.load_store_queue[lsq_index].mark_completed();
                O3LiveScalarMemoryOutcome::Completed
            }
            Some(RiscvDataAccessEventKind::Retry) => O3LiveScalarMemoryOutcome::Retried,
            Some(RiscvDataAccessEventKind::Failed) => O3LiveScalarMemoryOutcome::Failed,
            Some(
                RiscvDataAccessEventKind::Issued | RiscvDataAccessEventKind::ConditionalFailed,
            )
            | None => return false,
        };

        let live = &mut self.live_scalar_memories[index];
        live.execution = execution.clone();
        live.response_tick = Some(response_tick);
        live.latency_ticks = Some(latency_ticks);
        live.commit_tick = None;
        live.load_data = load_data.map(ToOwned::to_owned);
        live.forwarded = forwarded;
        live.outcome = outcome;
        live.event_taken = false;
        let remove_rows = matches!(
            outcome,
            O3LiveScalarMemoryOutcome::Retried | O3LiveScalarMemoryOutcome::Failed
        );
        if remove_rows {
            for stale in self.live_scalar_memories.iter().skip(index + 1) {
                self.data_access_sequences.remove(&stale.fetch_request);
                self.trace_data_access_sequences
                    .remove(&stale.fetch_request);
            }
            self.live_scalar_memories.truncate(index + 1);
            self.discard_live_scalar_memory_window_rows(sequence);
        }
        true
    }

    pub(crate) fn younger_live_scalar_memory_requests(
        &self,
        fetch_request: MemoryRequestId,
        data_request: MemoryRequestId,
    ) -> Vec<(MemoryRequestId, MemoryRequestId)> {
        let Some(index) = self.live_scalar_memories.iter().position(|live| {
            live.fetch_request == fetch_request && live.data_request == data_request
        }) else {
            return Vec::new();
        };
        self.live_scalar_memories
            .iter()
            .skip(index + 1)
            .map(|live| (live.data_request, live.fetch_request))
            .collect()
    }

    pub(crate) fn take_ready_live_scalar_memory_event(&mut self) -> Option<RiscvCpuExecutionEvent> {
        let live = self.live_scalar_memories.first_mut()?;
        if live.outcome == O3LiveScalarMemoryOutcome::Resident || live.event_taken {
            return None;
        }
        let event = live.execution.clone();
        if live.outcome == O3LiveScalarMemoryOutcome::Completed {
            let response_tick = live
                .response_tick
                .expect("completed live scalar memory has a response tick");
            live.commit_tick = Some(
                response_tick.max(self.last_scalar_memory_commit_tick.unwrap_or(response_tick)),
            );
        }
        live.event_taken = true;
        Some(event)
    }

    pub(super) fn consume_live_scalar_memory_retirement(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
    ) -> Option<O3LiveScalarMemory> {
        let live = self.live_scalar_memories.first()?;
        if live.fetch_request != execution.fetch().request_id()
            || live.execution != *execution
            || !live.event_taken
            || live.outcome == O3LiveScalarMemoryOutcome::Resident
        {
            return None;
        }
        let fetch_request = live.fetch_request;
        self.data_access_sequences.remove(&fetch_request);
        self.trace_data_access_sequences.remove(&fetch_request);
        let live = self.live_scalar_memories.remove(0);
        if live.outcome == O3LiveScalarMemoryOutcome::Completed {
            self.last_scalar_memory_commit_tick = live.commit_tick;
        }
        Some(live)
    }

    pub(crate) fn ready_live_scalar_load_writeback(&self) -> Option<(MemoryAccessKind, Vec<u8>)> {
        let live = self.live_scalar_memories.first()?;
        if live.outcome != O3LiveScalarMemoryOutcome::Completed || live.event_taken {
            return None;
        }
        let access = live.execution.execution().memory_access()?.clone();
        if !matches!(access, MemoryAccessKind::Load { .. }) {
            return None;
        }
        Some((access, live.load_data.clone()?))
    }

    pub(crate) fn discard_live_scalar_memory_lifecycle(&mut self) {
        self.deferred_scalar_memory_execution = None;
        let live = std::mem::take(&mut self.live_scalar_memories);
        for live in &live {
            self.data_access_sequences.remove(&live.fetch_request);
            self.trace_data_access_sequences.remove(&live.fetch_request);
        }
        let boundary_sequence = live
            .first()
            .map(|live| live.sequence)
            .or_else(|| self.live_scalar_memory_younger_sequences.first().copied());
        if let Some(sequence) = boundary_sequence {
            self.discard_live_scalar_memory_window_rows(sequence);
        }
    }

    fn discard_live_scalar_memory_window_rows(&mut self, sequence: u64) {
        self.discard_live_staged_window_from(sequence);
        self.snapshot
            .load_store_queue
            .retain(|entry| entry.sequence() < sequence);
    }

    pub(super) fn remove_live_scalar_memory_rows(&mut self, sequence: u64) {
        self.snapshot
            .reorder_buffer
            .retain(|entry| entry.sequence() != sequence);
        self.snapshot
            .load_store_queue
            .retain(|entry| entry.sequence() != sequence);
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
    }
}

impl crate::RiscvCore {
    pub fn o3_scalar_memory_lifecycle_is_quiescent(&self) -> bool {
        self.with_o3_runtime(|runtime| runtime.scalar_memory_lifecycle_is_quiescent())
    }

    pub fn has_pending_o3_scalar_memory_retirement(&self) -> bool {
        self.with_o3_runtime(|runtime| runtime.has_pending_scalar_memory_retirement())
    }

    pub fn pending_o3_scalar_memory_retirement_count(&self) -> usize {
        self.with_o3_runtime(|runtime| runtime.pending_scalar_memory_retirement_count())
    }

    pub fn owns_pending_o3_scalar_memory_retirement(&self, fetch_request: MemoryRequestId) -> bool {
        self.with_o3_runtime(|runtime| runtime.owns_pending_scalar_memory_retirement(fetch_request))
    }

    pub fn ready_o3_scalar_memory_event_kind(&self) -> Option<RiscvDataAccessEventKind> {
        self.with_o3_runtime(|runtime| runtime.ready_live_scalar_memory_event_kind())
    }

    pub(crate) fn clear_deferred_o3_scalar_memory_execution(&self) -> bool {
        self.with_o3_runtime(O3RuntimeState::clear_deferred_scalar_memory_execution)
    }
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord, RiscvInstruction,
    };
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId, CacheLineLayout};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::{
        CpuCore, CpuFetchConfig, CpuFetchEvent, CpuFetchRecord, CpuId, CpuResetState, RiscvCore,
    };

    #[test]
    fn scalar_load_issue_allocates_same_sequence_rob_and_lsq_rows() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);

        assert!(runtime.stage_live_scalar_memory_issue(&execution, memory_request(20), 31));

        let snapshot = runtime.snapshot();
        assert_eq!(snapshot.reorder_buffer().len(), 1);
        assert_eq!(snapshot.load_store_queue().len(), 1);
        let rob = snapshot.reorder_buffer()[0];
        let lsq = snapshot.load_store_queue()[0];
        assert_eq!(rob.sequence(), lsq.sequence());
        assert!(!rob.is_ready());
        assert!(!lsq.is_completed());
        assert_eq!(runtime.stats().max_rob_occupancy(), 1);
        assert_eq!(runtime.stats().max_lsq_occupancy(), 1);
        assert_eq!(runtime.live_scalar_memories.first().unwrap().issue_tick, 31);
    }

    #[test]
    fn scalar_store_issue_records_real_issue_tick_and_single_occupancy() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_store_event(0x8004, 11);

        assert!(runtime.stage_live_scalar_memory_issue(&execution, memory_request(21), 37));

        let live = runtime.live_scalar_memories.first().unwrap();
        assert_eq!(live.fetch_request, execution.fetch().request_id());
        assert_eq!(live.data_request, memory_request(21));
        assert_eq!(live.issue_tick, 37);
        assert_eq!(live.issue_rob_occupancy, 1);
        assert_eq!(live.issue_lsq_occupancy, 1);
        assert_eq!(live.outcome, O3LiveScalarMemoryOutcome::Resident);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
    }

    #[test]
    fn excluded_memory_kinds_do_not_stage_live_scalar_rows() {
        let mut runtime = O3RuntimeState::default();
        let execution = store_conditional_event(0x8008, 12);

        assert!(!runtime.stage_live_scalar_memory_issue(&execution, memory_request(22), 41));
        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert!(runtime.live_scalar_memories.is_empty());
    }

    #[test]
    fn completed_response_marks_only_matching_rows_ready() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        let data_request = memory_request(20);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
        let live_sequence = runtime.live_scalar_memories.first().unwrap().sequence;
        let unrelated_sequence = runtime.allocate_sequence();
        runtime.snapshot.reorder_buffer.insert(
            0,
            O3ReorderBufferEntry::new(unrelated_sequence, Address::new(0x7ffc), None),
        );
        runtime.snapshot.load_store_queue.insert(
            0,
            O3LoadStoreQueueEntry::store(unrelated_sequence, Some(Address::new(0xa000)), 4),
        );
        let mut completed = execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);

        assert!(runtime.complete_live_scalar_memory_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ));

        let snapshot = runtime.snapshot();
        let unrelated_rob = snapshot
            .reorder_buffer()
            .iter()
            .find(|entry| entry.sequence() == unrelated_sequence)
            .unwrap();
        let live_rob = snapshot
            .reorder_buffer()
            .iter()
            .find(|entry| entry.sequence() == live_sequence)
            .unwrap();
        let unrelated_lsq = snapshot
            .load_store_queue()
            .iter()
            .find(|entry| entry.sequence() == unrelated_sequence)
            .unwrap();
        let live_lsq = snapshot
            .load_store_queue()
            .iter()
            .find(|entry| entry.sequence() == live_sequence)
            .unwrap();
        assert!(!unrelated_rob.is_ready());
        assert!(!unrelated_lsq.is_completed());
        assert!(live_rob.is_ready());
        assert_eq!(live_rob.ready_tick(), 41);
        assert!(live_lsq.is_completed());
        let live = runtime.live_scalar_memories.first().unwrap();
        assert_eq!(live.outcome, O3LiveScalarMemoryOutcome::Completed);
        assert_eq!(live.response_tick, Some(41));
        assert_eq!(live.latency_ticks, Some(10));
        assert_eq!(live.load_data.as_deref(), Some(&[0x2a, 0, 0, 0][..]));
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(completed)
        );
        assert!(runtime.take_ready_live_scalar_memory_event().is_none());
    }

    #[test]
    fn two_live_scalar_loads_complete_out_of_order_and_retire_in_order() {
        let mut runtime = O3RuntimeState::default();
        let older = scalar_load_event_with(0x8000, 10, 12, 10, 0x9000);
        let younger = scalar_load_event_with(0x8004, 11, 13, 10, 0x9040);
        let older_data_request = memory_request(20);
        let younger_data_request = memory_request(21);

        assert!(runtime.stage_live_scalar_memory_issue(&older, older_data_request, 31));
        assert!(runtime.stage_live_scalar_memory_issue(&younger, younger_data_request, 32));

        let sequences = runtime
            .snapshot()
            .reorder_buffer()
            .iter()
            .map(|entry| entry.sequence())
            .collect::<Vec<_>>();
        assert_eq!(sequences.len(), 2);
        let mut younger_completed = younger.clone();
        younger_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &younger_completed,
            younger_data_request,
            40,
            8,
            Some(&[0x63, 0, 0, 0]),
        ));

        let snapshot = runtime.snapshot();
        assert!(!snapshot.reorder_buffer()[0].is_ready());
        assert!(snapshot.reorder_buffer()[1].is_ready());
        assert!(!snapshot.load_store_queue()[0].is_completed());
        assert!(snapshot.load_store_queue()[1].is_completed());
        assert!(runtime.take_ready_live_scalar_memory_event().is_none());

        let mut older_completed = older.clone();
        older_completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &older_completed,
            older_data_request,
            45,
            14,
            Some(&[0x2a, 0, 0, 0]),
        ));

        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(older_completed.clone())
        );
        runtime.record_retired_instruction_with_trace(&older_completed, true);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(younger_completed.clone())
        );
        runtime.record_retired_instruction_with_trace(&younger_completed, true);

        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        let trace = runtime.trace_records();
        assert_eq!(trace.len(), 2);
        assert_eq!(trace[0].sequence(), sequences[0]);
        assert_eq!(trace[1].sequence(), sequences[1]);
        assert_eq!(trace[0].lsq_data_response_tick(), 45);
        assert_eq!(trace[1].lsq_data_response_tick(), 40);
        assert_eq!(trace[0].commit_tick(), 45);
        assert_eq!(trace[1].commit_tick(), 45);
    }

    #[test]
    fn retry_response_removes_load_head_younger_rows_and_readies_one_abort_event() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8004, 11);
        let data_request = memory_request(21);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 37));
        stage_independent_younger(&mut runtime, &execution);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        let mut retry = execution.clone();
        retry.set_data_access_event_kind(RiscvDataAccessEventKind::Retry);

        assert!(runtime.complete_live_scalar_memory_response(&retry, data_request, 44, 7, None,));

        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert_eq!(
            runtime.live_scalar_memories.first().unwrap().outcome,
            O3LiveScalarMemoryOutcome::Retried
        );
        assert_eq!(runtime.take_ready_live_scalar_memory_event(), Some(retry));
        assert!(runtime.take_ready_live_scalar_memory_event().is_none());
    }

    #[test]
    fn failed_response_drains_rows_and_never_counts_o3_retirement() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        let data_request = memory_request(20);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
        stage_independent_younger(&mut runtime, &execution);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        let mut failed = execution.clone();
        failed.set_data_access_event_kind(RiscvDataAccessEventKind::Failed);

        assert!(runtime.complete_live_scalar_memory_response(&failed, data_request, 43, 12, None,));

        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert_eq!(
            runtime.live_scalar_memories.first().unwrap().outcome,
            O3LiveScalarMemoryOutcome::Failed
        );
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(failed.clone())
        );
        assert!(runtime.take_ready_live_scalar_memory_event().is_none());

        runtime.record_retired_instruction_with_trace(&failed, true);

        assert!(runtime.live_scalar_memories.is_empty());
        assert_eq!(runtime.stats().instructions(), 0);
        assert_eq!(runtime.stats().lsq_loads(), 0);
        assert!(runtime.trace_records().is_empty());
    }

    #[test]
    fn pending_retirement_tracks_deferred_and_live_scalar_memory() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        assert!(!runtime.has_pending_scalar_memory_retirement());
        assert!(runtime.defer_scalar_memory_execution(&execution));
        assert!(runtime.has_pending_scalar_memory_retirement());
        assert!(runtime.stage_live_scalar_memory_issue(&execution, memory_request(20), 31));
        assert!(runtime.has_pending_scalar_memory_retirement());
        runtime.discard_live_scalar_memory_lifecycle();
        assert!(!runtime.has_pending_scalar_memory_retirement());
    }

    #[test]
    fn stats_reset_preserves_live_rows_and_request_identity() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_store_event(0x8004, 11);
        let data_request = memory_request(21);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 37));

        runtime.reset_stats();

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
        let live = runtime.live_scalar_memories.first().unwrap();
        assert_eq!(live.fetch_request, execution.fetch().request_id());
        assert_eq!(live.data_request, data_request);
        assert_eq!(runtime.stats().max_rob_occupancy(), 1);
        assert_eq!(runtime.stats().max_lsq_occupancy(), 1);
    }

    #[test]
    fn stats_reset_preserves_completed_scalar_younger_window_provenance() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        let data_request = memory_request(20);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
        stage_independent_younger(&mut runtime, &execution);
        let mut completed = execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ));
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(completed.clone())
        );
        runtime.record_retired_instruction_with_trace(&completed, true);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);

        runtime.reset_stats();

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert!(!runtime.scalar_memory_lifecycle_is_quiescent());
        assert_eq!(runtime.stats().max_rob_occupancy(), 1);
        assert_eq!(runtime.stats().max_lsq_occupancy(), 0);
    }

    #[test]
    fn completed_retirement_uses_issue_and_response_ticks_then_drains_rows() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        let data_request = memory_request(20);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
        let sequence = runtime.snapshot().reorder_buffer()[0].sequence();
        let destination = runtime.snapshot().reorder_buffer()[0].destination();
        let mut completed = execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ));
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(completed.clone())
        );

        runtime.record_retired_instruction_with_trace(&completed, true);

        assert!(runtime.live_scalar_memories.is_empty());
        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert_eq!(runtime.stats().instructions(), 1);
        assert_eq!(runtime.stats().lsq_loads(), 1);
        assert_eq!(runtime.stats().lsq_data_latency_samples(), 1);
        assert_eq!(runtime.stats().lsq_data_latency_ticks(), 10);
        assert_eq!(runtime.stats().max_rob_occupancy(), 1);
        assert_eq!(runtime.stats().max_lsq_occupancy(), 1);
        let trace = runtime.trace_records()[0];
        assert_eq!(trace.sequence(), sequence);
        assert_eq!(trace.issue_tick(), 31);
        assert_eq!(trace.writeback_tick(), 41);
        assert_eq!(trace.commit_tick(), 41);
        assert_eq!(trace.rob_occupancy(), 1);
        assert_eq!(trace.lsq_occupancy(), 1);
        assert_eq!(trace.lsq_data_response_tick(), 41);
        assert_eq!(trace.lsq_data_latency_ticks(), 10);
        assert!(runtime.snapshot().rename_map().iter().any(|entry| {
            entry.register_class() == O3RegisterClass::Integer
                && entry.architectural() == 12
                && Some(entry.physical()) == destination
        }));
    }

    #[test]
    fn completed_load_retirement_preserves_staged_younger_row() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        let data_request = memory_request(20);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
        stage_independent_younger(&mut runtime, &execution);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        let mut completed = execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ));
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(completed.clone())
        );

        runtime.record_retired_instruction_with_trace(&completed, true);

        assert!(runtime.live_scalar_memories.is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(
            runtime.snapshot().reorder_buffer()[0].pc(),
            Address::new(0x8004)
        );
        assert!(runtime.snapshot().reorder_buffer()[0].is_live_staged());
        assert_eq!(runtime.stats().max_rob_occupancy(), 2);
        assert!(!runtime.scalar_memory_lifecycle_is_quiescent());

        let instruction = RiscvInstruction::Addi {
            rd: reg(13),
            rs1: reg(0),
            imm: Immediate::new(7),
        };
        let younger = RiscvCpuExecutionEvent::new(
            fetch_event(0x8004, 11),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(reg(13), 7)],
                None,
            ),
        );
        runtime.retire_live_staged_instruction(&younger, &[younger.fetch().request_id()], 42);
        runtime.record_retired_instruction_with_trace(&younger, true);
        assert!(runtime.scalar_memory_lifecycle_is_quiescent());
    }

    #[test]
    fn cleanup_mode_disable_removes_completed_scalar_younger_window() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        let data_request = memory_request(20);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
        stage_independent_younger(&mut runtime, &execution);
        let mut completed = execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ));
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(completed.clone())
        );
        runtime.record_retired_instruction_with_trace(&completed, true);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        let core = core_with_runtime(runtime);
        core.set_detailed_live_retire_gate_enabled(true);
        assert!(!core.data_access_lifecycle_is_quiescent());

        core.set_detailed_live_retire_gate_enabled(false);

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
        assert!(state.o3_runtime.scalar_memory_lifecycle_is_quiescent());
    }

    #[test]
    fn retry_retirement_clears_lifecycle_without_counting_instruction() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_store_event(0x8004, 11);
        let data_request = memory_request(21);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 37));
        let mut retry = execution.clone();
        retry.set_data_access_event_kind(RiscvDataAccessEventKind::Retry);
        assert!(runtime.complete_live_scalar_memory_response(&retry, data_request, 44, 7, None,));
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(retry.clone())
        );

        runtime.record_retired_instruction_with_trace(&retry, true);

        assert!(runtime.live_scalar_memories.is_empty());
        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert_eq!(runtime.stats().instructions(), 0);
        assert_eq!(runtime.stats().lsq_stores(), 0);
        assert!(runtime.trace_records().is_empty());
    }

    #[test]
    fn cleanup_after_ready_event_prevents_stale_terminal_retirement() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        let data_request = memory_request(20);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, data_request, 31));
        let mut completed = execution.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime.complete_live_scalar_memory_response(
            &completed,
            data_request,
            41,
            10,
            Some(&[0x2a, 0, 0, 0]),
        ));
        assert_eq!(
            runtime.take_ready_live_scalar_memory_event(),
            Some(completed.clone())
        );

        runtime.discard_live_scalar_memory_lifecycle();
        runtime.record_retired_instruction_with_trace(&completed, true);

        assert_eq!(runtime.stats().instructions(), 0);
        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert!(runtime.trace_records().is_empty());
    }

    #[test]
    fn cleanup_discard_removes_resident_scalar_rows_and_identity() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, memory_request(20), 31));

        runtime.discard_live_staged_instructions();

        assert!(runtime.live_scalar_memories.is_empty());
        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
    }

    #[test]
    fn cleanup_pc_redirect_removes_resident_scalar_rows_and_identity() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, memory_request(20), 31));
        stage_independent_younger(&mut runtime, &execution);
        let core = core_with_runtime(runtime);

        core.redirect_pc(Address::new(0x9000));

        let state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.live_scalar_memories.is_empty());
        assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
        assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    }

    #[test]
    fn cleanup_hart_reset_removes_scalar_lifecycle_without_reissuing_stale_event() {
        let mut runtime = O3RuntimeState::default();
        let execution = scalar_load_event(0x8000, 10);
        assert!(runtime.stage_live_scalar_memory_issue(&execution, memory_request(20), 31));
        let core = core_with_runtime(runtime);
        {
            let mut state = core.state.lock().expect("riscv core lock");
            state.events.push(execution.clone());
            state
                .issued_data_for_fetches
                .insert(execution.fetch().request_id());
        }

        core.resume_nonretentive_supervisor_hart(Address::new(0x9000), 0);

        assert!(!core.has_unissued_data_access());
        let state = core.state.lock().expect("riscv core lock");
        assert!(state.o3_runtime.scalar_memory_lifecycle_is_quiescent());
        assert!(state.o3_runtime.snapshot().reorder_buffer().is_empty());
        assert!(state.o3_runtime.snapshot().load_store_queue().is_empty());
    }

    fn scalar_load_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
        scalar_load_event_with(pc, sequence, 12, 10, 0x9000)
    }

    fn scalar_load_event_with(
        pc: u64,
        sequence: u64,
        rd: u8,
        rs1: u8,
        address: u64,
    ) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Load {
            rd: reg(rd),
            rs1: reg(rs1),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
            signed: false,
        };
        let access = MemoryAccessKind::Load {
            rd: reg(rd),
            address,
            width: MemoryWidth::Word,
            signed: false,
        };
        execution_event(pc, sequence, instruction, access)
    }

    fn stage_independent_younger(runtime: &mut O3RuntimeState, execution: &RiscvCpuExecutionEvent) {
        runtime.stage_live_scalar_memory_younger_window(
            execution.fetch().request_id(),
            [(
                Address::new(execution.execution().next_pc()),
                RiscvInstruction::Addi {
                    rd: reg(13),
                    rs1: reg(0),
                    imm: Immediate::new(7),
                },
            )],
        );
    }

    fn scalar_store_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Store {
            rs1: reg(10),
            rs2: reg(11),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
        };
        let access = MemoryAccessKind::Store {
            address: 0x9000,
            width: MemoryWidth::Word,
            value: 0x2a,
        };
        execution_event(pc, sequence, instruction, access)
    }

    fn store_conditional_event(pc: u64, sequence: u64) -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::StoreConditional {
            rd: reg(7),
            rs1: reg(10),
            rs2: reg(11),
            width: MemoryWidth::Word,
            acquire: false,
            release: false,
        };
        let access = MemoryAccessKind::StoreConditional {
            rd: reg(7),
            address: 0x9000,
            width: MemoryWidth::Word,
            value: 0x2a,
            acquire: false,
            release: false,
        };
        execution_event(pc, sequence, instruction, access)
    }

    fn execution_event(
        pc: u64,
        sequence: u64,
        instruction: RiscvInstruction,
        access: MemoryAccessKind,
    ) -> RiscvCpuExecutionEvent {
        RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(instruction, pc, pc + 4, Vec::new(), Some(access)),
        )
    }

    fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
        CpuFetchEvent::completed(
            CpuFetchRecord::new(
                10 + sequence,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                memory_request(sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            0x0000_0073_u32.to_le_bytes().to_vec(),
        )
    }

    fn memory_request(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(7), sequence)
    }

    fn core_with_runtime(runtime: O3RuntimeState) -> RiscvCore {
        let core = RiscvCore::new(
            CpuCore::new(
                CpuResetState::new(
                    CpuId::new(0),
                    PartitionId::new(0),
                    AgentId::new(7),
                    Address::new(0x8000),
                ),
                CpuFetchConfig::new(
                    TransportEndpointId::new("cpu0.ifetch").unwrap(),
                    MemoryRouteId::new(0),
                    CacheLineLayout::new(16).unwrap(),
                    AccessSize::new(4).unwrap(),
                ),
            )
            .unwrap(),
        );
        core.state.lock().expect("riscv core lock").o3_runtime = runtime;
        core
    }

    fn reg(index: u8) -> Register {
        Register::new(index).unwrap()
    }
}
