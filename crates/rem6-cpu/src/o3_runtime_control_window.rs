use rem6_isa_riscv::{Register, RegisterWrite, RiscvExecutionRecord};

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3LiveControlLineage {
    control_sequence: Option<u64>,
    dependency_pending: bool,
}

impl O3LiveControlLineage {
    const fn root() -> Self {
        Self {
            control_sequence: None,
            dependency_pending: false,
        }
    }

    const fn pending(control_sequence: u64) -> Self {
        Self {
            control_sequence: Some(control_sequence),
            dependency_pending: true,
        }
    }

    pub(super) const fn control_sequence(self) -> Option<u64> {
        self.control_sequence
    }

    pub(super) const fn pending_control_sequence(self) -> Option<u64> {
        if self.dependency_pending {
            self.control_sequence()
        } else {
            None
        }
    }

    fn bind(&mut self, control_sequence: u64) {
        *self = Self::pending(control_sequence);
    }

    fn resolve(&mut self) {
        self.dependency_pending = false;
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveSpeculativeExecution {
    pub(super) consumed_requests: Vec<MemoryRequestId>,
    pub(super) sequence: u64,
    pub(super) producer_sequences: Vec<u64>,
    pub(super) issue_tick: u64,
    pub(super) raw_ready_tick: u64,
    pub(super) admitted_writeback_tick: u64,
    pub(super) writeback_slot: Option<usize>,
    pub(super) execution: RiscvExecutionRecord,
}

impl O3RuntimeState {
    pub(crate) fn recorded_producer_forwarded_control_speculation_matches(
        &self,
        fetch_sequence: u64,
        speculation: BranchSpeculationId,
    ) -> bool {
        self.live_staged_fetch_identities.values().any(|identity| {
            identity
                .producer_forwarded_control_target
                .is_some_and(|target| target.fetch_request().sequence() == fetch_sequence)
                && identity.producer_forwarded_control_speculation == Some(speculation)
        })
    }

    pub(crate) fn clear_recorded_producer_forwarded_control_speculation(
        &mut self,
        fetch_sequence: u64,
        speculation: BranchSpeculationId,
    ) {
        if let Some(identity) = self
            .live_staged_fetch_identities
            .values_mut()
            .find(|identity| {
                identity
                    .producer_forwarded_control_target
                    .is_some_and(|target| target.fetch_request().sequence() == fetch_sequence)
            })
        {
            if identity.producer_forwarded_control_speculation == Some(speculation) {
                identity.producer_forwarded_control_speculation = None;
            }
        }
    }

    pub(crate) fn record_live_speculative_execution(
        &mut self,
        candidate: O3LiveSpeculativeIssueCandidate,
        consumed_requests: &[MemoryRequestId],
        issue_tick: u64,
        execution: RiscvExecutionRecord,
    ) -> Result<bool, O3RuntimeError> {
        let issue_tick = if candidate.request_index() == usize::MAX {
            candidate.issue_tick(issue_tick)
        } else {
            issue_tick
        };
        if !candidate.recorded_consumed_requests_match(consumed_requests)
            || !self
                .live_staged_issue_packet(candidate.sequence())
                .is_some_and(|packet| packet.matches_execution(&execution, consumed_requests))
            || !candidate.valid_recorded_execution(&execution)
        {
            return Ok(false);
        }
        let raw_ready_tick = issue_tick
            .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                execution.instruction(),
            ))
            .ok_or(O3RuntimeError::WritebackTickOverflow { tick: issue_tick })?;
        let (admitted_writeback_tick, writeback_slot) = self.reserve_fixed_fu_writeback(
            candidate.sequence(),
            raw_ready_tick,
            candidate.consumes_writeback_slot(),
        )?;

        self.live_speculative_executions
            .push(O3LiveSpeculativeExecution {
                consumed_requests: consumed_requests.to_vec(),
                sequence: candidate.sequence(),
                producer_sequences: candidate.producer_sequences().to_vec(),
                issue_tick,
                raw_ready_tick,
                admitted_writeback_tick,
                writeback_slot,
                execution,
            });
        self.record_producer_forwarded_return_descendant();
        Ok(true)
    }

    pub(crate) fn live_speculative_execution_ready_tick(
        &self,
        consumed_requests: &[MemoryRequestId],
        execution: &RiscvExecutionRecord,
    ) -> Option<u64> {
        let issued = self.live_speculative_executions.iter().find(|issued| {
            issued.producer_sequences.is_empty()
                && issued.consumed_requests.as_slice() == consumed_requests
                && issued.execution == *execution
        })?;
        Some(issued.admitted_writeback_tick)
    }

    pub(super) fn live_issue_source_value(
        &self,
        sequence: u64,
        source: Register,
    ) -> Option<(RegisterWrite, u64)> {
        self.live_speculative_executions
            .iter()
            .find(|issued| issued.sequence == sequence)
            .and_then(|issued| {
                issued
                    .execution
                    .register_writes()
                    .iter()
                    .find(|write| write.register() == source)
                    .cloned()
                    .map(|write| (write, issued.admitted_writeback_tick))
            })
            .or_else(|| self.completed_live_data_access_source(sequence, source))
    }

    pub(super) fn completed_live_data_access_ready_tick(&self, sequence: u64) -> Option<u64> {
        let live = self.live_data_accesses.iter().find(|live| {
            live.sequence == sequence && live.outcome == O3LiveDataAccessOutcome::Completed
        })?;
        let rob = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.sequence() == sequence)?;
        if !rob.is_live_staged() || !rob.is_ready() {
            return None;
        }
        Some(
            self.memory_result_writeback_reservation(live.sequence)?
                .admitted_tick(),
        )
    }

    fn completed_live_data_access_source(
        &self,
        sequence: u64,
        source: Register,
    ) -> Option<(RegisterWrite, u64)> {
        let ready_tick = self.completed_live_data_access_ready_tick(sequence)?;
        let live = self.live_data_accesses.iter().find(|live| {
            live.sequence == sequence && live.outcome == O3LiveDataAccessOutcome::Completed
        })?;
        let data = live.load_data.as_deref()?;
        let writeback = live
            .execution
            .execution()
            .memory_access()?
            .read_response_writeback(data)
            .ok()??;
        if writeback.integer_register() != Some(source) {
            return None;
        }
        Some((RegisterWrite::new(source, writeback.value()), ready_tick))
    }

    pub(super) fn take_live_speculative_issue_timing_at(
        &mut self,
        entry: O3ReorderBufferEntry,
        execution: &RiscvCpuExecutionEvent,
        consumed_requests: &[MemoryRequestId],
        now: u64,
    ) -> Option<(u64, u64)> {
        let index = self
            .live_speculative_executions
            .iter()
            .position(|issued| issued.sequence == entry.sequence())?;
        let issued = self.live_speculative_executions.remove(index);
        let valid = issued.producer_sequences.is_empty()
            && issued.consumed_requests.as_slice() == consumed_requests
            && consumed_requests.first() == Some(&execution.fetch().request_id())
            && issued.execution == *execution.execution();
        if valid {
            self.validate_live_speculative_producer(entry.sequence());
            Some((issued.issue_tick, issued.admitted_writeback_tick))
        } else {
            self.discard_future_writeback_sequence(issued.sequence, now);
            self.invalidate_live_speculative_execution_chain_at(entry.sequence(), now);
            None
        }
    }

    pub(super) fn validate_live_speculative_producer(&mut self, sequence: u64) {
        for issued in &mut self.live_speculative_executions {
            issued
                .producer_sequences
                .retain(|producer| *producer != sequence);
        }
        for lineage in self.live_control_lineages.values_mut() {
            if lineage.pending_control_sequence() == Some(sequence) {
                lineage.resolve();
            }
        }
        self.live_serializing_control_sequences.remove(&sequence);
    }

    pub(crate) fn has_live_control_descendants(&self, branch_sequence: u64) -> bool {
        self.live_control_lineages
            .values()
            .copied()
            .any(|lineage| lineage.pending_control_sequence() == Some(branch_sequence))
    }

    pub(super) fn pending_control_sequence_for(&self, sequence: u64) -> Option<u64> {
        self.live_control_lineages
            .get(&sequence)
            .copied()
            .and_then(O3LiveControlLineage::pending_control_sequence)
    }

    #[cfg(test)]
    pub(crate) fn live_control_lineage_parent_for_test(&self, sequence: u64) -> Option<u64> {
        self.live_control_lineages
            .get(&sequence)
            .copied()
            .and_then(O3LiveControlLineage::control_sequence)
    }

    #[cfg(test)]
    pub(crate) fn pending_live_control_lineage_parent_for_test(
        &self,
        sequence: u64,
    ) -> Option<u64> {
        self.pending_control_sequence_for(sequence)
    }

    pub(super) fn record_live_control_sequence(&mut self, sequence: u64) {
        self.live_control_lineages
            .entry(sequence)
            .or_insert_with(O3LiveControlLineage::root);
    }

    pub(super) fn record_live_control_descendant(&mut self, sequence: u64, control_sequence: u64) {
        self.live_control_lineages
            .entry(sequence)
            .and_modify(|lineage| lineage.bind(control_sequence))
            .or_insert_with(|| O3LiveControlLineage::pending(control_sequence));
    }

    fn live_control_window_entry(&self, entry: &O3ReorderBufferEntry) -> bool {
        entry.is_live_staged() && self.live_control_lineages.contains_key(&entry.sequence())
    }

    pub(super) fn is_live_control_window_sequence(&self, sequence: u64) -> bool {
        self.snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.sequence() == sequence)
            .is_some_and(|entry| self.live_control_window_entry(entry))
    }

    pub(crate) fn has_live_control_window(&self) -> bool {
        self.snapshot
            .reorder_buffer
            .iter()
            .any(|entry| self.live_control_window_entry(entry))
    }

    pub(crate) fn discard_live_control_descendants_from_at(
        &mut self,
        branch_sequence: u64,
        now: u64,
    ) {
        if let Some(descendant_sequence) = branch_sequence.checked_add(1) {
            self.discard_live_writeback_from_sequence(descendant_sequence);
        }
        self.discard_live_control_descendant_rows_from_at(branch_sequence, now);
    }

    fn discard_live_control_descendant_rows_from_at(&mut self, branch_sequence: u64, now: u64) {
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged() || entry.sequence() <= branch_sequence);
        self.live_data_access_younger_sequences
            .retain(|sequence| *sequence <= branch_sequence);
        self.retain_live_speculative_executions_at(
            |execution| execution.sequence <= branch_sequence,
            now,
        );
        self.live_control_lineages
            .retain(|sequence, _| *sequence <= branch_sequence);
        self.live_serializing_control_sequences
            .retain(|sequence| *sequence <= branch_sequence);
        self.live_staged_fetch_identities
            .retain(|sequence, _| *sequence <= branch_sequence);
        self.invalidated_live_staged_fetch_identities
            .retain(|sequence, _| *sequence <= branch_sequence);
        self.live_retired_instructions
            .retain(|instruction| instruction.sequence <= branch_sequence);
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
    }

    fn invalidate_live_speculative_execution_chain_at(&mut self, sequence: u64, now: u64) {
        let mut invalidated = BTreeSet::from([sequence]);
        let mut pending = vec![sequence];
        while let Some(producer) = pending.pop() {
            let mut index = 0;
            while index < self.live_speculative_executions.len() {
                if self.live_speculative_executions[index]
                    .producer_sequences
                    .contains(&producer)
                {
                    let removed = self.live_speculative_executions.remove(index).sequence;
                    self.discard_future_writeback_sequence(removed, now);
                    if invalidated.insert(removed) {
                        pending.push(removed);
                    }
                } else {
                    index += 1;
                }
            }
        }
        self.live_control_lineages.retain(|dependent, lineage| {
            !invalidated.contains(dependent)
                && !lineage
                    .pending_control_sequence()
                    .is_some_and(|control| invalidated.contains(&control))
        });
        self.live_serializing_control_sequences
            .retain(|sequence| !invalidated.contains(sequence));
    }
}

pub(super) fn valid_live_speculative_fetch_identity(consumed_requests: &[MemoryRequestId]) -> bool {
    let Some(first) = consumed_requests.first() else {
        return false;
    };
    consumed_requests.len() <= 2
        && consumed_requests
            .iter()
            .all(|request| request.agent() == first.agent())
        && consumed_requests
            .windows(2)
            .all(|requests| requests[0].sequence() < requests[1].sequence())
}

pub(super) fn execution_writes_rename_destination(
    record: &rem6_isa_riscv::RiscvExecutionRecord,
    destination: O3RenameMapEntry,
) -> bool {
    if record.trap().is_some() || record.system_event().is_some() {
        return false;
    }
    let direct_write = match destination.register_class() {
        O3RegisterClass::Integer => record
            .register_writes()
            .iter()
            .any(|write| u32::from(write.register().index()) == destination.architectural()),
        O3RegisterClass::FloatingPoint => record
            .float_register_writes()
            .iter()
            .any(|write| u32::from(write.register().index()) == destination.architectural()),
        O3RegisterClass::Vector | O3RegisterClass::ConditionCode | O3RegisterClass::Misc => false,
    };
    direct_write
        || record.memory_access().is_some_and(|access| {
            o3_memory_destination_registers(access)
                .contains(&(destination.register_class(), destination.architectural()))
        })
}
