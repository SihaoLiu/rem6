use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct O3RobRetireObservation {
    sequence: u64,
    issue_tick: u64,
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
    ) -> O3RuntimeTraceRecord {
        let record = execution.execution();
        let live_retired = self.take_live_retired_instruction(execution.fetch().request_id());
        let observation = if let Some(live) = live_retired {
            self.record_rename_map_entries(record, live.rename_destination);
            O3RobRetireObservation {
                sequence: live.sequence,
                issue_tick: live.issue_tick,
                commit_tick: live.commit_tick,
                occupancy: live.rob_occupancy,
                commits: live.rob_commits,
                commit_blocked: live.rob_commit_blocked,
                iew_dependency_producers: live.iew_dependency_producers,
                iew_dependency_consumers: live.iew_dependency_consumers,
                drains_runtime_rob: false,
            }
        } else {
            let sequence = self.allocate_sequence();
            let dependencies = self.record_scalar_integer_dependencies(&record.instruction());
            let destination = self.record_rename_map_entries(record, None);
            for entry in &mut self.snapshot.reorder_buffer {
                entry.mark_ready();
            }
            let fu_latency_cycles =
                crate::riscv_execute::in_order_execute_wait_cycles(execution.instruction());
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
                commit_tick: rob_commit_tick(&self.snapshot, commits).unwrap_or(writeback_tick),
                occupancy,
                commits,
                commit_blocked,
                iew_dependency_producers: dependencies.newly_observed_producers,
                iew_dependency_consumers: dependencies.consumers,
                drains_runtime_rob: true,
            }
        };
        if matches!(
            record.memory_access(),
            Some(MemoryAccessKind::AtomicMemory { .. })
        ) {
            self.next_sequence = self.next_sequence.saturating_add(1);
        }
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
        let branch_predicted_taken = branch_update.is_some_and(|update| update.predicted_taken());
        let branch_resolved_taken = branch_update.is_some_and(|update| update.actual_taken());
        let branch_link_register_write =
            branch_update.is_some() && o3_branch_link_register_write(record);
        let branch_predicted_target = branch_update.and_then(|update| update.predicted_target());
        let branch_resolved_target = branch_update.and_then(|update| update.actual_target());
        let branch_fallthrough_target = Address::new(
            record
                .pc()
                .saturating_add(u64::from(record.instruction_bytes())),
        );
        let branch_squashed_target = branch_update.and_then(|update| {
            o3_branch_squashed_target(branch_kind, update, branch_fallthrough_target)
        });
        let fu_latency_cycles =
            crate::riscv_execute::in_order_execute_wait_cycles(execution.instruction());

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
        let lsq_occupancy = self.snapshot.load_store_queue.len();
        let rename_map_entries = self.snapshot_with_live_rename_map().rename_map.len();
        let trace_record = O3RuntimeTraceRecord::new(
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

        if observation.drains_runtime_rob {
            self.snapshot.reorder_buffer.drain(0..observation.commits);
        }
        let lsq_commits = self
            .snapshot
            .load_store_queue
            .partition_point(|entry| entry.is_completed());
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
        let mut producer_physical_registers = BTreeSet::new();
        let mut newly_observed_producers = 0_u64;
        let mut consumers = 0_u64;
        for register in o3_scalar_integer_source_registers(instruction) {
            if register.is_zero() {
                continue;
            }
            let Some(source) = self.snapshot.rename_map.iter().find(|entry| {
                entry.register_class() == O3RegisterClass::Integer
                    && entry.architectural() == u32::from(register.index())
            }) else {
                continue;
            };
            producer_physical_registers.insert(source.physical());
            if self
                .dependency_producers_with_consumers
                .insert(source.physical())
            {
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

    pub(super) fn allocate_sequence(&mut self) -> u64 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.saturating_add(1);
        sequence
    }

    pub(super) fn allocate_physical_register(&mut self) -> O3PhysicalRegisterId {
        let physical = O3PhysicalRegisterId::new(self.next_physical_register);
        self.next_physical_register = self.next_physical_register.saturating_add(1);
        physical
    }
}
