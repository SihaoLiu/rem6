use rem6_isa_riscv::{RegisterWrite, RiscvExecutionRecord, RiscvInstruction};

use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveRetiredInstruction {
    pub(super) request: MemoryRequestId,
    pub(super) sequence: u64,
    pub(super) issue_tick: u64,
    pub(super) commit_tick: u64,
    pub(super) rob_occupancy: usize,
    pub(super) rob_commits: usize,
    pub(super) rob_commit_blocked: bool,
    pub(super) iew_dependency_producers: u64,
    pub(super) iew_dependency_producer_registers: Vec<O3PhysicalRegisterId>,
    pub(super) iew_dependency_consumers: u64,
    pub(super) rename_destination: Option<O3RenameMapEntry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveSpeculativeExecution {
    consumed_requests: Vec<MemoryRequestId>,
    sequence: u64,
    producer_sequences: Vec<u64>,
    issue_tick: u64,
    execution: RiscvExecutionRecord,
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
    pub(crate) fn stage_live_retire_window(
        &mut self,
        current_pc: Address,
        current: RiscvInstruction,
        current_ready_tick: u64,
        younger: impl IntoIterator<Item = (Address, RiscvInstruction)>,
    ) {
        if is_deferred_o3_scalar_memory_instruction(current) {
            return;
        }
        self.stage_live_instruction(current_pc, current, current_ready_tick);
        for (pc, instruction) in younger {
            if is_deferred_o3_scalar_memory_instruction(instruction) {
                break;
            }
            self.stage_live_instruction(pc, instruction, 0);
        }
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
    }

    pub(crate) fn retire_live_staged_instruction(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        consumed_requests: &[MemoryRequestId],
        retire_tick: u64,
    ) {
        let pc = Address::new(execution.execution().pc());
        let Some(index) = self
            .snapshot
            .reorder_buffer
            .iter()
            .position(|entry| entry.is_live_staged() && entry.pc() == pc)
        else {
            return;
        };
        if is_deferred_o3_scalar_memory_access(execution.execution().memory_access()) {
            let boundary_sequence = self.snapshot.reorder_buffer[index].sequence();
            self.snapshot.reorder_buffer.drain(index..);
            self.live_speculative_executions
                .retain(|speculative| speculative.sequence < boundary_sequence);
            self.live_retired_instructions
                .retain(|instruction| instruction.request != execution.fetch().request_id());
            self.stats
                .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
            return;
        }
        let entry = self.snapshot.reorder_buffer[index];
        let speculative_issue_tick =
            self.take_live_speculative_issue_tick(entry, execution, consumed_requests);
        let dependencies = self.record_scalar_integer_dependencies(&execution.instruction());
        let rename_destination = staged_rename_entry(entry).filter(|destination| {
            execution_writes_rename_destination(execution.execution(), *destination)
        });
        if let Some(rename_destination) = rename_destination {
            self.publish_live_rename_entry(rename_destination);
        }

        self.snapshot.reorder_buffer[index].mark_ready_at(retire_tick);
        let rob_occupancy = self.snapshot.reorder_buffer.len();
        let (rob_commits, _) = rob_commit_boundary(&self.snapshot);
        let rob_commit_blocked = rob_commits <= index;
        let commit_tick = rob_commit_tick(&self.snapshot, rob_commits).unwrap_or(retire_tick);
        self.snapshot.reorder_buffer.drain(0..rob_commits);
        let rename_map_entries = self.snapshot_with_live_rename_map().rename_map.len();
        self.stats.set_rename_map_entries(rename_map_entries);

        let fu_latency_cycles =
            crate::riscv_execute::in_order_execute_wait_cycles(execution.instruction());
        self.live_retired_instructions
            .push(O3LiveRetiredInstruction {
                request: execution.fetch().request_id(),
                sequence: entry.sequence(),
                issue_tick: speculative_issue_tick
                    .unwrap_or_else(|| retire_tick.saturating_sub(fu_latency_cycles)),
                commit_tick,
                rob_occupancy,
                rob_commits,
                rob_commit_blocked,
                iew_dependency_producers: dependencies.newly_observed_producers,
                iew_dependency_producer_registers: dependencies.producer_physical_registers,
                iew_dependency_consumers: dependencies.consumers,
                rename_destination,
            });
    }

    pub(crate) fn discard_live_retire_window(&mut self) {
        self.discard_live_staged_instructions();
        self.live_retired_instructions.clear();
    }

    pub(crate) fn discard_live_staged_instructions(&mut self) {
        self.discard_live_scalar_memory_lifecycle();
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged());
        self.discard_live_speculative_executions();
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());
    }

    pub(crate) fn discard_live_speculative_executions(&mut self) {
        self.live_speculative_executions.clear();
    }

    pub(super) fn take_live_retired_instruction(
        &mut self,
        request: MemoryRequestId,
    ) -> Option<O3LiveRetiredInstruction> {
        let index = self
            .live_retired_instructions
            .iter()
            .position(|instruction| instruction.request == request)?;
        Some(self.live_retired_instructions.remove(index))
    }

    pub(super) fn snapshot_with_live_rename_map(&self) -> O3RuntimeSnapshot {
        let committed_rename_map = self.snapshot.rename_map.clone();
        let mut live_rename_map = committed_rename_map.clone();
        for entry in &self.snapshot.reorder_buffer {
            let Some(rename_entry) = staged_rename_entry(*entry) else {
                continue;
            };
            if let Some(existing) = live_rename_map.iter_mut().find(|existing| {
                existing.register_class() == rename_entry.register_class()
                    && existing.architectural() == rename_entry.architectural()
            }) {
                *existing = rename_entry;
            } else {
                live_rename_map.push(rename_entry);
            }
        }
        live_rename_map.sort_by_key(|entry| {
            (
                encode_register_class(entry.register_class()),
                entry.architectural(),
            )
        });
        let mut snapshot = O3RuntimeSnapshot::new(
            self.snapshot.reorder_buffer.iter().copied(),
            self.snapshot.load_store_queue.iter().copied(),
            committed_rename_map.clone(),
            self.snapshot.pending_state.clone(),
        )
        .expect("committed O3 runtime snapshot is internally consistent");
        snapshot.rename_map = live_rename_map;
        snapshot.with_committed_rename_map(committed_rename_map)
    }

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
            let issued = self
                .live_speculative_executions
                .iter()
                .find(|issued| issued.sequence == producer.sequence())?;
            let write = issued
                .execution
                .register_writes()
                .iter()
                .find(|write| write.register() == source)?;
            dependency_ready_tick = dependency_ready_tick.max(issued.issue_tick.saturating_add(1));
            if !producer_sequences.contains(&producer.sequence()) {
                producer_sequences.push(producer.sequence());
            }
            if !forwarded_register_writes
                .iter()
                .any(|forwarded: &RegisterWrite| forwarded.register() == source)
            {
                forwarded_register_writes.push(write.clone());
            }
        }
        Some((
            producer_sequences,
            forwarded_register_writes,
            dependency_ready_tick,
        ))
    }

    fn stage_live_instruction(
        &mut self,
        pc: Address,
        instruction: RiscvInstruction,
        ready_tick: u64,
    ) {
        if self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.is_live_staged() && entry.pc() == pc)
        {
            return;
        }
        let sequence = self.allocate_sequence();
        let rename_destination = o3_scalar_integer_destination(instruction)
            .filter(|register| !register.is_zero())
            .map(|register| (O3RegisterClass::Integer, u32::from(register.index())));
        let destination = rename_destination.map(|_| self.allocate_physical_register());
        self.snapshot.reorder_buffer.push(
            O3ReorderBufferEntry::new(sequence, pc, destination)
                .with_ready_tick(ready_tick)
                .with_live_staged_rename_destination(rename_destination),
        );
    }

    pub(super) fn publish_live_rename_entry(&mut self, entry: O3RenameMapEntry) {
        if let Some(existing) = self.snapshot.rename_map.iter_mut().find(|existing| {
            existing.register_class() == entry.register_class()
                && existing.architectural() == entry.architectural()
        }) {
            *existing = entry;
        } else {
            self.snapshot.rename_map.push(entry);
        }
        self.snapshot.rename_map.sort_by_key(|entry| {
            (
                encode_register_class(entry.register_class()),
                entry.architectural(),
            )
        });
    }

    fn take_live_speculative_issue_tick(
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

    fn validate_live_speculative_producer(&mut self, sequence: u64) {
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

pub(super) fn staged_rename_entry(entry: O3ReorderBufferEntry) -> Option<O3RenameMapEntry> {
    let (register_class, architectural) = entry.rename_destination()?;
    Some(O3RenameMapEntry::new(
        register_class,
        architectural,
        entry.destination()?,
    ))
}

fn execution_writes_rename_destination(
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

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord, RiscvTrap,
        RiscvTrapKind,
    };
    use rem6_kernel::PartitionId;
    use rem6_memory::{AccessSize, AgentId};
    use rem6_transport::{MemoryRouteId, TransportEndpointId};

    use super::*;
    use crate::{CpuFetchEvent, CpuFetchRecord};

    fn div_x3() -> RiscvInstruction {
        RiscvInstruction::Div {
            rd: Register::new(3).unwrap(),
            rs1: Register::new(1).unwrap(),
            rs2: Register::new(2).unwrap(),
        }
    }

    fn addi(rd: u8, rs1: u8) -> RiscvInstruction {
        RiscvInstruction::Addi {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
            imm: Immediate::new(1),
        }
    }

    fn load_x4() -> RiscvInstruction {
        RiscvInstruction::Load {
            rd: Register::new(4).unwrap(),
            rs1: Register::new(10).unwrap(),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
            signed: false,
        }
    }

    fn request(sequence: u64) -> MemoryRequestId {
        MemoryRequestId::new(AgentId::new(7), sequence)
    }

    #[test]
    fn scalar_memory_stops_live_retire_window_before_memory_and_younger_rows() {
        let mut runtime = O3RuntimeState::default();

        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            [
                (Address::new(0x8004), load_x4()),
                (Address::new(0x8008), addi(5, 4)),
            ],
        );

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 1);
        assert_eq!(
            runtime.snapshot().reorder_buffer()[0].pc(),
            Address::new(0x8000)
        );
        assert_eq!(integer_mapping(&runtime, 4), None);
        assert_eq!(integer_mapping(&runtime, 5), None);
    }

    fn retire_live(
        runtime: &mut O3RuntimeState,
        execution: &RiscvCpuExecutionEvent,
        retire_tick: u64,
    ) {
        runtime.retire_live_staged_instruction(
            execution,
            &[execution.fetch().request_id()],
            retire_tick,
        );
    }

    #[test]
    fn live_rename_overlay_rolls_back_to_committed_mapping_on_discard() {
        let mut runtime = O3RuntimeState::default();
        runtime.publish_live_rename_entry(O3RenameMapEntry::new(
            O3RegisterClass::Integer,
            3,
            O3PhysicalRegisterId::new(40),
        ));
        runtime.next_physical_register = 41;

        runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);

        assert_eq!(
            runtime.snapshot().rename_map()[0].physical(),
            O3PhysicalRegisterId::new(41)
        );
        runtime.discard_live_retire_window();
        assert_eq!(
            runtime.snapshot().rename_map()[0].physical(),
            O3PhysicalRegisterId::new(40)
        );
    }

    #[test]
    fn live_staged_rob_checkpoint_reconstructs_rename_overlay() {
        let mut runtime = O3RuntimeState::default();
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            [
                (Address::new(0x8004), addi(4, 0)),
                (Address::new(0x8008), addi(5, 4)),
            ],
        );
        let live_snapshot = runtime.snapshot();
        let encoded = runtime.checkpoint_payload().encode();

        let mut restored = O3RuntimeState::default();
        restored
            .restore_checkpoint_payload(O3RuntimeCheckpointPayload::decode(&encoded).unwrap())
            .unwrap();

        assert_eq!(restored.snapshot(), live_snapshot);
        assert!(restored.snapshot().reorder_buffer()[0].is_live_staged());
        assert!(restored.snapshot().reorder_buffer()[1].is_live_staged());
        assert!(restored.snapshot().reorder_buffer()[2].is_live_staged());

        let restored_destinations = restored
            .snapshot()
            .reorder_buffer()
            .iter()
            .filter_map(|entry| entry.destination())
            .collect::<BTreeSet<_>>();
        restored.stage_live_retire_window(Address::new(0x800c), addi(6, 0), 0, None);
        let next_destination = restored
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.pc() == Address::new(0x800c))
            .and_then(|entry| entry.destination())
            .unwrap();
        assert!(
            !restored_destinations.contains(&next_destination),
            "post-restore allocation must not reuse a live staged physical register"
        );
    }

    #[test]
    fn independent_live_staged_scalar_execution_keeps_early_issue_timing() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime.record_live_speculative_execution(
            candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = execution_event(younger_instruction, 0x8004, 2, 4);
        retire_live(&mut runtime, &younger, 30);
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 10);
        assert_eq!(trace.writeback_tick(), 10);
        assert_eq!(trace.commit_tick(), 30);
    }

    #[test]
    fn matching_split_fetch_identity_keeps_early_issue_timing() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime.record_live_speculative_execution(
            candidate,
            &[request(2), request(3)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = execution_event(younger_instruction, 0x8004, 2, 4);
        runtime.retire_live_staged_instruction(&younger, &[request(2), request(3)], 30);
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 10);
        assert_eq!(trace.writeback_tick(), 10);
        assert_eq!(trace.commit_tick(), 30);
    }

    #[test]
    fn dependent_live_staged_scalar_execution_cannot_issue_early() {
        let mut runtime = O3RuntimeState::default();
        let dependent = addi(4, 3);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), dependent)),
        );

        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .is_none());
        runtime.snapshot.reorder_buffer[0].mark_ready_at(20);
        assert!(
            runtime
                .live_speculative_issue_candidate(Address::new(0x8004), dependent)
                .is_none(),
            "a ready-but-uncommitted producer is still absent from the architectural hart clone"
        );
    }

    #[test]
    fn speculative_predecessor_wakes_and_forwards_to_dependent_younger() {
        let mut runtime = O3RuntimeState::default();
        let producer = addi(4, 0);
        let consumer = addi(5, 4);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), producer)),
        );
        runtime.stage_live_retire_window(Address::new(0x8008), consumer, 0, None);
        let producer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), producer)
            .unwrap();
        runtime.record_live_speculative_execution(
            producer_candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );

        let consumer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), consumer)
            .expect("speculative x4 write should wake the x5 consumer");

        assert_eq!(
            consumer_candidate.forwarded_register_writes(),
            &[RegisterWrite::new(Register::new(4).unwrap(), 1)]
        );
        assert_eq!(consumer_candidate.issue_tick(10), 11);
    }

    #[test]
    fn speculative_consumer_uses_nearest_live_producer() {
        let mut runtime = O3RuntimeState::default();
        let producer = addi(3, 0);
        let consumer = addi(5, 3);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), producer)),
        );
        runtime.stage_live_retire_window(Address::new(0x8008), consumer, 0, None);
        let producer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), producer)
            .unwrap();
        runtime.record_live_speculative_execution(
            producer_candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(3).unwrap(), 1)],
                None,
            ),
        );

        let consumer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), consumer)
            .expect("the nearer x3 writer should hide the older pending divide destination");

        assert_eq!(
            consumer_candidate.forwarded_register_writes(),
            &[RegisterWrite::new(Register::new(3).unwrap(), 1)]
        );
        assert_eq!(consumer_candidate.issue_tick(10), 11);
    }

    #[test]
    fn invalidated_speculative_producer_revokes_dependent_issue_timing() {
        let mut runtime = O3RuntimeState::default();
        let producer = addi(4, 0);
        let consumer = addi(5, 4);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            [
                (Address::new(0x8004), producer),
                (Address::new(0x8008), consumer),
            ],
        );
        let producer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), producer)
            .unwrap();
        runtime.record_live_speculative_execution(
            producer_candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                producer,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );
        let consumer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), consumer)
            .unwrap();
        runtime.record_live_speculative_execution(
            consumer_candidate,
            &[request(3)],
            10,
            RiscvExecutionRecord::new(
                consumer,
                0x8008,
                0x800c,
                vec![RegisterWrite::new(Register::new(5).unwrap(), 1)],
                None,
            ),
        );

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let producer = execution_event(producer, 0x8004, 2, 4);
        runtime.retire_live_staged_instruction(&producer, &[request(4)], 30);
        runtime.record_retired_instruction_with_trace(&producer, true);
        let consumer = execution_event(consumer, 0x8008, 3, 5);
        retire_live(&mut runtime, &consumer, 31);
        runtime.record_retired_instruction_with_trace(&consumer, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 31);
        assert_eq!(trace.commit_tick(), 31);
    }

    #[test]
    fn mismatched_live_speculative_record_does_not_claim_early_issue() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime.record_live_speculative_execution(
            candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = RiscvCpuExecutionEvent::new(
            fetch_event(0x8004, 2),
            younger_instruction,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 2)],
                None,
            ),
        );
        retire_live(&mut runtime, &younger, 30);
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 30);
        assert_eq!(trace.commit_tick(), 30);
    }

    #[test]
    fn mismatched_split_fetch_suffix_does_not_claim_early_issue() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime.record_live_speculative_execution(
            candidate,
            &[request(2), request(3)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = execution_event(younger_instruction, 0x8004, 2, 4);
        runtime.retire_live_staged_instruction(&younger, &[request(2), request(4)], 30);
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 30);
        assert_eq!(trace.commit_tick(), 30);
    }

    #[test]
    fn malformed_live_speculative_fetch_identity_does_not_occupy_candidate() {
        let younger_instruction = addi(4, 0);
        let malformed_identities = [
            Vec::new(),
            vec![request(2), request(2)],
            vec![request(3), request(2)],
            vec![request(2), MemoryRequestId::new(AgentId::new(8), 3)],
            vec![request(2), request(3), request(4)],
        ];

        for consumed_requests in malformed_identities {
            let mut runtime = O3RuntimeState::default();
            runtime.stage_live_retire_window(
                Address::new(0x8000),
                div_x3(),
                29,
                Some((Address::new(0x8004), younger_instruction)),
            );
            let candidate = runtime
                .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
                .unwrap();
            runtime.record_live_speculative_execution(
                candidate,
                &consumed_requests,
                10,
                RiscvExecutionRecord::new(
                    younger_instruction,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                    None,
                ),
            );

            assert!(runtime.live_speculative_executions.is_empty());
            assert!(runtime
                .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
                .is_some());
        }
    }

    #[test]
    fn live_speculative_execution_is_transient_across_discard_and_restore() {
        let mut runtime = O3RuntimeState::default();
        let younger_instruction = addi(4, 0);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            Some((Address::new(0x8004), younger_instruction)),
        );
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime.record_live_speculative_execution(
            candidate,
            &[request(2)],
            10,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );
        assert_eq!(runtime.live_speculative_executions.len(), 1);

        let checkpoint = runtime.checkpoint_payload();
        runtime.restore_checkpoint_payload(checkpoint).unwrap();
        assert!(runtime.live_speculative_executions.is_empty());
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();

        runtime.record_live_speculative_execution(
            candidate,
            &[request(3)],
            20,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );
        runtime.discard_live_speculative_executions();
        assert!(runtime.live_speculative_executions.is_empty());
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime.record_live_speculative_execution(
            candidate,
            &[request(4)],
            21,
            RiscvExecutionRecord::new(
                younger_instruction,
                0x8004,
                0x8008,
                vec![RegisterWrite::new(Register::new(4).unwrap(), 1)],
                None,
            ),
        );
        runtime.discard_live_staged_instructions();
        assert!(runtime.live_speculative_executions.is_empty());
    }

    #[test]
    fn public_snapshot_checkpoint_preserves_committed_rename_rollback() {
        let mut runtime = O3RuntimeState::default();
        runtime.publish_live_rename_entry(O3RenameMapEntry::new(
            O3RegisterClass::Integer,
            3,
            O3PhysicalRegisterId::new(40),
        ));
        runtime.next_physical_register = 41;
        runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);

        let payload = O3RuntimeCheckpointPayload::from_snapshot(runtime.snapshot()).unwrap();
        let mut restored = O3RuntimeState::default();
        restored.restore_checkpoint_payload(payload).unwrap();
        assert_eq!(
            integer_mapping(&restored, 3),
            Some(O3PhysicalRegisterId::new(41))
        );

        restored.discard_live_retire_window();
        assert_eq!(
            integer_mapping(&restored, 3),
            Some(O3PhysicalRegisterId::new(40))
        );
    }

    #[test]
    fn trapping_live_staged_instruction_does_not_publish_rename() {
        let mut runtime = O3RuntimeState::default();
        runtime.publish_live_rename_entry(O3RenameMapEntry::new(
            O3RegisterClass::Integer,
            3,
            O3PhysicalRegisterId::new(40),
        ));
        runtime.next_physical_register = 41;
        runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);
        let event = RiscvCpuExecutionEvent::new(
            fetch_event(0x8000, 1),
            div_x3(),
            RiscvExecutionRecord::with_trap(
                div_x3(),
                0x8000,
                0x9000,
                RiscvTrap::new(RiscvTrapKind::Interrupt { code: 1 }, 0x8000),
            ),
        );

        retire_live(&mut runtime, &event, 29);
        runtime.discard_live_staged_instructions();
        runtime.record_retired_instruction(&event);

        assert_eq!(
            integer_mapping(&runtime, 3),
            Some(O3PhysicalRegisterId::new(40))
        );
    }

    #[test]
    fn stats_reset_preserves_pending_live_dependency_producer_identity() {
        let mut runtime = O3RuntimeState::default();
        runtime.publish_live_rename_entry(O3RenameMapEntry::new(
            O3RegisterClass::Integer,
            1,
            O3PhysicalRegisterId::new(40),
        ));
        runtime.next_physical_register = 41;
        let first_instruction = addi(2, 1);
        let first = execution_event(first_instruction, 0x8000, 1, 2);
        runtime.stage_live_retire_window(Address::new(0x8000), first_instruction, 0, None);
        retire_live(&mut runtime, &first, 10);

        runtime.reset_stats();
        runtime.record_retired_instruction(&first);
        let second_instruction = addi(3, 1);
        runtime.record_retired_instruction(&execution_event(second_instruction, 0x8004, 2, 3));

        assert_eq!(runtime.stats().iew_producer_insts(), 1);
        assert_eq!(runtime.stats().iew_consumer_insts(), 2);
    }

    #[test]
    fn stats_reset_rebases_pending_consumer_onto_previously_seen_producer() {
        let mut runtime = O3RuntimeState::default();
        runtime.publish_live_rename_entry(O3RenameMapEntry::new(
            O3RegisterClass::Integer,
            1,
            O3PhysicalRegisterId::new(40),
        ));
        runtime.next_physical_register = 41;
        let prior_instruction = addi(2, 1);
        runtime.record_retired_instruction(&execution_event(prior_instruction, 0x7ffc, 0, 2));
        let pending_instruction = addi(3, 1);
        let pending = execution_event(pending_instruction, 0x8000, 1, 3);
        runtime.stage_live_retire_window(Address::new(0x8000), pending_instruction, 0, None);
        retire_live(&mut runtime, &pending, 10);

        runtime.reset_stats();

        assert_eq!(runtime.stats().iew_producer_insts(), 1);
        assert_eq!(runtime.stats().iew_consumer_insts(), 1);
    }

    #[test]
    fn snapshot_without_live_rename_overlay_equals_public_reconstruction() {
        let runtime = O3RuntimeState::default();

        assert_eq!(runtime.snapshot(), default_o3_runtime_snapshot());
    }

    #[test]
    fn live_rename_overlay_preserves_canonical_register_order() {
        let mut runtime = O3RuntimeState::default();
        runtime.publish_live_rename_entry(O3RenameMapEntry::new(
            O3RegisterClass::Integer,
            10,
            O3PhysicalRegisterId::new(40),
        ));
        runtime.next_physical_register = 41;
        runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);

        assert_eq!(
            runtime
                .snapshot()
                .rename_map()
                .iter()
                .map(|entry| entry.architectural())
                .collect::<Vec<_>>(),
            vec![3, 10]
        );
    }

    fn execution_event(
        instruction: RiscvInstruction,
        pc: u64,
        sequence: u64,
        destination: u8,
    ) -> RiscvCpuExecutionEvent {
        RiscvCpuExecutionEvent::new(
            fetch_event(pc, sequence),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                pc,
                pc + 4,
                vec![RegisterWrite::new(Register::new(destination).unwrap(), 1)],
                None,
            ),
        )
    }

    fn fetch_event(pc: u64, sequence: u64) -> CpuFetchEvent {
        CpuFetchEvent::completed(
            CpuFetchRecord::new(
                10 + sequence,
                PartitionId::new(0),
                MemoryRouteId::new(0),
                TransportEndpointId::new("cpu0.ifetch").unwrap(),
                MemoryRequestId::new(AgentId::new(7), sequence),
                Address::new(pc),
                AccessSize::new(4).unwrap(),
            ),
            0x0000_0013_u32.to_le_bytes().to_vec(),
        )
    }

    fn integer_mapping(
        runtime: &O3RuntimeState,
        architectural: u32,
    ) -> Option<O3PhysicalRegisterId> {
        runtime
            .snapshot()
            .rename_map()
            .iter()
            .find(|entry| {
                entry.register_class() == O3RegisterClass::Integer
                    && entry.architectural() == architectural
            })
            .map(|entry| entry.physical())
    }
}
