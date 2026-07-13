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
    destination: O3RenameMapEntry,
    producer_sequences: Vec<u64>,
    forwarded_register_writes: Vec<RegisterWrite>,
    dependency_ready_tick: u64,
}

impl O3LiveSpeculativeIssueCandidate {
    pub(crate) fn forwarded_register_writes(&self) -> &[RegisterWrite] {
        &self.forwarded_register_writes
    }

    pub(crate) fn issue_tick(&self, earliest_tick: u64) -> u64 {
        earliest_tick.max(self.dependency_ready_tick)
    }
}

impl O3RuntimeState {
    pub(crate) fn live_speculative_issue_candidate(
        &self,
        pc: Address,
        instruction: RiscvInstruction,
    ) -> Option<O3LiveSpeculativeIssueCandidate> {
        let Some((destination, sources)) = o3_speculative_scalar_alu_operands(instruction) else {
            return None;
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
            || entry.rename_destination()
                != Some((O3RegisterClass::Integer, u32::from(destination.index())))
            || self
                .live_speculative_executions
                .iter()
                .any(|issued| issued.sequence == entry.sequence())
        {
            return None;
        }

        let (producer_sequences, forwarded_register_writes, dependency_ready_tick) =
            self.live_speculative_source_forwarding(index, &sources)?;
        Some(O3LiveSpeculativeIssueCandidate {
            sequence: entry.sequence(),
            pc,
            instruction,
            destination: staged_rename_entry(entry)
                .expect("eligible live speculative execution has a rename destination"),
            producer_sequences,
            forwarded_register_writes,
            dependency_ready_tick,
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
            || execution.next_pc()
                != execution
                    .pc()
                    .wrapping_add(u64::from(execution.instruction_bytes()))
        {
            return;
        }
        if execution.register_writes().len() != 1
            || !execution_writes_rename_destination(&execution, candidate.destination)
        {
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

    fn live_speculative_source_forwarding(
        &self,
        consumer_index: usize,
        sources: &[rem6_isa_riscv::Register],
    ) -> Option<(Vec<u64>, Vec<RegisterWrite>, u64)> {
        let mut producer_sequences = Vec::new();
        let mut forwarded_register_writes = Vec::new();
        let mut dependency_ready_tick = 0;
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
                        .map(|write| (write, issued.issue_tick.saturating_add(1)))
                });
            let (write, producer_ready_tick) = speculative
                .or_else(|| self.completed_live_scalar_load_source(producer.sequence(), source))?;
            dependency_ready_tick = dependency_ready_tick.max(producer_ready_tick);
            if !producer_sequences.contains(&producer.sequence()) {
                producer_sequences.push(producer.sequence());
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
            forwarded_register_writes,
            dependency_ready_tick,
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
    }

    fn invalidate_live_speculative_execution_chain(&mut self, sequence: u64) {
        let mut invalidated = vec![sequence];
        while let Some(producer) = invalidated.pop() {
            let mut index = 0;
            while index < self.live_speculative_executions.len() {
                if self.live_speculative_executions[index]
                    .producer_sequences
                    .contains(&producer)
                {
                    invalidated.push(self.live_speculative_executions.remove(index).sequence);
                } else {
                    index += 1;
                }
            }
        }
    }
}

fn valid_live_speculative_fetch_identity(consumed_requests: &[MemoryRequestId]) -> bool {
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
