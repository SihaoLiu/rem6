use rem6_isa_riscv::{Register, RiscvInstruction};

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct O3LiveRetiredInstruction {
    pub(super) request: MemoryRequestId,
    pub(super) sequence: u64,
    pub(super) issue_tick: u64,
    pub(super) commit_tick: u64,
    pub(super) rob_occupancy: usize,
    pub(super) rob_commits: usize,
    pub(super) rob_commit_blocked: bool,
    pub(super) iew_dependency_producers: u64,
    pub(super) iew_dependency_consumers: u64,
    pub(super) rename_destination: Option<O3RenameMapEntry>,
}

impl O3RuntimeState {
    pub(crate) fn stage_live_retire_window(
        &mut self,
        current_pc: Address,
        current: RiscvInstruction,
        current_ready_tick: u64,
        younger: Option<(Address, RiscvInstruction)>,
    ) {
        self.stage_live_instruction(current_pc, current, current_ready_tick);
        if let Some((pc, instruction)) = younger {
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
        let entry = self.snapshot.reorder_buffer[index];
        let (iew_dependency_producers, iew_dependency_consumers) =
            self.record_scalar_integer_dependencies(&execution.instruction());
        let rename_destination = staged_rename_entry(entry);
        if let Some(rename_destination) = rename_destination {
            self.publish_live_rename_entry(rename_destination);
        }

        self.snapshot.reorder_buffer[index].mark_ready_at(retire_tick);
        let rob_occupancy = self.snapshot.reorder_buffer.len();
        let (rob_commits, rob_commit_blocked) = rob_commit_boundary(&self.snapshot);
        let commit_tick = rob_commit_tick(&self.snapshot, rob_commits).unwrap_or(retire_tick);
        self.snapshot.reorder_buffer.drain(0..rob_commits);
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());

        let fu_latency_cycles =
            crate::riscv_execute::in_order_execute_wait_cycles(execution.instruction());
        self.live_retired_instructions
            .push(O3LiveRetiredInstruction {
                request: execution.fetch().request_id(),
                sequence: entry.sequence(),
                issue_tick: retire_tick.saturating_sub(fu_latency_cycles),
                commit_tick,
                rob_occupancy,
                rob_commits,
                rob_commit_blocked,
                iew_dependency_producers,
                iew_dependency_consumers,
                rename_destination,
            });
    }

    pub(crate) fn discard_live_retire_window(&mut self) {
        self.discard_live_staged_instructions();
        self.live_retired_instructions.clear();
    }

    pub(crate) fn discard_live_staged_instructions(&mut self) {
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged());
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());
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
        let mut rename_map = self.snapshot.rename_map.clone();
        for entry in &self.snapshot.reorder_buffer {
            let Some(rename_entry) = staged_rename_entry(*entry) else {
                continue;
            };
            if let Some(existing) = rename_map.iter_mut().find(|existing| {
                existing.register_class() == rename_entry.register_class()
                    && existing.architectural() == rename_entry.architectural()
            }) {
                *existing = rename_entry;
            } else {
                rename_map.push(rename_entry);
            }
        }
        O3RuntimeSnapshot::new(
            self.snapshot.reorder_buffer.iter().copied(),
            self.snapshot.load_store_queue.iter().copied(),
            rename_map,
            self.snapshot.pending_state.clone(),
        )
        .expect("live O3 runtime snapshot is internally consistent")
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
        let rename_destination = staged_scalar_integer_destination(instruction)
            .filter(|register| !register.is_zero())
            .map(|register| (O3RegisterClass::Integer, u32::from(register.index())));
        let destination = rename_destination.map(|_| self.allocate_physical_register());
        self.snapshot.reorder_buffer.push(
            O3ReorderBufferEntry::new(sequence, pc, destination)
                .with_ready_tick(ready_tick)
                .with_live_staged_rename_destination(rename_destination),
        );
    }

    fn publish_live_rename_entry(&mut self, entry: O3RenameMapEntry) {
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
}

fn staged_rename_entry(entry: O3ReorderBufferEntry) -> Option<O3RenameMapEntry> {
    let (register_class, architectural) = entry.rename_destination()?;
    Some(O3RenameMapEntry::new(
        register_class,
        architectural,
        entry.destination()?,
    ))
}

fn staged_scalar_integer_destination(instruction: RiscvInstruction) -> Option<Register> {
    match instruction {
        RiscvInstruction::Lui { rd, .. }
        | RiscvInstruction::Auipc { rd, .. }
        | RiscvInstruction::Addi { rd, .. }
        | RiscvInstruction::Slti { rd, .. }
        | RiscvInstruction::Sltiu { rd, .. }
        | RiscvInstruction::Xori { rd, .. }
        | RiscvInstruction::Ori { rd, .. }
        | RiscvInstruction::Andi { rd, .. }
        | RiscvInstruction::Slli { rd, .. }
        | RiscvInstruction::Srli { rd, .. }
        | RiscvInstruction::Srai { rd, .. }
        | RiscvInstruction::Addiw { rd, .. }
        | RiscvInstruction::Slliw { rd, .. }
        | RiscvInstruction::Srliw { rd, .. }
        | RiscvInstruction::Sraiw { rd, .. }
        | RiscvInstruction::Add { rd, .. }
        | RiscvInstruction::Sub { rd, .. }
        | RiscvInstruction::Sll { rd, .. }
        | RiscvInstruction::Slt { rd, .. }
        | RiscvInstruction::Sltu { rd, .. }
        | RiscvInstruction::Xor { rd, .. }
        | RiscvInstruction::Srl { rd, .. }
        | RiscvInstruction::Sra { rd, .. }
        | RiscvInstruction::Or { rd, .. }
        | RiscvInstruction::And { rd, .. }
        | RiscvInstruction::Mul { rd, .. }
        | RiscvInstruction::Mulh { rd, .. }
        | RiscvInstruction::Mulhsu { rd, .. }
        | RiscvInstruction::Mulhu { rd, .. }
        | RiscvInstruction::Div { rd, .. }
        | RiscvInstruction::Divu { rd, .. }
        | RiscvInstruction::Rem { rd, .. }
        | RiscvInstruction::Remu { rd, .. }
        | RiscvInstruction::Mulw { rd, .. }
        | RiscvInstruction::Divw { rd, .. }
        | RiscvInstruction::Divuw { rd, .. }
        | RiscvInstruction::Remw { rd, .. }
        | RiscvInstruction::Remuw { rd, .. }
        | RiscvInstruction::Addw { rd, .. }
        | RiscvInstruction::Subw { rd, .. }
        | RiscvInstruction::Sllw { rd, .. }
        | RiscvInstruction::Srlw { rd, .. }
        | RiscvInstruction::Sraw { rd, .. }
        | RiscvInstruction::Jal { rd, .. }
        | RiscvInstruction::Jalr { rd, .. }
        | RiscvInstruction::VectorSetVli { rd, .. }
        | RiscvInstruction::VectorSetIvli { rd, .. }
        | RiscvInstruction::VectorSetVl { rd, .. }
        | RiscvInstruction::Load { rd, .. }
        | RiscvInstruction::LoadReserved { rd, .. }
        | RiscvInstruction::StoreConditional { rd, .. }
        | RiscvInstruction::AtomicMemory { rd, .. } => Some(rd),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn div_x3() -> RiscvInstruction {
        RiscvInstruction::Div {
            rd: Register::new(3).unwrap(),
            rs1: Register::new(1).unwrap(),
            rs2: Register::new(2).unwrap(),
        }
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
        runtime.stage_live_retire_window(Address::new(0x8000), div_x3(), 29, None);
        let live_snapshot = runtime.snapshot();
        let encoded = runtime.checkpoint_payload().encode();

        let mut restored = O3RuntimeState::default();
        restored
            .restore_checkpoint_payload(O3RuntimeCheckpointPayload::decode(&encoded).unwrap())
            .unwrap();

        assert_eq!(restored.snapshot(), live_snapshot);
        assert!(restored.snapshot().reorder_buffer()[0].is_live_staged());
    }
}
