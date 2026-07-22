use rem6_isa_riscv::{RiscvDecodedInstruction, RiscvInstruction};

use super::o3_runtime_issue::queue::O3LiveIssuePacket;
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct O3LiveStagedFetchIdentity {
    instruction: RiscvInstruction,
    issue_packet: Option<O3LiveIssuePacket>,
    pub(super) producer_forwarded_control_target: Option<O3ProducerForwardedControlTarget>,
    pub(super) producer_forwarded_control_speculation: Option<BranchSpeculationId>,
    producer_forwarded_return_descendant: Option<O3ProducerForwardedReturnDescendant>,
}

impl O3LiveStagedFetchIdentity {
    pub(super) const fn new(instruction: RiscvInstruction) -> Self {
        Self {
            instruction,
            issue_packet: None,
            producer_forwarded_control_target: None,
            producer_forwarded_control_speculation: None,
            producer_forwarded_return_descendant: None,
        }
    }
    pub(super) const fn forwarded_control_target_identity(
        &self,
    ) -> Option<O3ProducerForwardedControlTarget> {
        self.producer_forwarded_control_target
    }
    pub(super) fn record_forwarded_return_identity(
        &mut self,
        descendant: O3ProducerForwardedReturnDescendant,
    ) {
        self.producer_forwarded_return_descendant = Some(descendant);
    }
    pub(super) const fn forwarded_return_identity(
        &self,
    ) -> Option<&O3ProducerForwardedReturnDescendant> {
        self.producer_forwarded_return_descendant.as_ref()
    }
    fn bind_issue_packet(
        &mut self,
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        if decoded.instruction() != self.instruction
            || !valid_live_speculative_fetch_identity(consumed_requests)
        {
            return false;
        }
        let packet = O3LiveIssuePacket::new(decoded, consumed_requests);
        if let Some(bound) = &self.issue_packet {
            return *bound == packet;
        }
        self.issue_packet = Some(packet);
        true
    }
    fn matches(
        &self,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.instruction == instruction
            && valid_live_speculative_fetch_identity(consumed_requests)
            && self.issue_packet.as_ref().is_none_or(|packet| {
                packet.instruction() == instruction
                    && packet.consumed_requests() == consumed_requests
            })
    }
    fn matches_bound(
        &self,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.issue_packet.as_ref().is_some_and(|packet| {
            packet.decoded().instruction() == instruction
                && packet.consumed_requests() == consumed_requests
        })
    }
    pub(super) fn issue_packet(&self) -> Option<&O3LiveIssuePacket> {
        self.issue_packet.as_ref()
    }
    pub(super) fn owns_fetch_request(&self, request: MemoryRequestId) -> bool {
        self.issue_packet
            .as_ref()
            .and_then(|packet| packet.consumed_requests().first())
            .copied()
            == Some(request)
    }
}
impl O3RuntimeState {
    pub(crate) fn append_producer_forwarded_control_descendant(
        &mut self,
        authority: O3ProducerForwardedControlTarget,
        pc: Address,
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<u64> {
        let instruction = decoded.instruction();
        let live_data_head = self.retained_producer_forwarded_control_target() == Some(authority);
        let retired_data_head =
            self.producer_forwarded_control_target_after_head_retire() == Some(authority);
        if (!live_data_head && !retired_data_head)
            || pc != authority.target()
            || self.live_data_accesses.len() + self.live_data_access_younger_sequences.len()
                >= self.scalar_memory_window_limit
        {
            return None;
        }
        let unresolved = match self.live_data_accesses.first() {
            Some(live) => match live.execution.instruction() {
                RiscvInstruction::Load { rd, .. } if !rd.is_zero() => Some(rd),
                _ => return None,
            },
            None => None,
        };
        let scalar = o3_predicted_scalar_descendant_operands(instruction);
        let sources = match scalar {
            Some((destination, sources)) if live_data_head && !destination.is_zero() => sources,
            Some(_) => return None,
            None if o3_exact_link_return_source(instruction) == authority.link_destination() => {
                vec![authority.link_destination()?]
            }
            None => return None,
        };
        if unresolved.is_some_and(|unresolved| sources.contains(&unresolved)) {
            return None;
        }
        let sequence = self.stage_live_instruction(pc, instruction, 0)?;
        if !self.bind_live_staged_issue_packet_at_sequence(sequence, decoded, consumed_requests) {
            self.discard_live_staged_window_from(sequence);
            return None;
        }
        self.record_live_control_descendant(sequence, authority.consumer_sequence());
        self.live_data_access_younger_sequences.insert(sequence);
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        Some(sequence)
    }

    pub(crate) fn stage_live_retire_window(
        &mut self,
        current_pc: Address,
        current: RiscvInstruction,
        current_ready_tick: u64,
        younger: impl IntoIterator<Item = (Address, RiscvInstruction)>,
    ) -> Option<u64> {
        if is_deferred_o3_data_instruction(current) {
            return None;
        }
        let head_sequence =
            self.stage_or_existing_live_instruction(current_pc, current, current_ready_tick);
        let mut control_sequence =
            head_sequence.filter(|_| o3_live_control_operands(current).is_some());
        if let Some(sequence) = control_sequence {
            self.record_live_control_sequence(sequence);
        }
        for (pc, instruction) in younger {
            if is_deferred_o3_data_instruction(instruction) {
                break;
            }
            let Some(sequence) = self.stage_or_existing_live_instruction(pc, instruction, 0) else {
                continue;
            };
            if let Some(control_sequence) = control_sequence {
                self.record_live_control_descendant(sequence, control_sequence);
            }
            let live_control = o3_live_control_operands(instruction).is_some();
            if live_control {
                self.record_live_control_sequence(sequence);
                control_sequence = Some(sequence);
            }
        }
        self.stats
            .observe_rob_occupancy(self.snapshot.reorder_buffer.len());
        self.stats
            .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
        head_sequence
    }

    pub(crate) fn stage_live_data_access_younger_window(
        &mut self,
        fetch_request: MemoryRequestId,
        younger: impl IntoIterator<Item = (Address, RiscvInstruction)>,
    ) -> usize {
        let Some(mut window) = self.data_access_integer_window(fetch_request) else {
            return 0;
        };
        let live_sequence = self
            .live_data_accesses
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
                self.record_live_control_descendant(sequence, control_sequence);
            }
            if matches!(
                decision,
                RiscvScalarIntegerYoungerDecision::AdmitTerminalControl
                    | RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
                    | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            ) {
                self.record_live_control_sequence(sequence);
            }
            if decision == RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl {
                self.live_serializing_control_sequences.insert(sequence);
            }
            self.live_data_access_younger_sequences.insert(sequence);
            if matches!(
                decision,
                RiscvScalarIntegerYoungerDecision::AdmitPredictedControl
                    | RiscvScalarIntegerYoungerDecision::AdmitPredictedRasControl
            ) {
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
            let invalidated_sequence = self
                .invalidated_live_staged_fetch_identities
                .iter()
                .find_map(|(sequence, identity)| {
                    identity
                        .matches_bound(execution.instruction(), consumed_requests)
                        .then_some(*sequence)
                });
            if let Some(sequence) = invalidated_sequence {
                self.invalidated_live_staged_fetch_identities
                    .remove(&sequence);
                self.record_untrusted_live_retirement(execution, sequence, retire_tick);
            }
            return;
        };
        let entry = self.snapshot.reorder_buffer[index];
        let identity_matches = self
            .live_staged_fetch_identities
            .get(&entry.sequence())
            .is_some_and(|identity| {
                identity.matches_bound(execution.instruction(), consumed_requests)
            });
        if !identity_matches {
            let invalidated_descendants = self
                .snapshot
                .reorder_buffer
                .iter()
                .filter(|younger| younger.is_live_staged() && younger.sequence() > entry.sequence())
                .filter_map(|younger| {
                    self.live_staged_fetch_identities
                        .get(&younger.sequence())
                        .cloned()
                        .map(|identity| (younger.sequence(), identity))
                })
                .collect::<Vec<_>>();
            self.discard_live_staged_window_from_at(entry.sequence(), retire_tick);
            self.invalidated_live_staged_fetch_identities
                .extend(invalidated_descendants);
            self.record_untrusted_live_retirement(execution, entry.sequence(), retire_tick);
            return;
        }
        if is_deferred_o3_data_access(execution.execution().memory_access()) {
            let boundary_sequence = entry.sequence();
            self.snapshot.reorder_buffer.drain(index..);
            self.discard_future_writeback_from_sequence(boundary_sequence, retire_tick);
            self.live_data_access_younger_sequences
                .retain(|sequence| *sequence < boundary_sequence);
            self.retain_live_speculative_executions_at(
                |speculative| speculative.sequence < boundary_sequence,
                retire_tick,
            );
            self.live_control_lineages
                .retain(|sequence, _| *sequence < boundary_sequence);
            self.live_serializing_control_sequences
                .retain(|sequence| *sequence < boundary_sequence);
            self.live_staged_fetch_identities
                .retain(|sequence, _| *sequence < boundary_sequence);
            self.live_retired_instructions
                .retain(|instruction| instruction.request != execution.fetch().request_id());
            self.stats
                .set_rename_map_entries(self.snapshot_with_live_rename_map().rename_map.len());
            return;
        }
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

    fn record_untrusted_live_retirement(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        sequence: u64,
        retire_tick: u64,
    ) {
        if execution.execution().memory_access().is_some() {
            return;
        }
        let dependencies = self.record_scalar_integer_dependencies(&execution.instruction());
        let fu_latency_cycles =
            crate::riscv_fu_latency::riscv_execute_wait_cycles(execution.instruction());
        let rob_occupancy = self.snapshot.reorder_buffer.len().saturating_add(1);
        self.live_retired_instructions
            .push(O3LiveRetiredInstruction {
                request: execution.fetch().request_id(),
                sequence,
                issue_tick: retire_tick.saturating_sub(fu_latency_cycles),
                admitted_writeback_tick: retire_tick,
                commit_tick: retire_tick,
                rob_occupancy,
                rob_commits: 1,
                rob_commit_blocked: false,
                iew_dependency_producers: dependencies.newly_observed_producers,
                iew_dependency_producer_registers: dependencies.producer_physical_registers,
                iew_dependency_consumers: dependencies.consumers,
                rename_destination: None,
            });
    }

    pub(crate) fn discard_live_retire_window(&mut self) {
        self.discard_live_staged_instructions();
        self.live_retired_instructions.clear();
    }

    pub(crate) fn discard_live_staged_instructions(&mut self) {
        self.discard_live_writeback_reservations();
        self.discard_live_data_access_lifecycle();
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged());
        self.live_data_access_younger_sequences.clear();
        self.live_control_lineages.clear();
        self.live_serializing_control_sequences.clear();
        self.live_staged_fetch_identities.clear();
        self.invalidated_live_staged_fetch_identities.clear();
        self.discard_live_speculative_executions();
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());
    }

    pub(crate) fn discard_live_staged_instructions_at(&mut self, now: u64) {
        self.discard_live_writeback_reservations();
        self.discard_live_data_access_lifecycle_at(now);
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged());
        self.live_data_access_younger_sequences.clear();
        self.live_control_lineages.clear();
        self.live_serializing_control_sequences.clear();
        self.live_staged_fetch_identities.clear();
        self.invalidated_live_staged_fetch_identities.clear();
        self.discard_live_speculative_executions_at(now);
        self.stats
            .set_rename_map_entries(self.snapshot.rename_map.len());
    }

    pub(crate) fn discard_live_speculative_executions(&mut self) {
        self.discard_live_writeback_reservations();
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

    pub(crate) fn live_data_access_younger_wakeup_seed(
        &self,
    ) -> Option<(MemoryRequestId, Vec<Address>)> {
        if self.live_data_access_younger_sequences.is_empty() {
            return None;
        }
        let tail = self.live_data_accesses.last()?;
        let younger_sequences = &self.live_data_access_younger_sequences;
        let younger_pcs = self
            .snapshot
            .reorder_buffer
            .iter()
            .filter(|entry| younger_sequences.contains(&entry.sequence()))
            .map(|entry| entry.pc())
            .collect::<Vec<_>>();
        (younger_pcs.len() == self.live_data_access_younger_sequences.len())
            .then_some((tail.fetch_request, younger_pcs))
    }

    pub(super) fn discard_live_staged_window_from(&mut self, sequence: u64) {
        self.discard_pending_data_address_from(sequence);
        self.discard_live_writeback_from_sequence(sequence);
        self.discard_live_staged_window_rows_from_at(sequence, None);
    }

    pub(super) fn discard_live_staged_window_from_at(&mut self, sequence: u64, now: u64) {
        if self
            .pending_data_addresses
            .first()
            .is_some_and(|pending| pending.sequence >= sequence)
        {
            self.discard_pending_data_address_at(now);
        }
        self.discard_future_writeback_from_sequence(sequence, now);
        self.discard_live_staged_window_rows_from_at(sequence, Some(now));
    }

    fn discard_live_staged_window_rows_from_at(&mut self, sequence: u64, now: Option<u64>) {
        self.snapshot
            .reorder_buffer
            .retain(|entry| !entry.is_live_staged() || entry.sequence() < sequence);
        self.live_data_access_younger_sequences
            .retain(|younger| *younger < sequence);
        if let Some(now) = now {
            self.retain_live_speculative_executions_at(
                |execution| execution.sequence < sequence,
                now,
            );
        } else {
            self.live_speculative_executions
                .retain(|execution| execution.sequence < sequence);
        }
        self.live_control_lineages
            .retain(|dependent, _| *dependent < sequence);
        self.live_serializing_control_sequences
            .retain(|control| *control < sequence);
        self.live_staged_fetch_identities
            .retain(|staged, _| *staged < sequence);
        self.invalidated_live_staged_fetch_identities
            .retain(|staged, _| *staged < sequence);
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
        let instruction = self.live_retired_instructions.remove(index);
        self.finalize_writeback_publication(instruction.sequence);
        Some(instruction)
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

    pub(crate) fn bind_live_staged_issue_packet(
        &mut self,
        pc: Address,
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        let Some(sequence) = self
            .snapshot
            .reorder_buffer
            .iter()
            .find(|entry| entry.is_live_staged() && entry.pc() == pc)
            .map(|entry| entry.sequence())
        else {
            return false;
        };
        self.bind_live_staged_issue_packet_at_sequence(sequence, decoded, consumed_requests)
    }
    pub(super) fn bind_live_staged_issue_packet_at_sequence(
        &mut self,
        sequence: u64,
        decoded: RiscvDecodedInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.live_staged_fetch_identities
            .get_mut(&sequence)
            .is_some_and(|identity| identity.bind_issue_packet(decoded, consumed_requests))
    }
    pub(in crate::o3_runtime) fn live_staged_issue_packet(
        &self,
        sequence: u64,
    ) -> Option<&O3LiveIssuePacket> {
        self.live_staged_fetch_identities
            .get(&sequence)
            .and_then(O3LiveStagedFetchIdentity::issue_packet)
    }
    pub(super) fn live_staged_instruction_matches(
        &self,
        sequence: u64,
        instruction: RiscvInstruction,
    ) -> bool {
        self.live_staged_fetch_identities
            .get(&sequence)
            .is_some_and(|identity| identity.instruction == instruction)
    }
    pub(super) fn live_staged_fetch_identity_matches(
        &self,
        sequence: u64,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> bool {
        self.live_staged_fetch_identities
            .get(&sequence)
            .is_some_and(|identity| identity.matches(instruction, consumed_requests))
    }

    pub(crate) fn live_staged_sequence_for_fetch_identity(
        &self,
        pc: Address,
        instruction: RiscvInstruction,
        consumed_requests: &[MemoryRequestId],
    ) -> Option<u64> {
        self.snapshot
            .reorder_buffer
            .iter()
            .filter(|entry| entry.is_live_staged() && entry.pc() == pc)
            .map(|entry| entry.sequence())
            .find(|sequence| {
                self.live_staged_issue_packet(*sequence)
                    .is_some_and(|packet| {
                        packet.instruction() == instruction
                            && packet.consumed_requests() == consumed_requests
                    })
            })
    }

    pub(super) fn stage_live_instruction(
        &mut self,
        pc: Address,
        instruction: RiscvInstruction,
        ready_tick: u64,
    ) -> Option<u64> {
        let rename_destination = o3_scalar_integer_destination(instruction)
            .filter(|register| !register.is_zero())
            .map(|register| (O3RegisterClass::Integer, u32::from(register.index())));
        self.stage_live_instruction_with_rename_destination(
            pc,
            instruction,
            ready_tick,
            rename_destination,
        )
        .map(|(sequence, _)| sequence)
    }

    pub(super) fn stage_live_instruction_with_rename_destination(
        &mut self,
        pc: Address,
        instruction: RiscvInstruction,
        ready_tick: u64,
        rename_destination: Option<(O3RegisterClass, u32)>,
    ) -> Option<(u64, Option<O3PhysicalRegisterId>)> {
        if self
            .snapshot
            .reorder_buffer
            .iter()
            .any(|entry| entry.is_live_staged() && entry.pc() == pc)
        {
            return None;
        }
        let sequence = self.allocate_sequence();
        let destination = rename_destination.map(|_| self.allocate_physical_register());
        self.snapshot.reorder_buffer.push(
            O3ReorderBufferEntry::new(sequence, pc, destination)
                .with_ready_tick(ready_tick)
                .with_live_staged_rename_destination(rename_destination),
        );
        self.live_staged_fetch_identities
            .insert(sequence, O3LiveStagedFetchIdentity::new(instruction));
        Some((sequence, destination))
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
            .find(|entry| {
                entry.is_live_staged()
                    && entry.pc() == pc
                    && self.live_staged_instruction_matches(entry.sequence(), instruction)
            })
            .map(|entry| entry.sequence())
    }

    pub(super) fn retain_live_data_access_younger_sequences_in_rob(&mut self) {
        let resident_sequences = self
            .snapshot
            .reorder_buffer
            .iter()
            .map(|entry| entry.sequence())
            .collect::<BTreeSet<_>>();
        self.live_data_access_younger_sequences
            .retain(|sequence| resident_sequences.contains(sequence));
        self.live_control_lineages
            .retain(|sequence, _| resident_sequences.contains(sequence));
        self.live_serializing_control_sequences
            .retain(|sequence| resident_sequences.contains(sequence));
        self.live_staged_fetch_identities
            .retain(|sequence, _| resident_sequences.contains(sequence));
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
#[path = "o3_runtime_live_window_tests.rs"]
mod tests;
