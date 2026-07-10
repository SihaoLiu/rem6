use rem6_isa_riscv::{Register, RiscvInstruction};

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
                issue_tick: retire_tick.saturating_sub(fu_latency_cycles),
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
    use rem6_isa_riscv::{
        Immediate, RegisterWrite, RiscvExecutionRecord, RiscvTrap, RiscvTrapKind,
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
            Some((Address::new(0x8004), addi(4, 0))),
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

        let restored_destinations = restored
            .snapshot()
            .reorder_buffer()
            .iter()
            .filter_map(|entry| entry.destination())
            .collect::<BTreeSet<_>>();
        restored.stage_live_retire_window(Address::new(0x8008), addi(5, 0), 0, None);
        let next_destination = restored
            .snapshot()
            .reorder_buffer()
            .iter()
            .find(|entry| entry.pc() == Address::new(0x8008))
            .and_then(|entry| entry.destination())
            .unwrap();
        assert!(
            !restored_destinations.contains(&next_destination),
            "post-restore allocation must not reuse a live staged physical register"
        );
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

        runtime.retire_live_staged_instruction(&event, 29);
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
        runtime.retire_live_staged_instruction(&first, 10);

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
        runtime.retire_live_staged_instruction(&pending, 10);

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
