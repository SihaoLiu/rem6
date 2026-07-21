use rem6_isa_riscv::{Register, RegisterWrite, RiscvExecutionRecord, RiscvInstruction};

use super::o3_runtime_live_window::staged_rename_entry;
use super::*;
use crate::o3_pipeline::O3IssueOpClass;
use crate::O3RuntimeFuLatencyClass;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueSchedulingCandidate {
    request_index: usize,
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    consumed_requests: Vec<MemoryRequestId>,
    kind: O3LiveSpeculativeIssueKind,
    op_class: O3IssueOpClass,
    control_dependency: Option<u64>,
    data_producers: Vec<O3LiveIssueSourceProducer>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveIssueSourceProducer {
    sequence: u64,
    source: Register,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveSpeculativeIssueCandidate {
    scheduling: O3LiveIssueSchedulingCandidate,
    producer_sequences: Vec<u64>,
    forwarded_register_writes: Vec<RegisterWrite>,
    forwarded_ready_tick: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveSpeculativeIssueKind {
    PendingDataAddress {
        destination: O3RenameMapEntry,
    },
    Scalar {
        destination: O3RenameMapEntry,
    },
    Control {
        kind: BranchTargetKind,
        destination: Option<O3RenameMapEntry>,
    },
}

impl O3LiveIssueSourceProducer {
    pub(crate) const fn sequence(self) -> u64 {
        self.sequence
    }

    pub(crate) const fn source(self) -> Register {
        self.source
    }
}

impl O3LiveIssueSchedulingCandidate {
    pub(crate) const fn request_index(&self) -> usize {
        self.request_index
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(crate) const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub(crate) const fn op_class(&self) -> O3IssueOpClass {
        self.op_class
    }

    pub(crate) const fn control_dependency(&self) -> Option<u64> {
        self.control_dependency
    }

    pub(crate) fn data_producers(&self) -> &[O3LiveIssueSourceProducer] {
        &self.data_producers
    }

    pub(crate) fn consumed_requests(&self) -> &[MemoryRequestId] {
        &self.consumed_requests
    }

    pub(crate) const fn is_pending_data_address(&self) -> bool {
        matches!(
            self.kind,
            O3LiveSpeculativeIssueKind::PendingDataAddress { .. }
        )
    }
}

impl O3LiveSpeculativeIssueCandidate {
    #[cfg(test)]
    pub(crate) const fn destination(&self) -> Option<O3RenameMapEntry> {
        match self.scheduling.kind {
            O3LiveSpeculativeIssueKind::PendingDataAddress { destination } => Some(destination),
            O3LiveSpeculativeIssueKind::Scalar { destination } => Some(destination),
            O3LiveSpeculativeIssueKind::Control { destination, .. } => destination,
        }
    }

    pub(crate) fn forwarded_register_writes(&self) -> &[RegisterWrite] {
        &self.forwarded_register_writes
    }

    pub(crate) const fn sequence(&self) -> u64 {
        self.scheduling.sequence
    }

    pub(crate) const fn request_index(&self) -> usize {
        self.scheduling.request_index
    }

    pub(crate) fn producer_sequences(&self) -> &[u64] {
        &self.producer_sequences
    }

    pub(crate) const fn is_pending_data_address(&self) -> bool {
        matches!(
            self.scheduling.kind,
            O3LiveSpeculativeIssueKind::PendingDataAddress { .. }
        )
    }

    pub(crate) const fn pending_data_address_destination(&self) -> Option<O3RenameMapEntry> {
        match self.scheduling.kind {
            O3LiveSpeculativeIssueKind::PendingDataAddress { destination } => Some(destination),
            O3LiveSpeculativeIssueKind::Scalar { .. }
            | O3LiveSpeculativeIssueKind::Control { .. } => None,
        }
    }

    pub(crate) fn data_producers(&self) -> &[O3LiveIssueSourceProducer] {
        &self.scheduling.data_producers
    }

    pub(crate) const fn instruction(&self) -> RiscvInstruction {
        self.scheduling.instruction
    }

    #[cfg(test)]
    pub(crate) const fn control_dependency(&self) -> Option<u64> {
        self.scheduling.control_dependency
    }

    pub(crate) fn issue_tick(&self, earliest_tick: u64) -> u64 {
        earliest_tick.max(self.forwarded_ready_tick)
    }
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

    #[cfg(test)]
    pub(crate) fn live_speculative_issue_candidate(
        &self,
        pc: Address,
        instruction: RiscvInstruction,
    ) -> Option<O3LiveSpeculativeIssueCandidate> {
        let scheduling = self.live_issue_scheduling_candidate_from_metadata(
            usize::MAX,
            pc,
            instruction,
            Vec::new(),
        )?;
        self.materialize_live_speculative_issue_candidate(&scheduling)
    }

    pub(crate) fn live_issue_scheduling_candidate(
        &self,
        request_index: usize,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<O3LiveIssueSchedulingCandidate> {
        let candidate = self.live_issue_scheduling_candidate_from_metadata(
            request_index,
            pc,
            instruction,
            consumed_requests.to_vec(),
        )?;
        (candidate.is_pending_data_address()
            || self.live_staged_fetch_identity_matches(
                candidate.sequence(),
                candidate.instruction(),
                consumed_requests,
            ))
        .then_some(candidate)
    }

    fn live_issue_scheduling_candidate_from_metadata(
        &self,
        request_index: usize,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: Vec<MemoryRequestId>,
    ) -> Option<O3LiveIssueSchedulingCandidate> {
        let Some((index, entry)) = self
            .snapshot
            .reorder_buffer
            .iter()
            .copied()
            .enumerate()
            .find(|(_, entry)| entry.is_live_staged() && entry.pc() == pc)
        else {
            return None;
        };
        if self
            .live_speculative_executions
            .iter()
            .any(|issued| issued.sequence == entry.sequence())
        {
            return None;
        }
        if let Some((destination, producer_register, producer_sequence)) = self
            .pending_data_address_candidate_metadata(
                entry.sequence(),
                pc,
                instruction,
                &consumed_requests,
            )
        {
            let expected_producer = O3LiveIssueSourceProducer {
                sequence: producer_sequence,
                source: producer_register,
            };
            let mut data_producers = self.live_issue_source_producers(index, &[producer_register]);
            if data_producers.is_empty()
                && self
                    .pending_data_address_committed_producer_ready_tick(
                        producer_sequence,
                        producer_register,
                    )
                    .is_some()
            {
                data_producers.push(expected_producer);
            }
            if data_producers.as_slice() != [expected_producer] {
                return None;
            }
            return Some(O3LiveIssueSchedulingCandidate {
                request_index,
                sequence: entry.sequence(),
                pc,
                instruction,
                consumed_requests,
                kind: O3LiveSpeculativeIssueKind::PendingDataAddress { destination },
                op_class: O3IssueOpClass::Memory,
                control_dependency: None,
                data_producers,
            });
        }
        if index == 0 || !self.live_staged_instruction_matches(entry.sequence(), instruction) {
            return None;
        }
        let (scalar_destination, control, sources) = if let Some((destination, sources)) =
            o3_predicted_scalar_descendant_operands(instruction)
        {
            (Some(destination), None, sources)
        } else {
            let control = o3_live_control_operands(instruction)?;
            let sources = control.sources().to_vec();
            (None, Some(control), sources)
        };
        let staged_destination = staged_rename_entry(entry);
        let kind = if let Some(destination) = scalar_destination {
            let Some(staged_destination) = staged_destination else {
                return None;
            };
            if staged_destination.register_class() != O3RegisterClass::Integer
                || staged_destination.architectural() != u32::from(destination.index())
            {
                return None;
            }
            O3LiveSpeculativeIssueKind::Scalar {
                destination: staged_destination,
            }
        } else {
            let control = control.expect("control candidate has control operands");
            let destination = match control.destination() {
                Some(destination) => {
                    let staged_destination = staged_destination?;
                    if staged_destination.register_class() != O3RegisterClass::Integer
                        || staged_destination.architectural() != u32::from(destination.index())
                    {
                        return None;
                    }
                    Some(staged_destination)
                }
                None if entry.destination().is_none() && entry.rename_destination().is_none() => {
                    None
                }
                None => return None,
            };
            O3LiveSpeculativeIssueKind::Control {
                kind: control.kind(),
                destination,
            }
        };

        let control_sequence = self.pending_control_sequence_for(entry.sequence());
        Some(O3LiveIssueSchedulingCandidate {
            request_index,
            sequence: entry.sequence(),
            pc,
            instruction,
            consumed_requests,
            kind,
            op_class: live_issue_op_class(instruction),
            control_dependency: control_sequence,
            data_producers: self.live_issue_source_producers(index, &sources),
        })
    }

    pub(crate) fn materialize_live_speculative_issue_candidate(
        &self,
        scheduling: &O3LiveIssueSchedulingCandidate,
    ) -> Option<O3LiveSpeculativeIssueCandidate> {
        let mut producer_sequences = Vec::new();
        let mut forwarded_register_writes = Vec::new();
        let mut forwarded_ready_tick = 0;
        for producer in scheduling.data_producers.iter().copied() {
            let (write, ready_tick) =
                match self.live_issue_source_value(producer.sequence(), producer.source()) {
                    Some((write, ready_tick)) => (Some(write), ready_tick),
                    None if matches!(
                        scheduling.kind,
                        O3LiveSpeculativeIssueKind::PendingDataAddress { .. }
                    ) =>
                    {
                        (
                            None,
                            self.pending_data_address_committed_producer_ready_tick(
                                producer.sequence(),
                                producer.source(),
                            )?,
                        )
                    }
                    None => return None,
                };
            if !producer_sequences.contains(&producer.sequence()) {
                producer_sequences.push(producer.sequence());
            }
            if let Some(write) = write {
                if !forwarded_register_writes
                    .iter()
                    .any(|forwarded: &RegisterWrite| forwarded.register() == producer.source())
                {
                    forwarded_register_writes.push(write);
                }
            }
            forwarded_ready_tick = forwarded_ready_tick.max(ready_tick);
        }
        if let Some(control_sequence) = scheduling.control_dependency {
            if !producer_sequences.contains(&control_sequence) {
                producer_sequences.push(control_sequence);
            }
        }
        Some(O3LiveSpeculativeIssueCandidate {
            scheduling: scheduling.clone(),
            producer_sequences,
            forwarded_register_writes,
            forwarded_ready_tick,
        })
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
        if !candidate.scheduling.consumed_requests.is_empty()
            && candidate.scheduling.consumed_requests != consumed_requests
        {
            return Ok(false);
        }
        if !self.live_staged_fetch_identity_matches(
            candidate.sequence(),
            candidate.instruction(),
            consumed_requests,
        ) || Address::new(execution.pc()) != candidate.scheduling.pc
            || execution.instruction() != candidate.instruction()
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
        {
            return Ok(false);
        }
        let valid_kind = match candidate.scheduling.kind {
            O3LiveSpeculativeIssueKind::PendingDataAddress { .. } => false,
            O3LiveSpeculativeIssueKind::Scalar { destination } => {
                execution.next_pc()
                    == execution
                        .pc()
                        .wrapping_add(u64::from(execution.instruction_bytes()))
                    && execution.register_writes().len() == 1
                    && execution_writes_rename_destination(&execution, destination)
            }
            O3LiveSpeculativeIssueKind::Control { kind, destination } => {
                o3_live_control_operands(execution.instruction()).is_some_and(|control| {
                    control.kind() == kind
                        && control_destination_matches_rename_entry(
                            control.destination(),
                            destination,
                        )
                }) && match destination {
                    Some(destination) => {
                        execution.register_writes().len() == 1
                            && execution_writes_rename_destination(&execution, destination)
                    }
                    None => execution.register_writes().is_empty(),
                }
            }
        };
        if !valid_kind {
            return Ok(false);
        }
        if !self.bind_live_staged_fetch_identity_at_sequence(
            candidate.sequence(),
            candidate.instruction(),
            consumed_requests,
        ) {
            return Ok(false);
        }
        let raw_ready_tick = issue_tick
            .checked_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                execution.instruction(),
            ))
            .ok_or(O3RuntimeError::WritebackTickOverflow { tick: issue_tick })?;
        let consumes_writeback_slot = matches!(
            candidate.scheduling.kind,
            O3LiveSpeculativeIssueKind::Scalar { .. }
                | O3LiveSpeculativeIssueKind::Control {
                    destination: Some(_),
                    ..
                }
        );
        let (admitted_writeback_tick, writeback_slot) = self.reserve_fixed_fu_writeback(
            candidate.sequence(),
            raw_ready_tick,
            consumes_writeback_slot,
        )?;

        self.live_speculative_executions
            .push(O3LiveSpeculativeExecution {
                consumed_requests: consumed_requests.to_vec(),
                sequence: candidate.sequence(),
                producer_sequences: candidate.producer_sequences,
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

    fn live_issue_source_producers(
        &self,
        consumer_index: usize,
        sources: &[Register],
    ) -> Vec<O3LiveIssueSourceProducer> {
        let mut producers = Vec::new();
        for source in sources.iter().copied().filter(|source| !source.is_zero()) {
            let producer = self.snapshot.reorder_buffer[..consumer_index]
                .iter()
                .rev()
                .copied()
                .find(|producer| {
                    producer.is_live_staged()
                        && producer.rename_destination()
                            == Some((O3RegisterClass::Integer, u32::from(source.index())))
                });
            if let Some(producer) = producer {
                let source_producer = O3LiveIssueSourceProducer {
                    sequence: producer.sequence(),
                    source,
                };
                if !producers.contains(&source_producer) {
                    producers.push(source_producer);
                }
            }
        }
        producers
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

pub(super) fn live_issue_op_class(instruction: RiscvInstruction) -> O3IssueOpClass {
    if o3_live_control_operands(instruction).is_some() {
        return O3IssueOpClass::Branch;
    }
    if matches!(
        o3_fu_latency_class(instruction),
        Some(O3RuntimeFuLatencyClass::ScalarIntegerMul | O3RuntimeFuLatencyClass::ScalarIntegerDiv)
    ) {
        O3IssueOpClass::IntMult
    } else {
        O3IssueOpClass::IntAlu
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

fn control_destination_matches_rename_entry(
    destination: Option<rem6_isa_riscv::Register>,
    rename: Option<O3RenameMapEntry>,
) -> bool {
    match (destination, rename) {
        (Some(destination), Some(rename)) => {
            rename.register_class() == O3RegisterClass::Integer
                && rename.architectural() == u32::from(destination.index())
        }
        (None, None) => true,
        (Some(_), None) | (None, Some(_)) => false,
    }
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
