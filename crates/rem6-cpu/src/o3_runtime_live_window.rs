use rem6_isa_riscv::RiscvInstruction;

use super::*;
use crate::riscv_o3_window_policy::RiscvScalarIntegerYoungerDecision;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveRetiredInstruction {
    pub(super) request: MemoryRequestId,
    pub(super) sequence: u64,
    pub(super) issue_tick: u64,
    pub(super) admitted_writeback_tick: u64,
    pub(super) commit_tick: u64,
    pub(super) rob_occupancy: usize,
    pub(super) rob_commits: usize,
    pub(super) rob_commit_blocked: bool,
    pub(super) iew_dependency_producers: u64,
    pub(super) iew_dependency_producer_registers: Vec<O3PhysicalRegisterId>,
    pub(super) iew_dependency_consumers: u64,
    pub(super) rename_destination: Option<O3RenameMapEntry>,
}

impl O3RuntimeState {
    pub(crate) fn stage_live_retire_window(
        &mut self,
        current_pc: Address,
        current: RiscvInstruction,
        current_ready_tick: u64,
        younger: impl IntoIterator<Item = (Address, RiscvInstruction)>,
    ) -> Option<u64> {
        if is_deferred_o3_scalar_memory_instruction(current) {
            return None;
        }
        let head_sequence =
            self.stage_or_existing_live_instruction(current_pc, current, current_ready_tick);
        let mut control_sequence =
            head_sequence.filter(|_| o3_direct_conditional_sources(current).is_some());
        if let Some(sequence) = control_sequence {
            self.live_control_window_sequences.insert(sequence);
        }
        for (pc, instruction) in younger {
            if is_deferred_o3_scalar_memory_instruction(instruction) {
                break;
            }
            let Some(sequence) = self.stage_or_existing_live_instruction(pc, instruction, 0) else {
                continue;
            };
            if let Some(control_sequence) = control_sequence {
                self.live_control_dependencies
                    .insert(sequence, control_sequence);
            }
            let direct_conditional = o3_direct_conditional_sources(instruction).is_some();
            if control_sequence.is_some() || direct_conditional {
                self.live_control_window_sequences.insert(sequence);
            }
            if direct_conditional {
                control_sequence = Some(sequence);
            }
        }
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        head_sequence
    }

    pub(crate) fn stage_live_scalar_memory_younger_window(
        &mut self,
        fetch_request: MemoryRequestId,
        younger: impl IntoIterator<Item = (Address, RiscvInstruction)>,
    ) -> usize {
        let Some(mut window) = self.scalar_memory_integer_window(fetch_request) else {
            return 0;
        };
        let live_sequence = self
            .live_scalar_memories
            .last()
            .expect("scalar memory integer window has a live tail")
            .sequence;
        if !self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.sequence() == live_sequence && entry.is_live_staged())
            || self
                .snapshot
                .reorder_buffer
                .iter()
                .any(|entry| entry.is_live_staged() && entry.sequence() > live_sequence)
        {
            return 0;
        }
        let mut control_sequence = None;
        let mut staged_rows = 0;
        // Revalidate the caller-selected prefix before allocating live rows.
        for (pc, instruction) in younger {
            let decision = window.classify_younger(instruction);
            if decision == RiscvScalarIntegerYoungerDecision::Reject {
                break;
            }
            let Some(sequence) = self.stage_live_instruction(pc, instruction, 0) else {
                break;
            };
            staged_rows += 1;
            if let Some(control_sequence) = control_sequence {
                self.live_control_dependencies
                    .insert(sequence, control_sequence);
            }
            if control_sequence.is_some()
                || matches!(
                    decision,
                    RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
                        | RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
                )
            {
                self.live_control_window_sequences.insert(sequence);
            }
            self.live_scalar_memory_younger_sequences.insert(sequence);
            if decision == RiscvScalarIntegerYoungerDecision::AdmitPredictedControl {
                control_sequence = Some(sequence);
            }
            if matches!(
                decision,
                RiscvScalarIntegerYoungerDecision::AdmitStop
                    | RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
            ) {
                break;
            }
        }
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        staged_rows
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
            self.discard_future_writeback_from_sequence(boundary_sequence, retire_tick);
            self.live_scalar_memory_younger_sequences
                .retain(|sequence| *sequence < boundary_sequence);
            self.retain_live_speculative_executions_at(
                |speculative| speculative.sequence < boundary_sequence,
                retire_tick,
            );
            self.live_control_dependencies
                .retain(|sequence, _| *sequence < boundary_sequence);
            self.live_control_window_sequences
                .retain(|sequence| *sequence < boundary_sequence);
            self.live_retired_instructions
                .retain(|instruction| instruction.request != execution.fetch().request_id());
            self.stats
                .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
            return;
        }
        let entry = self.snapshot.reorder_buffer[index];
        let speculative_timing = self.take_live_speculative_issue_timing_at(
            entry,
            execution,
            consumed_requests,
            retire_tick,
        );
        let dependencies =
            self.record_live_staged_scalar_integer_dependencies(&execution.instruction(), index);
        let rename_destination = staged_rename_entry(entry).filter(|destination| {
            execution_writes_rename_destination(execution.execution(), *destination)
        });
        if rename_destination.is_none() && entry.rename_destination().is_some() {
            self.snapshot.reorder_buffer[index].clear_live_staged_destination();
        }

        let admitted_writeback_tick =
            speculative_timing.map_or(retire_tick, |(_, admitted_tick)| admitted_tick);
        self.snapshot.reorder_buffer[index].mark_ready_at(admitted_writeback_tick);
        let rob_occupancy = self.snapshot.reorder_buffer.len();
        let (rob_commits, _) = rob_commit_boundary(&self.snapshot);
        let rob_commit_blocked = rob_commits <= index;
        let commit_tick = rob_commit_tick(&self.snapshot, rob_commits).unwrap_or(retire_tick);
        let commit_tick = self.commit_live_rob_prefix(rob_commits, commit_tick);
        let rename_map_entries = self.snapshot_with_live_rename_map().rename_map.len();
        self.stats.set_rename_map_entries(rename_map_entries);

        let fu_latency_cycles =
            crate::riscv_fu_latency::riscv_execute_wait_cycles(execution.instruction());
        self.live_retired_instructions
            .push(O3LiveRetiredInstruction {
                request: execution.fetch().request_id(),
                sequence: entry.sequence(),
                issue_tick: speculative_timing
                    .map(|(issue_tick, _)| issue_tick)
                    .unwrap_or_else(|| retire_tick.saturating_sub(fu_latency_cycles)),
                admitted_writeback_tick,
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
        self.discard_all_writeback_reservations();
        self.discard_live_scalar_memory_lifecycle();
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged());
        self.live_scalar_memory_younger_sequences.clear();
        self.live_control_dependencies.clear();
        self.live_control_window_sequences.clear();
        self.discard_live_speculative_executions();
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());
    }

    pub(crate) fn discard_live_staged_instructions_at(&mut self, now: u64) {
        self.discard_live_scalar_memory_lifecycle_at(now);
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged());
        self.live_scalar_memory_younger_sequences.clear();
        self.live_control_dependencies.clear();
        self.live_control_window_sequences.clear();
        self.discard_live_speculative_executions_at(now);
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());
    }

    pub(crate) fn discard_live_speculative_executions(&mut self) {
        self.discard_all_writeback_reservations();
        self.live_speculative_executions.clear();
    }

    pub(crate) fn discard_live_speculative_executions_at(&mut self, now: u64) {
        let sequences = self
            .live_speculative_executions
            .iter()
            .map(|execution| execution.sequence)
            .collect::<Vec<_>>();
        for sequence in sequences {
            self.discard_future_writeback_sequence(sequence, now);
        }
        self.live_speculative_executions.clear();
    }

    pub(crate) fn live_scalar_memory_younger_wakeup_seed(
        &self,
    ) -> Option<(MemoryRequestId, Vec<Address>)> {
        if self.live_scalar_memory_younger_sequences.is_empty() {
            return None;
        }
        let tail = self.live_scalar_memories.last()?;
        let younger_pcs = self
            .snapshot
            .reorder_buffer
            .iter()
            .filter(|entry| {
                self.live_scalar_memory_younger_sequences
                    .contains(&entry.sequence())
            })
            .map(|entry| entry.pc())
            .collect::<Vec<_>>();
        (younger_pcs.len() == self.live_scalar_memory_younger_sequences.len())
            .then_some((tail.fetch_request, younger_pcs))
    }

    pub(super) fn discard_live_staged_window_from(&mut self, sequence: u64) {
        self.discard_all_writeback_reservations();
        self.discard_live_staged_window_rows_from_at(sequence, 0);
    }

    pub(super) fn discard_live_staged_window_from_at(&mut self, sequence: u64, now: u64) {
        self.discard_future_writeback_from_sequence(sequence, now);
        self.discard_live_staged_window_rows_from_at(sequence, now);
    }

    fn discard_live_staged_window_rows_from_at(&mut self, sequence: u64, now: u64) {
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged() || entry.sequence() < sequence);
        self.live_scalar_memory_younger_sequences
            .retain(|younger| *younger < sequence);
        self.retain_live_speculative_executions_at(|execution| execution.sequence < sequence, now);
        self.live_control_dependencies
            .retain(|dependent, _| *dependent < sequence);
        self.live_control_window_sequences
            .retain(|control| *control < sequence);
        self.live_retired_instructions
            .retain(|instruction| instruction.sequence < sequence);
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
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

    fn stage_live_instruction(
        &mut self,
        pc: Address,
        instruction: RiscvInstruction,
        ready_tick: u64,
    ) -> Option<u64> {
        if self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.is_live_staged() && entry.pc() == pc)
        {
            return None;
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
        Some(sequence)
    }

    fn stage_or_existing_live_instruction(
        &mut self,
        pc: Address,
        instruction: RiscvInstruction,
        ready_tick: u64,
    ) -> Option<u64> {
        if let Some(sequence) = self.stage_live_instruction(pc, instruction, ready_tick) {
            return Some(sequence);
        }
        self.snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.pc() == pc)
            .map(|entry| entry.sequence())
    }

    pub(super) fn retain_live_scalar_memory_younger_sequences_in_rob(&mut self) {
        let resident_sequences = self
            .snapshot
            .reorder_buffer
            .iter()
            .map(|entry| entry.sequence())
            .collect::<BTreeSet<_>>();
        self.live_scalar_memory_younger_sequences
            .retain(|sequence| resident_sequences.contains(sequence));
        self.live_control_dependencies
            .retain(|sequence, _| resident_sequences.contains(sequence));
        self.live_control_window_sequences
            .retain(|sequence| resident_sequences.contains(sequence));
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

    pub(super) fn retain_live_speculative_executions_at<F>(&mut self, mut keep: F, now: u64)
    where
        F: FnMut(&O3LiveSpeculativeExecution) -> bool,
    {
        let mut retained = Vec::with_capacity(self.live_speculative_executions.len());
        for execution in self
            .live_speculative_executions
            .drain(..)
            .collect::<Vec<_>>()
        {
            if keep(&execution) {
                retained.push(execution);
            } else {
                self.discard_future_writeback_sequence(execution.sequence, now);
            }
        }
        self.live_speculative_executions = retained;
    }
}

pub(super) fn staged_rename_entry(entry: O3ReorderBufferEntry) -> Option<O3RenameMapEntry> {
    let (register_class, architectural) = entry.rename_destination()?;
    Some(O3RenameMapEntry::new(
        register_class,
        architectural,
        entry.destination()?,
    ))
}

#[cfg(test)]
mod tests {
    use rem6_isa_riscv::{
        Immediate, MemoryAccessKind, MemoryWidth, Register, RegisterWrite, RiscvExecutionRecord,
        RiscvTrap, RiscvTrapKind,
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

    fn add(rd: u8, rs1: u8, rs2: u8) -> RiscvInstruction {
        RiscvInstruction::Add {
            rd: Register::new(rd).unwrap(),
            rs1: Register::new(rs1).unwrap(),
            rs2: Register::new(rs2).unwrap(),
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

    #[test]
    fn scalar_load_window_allows_one_independent_younger_issue_candidate() {
        let mut runtime = O3RuntimeState::default();
        let load = scalar_load_event();
        let independent = addi(5, 0);
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [(Address::new(0x8004), independent)],
        );

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), independent)
            .is_some());
    }

    #[test]
    fn scalar_load_window_blocks_younger_load_destination_consumer() {
        let mut runtime = O3RuntimeState::default();
        let load = scalar_load_event();
        let dependent = addi(5, 4);
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [(Address::new(0x8004), dependent)],
        );

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 2);
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .is_none());
    }

    #[test]
    fn scalar_load_head_stages_three_younger_scalar_alus() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        let first = addi(5, 0);
        let second = addi(6, 5);
        let third = add(7, 5, 6);
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), first),
                (Address::new(0x8008), second),
                (Address::new(0x800c), third),
            ],
        );

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 4);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 1);
        assert_eq!(runtime.live_scalar_memory_younger_sequences.len(), 3);
        assert_eq!(
            runtime.snapshot().reorder_buffer()[3].pc(),
            Address::new(0x800c)
        );
    }

    #[test]
    fn scalar_load_backedge_stages_only_the_unique_prefix() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        let repeated = addi(5, 0);
        let branch = RiscvInstruction::Beq {
            rs1: Register::new(1).unwrap(),
            rs2: Register::new(2).unwrap(),
            offset: Immediate::new(-4),
        };
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

        let staged = runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), repeated),
                (Address::new(0x8008), branch),
                (Address::new(0x8004), repeated),
            ],
        );

        assert_eq!(staged, 2);
        assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
    }

    #[test]
    fn scalar_load_head_stages_terminal_branch_without_rename_or_younger_rows() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        let branch = RiscvInstruction::decode(0x00b2_0463).unwrap();
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), addi(5, 0)),
                (Address::new(0x8008), addi(6, 5)),
                (Address::new(0x800c), branch),
                (Address::new(0x8010), addi(7, 0)),
            ],
        );

        let snapshot = runtime.snapshot();
        let rob = snapshot.reorder_buffer();
        assert_eq!(
            rob.iter().map(|entry| entry.pc()).collect::<Vec<_>>(),
            [0x8000, 0x8004, 0x8008, 0x800c].map(Address::new)
        );
        let branch_row = rob[3];
        assert_eq!(
            (branch_row.destination(), branch_row.rename_destination()),
            (None, None)
        );
        assert!(branch_row.is_live_staged() && !branch_row.is_ready());
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x800c), branch)
            .is_none());
    }

    #[test]
    fn scalar_load_head_younger_alus_wake_transitively_with_fan_in() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        let first = addi(5, 0);
        let second = addi(6, 5);
        let third = add(7, 5, 6);
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), first),
                (Address::new(0x8008), second),
                (Address::new(0x800c), third),
            ],
        );

        let first_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), first)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                first_candidate,
                &[request(11)],
                10,
                RiscvExecutionRecord::new(
                    first,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(5).unwrap(), 5)],
                    None,
                ),
            )
            .unwrap();
        let second_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), second)
            .expect("the first younger ALU should wake the second");
        assert_eq!(second_candidate.issue_tick(10), 10);
        runtime
            .record_live_speculative_execution(
                second_candidate,
                &[request(12)],
                10,
                RiscvExecutionRecord::new(
                    second,
                    0x8008,
                    0x800c,
                    vec![RegisterWrite::new(Register::new(6).unwrap(), 16)],
                    None,
                ),
            )
            .unwrap();

        let third_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x800c), third)
            .expect("both younger ALU producers should wake the fan-in row");

        assert_eq!(
            third_candidate.forwarded_register_writes(),
            &[
                RegisterWrite::new(Register::new(5).unwrap(), 5),
                RegisterWrite::new(Register::new(6).unwrap(), 16),
            ]
        );
        assert_eq!(third_candidate.issue_tick(10), 11);
    }

    #[test]
    fn scalar_load_head_dependency_row_remains_blocked_before_load_writeback() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        let independent = addi(5, 0);
        let dependent = addi(6, 4);
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), independent),
                (Address::new(0x8008), dependent),
            ],
        );

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), independent)
            .is_some());
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8008), dependent)
            .is_none());
    }

    #[test]
    fn completed_live_load_forwards_into_dependent_alu_candidate() {
        let mut runtime = O3RuntimeState::default();
        let load = scalar_load_event();
        let dependent = addi(5, 4);
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [(Address::new(0x8004), dependent)],
        );
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .is_none());

        let mut completed = load.clone();
        completed.set_data_access_event_kind(RiscvDataAccessEventKind::Completed);
        assert!(runtime
            .complete_live_scalar_memory_response(
                &completed,
                request(20),
                41,
                10,
                Some(&0x2a_u32.to_le_bytes()),
            )
            .unwrap());

        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .is_none());
        assert!(runtime.take_ready_live_scalar_memory_event(41).is_none());
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .is_none());
        assert!(runtime.take_ready_live_scalar_memory_event(42).is_some());

        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .expect("admitted load should wake its dependent scalar ALU");
        assert_eq!(
            candidate.forwarded_register_writes(),
            &[RegisterWrite::new(Register::new(4).unwrap(), 0x2a)]
        );
        assert_eq!(candidate.issue_tick(10), 42);
    }

    #[test]
    fn scalar_memory_prefix_stages_load_dependent_terminal_alu() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(3);
        let store = scalar_store_event();
        let load = scalar_load_event();
        let dependent = addi(5, 4);
        assert!(runtime.stage_live_scalar_memory_issue(&store, request(19), 30));
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));

        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [(Address::new(0x8004), dependent)],
        );

        assert_eq!(runtime.snapshot().reorder_buffer().len(), 3);
        assert_eq!(runtime.snapshot().load_store_queue().len(), 2);
        assert_eq!(runtime.live_scalar_memory_younger_sequences.len(), 1);
        assert!(runtime
            .live_speculative_issue_candidate(Address::new(0x8004), dependent)
            .is_none());
    }

    #[test]
    fn scalar_load_head_discard_removes_every_younger_row() {
        let mut runtime = O3RuntimeState::default();
        runtime.set_scalar_memory_window_limit(4);
        let load = scalar_load_event();
        assert!(runtime.stage_live_scalar_memory_issue(&load, request(20), 31));
        runtime.stage_live_scalar_memory_younger_window(
            load.fetch().request_id(),
            [
                (Address::new(0x8004), addi(5, 0)),
                (Address::new(0x8008), addi(6, 5)),
                (Address::new(0x800c), add(7, 5, 6)),
            ],
        );

        runtime.discard_live_staged_instructions();

        assert!(runtime.snapshot().reorder_buffer().is_empty());
        assert!(runtime.snapshot().load_store_queue().is_empty());
        assert!(runtime.live_scalar_memory_younger_sequences.is_empty());
        assert!(runtime.live_speculative_executions.is_empty());
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
                (Address::new(0x800c), add(6, 4, 5)),
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
        assert!(restored.snapshot().reorder_buffer()[3].is_live_staged());

        let restored_destinations = restored
            .snapshot()
            .reorder_buffer()
            .iter()
            .filter_map(|entry| entry.destination())
            .collect::<BTreeSet<_>>();
        restored.stage_live_retire_window(Address::new(0x8010), addi(7, 0), 0, None);
        let next_destination = restored
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.pc() == Address::new(0x8010))
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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();
        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = execution_event(younger_instruction, 0x8004, 2, 4);
        retire_live(&mut runtime, &younger, 30);
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 10);
        assert_eq!(trace.writeback_tick(), 10);
        assert_eq!(trace.admitted_writeback_tick(), Some(10));
        assert_eq!(trace.fu_latency_cycles(), 0);
        assert_eq!(trace.commit_tick(), 29);
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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = execution_event(younger_instruction, 0x8004, 2, 4);
        runtime.retire_live_staged_instruction(&younger, &[request(2), request(3)], 30);
        runtime.record_retired_instruction_with_trace(&younger, true);

        let trace = runtime.trace_records().last().copied().unwrap();
        assert_eq!(trace.issue_tick(), 10);
        assert_eq!(trace.writeback_tick(), 10);
        assert_eq!(trace.admitted_writeback_tick(), Some(10));
        assert_eq!(trace.fu_latency_cycles(), 0);
        assert_eq!(trace.commit_tick(), 29);
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
    fn repeated_live_window_staging_extends_past_existing_rows() {
        let mut runtime = O3RuntimeState::default();
        let head = div_x3();
        let first = addi(4, 0);
        let second = addi(5, 4);
        let third = addi(6, 5);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            head,
            29,
            Some((Address::new(0x8004), first)),
        );

        runtime.stage_live_retire_window(
            Address::new(0x8000),
            head,
            29,
            [
                (Address::new(0x8004), first),
                (Address::new(0x8008), second),
                (Address::new(0x800c), third),
            ],
        );

        assert_eq!(
            runtime
                .snapshot()
                .reorder_buffer()
                .iter()
                .map(|entry| entry.pc())
                .collect::<Vec<_>>(),
            [0x8000, 0x8004, 0x8008, 0x800c].map(Address::new)
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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();

        let consumer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), consumer)
            .expect("speculative x4 write should wake the x5 consumer");

        assert_eq!(
            consumer_candidate.forwarded_register_writes(),
            &[RegisterWrite::new(Register::new(4).unwrap(), 1)]
        );
        assert_eq!(consumer_candidate.issue_tick(10), 10);
    }

    #[test]
    fn speculative_scalar_chain_wakes_transitively_with_fan_in() {
        let mut runtime = O3RuntimeState::default();
        let first = addi(4, 0);
        let second = addi(5, 4);
        let third = add(6, 4, 5);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            [
                (Address::new(0x8004), first),
                (Address::new(0x8008), second),
                (Address::new(0x800c), third),
            ],
        );

        let first_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), first)
            .unwrap();
        runtime
            .record_live_speculative_execution(
                first_candidate,
                &[request(2)],
                10,
                RiscvExecutionRecord::new(
                    first,
                    0x8004,
                    0x8008,
                    vec![RegisterWrite::new(Register::new(4).unwrap(), 5)],
                    None,
                ),
            )
            .unwrap();
        let second_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), second)
            .unwrap();
        assert_eq!(second_candidate.issue_tick(10), 10);
        runtime
            .record_live_speculative_execution(
                second_candidate,
                &[request(3)],
                10,
                RiscvExecutionRecord::new(
                    second,
                    0x8008,
                    0x800c,
                    vec![RegisterWrite::new(Register::new(5).unwrap(), 16)],
                    None,
                ),
            )
            .unwrap();

        let third_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x800c), third)
            .expect("both speculative producers should wake the fan-in consumer");

        assert_eq!(
            third_candidate.forwarded_register_writes(),
            &[
                RegisterWrite::new(Register::new(4).unwrap(), 5),
                RegisterWrite::new(Register::new(5).unwrap(), 16),
            ]
        );
        assert_eq!(third_candidate.issue_tick(10), 11);
    }

    #[test]
    fn invalidated_first_speculative_row_discards_two_downstream_rows() {
        let mut runtime = O3RuntimeState::default();
        let first = addi(4, 0);
        let second = addi(5, 4);
        let third = add(6, 4, 5);
        runtime.stage_live_retire_window(
            Address::new(0x8000),
            div_x3(),
            29,
            [
                (Address::new(0x8004), first),
                (Address::new(0x8008), second),
                (Address::new(0x800c), third),
            ],
        );
        for (pc, instruction, request_id, destination, value) in [
            (0x8004, first, 2, 4, 5),
            (0x8008, second, 3, 5, 16),
            (0x800c, third, 4, 6, 21),
        ] {
            let candidate = runtime
                .live_speculative_issue_candidate(Address::new(pc), instruction)
                .unwrap();
            runtime
                .record_live_speculative_execution(
                    candidate,
                    &[request(request_id)],
                    10,
                    RiscvExecutionRecord::new(
                        instruction,
                        pc,
                        pc + 4,
                        vec![RegisterWrite::new(
                            Register::new(destination).unwrap(),
                            value,
                        )],
                        None,
                    ),
                )
                .unwrap();
        }
        assert_eq!(runtime.live_speculative_executions.len(), 3);
        let mismatched = execution_event(first, 0x8004, 2, 4);

        runtime.retire_live_staged_instruction(&mismatched, &[request(2)], 20);

        assert!(runtime.live_speculative_executions.is_empty());
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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();

        let consumer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), consumer)
            .expect("the nearer x3 writer should hide the older pending divide destination");

        assert_eq!(
            consumer_candidate.forwarded_register_writes(),
            &[RegisterWrite::new(Register::new(3).unwrap(), 1)]
        );
        assert_eq!(consumer_candidate.issue_tick(10), 10);
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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();
        let consumer_candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8008), consumer)
            .unwrap();
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();

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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();

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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();
        let speculative_sequence = runtime.live_speculative_executions[0].sequence;

        let divide = execution_event(div_x3(), 0x8000, 1, 3);
        retire_live(&mut runtime, &divide, 29);
        runtime.record_retired_instruction_with_trace(&divide, true);
        let younger = execution_event(younger_instruction, 0x8004, 2, 4);
        runtime.retire_live_staged_instruction(&younger, &[request(2), request(4)], 30);
        assert_eq!(
            runtime
                .writeback_reservation(speculative_sequence)
                .map(O3WritebackReservation::admitted_tick),
            Some(10),
            "rebinding after the admitted tick must preserve historical occupancy"
        );
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
            runtime
                .record_live_speculative_execution(
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
                )
                .unwrap();

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
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();
        assert_eq!(runtime.live_speculative_executions.len(), 1);

        let checkpoint = runtime.checkpoint_payload();
        runtime.restore_checkpoint_payload(checkpoint).unwrap();
        assert!(runtime.live_speculative_executions.is_empty());
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();

        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();
        runtime.discard_live_speculative_executions();
        assert!(runtime.live_speculative_executions.is_empty());
        let candidate = runtime
            .live_speculative_issue_candidate(Address::new(0x8004), younger_instruction)
            .unwrap();
        runtime
            .record_live_speculative_execution(
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
            )
            .unwrap();
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

    fn scalar_load_event() -> RiscvCpuExecutionEvent {
        let instruction = load_x4();
        RiscvCpuExecutionEvent::new(
            fetch_event(0x8000, 10),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                0x8000,
                0x8004,
                Vec::new(),
                Some(MemoryAccessKind::Load {
                    rd: Register::new(4).unwrap(),
                    address: 0x9000,
                    width: MemoryWidth::Word,
                    signed: false,
                }),
            ),
        )
    }

    fn scalar_store_event() -> RiscvCpuExecutionEvent {
        let instruction = RiscvInstruction::Store {
            rs1: Register::new(10).unwrap(),
            rs2: Register::new(11).unwrap(),
            offset: Immediate::new(0),
            width: MemoryWidth::Word,
        };
        RiscvCpuExecutionEvent::new(
            fetch_event(0x7ffc, 9),
            instruction,
            RiscvExecutionRecord::new(
                instruction,
                0x7ffc,
                0x8000,
                Vec::new(),
                Some(MemoryAccessKind::Store {
                    address: 0x9000,
                    width: MemoryWidth::Word,
                    value: 0x2a,
                }),
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
