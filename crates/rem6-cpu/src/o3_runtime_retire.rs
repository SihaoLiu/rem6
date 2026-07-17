use crate::riscv_branch_kind::riscv_branch_target_kind;

use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3RobRetireObservation {
    sequence: u64,
    issue_tick: u64,
    admitted_writeback_tick: Option<u64>,
    commit_tick: u64,
    occupancy: usize,
    commits: usize,
    commit_blocked: bool,
    iew_dependency_producers: u64,
    iew_dependency_consumers: u64,
    drains_runtime_rob: bool,
}

impl O3RuntimeState {
    pub(super) fn record_runtime_state(
        &mut self,
        execution: &RiscvCpuExecutionEvent,
        live_data_access: Option<&O3LiveDataAccess>,
    ) -> O3RuntimeTraceRecord {
        let record = execution.execution();
        let live_retired = live_data_access
            .is_none()
            .then(|| self.take_live_retired_instruction(execution.fetch().request_id()))
            .flatten();
        let observation = if let Some(live) = live_data_access {
            let commit_tick = live
                .commit_tick
                .expect("taken live data access has an ordered commit tick");
            let rob = self
                .snapshot
                .reorder_buffer
                .iter()
                .find(|entry| entry.sequence() == live.sequence)
                .copied()
                .expect("completed live data access keeps its ROB row until retirement");
            assert!(rob.is_ready());
            let dependencies = self.record_scalar_integer_dependencies(&record.instruction());
            O3RobRetireObservation {
                sequence: live.sequence,
                issue_tick: live.issue_tick,
                admitted_writeback_tick: live.admitted_writeback_tick,
                commit_tick,
                occupancy: live.issue_rob_occupancy,
                commits: 1,
                commit_blocked: false,
                iew_dependency_producers: dependencies.newly_observed_producers,
                iew_dependency_consumers: dependencies.consumers,
                drains_runtime_rob: false,
            }
        } else if let Some(live) = live_retired {
            self.record_rename_map_entries(record, live.rename_destination);
            O3RobRetireObservation {
                sequence: live.sequence,
                issue_tick: live.issue_tick,
                admitted_writeback_tick: Some(live.admitted_writeback_tick),
                commit_tick: live.commit_tick,
                occupancy: live.rob_occupancy,
                commits: live.rob_commits,
                commit_blocked: live.rob_commit_blocked,
                iew_dependency_producers: live.iew_dependency_producers,
                iew_dependency_consumers: live.iew_dependency_consumers,
                drains_runtime_rob: false,
            }
        } else {
            let sequence =
                self.allocate_sequence_span(o3_instruction_sequence_span(record.memory_access()));
            let dependencies = self.record_scalar_integer_dependencies(&record.instruction());
            let destination = self.record_rename_map_entries(record, None);
            for entry in &mut self.snapshot.reorder_buffer {
                entry.mark_ready();
            }
            let fu_latency_cycles =
                crate::riscv_fu_latency::riscv_execute_wait_cycles(execution.instruction());
            let writeback_tick = execution.fetch().tick().saturating_add(fu_latency_cycles);
            self.snapshot.reorder_buffer.push(
                O3ReorderBufferEntry::new(sequence, Address::new(record.pc()), destination)
                    .with_ready(fu_latency_cycles == 0)
                    .with_ready_tick(writeback_tick),
            );
            let occupancy = self.snapshot.reorder_buffer.len();
            self.stats.observe_rob_occupancy(occupancy);
            let (commits, commit_blocked) = rob_commit_boundary(&self.snapshot);
            O3RobRetireObservation {
                sequence,
                issue_tick: execution.fetch().tick(),
                admitted_writeback_tick: None,
                commit_tick: rob_commit_tick(&self.snapshot, commits).unwrap_or(writeback_tick),
                occupancy,
                commits,
                commit_blocked,
                iew_dependency_producers: dependencies.newly_observed_producers,
                iew_dependency_consumers: dependencies.consumers,
                drains_runtime_rob: true,
            }
        };
        let (lsq_loads, lsq_stores) = record
            .memory_access()
            .map(o3_lsq_access_counts)
            .unwrap_or((0, 0));
        let lsq_operation = record
            .memory_access()
            .map(o3_lsq_operation)
            .unwrap_or(O3RuntimeLsqOperation::None);
        let lsq_ordering = record
            .memory_access()
            .map(o3_lsq_ordering)
            .unwrap_or(O3RuntimeLsqOrdering::None);
        let (lsq_load_bytes, lsq_store_bytes) = record
            .memory_access()
            .map(o3_lsq_access_bytes)
            .unwrap_or((0, 0));
        let (lsq_load_address, lsq_store_address) = record
            .memory_access()
            .map(o3_lsq_access_addresses)
            .unwrap_or((None, None));
        let branch_update = execution.branch_update();
        let branch_kind = branch_update
            .map(|_| riscv_branch_target_kind(record.instruction()))
            .unwrap_or(BranchTargetKind::NoBranch);
        let selected_branch_prediction = execution.selected_branch_prediction();
        let branch_predicted_taken = selected_branch_prediction
            .map(|(predicted_taken, _)| predicted_taken)
            .unwrap_or_else(|| branch_update.is_some_and(|update| update.predicted_taken()));
        let branch_resolved_taken = branch_update.is_some_and(|update| update.actual_taken());
        let branch_link_register_write =
            branch_update.is_some() && o3_execution_writes_link_register(record);
        let branch_predicted_target = selected_branch_prediction
            .map(|(_, predicted_target)| predicted_target)
            .unwrap_or_else(|| branch_update.and_then(|update| update.predicted_target()));
        let branch_resolved_target = branch_update.and_then(|update| update.actual_target());
        let branch_fallthrough_target = Address::new(
            record
                .pc()
                .saturating_add(u64::from(record.instruction_bytes())),
        );
        let branch_squashed_target = branch_update.and_then(|_| {
            o3_branch_squashed_target(
                branch_kind,
                branch_predicted_taken,
                branch_predicted_target,
                branch_resolved_taken,
                branch_resolved_target,
                branch_fallthrough_target,
            )
        });
        let fu_latency_cycles =
            crate::riscv_fu_latency::riscv_execute_wait_cycles(execution.instruction());

        if live_data_access.is_none() {
            for entry in &mut self.snapshot.load_store_queue {
                entry.mark_completed();
            }
            if let Some(access) = record.memory_access() {
                for entry in o3_lsq_entries(observation.sequence, access) {
                    self.snapshot.load_store_queue.push(entry);
                }
                self.stats
                    .observe_lsq_occupancy(self.snapshot.load_store_queue.len());
            }
        }
        let lsq_occupancy = live_data_access
            .map(|live| live.issue_lsq_occupancy)
            .unwrap_or(self.snapshot.load_store_queue.len());
        if let Some(live) = live_data_access {
            let rename_destination = self
                .snapshot
                .reorder_buffer
                .iter()
                .find(|entry| entry.sequence() == live.sequence)
                .copied()
                .and_then(staged_rename_entry);
            self.remove_live_data_access_rows(live.sequence, live.lsq_sequence_span);
            if let Some(rename_destination) = rename_destination {
                self.publish_live_rename_entry(rename_destination);
            }
            self.validate_live_speculative_producer(live.sequence);
        }
        let rename_map_entries = self.snapshot_with_live_rename_map().rename_map.len();
        let mut trace_record = O3RuntimeTraceRecord::new(
            observation.sequence,
            observation.issue_tick,
            observation.commit_tick,
            Address::new(record.pc()),
            observation.occupancy,
            observation.commits,
            observation.commit_blocked,
            o3_rename_write_count(record),
            lsq_loads,
            lsq_stores,
            lsq_occupancy,
            lsq_operation,
            lsq_ordering,
            lsq_load_address,
            lsq_store_address,
            lsq_load_bytes,
            lsq_store_bytes,
            o3_store_conditional_failed(execution),
            0,
            0,
            rename_map_entries,
            observation.iew_dependency_producers,
            observation.iew_dependency_consumers,
            branch_kind,
            branch_predicted_taken,
            branch_resolved_taken,
            branch_link_register_write,
            branch_predicted_target,
            branch_resolved_target,
            branch_squashed_target,
            o3_fu_latency_class(execution.instruction()),
            fu_latency_cycles,
            record.system_event().is_some(),
        );
        if let Some(admitted_writeback_tick) = observation.admitted_writeback_tick {
            trace_record.set_admitted_writeback_tick(admitted_writeback_tick);
        }

        if let Some(live) = live_data_access {
            let response_tick = live
                .response_tick
                .expect("completed live data access has a response tick");
            let latency_ticks = live
                .latency_ticks
                .expect("completed live data access has response latency");
            trace_record.set_lsq_data_response(response_tick, latency_ticks);
            if let Some(access) = record.memory_access() {
                self.stats
                    .record_lsq_operation_latency(o3_lsq_operation(access), latency_ticks);
            }
            let rename_map_entries = self.snapshot_with_live_rename_map().rename_map.len();
            self.stats.set_rename_map_entries(rename_map_entries);
            return trace_record;
        }

        if observation.drains_runtime_rob {
            let commit_tick =
                self.commit_live_rob_prefix(observation.commits, observation.commit_tick);
            trace_record.set_commit_tick(commit_tick);
        }
        let lsq_commits = self
            .snapshot
            .load_store_queue
            .iter()
            .take_while(|entry| entry.is_completed())
            .count();
        self.snapshot.load_store_queue.drain(0..lsq_commits);
        let rename_map_entries = self.snapshot_with_live_rename_map().rename_map.len();
        self.stats.set_rename_map_entries(rename_map_entries);
        trace_record
    }

    fn record_rename_map_entries(
        &mut self,
        record: &rem6_isa_riscv::RiscvExecutionRecord,
        mut staged: Option<O3RenameMapEntry>,
    ) -> Option<O3PhysicalRegisterId> {
        if record.system_event().is_some() {
            return None;
        }
        let mut first_destination = None;
        for write in record.register_writes() {
            if !write.register().is_zero() {
                let physical = self.install_or_reuse_rename_map_entry(
                    O3RegisterClass::Integer,
                    u32::from(write.register().index()),
                    &mut staged,
                );
                first_destination.get_or_insert(physical);
            }
        }
        for write in record.float_register_writes() {
            let physical = self.install_or_reuse_rename_map_entry(
                O3RegisterClass::FloatingPoint,
                u32::from(write.register().index()),
                &mut staged,
            );
            first_destination.get_or_insert(physical);
        }
        if let Some(access) = record.memory_access() {
            for (register_class, architectural) in o3_memory_destination_registers(access) {
                let physical = self.install_or_reuse_rename_map_entry(
                    register_class,
                    architectural,
                    &mut staged,
                );
                first_destination.get_or_insert(physical);
            }
        }

        first_destination
    }

    pub(super) fn record_scalar_integer_dependencies(
        &mut self,
        instruction: &rem6_isa_riscv::RiscvInstruction,
    ) -> O3ScalarIntegerDependencyObservation {
        let source_physical_registers = o3_scalar_integer_source_registers(instruction)
            .into_iter()
            .filter(|register| !register.is_zero())
            .filter_map(|register| {
                self.snapshot
                    .rename_map
                    .iter()
                    .find(|entry| {
                        entry.register_class() == O3RegisterClass::Integer
                            && entry.architectural() == u32::from(register.index())
                    })
                    .map(|entry| entry.physical())
            })
            .collect::<Vec<_>>();
        self.record_scalar_integer_dependency_sources(source_physical_registers)
    }

    pub(super) fn record_live_staged_scalar_integer_dependencies(
        &mut self,
        instruction: &rem6_isa_riscv::RiscvInstruction,
        rob_index: usize,
    ) -> O3ScalarIntegerDependencyObservation {
        let source_physical_registers = o3_scalar_integer_source_registers(instruction)
            .into_iter()
            .filter(|register| !register.is_zero())
            .filter_map(|register| {
                let architectural = u32::from(register.index());
                self.snapshot.reorder_buffer[..rob_index]
                    .iter()
                    .rev()
                    .filter_map(|entry| staged_rename_entry(*entry))
                    .find(|entry| {
                        entry.register_class() == O3RegisterClass::Integer
                            && entry.architectural() == architectural
                    })
                    .or_else(|| {
                        self.snapshot.rename_map.iter().copied().find(|entry| {
                            entry.register_class() == O3RegisterClass::Integer
                                && entry.architectural() == architectural
                        })
                    })
                    .map(|entry| entry.physical())
            })
            .collect::<Vec<_>>();
        self.record_scalar_integer_dependency_sources(source_physical_registers)
    }

    fn record_scalar_integer_dependency_sources(
        &mut self,
        source_physical_registers: impl IntoIterator<Item = O3PhysicalRegisterId>,
    ) -> O3ScalarIntegerDependencyObservation {
        let mut producer_physical_registers = BTreeSet::new();
        let mut newly_observed_producers = 0_u64;
        let mut consumers = 0_u64;
        for physical in source_physical_registers {
            producer_physical_registers.insert(physical);
            if self.dependency_producers_with_consumers.insert(physical) {
                newly_observed_producers = newly_observed_producers.saturating_add(1);
                self.stats.record_iew_dependency_producer();
            }
            consumers = consumers.saturating_add(1);
            self.stats.record_iew_dependency_consumer();
        }
        O3ScalarIntegerDependencyObservation {
            producer_physical_registers: producer_physical_registers.into_iter().collect(),
            newly_observed_producers,
            consumers,
        }
    }

    fn install_rename_map_entry(
        &mut self,
        register_class: O3RegisterClass,
        architectural: u32,
    ) -> O3PhysicalRegisterId {
        let physical = self.allocate_physical_register();
        let entry = O3RenameMapEntry::new(register_class, architectural, physical);
        if let Some(existing) = self.snapshot.rename_map.iter_mut().find(|existing| {
            existing.register_class() == register_class && existing.architectural() == architectural
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
        physical
    }

    fn install_or_reuse_rename_map_entry(
        &mut self,
        register_class: O3RegisterClass,
        architectural: u32,
        staged: &mut Option<O3RenameMapEntry>,
    ) -> O3PhysicalRegisterId {
        if staged.is_some_and(|entry| {
            entry.register_class() == register_class && entry.architectural() == architectural
        }) {
            return staged
                .take()
                .expect("matching staged rename entry is present")
                .physical();
        }
        self.install_rename_map_entry(register_class, architectural)
    }

    pub(super) fn commit_live_rob_prefix(&mut self, commits: usize, commit_tick: u64) -> u64 {
        if commits == 0 {
            return commit_tick;
        }
        let commit_tick = commit_tick.max(self.last_live_commit_tick.unwrap_or(commit_tick));
        let committed = self.snapshot.reorder_buffer[..commits].to_vec();
        for entry in &committed {
            if let Some(destination) = staged_rename_entry(*entry) {
                self.publish_live_rename_entry(destination);
            }
        }
        for instruction in &mut self.live_retired_instructions {
            if committed
                .iter()
                .any(|entry| entry.sequence() == instruction.sequence)
            {
                instruction.commit_tick = instruction.commit_tick.max(commit_tick);
            }
        }
        for (index, record) in self.trace_records.iter_mut().enumerate() {
            if committed
                .iter()
                .any(|entry| entry.sequence() == record.sequence())
                && record.set_commit_tick(commit_tick)
            {
                self.dirty_trace_record_indices.insert(index);
            }
        }
        self.snapshot.reorder_buffer.drain(0..commits);
        self.retain_live_scalar_memory_younger_sequences_in_rob();
        self.last_live_commit_tick = Some(commit_tick);
        commit_tick
    }

    pub(super) fn allocate_sequence(&mut self) -> u64 {
        self.allocate_sequence_span(1)
    }

    pub(super) fn allocate_sequence_span(&mut self, span: u64) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(span);
        sequence
    }

    pub(super) fn allocate_physical_register(&mut self) -> O3PhysicalRegisterId {
        let physical = O3PhysicalRegisterId::new(self.next_physical_register);
        self.next_physical_register = self.next_physical_register.saturating_add(1);
        physical
    }
}
