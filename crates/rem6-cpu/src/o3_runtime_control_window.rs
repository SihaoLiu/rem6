use rem6_isa_riscv::{RegisterWrite, RiscvExecutionRecord, RiscvInstruction};

use super::o3_runtime_live_window::staged_rename_entry;
use super::*;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct O3ProducerForwardedControlTarget {
    data_access_fetch_request: MemoryRequestId,
    fetch_request: MemoryRequestId,
    last_fetch_request: MemoryRequestId,
    pc: Address,
    sequential_pc: Address,
    instruction: RiscvInstruction,
    consumer_sequence: u64,
    producer_sequence: u64,
    ready_tick: u64,
    source: rem6_isa_riscv::Register,
    target: Address,
}

impl O3ProducerForwardedControlTarget {
    pub(super) fn same_control_identity(self, other: Self) -> bool {
        self.data_access_fetch_request == other.data_access_fetch_request
            && self.fetch_request == other.fetch_request
            && self.last_fetch_request == other.last_fetch_request
            && self.pc == other.pc
            && self.sequential_pc == other.sequential_pc
            && self.instruction == other.instruction
            && self.consumer_sequence == other.consumer_sequence
            && self.producer_sequence == other.producer_sequence
            && self.source == other.source
            && self.target == other.target
    }

    pub(crate) const fn data_access_fetch_request(self) -> MemoryRequestId {
        self.data_access_fetch_request
    }

    pub(crate) const fn fetch_request(self) -> MemoryRequestId {
        self.fetch_request
    }

    pub(crate) const fn last_fetch_request(self) -> MemoryRequestId {
        self.last_fetch_request
    }

    pub(crate) const fn pc(self) -> Address {
        self.pc
    }

    pub(crate) const fn sequential_pc(self) -> Address {
        self.sequential_pc
    }

    pub(crate) const fn instruction(self) -> RiscvInstruction {
        self.instruction
    }

    pub(crate) const fn consumer_sequence(self) -> u64 {
        self.consumer_sequence
    }

    pub(crate) const fn producer_sequence(self) -> u64 {
        self.producer_sequence
    }

    pub(crate) const fn ready_tick(self) -> u64 {
        self.ready_tick
    }

    pub(crate) const fn source(self) -> rem6_isa_riscv::Register {
        self.source
    }

    pub(crate) const fn target(self) -> Address {
        self.target
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveSpeculativeIssueCandidate {
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    kind: O3LiveSpeculativeIssueKind,
    control_dependency: Option<u64>,
    producer_sequences: Vec<u64>,
    data_dependencies: Vec<O3LiveIssueDependency>,
    forwarded_register_writes: Vec<RegisterWrite>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveSpeculativeIssueKind {
    Scalar {
        destination: O3RenameMapEntry,
    },
    Control {
        kind: BranchTargetKind,
        destination: Option<O3RenameMapEntry>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3LiveIssueDependency {
    pub(super) sequence: u64,
    pub(super) ready_tick: u64,
}

impl O3LiveSpeculativeIssueCandidate {
    #[cfg(test)]
    pub(crate) const fn destination(&self) -> Option<O3RenameMapEntry> {
        match self.kind {
            O3LiveSpeculativeIssueKind::Scalar { destination } => Some(destination),
            O3LiveSpeculativeIssueKind::Control { destination, .. } => destination,
        }
    }

    pub(super) fn forwarded_register_writes(&self) -> &[RegisterWrite] {
        &self.forwarded_register_writes
    }

    pub(super) const fn sequence(&self) -> u64 {
        self.sequence
    }

    pub(super) const fn instruction(&self) -> RiscvInstruction {
        self.instruction
    }

    pub(super) const fn control_dependency(&self) -> Option<u64> {
        self.control_dependency
    }

    pub(super) fn data_dependencies(&self) -> &[O3LiveIssueDependency] {
        &self.data_dependencies
    }

    #[cfg(test)]
    pub(crate) fn producer_sequences(&self) -> &[u64] {
        &self.producer_sequences
    }

    pub(crate) fn issue_tick(&self, earliest_tick: u64) -> u64 {
        self.data_dependencies
            .iter()
            .fold(earliest_tick, |tick, producer| {
                tick.max(producer.ready_tick)
            })
    }
}

impl O3LiveIssueDependency {
    pub(super) const fn new(sequence: u64, ready_tick: u64) -> Self {
        Self {
            sequence,
            ready_tick,
        }
    }
}

impl O3RuntimeState {
    pub(crate) fn producer_forwarded_same_link_control_target(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        self.producer_forwarded_same_link_control_target_with_completed(false)
    }

    pub(crate) fn retained_producer_forwarded_same_link_control_target(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        let forwarded = self.producer_forwarded_same_link_control_target_with_completed(true)?;
        let recorded = self
            .live_staged_fetch_identities
            .get(&forwarded.consumer_sequence())?
            .producer_forwarded_same_link_target()?;
        recorded
            .same_control_identity(forwarded)
            .then_some(forwarded)
    }

    fn producer_forwarded_same_link_control_target_with_completed(
        &self,
        allow_completed: bool,
    ) -> Option<O3ProducerForwardedControlTarget> {
        if self.live_data_access_younger_sequences.len() != 2 {
            return None;
        }
        let mut younger_sequences = self.live_data_access_younger_sequences.iter().copied();
        let producer_sequence = younger_sequences.next()?;
        let consumer_sequence = younger_sequences.next()?;
        self.producer_forwarded_same_link_control_target_for_sequences(
            allow_completed,
            producer_sequence,
            consumer_sequence,
        )
    }

    pub(super) fn producer_forwarded_same_link_control_target_for_sequences(
        &self,
        allow_completed: bool,
        producer_sequence: u64,
        consumer_sequence: u64,
    ) -> Option<O3ProducerForwardedControlTarget> {
        if self.live_data_accesses.len() != 1 {
            return None;
        }
        let live_data_access = &self.live_data_accesses[0];
        let valid_outcome = match live_data_access.outcome {
            O3LiveDataAccessOutcome::Resident => true,
            O3LiveDataAccessOutcome::Completed => allow_completed,
            O3LiveDataAccessOutcome::Retried | O3LiveDataAccessOutcome::Failed => false,
        };
        if live_data_access.event_taken || !valid_outcome {
            return None;
        }
        self.producer_forwarded_same_link_control_target_from_rows(
            producer_sequence,
            consumer_sequence,
            live_data_access.fetch_request,
        )
    }

    pub(super) fn producer_forwarded_same_link_control_target_from_rows(
        &self,
        producer_sequence: u64,
        consumer_sequence: u64,
        data_access_fetch_request: MemoryRequestId,
    ) -> Option<O3ProducerForwardedControlTarget> {
        let producer = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == producer_sequence)?;
        let consumer = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.sequence() == consumer_sequence)?;
        if !self
            .live_control_window_sequences
            .contains(&consumer_sequence)
            || self
                .live_control_dependencies
                .contains_key(&consumer_sequence)
        {
            return None;
        }
        let consumer_execution = self
            .live_speculative_executions
            .iter()
            .find(|execution| execution.sequence == consumer_sequence)?;
        let control = o3_live_control_operands(consumer_execution.execution.instruction())?;
        let same_link_target = control.same_link_target()?;
        let rd = control.destination()?;
        let rs1 = same_link_target.source();
        if consumer_execution.producer_sequences.as_slice() != [producer_sequence]
            || producer.rename_destination()
                != Some((O3RegisterClass::Integer, u32::from(rs1.index())))
            || consumer.rename_destination()
                != Some((O3RegisterClass::Integer, u32::from(rd.index())))
        {
            return None;
        }
        let producer_execution = self
            .live_speculative_executions
            .iter()
            .find(|execution| execution.sequence == producer_sequence)?;
        let value = producer_execution
            .execution
            .register_writes()
            .iter()
            .find(|write| write.register() == rs1)?
            .value();
        let target = value.checked_add_signed(same_link_target.offset())? & !1;
        if target != consumer_execution.execution.next_pc()
            || Address::new(consumer_execution.execution.pc()) != consumer.pc()
        {
            return None;
        }
        let fetch_request = *consumer_execution.consumed_requests.first()?;
        let last_fetch_request = *consumer_execution.consumed_requests.last()?;
        Some(O3ProducerForwardedControlTarget {
            data_access_fetch_request,
            fetch_request,
            last_fetch_request,
            pc: consumer.pc(),
            sequential_pc: Address::new(
                consumer_execution
                    .execution
                    .pc()
                    .wrapping_add(u64::from(consumer_execution.execution.instruction_bytes())),
            ),
            instruction: consumer_execution.execution.instruction(),
            consumer_sequence,
            producer_sequence,
            ready_tick: producer_execution.admitted_writeback_tick,
            source: rs1,
            target: Address::new(target),
        })
    }

    pub(crate) fn record_producer_forwarded_same_link_control_target(
        &mut self,
        forwarded: O3ProducerForwardedControlTarget,
    ) -> bool {
        let Some(current) = self.producer_forwarded_same_link_control_target() else {
            return false;
        };
        if !forwarded.same_control_identity(current) {
            return false;
        }
        let Some(identity) = self
            .live_staged_fetch_identities
            .get_mut(&forwarded.consumer_sequence())
        else {
            return false;
        };
        identity.record_producer_forwarded_same_link_target(current);
        true
    }

    pub(crate) fn has_recorded_producer_forwarded_same_link_control_target(
        &self,
        consumer_sequence: u64,
    ) -> bool {
        self.live_staged_fetch_identities
            .get(&consumer_sequence)
            .and_then(O3LiveStagedFetchIdentity::producer_forwarded_same_link_target)
            .is_some()
    }

    pub(crate) fn live_speculative_issue_candidate(
        &self,
        pc: Address,
        instruction: RiscvInstruction,
    ) -> Option<O3LiveSpeculativeIssueCandidate> {
        let (scalar_destination, control, sources) = if let Some((destination, sources)) =
            o3_predicted_scalar_descendant_operands(instruction)
        {
            (Some(destination), None, sources)
        } else {
            let control = o3_live_control_operands(instruction)?;
            let sources = control.sources().to_vec();
            (None, Some(control), sources)
        };
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
        if !self.live_staged_instruction_matches(entry.sequence(), instruction)
            || index == 0
            || self
                .live_speculative_executions
                .iter()
                .any(|issued| issued.sequence == entry.sequence())
        {
            return None;
        }
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

        let control_sequence = self
            .live_control_dependencies
            .get(&entry.sequence())
            .copied();
        let (mut producer_sequences, data_dependencies, forwarded_register_writes) =
            self.live_speculative_source_forwarding(index, &sources)?;
        if let Some(control_sequence) = control_sequence {
            if !producer_sequences.contains(&control_sequence) {
                producer_sequences.push(control_sequence);
            }
        }
        Some(O3LiveSpeculativeIssueCandidate {
            sequence: entry.sequence(),
            pc,
            instruction,
            kind,
            control_dependency: control_sequence,
            producer_sequences,
            data_dependencies,
            forwarded_register_writes,
        })
    }

    pub(crate) fn record_live_speculative_execution(
        &mut self,
        candidate: O3LiveSpeculativeIssueCandidate,
        consumed_requests: &[MemoryRequestId],
        issue_tick: u64,
        execution: RiscvExecutionRecord,
    ) -> Result<bool, O3RuntimeError> {
        let issue_tick = candidate.issue_tick(issue_tick);
        if !self.live_staged_fetch_identity_matches(
            candidate.sequence,
            candidate.instruction,
            consumed_requests,
        ) || Address::new(execution.pc()) != candidate.pc
            || execution.instruction() != candidate.instruction
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
        {
            return Ok(false);
        }
        let valid_kind = match candidate.kind {
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
            candidate.sequence,
            candidate.instruction,
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
            candidate.kind,
            O3LiveSpeculativeIssueKind::Scalar { .. }
                | O3LiveSpeculativeIssueKind::Control {
                    destination: Some(_),
                    ..
                }
        );
        let (admitted_writeback_tick, writeback_slot) = self.reserve_fixed_fu_writeback(
            candidate.sequence,
            raw_ready_tick,
            consumes_writeback_slot,
        )?;

        self.live_speculative_executions
            .push(O3LiveSpeculativeExecution {
                consumed_requests: consumed_requests.to_vec(),
                sequence: candidate.sequence,
                producer_sequences: candidate.producer_sequences,
                issue_tick,
                raw_ready_tick,
                admitted_writeback_tick,
                writeback_slot,
                execution,
            });
        self.record_producer_forwarded_same_link_return_descendant();
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

    fn live_speculative_source_forwarding(
        &self,
        consumer_index: usize,
        sources: &[rem6_isa_riscv::Register],
    ) -> Option<(Vec<u64>, Vec<O3LiveIssueDependency>, Vec<RegisterWrite>)> {
        let mut producer_sequences = Vec::new();
        let mut data_dependencies = Vec::new();
        let mut forwarded_register_writes = Vec::new();
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
            let Some(producer) = producer else {
                continue;
            };
            let speculative = self
                .live_speculative_executions
                .iter()
                .find(|issued| issued.sequence == producer.sequence())
                .and_then(|issued| {
                    issued
                        .execution
                        .register_writes()
                        .iter()
                        .find(|write| write.register() == source)
                        .cloned()
                        .map(|write| (write, issued.admitted_writeback_tick))
                });
            let (write, producer_ready_tick) = speculative
                .or_else(|| self.completed_live_data_access_source(producer.sequence(), source))?;
            if !producer_sequences.contains(&producer.sequence()) {
                producer_sequences.push(producer.sequence());
            }
            if !data_dependencies
                .iter()
                .any(|dependency: &O3LiveIssueDependency| {
                    dependency.sequence == producer.sequence()
                })
            {
                data_dependencies.push(O3LiveIssueDependency::new(
                    producer.sequence(),
                    producer_ready_tick,
                ));
            }
            if !forwarded_register_writes
                .iter()
                .any(|forwarded: &RegisterWrite| forwarded.register() == source)
            {
                forwarded_register_writes.push(write);
            }
        }
        Some((
            producer_sequences,
            data_dependencies,
            forwarded_register_writes,
        ))
    }

    fn completed_live_data_access_source(
        &self,
        sequence: u64,
        source: rem6_isa_riscv::Register,
    ) -> Option<(RegisterWrite, u64)> {
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
        Some((
            RegisterWrite::new(source, writeback.value()),
            live.admitted_writeback_tick?,
        ))
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
        self.live_control_dependencies
            .retain(|_, control| *control != sequence);
        self.live_serializing_control_sequences.remove(&sequence);
    }

    pub(crate) fn has_live_control_descendants(&self, branch_sequence: u64) -> bool {
        self.live_control_dependencies
            .values()
            .any(|control| *control == branch_sequence)
    }

    pub(crate) fn has_live_control_window(&self) -> bool {
        !self.live_control_window_sequences.is_empty()
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
        self.live_control_dependencies
            .retain(|sequence, _| *sequence <= branch_sequence);
        self.live_control_window_sequences
            .retain(|sequence| *sequence <= branch_sequence);
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
        self.live_control_dependencies.retain(|dependent, control| {
            !invalidated.contains(dependent) && !invalidated.contains(control)
        });
        self.live_control_window_sequences
            .retain(|sequence| !invalidated.contains(sequence));
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
