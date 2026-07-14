use rem6_isa_riscv::{RegisterWrite, RiscvExecutionRecord, RiscvInstruction};

use super::o3_runtime_live_window::staged_rename_entry;
use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveSpeculativeExecution {
    pub(super) consumed_requests: Vec<MemoryRequestId>,
    pub(super) sequence: u64,
    pub(super) producer_sequences: Vec<u64>,
    pub(super) issue_tick: u64,
    pub(super) execution: RiscvExecutionRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct O3LiveSpeculativeIssueCandidate {
    sequence: u64,
    pc: Address,
    instruction: RiscvInstruction,
    kind: O3LiveSpeculativeIssueKind,
    producer_sequences: Vec<u64>,
    data_dependencies: Vec<O3LiveIssueDependency>,
    forwarded_register_writes: Vec<RegisterWrite>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum O3LiveSpeculativeIssueKind {
    Scalar { destination: O3RenameMapEntry },
    DirectConditional,
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
            O3LiveSpeculativeIssueKind::DirectConditional => None,
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
    pub(crate) fn live_speculative_issue_candidate(
        &self,
        pc: Address,
        instruction: RiscvInstruction,
    ) -> Option<O3LiveSpeculativeIssueCandidate> {
        let (destination, sources) = if let Some((destination, sources)) =
            o3_predicted_scalar_descendant_operands(instruction)
        {
            (Some(destination), sources)
        } else {
            (None, o3_direct_conditional_sources(instruction)?)
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
        if index == 0
            || self
                .live_speculative_executions
                .iter()
                .any(|issued| issued.sequence == entry.sequence())
        {
            return None;
        }
        let kind = match destination {
            Some(destination)
                if entry.rename_destination()
                    == Some((O3RegisterClass::Integer, u32::from(destination.index()))) =>
            {
                O3LiveSpeculativeIssueKind::Scalar {
                    destination: staged_rename_entry(entry)
                        .expect("eligible scalar execution has a rename destination"),
                }
            }
            None if entry.destination().is_none() && entry.rename_destination().is_none() => {
                O3LiveSpeculativeIssueKind::DirectConditional
            }
            Some(_) | None => return None,
        };

        let control_sequence = self
            .live_control_dependencies
            .get(&entry.sequence())
            .copied();
        let (mut producer_sequences, data_dependencies, forwarded_register_writes) =
            self.live_speculative_source_forwarding(index, &sources)?;
        if let Some(control_sequence) = control_sequence {
            producer_sequences.push(control_sequence);
        }
        Some(O3LiveSpeculativeIssueCandidate {
            sequence: entry.sequence(),
            pc,
            instruction,
            kind,
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
    ) {
        let issue_tick = candidate.issue_tick(issue_tick);
        if !valid_live_speculative_fetch_identity(consumed_requests)
            || Address::new(execution.pc()) != candidate.pc
            || execution.instruction() != candidate.instruction
            || execution.trap().is_some()
            || execution.system_event().is_some()
            || execution.memory_access().is_some()
            || !execution.float_register_writes().is_empty()
        {
            return;
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
            O3LiveSpeculativeIssueKind::DirectConditional => {
                execution.register_writes().is_empty()
                    && o3_direct_conditional_sources(execution.instruction()).is_some()
            }
        };
        if !valid_kind {
            return;
        }

        self.live_speculative_executions
            .push(O3LiveSpeculativeExecution {
                consumed_requests: consumed_requests.to_vec(),
                sequence: candidate.sequence,
                producer_sequences: candidate.producer_sequences,
                issue_tick,
                execution,
            });
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
        Some(
            issued
                .issue_tick
                .saturating_add(crate::riscv_fu_latency::riscv_execute_wait_cycles(
                    execution.instruction(),
                )),
        )
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
                        .map(|write| {
                            let ready_cycles = crate::riscv_fu_latency::riscv_execute_wait_cycles(
                                issued.execution.instruction(),
                            )
                            .max(1);
                            (write, issued.issue_tick.saturating_add(ready_cycles))
                        })
                });
            let (write, producer_ready_tick) = speculative
                .or_else(|| self.completed_live_scalar_load_source(producer.sequence(), source))?;
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

    fn completed_live_scalar_load_source(
        &self,
        sequence: u64,
        source: rem6_isa_riscv::Register,
    ) -> Option<(RegisterWrite, u64)> {
        let live = self.live_scalar_memories.iter().find(|live| {
            live.sequence == sequence
                && live.outcome == O3LiveScalarMemoryOutcome::Completed
                && !live.event_taken
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
            live.response_tick?.saturating_add(1),
        ))
    }

    pub(super) fn take_live_speculative_issue_tick(
        &mut self,
        entry: O3ReorderBufferEntry,
        execution: &RiscvCpuExecutionEvent,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<u64> {
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
            Some(issued.issue_tick)
        } else {
            self.invalidate_live_speculative_execution_chain(entry.sequence());
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
    }

    pub(crate) fn has_live_control_descendants(&self, branch_sequence: u64) -> bool {
        self.live_control_dependencies
            .values()
            .any(|control| *control == branch_sequence)
    }

    pub(crate) fn has_live_control_window(&self) -> bool {
        !self.live_control_window_sequences.is_empty()
    }

    pub(crate) fn discard_live_control_descendants_from(&mut self, branch_sequence: u64) {
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged() || entry.sequence() <= branch_sequence);
        self.live_scalar_memory_younger_sequences
            .retain(|sequence| *sequence <= branch_sequence);
        self.live_speculative_executions
            .retain(|execution| execution.sequence <= branch_sequence);
        self.live_control_dependencies
            .retain(|sequence, _| *sequence <= branch_sequence);
        self.live_control_window_sequences
            .retain(|sequence| *sequence <= branch_sequence);
        self.live_retired_instructions
            .retain(|instruction| instruction.sequence <= branch_sequence);
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
    }

    fn invalidate_live_speculative_execution_chain(&mut self, sequence: u64) {
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
