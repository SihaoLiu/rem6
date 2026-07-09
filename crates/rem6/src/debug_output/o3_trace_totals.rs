use super::*;

const fn average_ticks(total: u64, samples: u64) -> u64 {
    if samples == 0 {
        0
    } else {
        total / samples
    }
}

const fn min_latency_ticks(current: Option<u64>, latency: u64) -> Option<u64> {
    Some(match current {
        Some(current) => {
            if current < latency {
                current
            } else {
                latency
            }
        }
        None => latency,
    })
}

fn add_counter(counter: &mut u64, value: u64) {
    *counter = (*counter).saturating_add(value);
}

fn add_bool_counter(counter: &mut u64, value: bool) {
    add_counter(counter, u64::from(value));
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3TraceTotals {
    records: u64,
    execution_modes: Rem6O3ExecutionModeTraceTotals,
    stats_epoch: u64,
    stats_reset_tick: u64,
    checkpoint_restores: u64,
    checkpoint_restore_records: u64,
    checkpoint_restore_tick: u64,
    checkpoint_restore_payload_bytes: u64,
    checkpoint_restore_authority: Rem6O3CheckpointRestoreAuthorityTotals,
    instructions: u64,
    rob_allocations: u64,
    rob_commits: u64,
    rename_writes: u64,
    lsq_loads: u64,
    lsq_stores: u64,
    lsq_load_bytes: u64,
    lsq_store_bytes: u64,
    float_loads: u64,
    float_stores: u64,
    store_load_forwarding_candidates: u64,
    store_load_forwarding_matches: u64,
    store_load_forwarding_suppressed: u64,
    store_load_forwarding_address_mismatches: u64,
    store_load_forwarding_byte_mismatches: u64,
    fu_latency_instructions: u64,
    fu_latency_cycles: u64,
    fu_integer_mul_instructions: u64,
    fu_integer_mul_latency_cycles: u64,
    fu_integer_div_instructions: u64,
    fu_integer_div_latency_cycles: u64,
    max_rob_occupancy: u64,
    max_lsq_occupancy: u64,
    rename_map_entries: u64,
    event_records: u64,
    event_first_tick: Option<u64>,
    event_last_tick: Option<u64>,
    event_max_rob_occupancy: u64,
    event_max_lsq_occupancy: u64,
    event_max_rename_map_entries: u64,
    event_system_events: u64,
    event_rob_allocations: u64,
    event_rob_commits: u64,
    event_rename_writes: u64,
    event_iew: Rem6O3EventIewTotals,
    event_lsq_loads: u64,
    event_lsq_stores: u64,
    event_lsq_operation_load: u64,
    event_lsq_operation_store: u64,
    event_lsq_operation_load_reserved: u64,
    event_lsq_operation_store_conditional: u64,
    event_lsq_operation_atomic: u64,
    event_lsq_operation_float_load: u64,
    event_lsq_operation_float_store: u64,
    event_lsq_operation_vector_load: u64,
    event_lsq_operation_vector_store: u64,
    event_lsq_ordering_acquire: u64,
    event_lsq_ordering_release: u64,
    event_lsq_ordering_acquire_release: u64,
    event_lsq_store_conditional_failures: u64,
    event_lsq_data_latency_samples: u64,
    event_lsq_data_latency_ticks: u64,
    event_lsq_data_latency_max_ticks: u64,
    event_lsq_data_latency_min_ticks: Option<u64>,
    event_lsq_operation_load_latency_ticks: u64,
    event_lsq_operation_store_latency_ticks: u64,
    event_lsq_operation_load_reserved_latency_ticks: u64,
    event_lsq_operation_store_conditional_latency_ticks: u64,
    event_lsq_operation_atomic_latency_ticks: u64,
    event_lsq_operation_float_load_latency_ticks: u64,
    event_lsq_operation_float_store_latency_ticks: u64,
    event_lsq_operation_vector_load_latency_ticks: u64,
    event_lsq_operation_vector_store_latency_ticks: u64,
    event_lsq_operation_load_latency_max_ticks: u64,
    event_lsq_operation_store_latency_max_ticks: u64,
    event_lsq_operation_load_reserved_latency_max_ticks: u64,
    event_lsq_operation_store_conditional_latency_max_ticks: u64,
    event_lsq_operation_atomic_latency_max_ticks: u64,
    event_lsq_operation_float_load_latency_max_ticks: u64,
    event_lsq_operation_float_store_latency_max_ticks: u64,
    event_lsq_operation_vector_load_latency_max_ticks: u64,
    event_lsq_operation_vector_store_latency_max_ticks: u64,
    event_lsq_operation_load_latency_min_ticks: Option<u64>,
    event_lsq_operation_store_latency_min_ticks: Option<u64>,
    event_lsq_operation_load_reserved_latency_min_ticks: Option<u64>,
    event_lsq_operation_store_conditional_latency_min_ticks: Option<u64>,
    event_lsq_operation_atomic_latency_min_ticks: Option<u64>,
    event_lsq_operation_float_load_latency_min_ticks: Option<u64>,
    event_lsq_operation_float_store_latency_min_ticks: Option<u64>,
    event_lsq_operation_vector_load_latency_min_ticks: Option<u64>,
    event_lsq_operation_vector_store_latency_min_ticks: Option<u64>,
    event_branches: u64,
    event_branch_taken: u64,
    event_branch_not_taken: u64,
    event_branch_predicted_taken: u64,
    event_branch_predicted_not_taken: u64,
    event_branch_predicted_targets: u64,
    event_branch_predicted_target_matches: u64,
    event_branch_predicted_target_mismatches: u64,
    event_branch_direction_mismatches: Rem6O3BranchDirectionMismatchTotals,
    event_branch_targetless_mismatches: u64,
    event_branch_targetless_mismatch_without_link_writes: u64,
    event_branch_targetless_mismatch_squashed_targets: u64,
    event_branch_targetless_mismatch_squashed_target_without_link_writes: u64,
    event_branch_wrong_targets: u64,
    event_branch_wrong_target_squashed_targets: u64,
    event_branch_wrong_target_squashed_target_without_link_writes: u64,
    event_branch_wrong_target_link_writes: u64,
    event_branch_repairs: Rem6O3BranchRepairTotals,
    event_branch_resolved_targets: u64,
    event_branch_mispredictions: u64,
    event_branch_squashes: u64,
    event_branch_squashed_targets: u64,
    event_branch_squashed_target_without_link_writes: u64,
    event_branch_link_writes: u64,
    event_branch_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_taken_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_not_taken_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_taken_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_not_taken_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_target_match_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_predicted_target_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_targetless_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_targetless_mismatch_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_targetless_mismatch_squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_targetless_mismatch_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    event_branch_wrong_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_wrong_target_squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_wrong_target_squashed_target_without_link_write_kinds:
        [u64; BranchTargetKind::COUNT],
    event_branch_wrong_target_link_write_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_resolved_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_link_write_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_misprediction_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_squash_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    event_branch_squashed_target_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    event_lsq_load_bytes: u64,
    event_lsq_store_bytes: u64,
    event_store_load_forwarding_candidates: u64,
    event_store_load_forwarding_matches: u64,
    event_store_load_forwarding_suppressed: u64,
    event_store_load_forwarding_address_mismatches: u64,
    event_store_load_forwarding_byte_mismatches: u64,
    event_fu_latency_instructions: u64,
    event_fu_latency_cycles: u64,
    event_fu_latency_max_cycles: u64,
    event_fu_latency_min_cycles: Option<u64>,
    event_fu_latency_classes: [Rem6O3FuLatencyClassTotals; O3RuntimeFuLatencyClass::COUNT],
}

impl Rem6O3TraceTotals {
    pub(super) fn add(&mut self, record: &Rem6O3TraceRecord) {
        let stats = record.stats();
        self.records = self.records.saturating_add(1);
        self.stats_epoch = self.stats_epoch.max(record.stats_epoch());
        self.stats_reset_tick = self.stats_reset_tick.max(record.stats_reset_tick());
        self.execution_modes.add_record(record);
        if let Some(restore) = record.checkpoint_restore() {
            self.checkpoint_restores = self.checkpoint_restores.max(restore.count);
            self.checkpoint_restore_records = self.checkpoint_restore_records.saturating_add(1);
            self.checkpoint_restore_tick = self.checkpoint_restore_tick.max(restore.tick);
            self.checkpoint_restore_payload_bytes = self
                .checkpoint_restore_payload_bytes
                .max(restore.payload_bytes);
            self.checkpoint_restore_authority.add(restore);
        }
        add_counter(&mut self.instructions, stats.instructions());
        add_counter(&mut self.rob_allocations, stats.rob_allocations());
        add_counter(&mut self.rob_commits, stats.rob_commits());
        add_counter(&mut self.rename_writes, stats.rename_writes());
        add_counter(&mut self.lsq_loads, stats.lsq_loads());
        add_counter(&mut self.lsq_stores, stats.lsq_stores());
        add_counter(&mut self.lsq_load_bytes, stats.lsq_load_bytes());
        add_counter(&mut self.lsq_store_bytes, stats.lsq_store_bytes());
        add_counter(
            &mut self.store_load_forwarding_candidates,
            stats.lsq_store_to_load_forwarding_candidates(),
        );
        add_counter(
            &mut self.store_load_forwarding_matches,
            stats.lsq_store_to_load_forwarding_matches(),
        );
        add_counter(
            &mut self.store_load_forwarding_suppressed,
            stats.lsq_store_to_load_forwarding_suppressed(),
        );
        add_counter(
            &mut self.store_load_forwarding_address_mismatches,
            stats.lsq_store_to_load_forwarding_address_mismatches(),
        );
        add_counter(
            &mut self.store_load_forwarding_byte_mismatches,
            stats.lsq_store_to_load_forwarding_byte_mismatches(),
        );
        add_counter(
            &mut self.fu_latency_instructions,
            stats.fu_latency_instructions(),
        );
        add_counter(&mut self.fu_latency_cycles, stats.fu_latency_cycles());
        add_counter(
            &mut self.fu_integer_mul_instructions,
            stats.fu_integer_mul_instructions(),
        );
        add_counter(
            &mut self.fu_integer_mul_latency_cycles,
            stats.fu_integer_mul_latency_cycles(),
        );
        add_counter(
            &mut self.fu_integer_div_instructions,
            stats.fu_integer_div_instructions(),
        );
        add_counter(
            &mut self.fu_integer_div_latency_cycles,
            stats.fu_integer_div_latency_cycles(),
        );
        self.max_rob_occupancy = self.max_rob_occupancy.max(stats.max_rob_occupancy());
        self.max_lsq_occupancy = self.max_lsq_occupancy.max(stats.max_lsq_occupancy());
        add_counter(&mut self.rename_map_entries, stats.rename_map_entries());
        for event in record.events() {
            let event_tick = event.tick();
            add_counter(&mut self.event_records, 1);
            self.event_first_tick = Some(
                self.event_first_tick
                    .map_or(event_tick, |tick| tick.min(event_tick)),
            );
            self.event_last_tick = Some(
                self.event_last_tick
                    .map_or(event_tick, |tick| tick.max(event_tick)),
            );
            self.event_max_rob_occupancy = self.event_max_rob_occupancy.max(event.rob_occupancy());
            self.event_max_lsq_occupancy = self.event_max_lsq_occupancy.max(event.lsq_occupancy());
            self.event_max_rename_map_entries = self
                .event_max_rename_map_entries
                .max(event.rename_map_entries());
            add_bool_counter(&mut self.event_system_events, event.system_event());
            add_bool_counter(&mut self.event_rob_allocations, event.rob_allocated());
            add_bool_counter(&mut self.event_rob_commits, event.rob_committed());
            add_counter(&mut self.event_rename_writes, event.rename_writes());
            self.event_iew.add_event(*event);
            add_counter(&mut self.event_lsq_loads, event.lsq_loads());
            add_counter(&mut self.event_lsq_stores, event.lsq_stores());
            let lsq_data_latency_ticks = event.lsq_data_latency_ticks();
            let lsq_operation = event.lsq_operation();
            if lsq_operation != O3RuntimeLsqOperation::None {
                add_counter(&mut self.event_lsq_data_latency_samples, 1);
                self.event_lsq_data_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_data_latency_min_ticks,
                    lsq_data_latency_ticks,
                );
            }
            self.add_event_lsq_operation(lsq_operation, lsq_data_latency_ticks);
            self.add_event_lsq_ordering(event.lsq_ordering());
            add_bool_counter(
                &mut self.event_lsq_store_conditional_failures,
                event.lsq_store_conditional_failed(),
            );
            add_counter(
                &mut self.event_lsq_data_latency_ticks,
                lsq_data_latency_ticks,
            );
            self.event_lsq_data_latency_max_ticks = self
                .event_lsq_data_latency_max_ticks
                .max(lsq_data_latency_ticks);
            self.add_event_branch(event);
            add_counter(&mut self.event_lsq_load_bytes, event.lsq_load_bytes());
            add_counter(&mut self.event_lsq_store_bytes, event.lsq_store_bytes());
            add_bool_counter(
                &mut self.event_store_load_forwarding_candidates,
                event.store_load_forwarding_candidate(),
            );
            add_bool_counter(
                &mut self.event_store_load_forwarding_matches,
                event.store_load_forwarding_match(),
            );
            add_bool_counter(
                &mut self.event_store_load_forwarding_suppressed,
                event.store_load_forwarding_suppressed(),
            );
            add_bool_counter(
                &mut self.event_store_load_forwarding_address_mismatches,
                event.store_load_forwarding_address_mismatch(),
            );
            add_bool_counter(
                &mut self.event_store_load_forwarding_byte_mismatches,
                event.store_load_forwarding_byte_mismatch(),
            );
            let fu_latency_cycles = event.fu_latency_cycles();
            self.event_fu_latency_cycles = self
                .event_fu_latency_cycles
                .saturating_add(fu_latency_cycles);
            if fu_latency_cycles > 0 {
                self.event_fu_latency_instructions =
                    self.event_fu_latency_instructions.saturating_add(1);
                self.event_fu_latency_max_cycles =
                    self.event_fu_latency_max_cycles.max(fu_latency_cycles);
                self.event_fu_latency_min_cycles =
                    min_latency_ticks(self.event_fu_latency_min_cycles, fu_latency_cycles);
                if let Some(class) = event.fu_latency_class() {
                    self.event_fu_latency_classes[class.index()].add(fu_latency_cycles);
                }
            }
        }
    }

    fn add_event_branch(&mut self, event: &O3RuntimeTraceRecord) {
        if !event.branch_event() {
            return;
        }
        self.event_branches = self.event_branches.saturating_add(1);
        self.event_branch_taken = self
            .event_branch_taken
            .saturating_add(u64::from(event.branch_resolved_taken()));
        self.event_branch_not_taken = self
            .event_branch_not_taken
            .saturating_add(u64::from(!event.branch_resolved_taken()));
        self.event_branch_predicted_taken = self
            .event_branch_predicted_taken
            .saturating_add(u64::from(event.branch_predicted_taken()));
        self.event_branch_predicted_not_taken = self
            .event_branch_predicted_not_taken
            .saturating_add(u64::from(!event.branch_predicted_taken()));
        self.event_branch_predicted_targets = self
            .event_branch_predicted_targets
            .saturating_add(u64::from(event.branch_predicted_target().is_some()));
        let predicted_target_matches = event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) == event.branch_resolved_target());
        let predicted_target_mismatches = event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) != event.branch_resolved_target());
        let direction_mismatch = event.branch_predicted_taken() != event.branch_resolved_taken();
        let targetless_mismatch = o3_branch_targetless_mismatch(event);
        let targetless_mismatch_without_link_write =
            targetless_mismatch && !event.branch_link_register_write();
        let targetless_mismatch_squashed_target =
            targetless_mismatch && event.branch_squashed_target().is_some();
        let targetless_mismatch_squashed_target_without_link_write =
            targetless_mismatch_squashed_target && !event.branch_link_register_write();
        let wrong_target = o3_branch_wrong_target(event);
        let wrong_target_squashed_target = wrong_target && event.branch_squashed_target().is_some();
        let wrong_target_squashed_target_without_link_write =
            wrong_target_squashed_target && !event.branch_link_register_write();
        let wrong_target_link_write = wrong_target && event.branch_link_register_write();
        let repair = o3_branch_repair_kind(event);
        let squashed_target = event.branch_squashed_target().is_some();
        let squashed_target_without_link_write =
            squashed_target && !event.branch_link_register_write();
        self.event_branch_predicted_target_matches = self
            .event_branch_predicted_target_matches
            .saturating_add(u64::from(predicted_target_matches));
        self.event_branch_predicted_target_mismatches = self
            .event_branch_predicted_target_mismatches
            .saturating_add(u64::from(predicted_target_mismatches));
        self.event_branch_direction_mismatches.add_event(
            event,
            direction_mismatch,
            squashed_target,
        );
        self.event_branch_targetless_mismatches = self
            .event_branch_targetless_mismatches
            .saturating_add(u64::from(targetless_mismatch));
        self.event_branch_targetless_mismatch_without_link_writes = self
            .event_branch_targetless_mismatch_without_link_writes
            .saturating_add(u64::from(targetless_mismatch_without_link_write));
        self.event_branch_targetless_mismatch_squashed_targets = self
            .event_branch_targetless_mismatch_squashed_targets
            .saturating_add(u64::from(targetless_mismatch_squashed_target));
        self.event_branch_targetless_mismatch_squashed_target_without_link_writes = self
            .event_branch_targetless_mismatch_squashed_target_without_link_writes
            .saturating_add(u64::from(
                targetless_mismatch_squashed_target_without_link_write,
            ));
        self.event_branch_wrong_targets = self
            .event_branch_wrong_targets
            .saturating_add(u64::from(wrong_target));
        self.event_branch_wrong_target_squashed_targets = self
            .event_branch_wrong_target_squashed_targets
            .saturating_add(u64::from(wrong_target_squashed_target));
        self.event_branch_wrong_target_squashed_target_without_link_writes = self
            .event_branch_wrong_target_squashed_target_without_link_writes
            .saturating_add(u64::from(wrong_target_squashed_target_without_link_write));
        self.event_branch_wrong_target_link_writes = self
            .event_branch_wrong_target_link_writes
            .saturating_add(u64::from(wrong_target_link_write));
        self.event_branch_repairs.add_event(event, repair);
        self.event_branch_resolved_targets = self
            .event_branch_resolved_targets
            .saturating_add(u64::from(event.branch_resolved_target().is_some()));
        self.event_branch_mispredictions = self
            .event_branch_mispredictions
            .saturating_add(u64::from(event.branch_mispredicted()));
        self.event_branch_squashes = self
            .event_branch_squashes
            .saturating_add(u64::from(event.branch_squash()));
        self.event_branch_squashed_targets = self
            .event_branch_squashed_targets
            .saturating_add(u64::from(squashed_target));
        self.event_branch_squashed_target_without_link_writes = self
            .event_branch_squashed_target_without_link_writes
            .saturating_add(u64::from(squashed_target_without_link_write));
        self.event_branch_link_writes = self
            .event_branch_link_writes
            .saturating_add(u64::from(event.branch_link_register_write()));
        let index = event.branch_kind().index();
        self.event_branch_kinds[index] = self.event_branch_kinds[index].saturating_add(1);
        if event.branch_predicted_target().is_some() {
            self.event_branch_predicted_target_kinds[index] =
                self.event_branch_predicted_target_kinds[index].saturating_add(1);
        }
        if event.branch_resolved_taken() {
            self.event_branch_taken_kinds[index] =
                self.event_branch_taken_kinds[index].saturating_add(1);
        } else {
            self.event_branch_not_taken_kinds[index] =
                self.event_branch_not_taken_kinds[index].saturating_add(1);
        }
        if event.branch_predicted_taken() {
            self.event_branch_predicted_taken_kinds[index] =
                self.event_branch_predicted_taken_kinds[index].saturating_add(1);
        } else {
            self.event_branch_predicted_not_taken_kinds[index] =
                self.event_branch_predicted_not_taken_kinds[index].saturating_add(1);
        }
        if predicted_target_matches {
            self.event_branch_predicted_target_match_kinds[index] =
                self.event_branch_predicted_target_match_kinds[index].saturating_add(1);
        }
        if predicted_target_mismatches {
            self.event_branch_predicted_target_mismatch_kinds[index] =
                self.event_branch_predicted_target_mismatch_kinds[index].saturating_add(1);
        }
        if targetless_mismatch {
            self.event_branch_targetless_mismatch_kinds[index] =
                self.event_branch_targetless_mismatch_kinds[index].saturating_add(1);
        }
        if targetless_mismatch_without_link_write {
            self.event_branch_targetless_mismatch_without_link_write_kinds[index] = self
                .event_branch_targetless_mismatch_without_link_write_kinds[index]
                .saturating_add(1);
        }
        if targetless_mismatch_squashed_target {
            self.event_branch_targetless_mismatch_squashed_target_kinds[index] = self
                .event_branch_targetless_mismatch_squashed_target_kinds[index]
                .saturating_add(1);
        }
        if targetless_mismatch_squashed_target_without_link_write {
            self.event_branch_targetless_mismatch_squashed_target_without_link_write_kinds[index] =
                self.event_branch_targetless_mismatch_squashed_target_without_link_write_kinds
                    [index]
                    .saturating_add(1);
        }
        if wrong_target {
            self.event_branch_wrong_target_kinds[index] =
                self.event_branch_wrong_target_kinds[index].saturating_add(1);
        }
        if wrong_target_squashed_target {
            self.event_branch_wrong_target_squashed_target_kinds[index] =
                self.event_branch_wrong_target_squashed_target_kinds[index].saturating_add(1);
        }
        if wrong_target_squashed_target_without_link_write {
            self.event_branch_wrong_target_squashed_target_without_link_write_kinds[index] = self
                .event_branch_wrong_target_squashed_target_without_link_write_kinds[index]
                .saturating_add(1);
        }
        if wrong_target_link_write {
            self.event_branch_wrong_target_link_write_kinds[index] =
                self.event_branch_wrong_target_link_write_kinds[index].saturating_add(1);
        }
        if event.branch_resolved_target().is_some() {
            self.event_branch_resolved_target_kinds[index] =
                self.event_branch_resolved_target_kinds[index].saturating_add(1);
        }
        if event.branch_link_register_write() {
            self.event_branch_link_write_kinds[index] =
                self.event_branch_link_write_kinds[index].saturating_add(1);
        }
        if event.branch_mispredicted() {
            self.event_branch_misprediction_kinds[index] =
                self.event_branch_misprediction_kinds[index].saturating_add(1);
        }
        if event.branch_squash() {
            self.event_branch_squash_kinds[index] =
                self.event_branch_squash_kinds[index].saturating_add(1);
        }
        if event.branch_squashed_target().is_some() {
            self.event_branch_squashed_target_kinds[index] =
                self.event_branch_squashed_target_kinds[index].saturating_add(1);
        }
        if squashed_target_without_link_write {
            self.event_branch_squashed_target_without_link_write_kinds[index] =
                self.event_branch_squashed_target_without_link_write_kinds[index].saturating_add(1);
        }
    }

    fn add_event_lsq_ordering(&mut self, ordering: O3RuntimeLsqOrdering) {
        match ordering {
            O3RuntimeLsqOrdering::None => {}
            O3RuntimeLsqOrdering::Acquire => {
                self.event_lsq_ordering_acquire = self.event_lsq_ordering_acquire.saturating_add(1);
            }
            O3RuntimeLsqOrdering::Release => {
                self.event_lsq_ordering_release = self.event_lsq_ordering_release.saturating_add(1);
            }
            O3RuntimeLsqOrdering::AcquireRelease => {
                self.event_lsq_ordering_acquire_release =
                    self.event_lsq_ordering_acquire_release.saturating_add(1);
            }
        }
    }

    fn add_event_lsq_operation(&mut self, operation: O3RuntimeLsqOperation, latency_ticks: u64) {
        match operation {
            O3RuntimeLsqOperation::None => {}
            O3RuntimeLsqOperation::Load => {
                self.event_lsq_operation_load = self.event_lsq_operation_load.saturating_add(1);
                self.event_lsq_operation_load_latency_ticks = self
                    .event_lsq_operation_load_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_load_latency_max_ticks = self
                    .event_lsq_operation_load_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_load_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_load_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::Store => {
                self.event_lsq_operation_store = self.event_lsq_operation_store.saturating_add(1);
                self.event_lsq_operation_store_latency_ticks = self
                    .event_lsq_operation_store_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_store_latency_max_ticks = self
                    .event_lsq_operation_store_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_store_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_store_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::LoadReserved => {
                self.event_lsq_operation_load_reserved =
                    self.event_lsq_operation_load_reserved.saturating_add(1);
                self.event_lsq_operation_load_reserved_latency_ticks = self
                    .event_lsq_operation_load_reserved_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_load_reserved_latency_max_ticks = self
                    .event_lsq_operation_load_reserved_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_load_reserved_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_load_reserved_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::StoreConditional => {
                self.event_lsq_operation_store_conditional =
                    self.event_lsq_operation_store_conditional.saturating_add(1);
                self.event_lsq_operation_store_conditional_latency_ticks = self
                    .event_lsq_operation_store_conditional_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_store_conditional_latency_max_ticks = self
                    .event_lsq_operation_store_conditional_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_store_conditional_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_store_conditional_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::Atomic => {
                self.event_lsq_operation_atomic = self.event_lsq_operation_atomic.saturating_add(1);
                self.event_lsq_operation_atomic_latency_ticks = self
                    .event_lsq_operation_atomic_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_atomic_latency_max_ticks = self
                    .event_lsq_operation_atomic_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_atomic_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_atomic_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::FloatLoad => {
                self.float_loads = self.float_loads.saturating_add(1);
                self.event_lsq_operation_float_load =
                    self.event_lsq_operation_float_load.saturating_add(1);
                self.event_lsq_operation_float_load_latency_ticks = self
                    .event_lsq_operation_float_load_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_float_load_latency_max_ticks = self
                    .event_lsq_operation_float_load_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_float_load_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_float_load_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::FloatStore => {
                self.float_stores = self.float_stores.saturating_add(1);
                self.event_lsq_operation_float_store =
                    self.event_lsq_operation_float_store.saturating_add(1);
                self.event_lsq_operation_float_store_latency_ticks = self
                    .event_lsq_operation_float_store_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_float_store_latency_max_ticks = self
                    .event_lsq_operation_float_store_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_float_store_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_float_store_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::VectorLoad => {
                self.event_lsq_operation_vector_load =
                    self.event_lsq_operation_vector_load.saturating_add(1);
                self.event_lsq_operation_vector_load_latency_ticks = self
                    .event_lsq_operation_vector_load_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_vector_load_latency_max_ticks = self
                    .event_lsq_operation_vector_load_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_vector_load_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_vector_load_latency_min_ticks,
                    latency_ticks,
                );
            }
            O3RuntimeLsqOperation::VectorStore => {
                self.event_lsq_operation_vector_store =
                    self.event_lsq_operation_vector_store.saturating_add(1);
                self.event_lsq_operation_vector_store_latency_ticks = self
                    .event_lsq_operation_vector_store_latency_ticks
                    .saturating_add(latency_ticks);
                self.event_lsq_operation_vector_store_latency_max_ticks = self
                    .event_lsq_operation_vector_store_latency_max_ticks
                    .max(latency_ticks);
                self.event_lsq_operation_vector_store_latency_min_ticks = min_latency_ticks(
                    self.event_lsq_operation_vector_store_latency_min_ticks,
                    latency_ticks,
                );
            }
        }
    }

    pub(super) fn stats(self) -> Vec<Rem6O3TraceStat> {
        let mut stats = Vec::new();
        for (suffix, value) in [
            ("records", self.records),
            ("stats_epoch", self.stats_epoch),
            ("checkpoint_restores", self.checkpoint_restores),
            (
                "checkpoint_restore_records",
                self.checkpoint_restore_records,
            ),
            ("instructions", self.instructions),
            ("rob_allocations", self.rob_allocations),
            ("rob_commits", self.rob_commits),
            ("rename_writes", self.rename_writes),
            ("lsq_loads", self.lsq_loads),
            ("lsq_stores", self.lsq_stores),
            ("float_loads", self.float_loads),
            ("float_stores", self.float_stores),
            (
                "store_load_forwarding_candidates",
                self.store_load_forwarding_candidates,
            ),
            (
                "store_load_forwarding_matches",
                self.store_load_forwarding_matches,
            ),
            (
                "store_load_forwarding_suppressed",
                self.store_load_forwarding_suppressed,
            ),
            (
                "store_load_forwarding_address_mismatches",
                self.store_load_forwarding_address_mismatches,
            ),
            (
                "store_load_forwarding_byte_mismatches",
                self.store_load_forwarding_byte_mismatches,
            ),
            ("fu_latency_instructions", self.fu_latency_instructions),
            (
                "fu_integer_mul_instructions",
                self.fu_integer_mul_instructions,
            ),
            (
                "fu_integer_div_instructions",
                self.fu_integer_div_instructions,
            ),
            ("max_rob_occupancy", self.max_rob_occupancy),
            ("max_lsq_occupancy", self.max_lsq_occupancy),
            ("rename_map_entries", self.rename_map_entries),
            ("event.records", self.event_records),
            ("event.max_rob_occupancy", self.event_max_rob_occupancy),
            ("event.max_lsq_occupancy", self.event_max_lsq_occupancy),
            (
                "event.max_rename_map_entries",
                self.event_max_rename_map_entries,
            ),
            ("event.system_events", self.event_system_events),
            ("event.rob_allocations", self.event_rob_allocations),
            ("event.rob_commits", self.event_rob_commits),
            ("event.rename_writes", self.event_rename_writes),
            ("event.lsq_loads", self.event_lsq_loads),
            ("event.lsq_stores", self.event_lsq_stores),
            ("event.lsq_operation.load", self.event_lsq_operation_load),
            ("event.lsq_operation.store", self.event_lsq_operation_store),
            (
                "event.lsq_operation.load_reserved",
                self.event_lsq_operation_load_reserved,
            ),
            (
                "event.lsq_operation.store_conditional",
                self.event_lsq_operation_store_conditional,
            ),
            (
                "event.lsq_operation.atomic",
                self.event_lsq_operation_atomic,
            ),
            (
                "event.lsq_operation.float_load",
                self.event_lsq_operation_float_load,
            ),
            (
                "event.lsq_operation.float_store",
                self.event_lsq_operation_float_store,
            ),
            (
                "event.lsq_operation.vector_load",
                self.event_lsq_operation_vector_load,
            ),
            (
                "event.lsq_operation.vector_store",
                self.event_lsq_operation_vector_store,
            ),
            (
                "event.lsq_ordering.acquire",
                self.event_lsq_ordering_acquire,
            ),
            (
                "event.lsq_ordering.release",
                self.event_lsq_ordering_release,
            ),
            (
                "event.lsq_ordering.acquire_release",
                self.event_lsq_ordering_acquire_release,
            ),
            (
                "event.lsq_store_conditional_failures",
                self.event_lsq_store_conditional_failures,
            ),
            ("event.branches", self.event_branches),
            ("event.branch_taken", self.event_branch_taken),
            ("event.branch_not_taken", self.event_branch_not_taken),
            (
                "event.branch_predicted_taken",
                self.event_branch_predicted_taken,
            ),
            (
                "event.branch_predicted_not_taken",
                self.event_branch_predicted_not_taken,
            ),
            (
                "event.branch_predicted_targets",
                self.event_branch_predicted_targets,
            ),
            (
                "event.branch_predicted_target_matches",
                self.event_branch_predicted_target_matches,
            ),
            (
                "event.branch_predicted_target_mismatches",
                self.event_branch_predicted_target_mismatches,
            ),
            (
                "event.branch_targetless_mismatches",
                self.event_branch_targetless_mismatches,
            ),
            (
                "event.branch_targetless_mismatch_without_link_writes",
                self.event_branch_targetless_mismatch_without_link_writes,
            ),
            (
                "event.branch_targetless_mismatch_squashed_targets",
                self.event_branch_targetless_mismatch_squashed_targets,
            ),
            (
                "event.branch_targetless_mismatch_squashed_target_without_link_writes",
                self.event_branch_targetless_mismatch_squashed_target_without_link_writes,
            ),
            (
                "event.branch_wrong_targets",
                self.event_branch_wrong_targets,
            ),
            (
                "event.branch_wrong_target_squashed_targets",
                self.event_branch_wrong_target_squashed_targets,
            ),
            (
                "event.branch_wrong_target_squashed_target_without_link_writes",
                self.event_branch_wrong_target_squashed_target_without_link_writes,
            ),
            (
                "event.branch_wrong_target_squashed_target_link_writes",
                self.event_branch_wrong_target_squashed_targets
                    .saturating_sub(
                        self.event_branch_wrong_target_squashed_target_without_link_writes,
                    ),
            ),
            (
                "event.branch_wrong_target_link_writes",
                self.event_branch_wrong_target_link_writes,
            ),
            (
                "event.branch_wrong_target_without_link_writes",
                self.event_branch_wrong_targets
                    .saturating_sub(self.event_branch_wrong_target_link_writes),
            ),
            (
                "event.branch_resolved_targets",
                self.event_branch_resolved_targets,
            ),
            (
                "event.branch_mispredictions",
                self.event_branch_mispredictions,
            ),
            ("event.branch_squashes", self.event_branch_squashes),
            (
                "event.branch_squashed_targets",
                self.event_branch_squashed_targets,
            ),
            (
                "event.branch_squashed_target_without_link_writes",
                self.event_branch_squashed_target_without_link_writes,
            ),
            (
                "event.branch_squashed_target_link_writes",
                self.event_branch_squashed_targets
                    .saturating_sub(self.event_branch_squashed_target_without_link_writes),
            ),
            ("event.branch_link_writes", self.event_branch_link_writes),
            (
                "event.store_load_forwarding_candidates",
                self.event_store_load_forwarding_candidates,
            ),
            (
                "event.store_load_forwarding_matches",
                self.event_store_load_forwarding_matches,
            ),
            (
                "event.store_load_forwarding_suppressed",
                self.event_store_load_forwarding_suppressed,
            ),
            (
                "event.store_load_forwarding_address_mismatches",
                self.event_store_load_forwarding_address_mismatches,
            ),
            (
                "event.store_load_forwarding_byte_mismatches",
                self.event_store_load_forwarding_byte_mismatches,
            ),
            (
                "event.fu_latency_instructions",
                self.event_fu_latency_instructions,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }
        self.execution_modes.push_stats(&mut stats);
        let event_tick_span = self
            .event_last_tick
            .unwrap_or(0)
            .saturating_sub(self.event_first_tick.unwrap_or(0));
        stats.push(Rem6O3TraceStat {
            suffix: "event.iew_writeback_rate_ppm",
            unit: "Ppm",
            value: self.event_iew.writeback_rate_ppm(event_tick_span),
        });
        for (suffix, value) in [
            ("event.iq_insts_issued", self.event_records),
            (
                "event.iq_mem_insts_issued",
                self.event_lsq_loads.saturating_add(self.event_lsq_stores),
            ),
            ("event.iq_branch_insts_issued", self.event_branches),
            ("event.iq_issued_inst_type.mem_read", self.event_lsq_loads),
            ("event.iq_issued_inst_type.mem_write", self.event_lsq_stores),
            (
                "event.commit_branch_mispredicts",
                self.event_iew.branch_mispredicts(),
            ),
            (
                "event.commit_committed_inst_type.mem_read",
                self.event_lsq_loads,
            ),
            (
                "event.commit_committed_inst_type.mem_write",
                self.event_lsq_stores,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }
        for (suffix, value) in self.event_iew.stats() {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }
        for class_stats in REM6_O3_FU_LATENCY_CLASS_STATS {
            let value = self.event_fu_latency_classes[class_stats.class.index()].instructions;
            stats.push(Rem6O3TraceStat {
                suffix: class_stats.instructions,
                unit: "Count",
                value,
            });
            stats.push(Rem6O3TraceStat {
                suffix: o3_event_iq_issued_inst_type_stat_suffix(class_stats.class),
                unit: "Count",
                value,
            });
            stats.push(Rem6O3TraceStat {
                suffix: o3_event_commit_committed_inst_type_stat_suffix(class_stats.class),
                unit: "Count",
                value,
            });
        }
        self.checkpoint_restore_authority.push_stats(&mut stats);
        self.event_branch_direction_mismatches
            .push_stats(&mut stats);
        self.event_branch_repairs.push_stats(&mut stats);
        push_o3_branch_kind_count_stats(&mut stats, o3_branch_kind_stat_suffix, |kind| {
            self.event_branch_kinds[kind.index()]
        });
        push_o3_branch_kind_count_stats(&mut stats, o3_branch_taken_kind_stat_suffix, |kind| {
            self.event_branch_taken_kinds[kind.index()]
        });
        push_o3_branch_kind_count_stats(&mut stats, o3_branch_not_taken_kind_stat_suffix, |kind| {
            self.event_branch_not_taken_kinds[kind.index()]
        });
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_predicted_taken_kind_stat_suffix,
            |kind| self.event_branch_predicted_taken_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_predicted_not_taken_kind_stat_suffix,
            |kind| self.event_branch_predicted_not_taken_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_predicted_target_kind_stat_suffix,
            |kind| self.event_branch_predicted_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_predicted_target_match_kind_stat_suffix,
            |kind| self.event_branch_predicted_target_match_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_predicted_target_mismatch_kind_stat_suffix,
            |kind| self.event_branch_predicted_target_mismatch_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_targetless_mismatch_kind_stat_suffix,
            |kind| self.event_branch_targetless_mismatch_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_targetless_mismatch_without_link_write_kind_stat_suffix,
            |kind| self.event_branch_targetless_mismatch_without_link_write_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_targetless_mismatch_squashed_target_kind_stat_suffix,
            |kind| self.event_branch_targetless_mismatch_squashed_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_targetless_mismatch_squashed_target_without_link_write_kind_stat_suffix,
            |kind| {
                self.event_branch_targetless_mismatch_squashed_target_without_link_write_kinds
                    [kind.index()]
            },
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_wrong_target_kind_stat_suffix,
            |kind| self.event_branch_wrong_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_wrong_target_squashed_target_kind_stat_suffix,
            |kind| self.event_branch_wrong_target_squashed_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_wrong_target_squashed_target_without_link_write_kind_stat_suffix,
            |kind| {
                self.event_branch_wrong_target_squashed_target_without_link_write_kinds
                    [kind.index()]
            },
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_wrong_target_squashed_target_link_write_kind_stat_suffix,
            |kind| {
                self.event_branch_wrong_target_squashed_target_kinds[kind.index()].saturating_sub(
                    self.event_branch_wrong_target_squashed_target_without_link_write_kinds
                        [kind.index()],
                )
            },
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_wrong_target_link_write_kind_stat_suffix,
            |kind| self.event_branch_wrong_target_link_write_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_wrong_target_without_link_write_kind_stat_suffix,
            |kind| {
                self.event_branch_wrong_target_kinds[kind.index()]
                    .saturating_sub(self.event_branch_wrong_target_link_write_kinds[kind.index()])
            },
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_resolved_target_kind_stat_suffix,
            |kind| self.event_branch_resolved_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_link_write_kind_stat_suffix,
            |kind| self.event_branch_link_write_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_misprediction_kind_stat_suffix,
            |kind| self.event_branch_misprediction_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(&mut stats, o3_branch_squash_kind_stat_suffix, |kind| {
            self.event_branch_squash_kinds[kind.index()]
        });
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_squashed_target_kind_stat_suffix,
            |kind| self.event_branch_squashed_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_squashed_target_without_link_write_kind_stat_suffix,
            |kind| self.event_branch_squashed_target_without_link_write_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            &mut stats,
            o3_branch_squashed_target_link_write_kind_stat_suffix,
            |kind| {
                self.event_branch_squashed_target_kinds[kind.index()].saturating_sub(
                    self.event_branch_squashed_target_without_link_write_kinds[kind.index()],
                )
            },
        );
        stats.push(Rem6O3TraceStat {
            suffix: "fu_latency_cycles",
            unit: "Cycle",
            value: self.fu_latency_cycles,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "stats_reset_tick",
            unit: "Tick",
            value: self.stats_reset_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "checkpoint_restore_tick",
            unit: "Tick",
            value: self.checkpoint_restore_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "checkpoint_restore_payload_bytes",
            unit: "Byte",
            value: self.checkpoint_restore_payload_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "lsq_load_bytes",
            unit: "Byte",
            value: self.lsq_load_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "lsq_store_bytes",
            unit: "Byte",
            value: self.lsq_store_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_load_bytes",
            unit: "Byte",
            value: self.event_lsq_load_bytes,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_store_bytes",
            unit: "Byte",
            value: self.event_lsq_store_bytes,
        });
        let first_event_tick = self.event_first_tick.unwrap_or(0);
        let last_event_tick = self.event_last_tick.unwrap_or(0);
        stats.push(Rem6O3TraceStat {
            suffix: "event.first_tick",
            unit: "Tick",
            value: first_event_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.last_tick",
            unit: "Tick",
            value: last_event_tick,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.tick_span",
            unit: "Tick",
            value: event_tick_span,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_data_latency_ticks",
            unit: "Tick",
            value: self.event_lsq_data_latency_ticks,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_data_latency_samples",
            unit: "Count",
            value: self.event_lsq_data_latency_samples,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_data_latency_max_ticks",
            unit: "Tick",
            value: self.event_lsq_data_latency_max_ticks,
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_data_latency_min_ticks",
            unit: "Tick",
            value: self.event_lsq_data_latency_min_ticks.unwrap_or(0),
        });
        stats.push(Rem6O3TraceStat {
            suffix: "event.lsq_data_latency_avg_ticks",
            unit: "Tick",
            value: average_ticks(
                self.event_lsq_data_latency_ticks,
                self.event_lsq_data_latency_samples,
            ),
        });
        for (suffix, value) in [
            (
                "event.lsq_operation.load_latency_ticks",
                self.event_lsq_operation_load_latency_ticks,
            ),
            (
                "event.lsq_operation.store_latency_ticks",
                self.event_lsq_operation_store_latency_ticks,
            ),
            (
                "event.lsq_operation.load_reserved_latency_ticks",
                self.event_lsq_operation_load_reserved_latency_ticks,
            ),
            (
                "event.lsq_operation.store_conditional_latency_ticks",
                self.event_lsq_operation_store_conditional_latency_ticks,
            ),
            (
                "event.lsq_operation.atomic_latency_ticks",
                self.event_lsq_operation_atomic_latency_ticks,
            ),
            (
                "event.lsq_operation.float_load_latency_ticks",
                self.event_lsq_operation_float_load_latency_ticks,
            ),
            (
                "event.lsq_operation.float_store_latency_ticks",
                self.event_lsq_operation_float_store_latency_ticks,
            ),
            (
                "event.lsq_operation.vector_load_latency_ticks",
                self.event_lsq_operation_vector_load_latency_ticks,
            ),
            (
                "event.lsq_operation.vector_store_latency_ticks",
                self.event_lsq_operation_vector_store_latency_ticks,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Tick",
                value,
            });
        }
        for (suffix, value) in [
            (
                "event.lsq_operation.load_latency_max_ticks",
                self.event_lsq_operation_load_latency_max_ticks,
            ),
            (
                "event.lsq_operation.store_latency_max_ticks",
                self.event_lsq_operation_store_latency_max_ticks,
            ),
            (
                "event.lsq_operation.load_reserved_latency_max_ticks",
                self.event_lsq_operation_load_reserved_latency_max_ticks,
            ),
            (
                "event.lsq_operation.store_conditional_latency_max_ticks",
                self.event_lsq_operation_store_conditional_latency_max_ticks,
            ),
            (
                "event.lsq_operation.atomic_latency_max_ticks",
                self.event_lsq_operation_atomic_latency_max_ticks,
            ),
            (
                "event.lsq_operation.float_load_latency_max_ticks",
                self.event_lsq_operation_float_load_latency_max_ticks,
            ),
            (
                "event.lsq_operation.float_store_latency_max_ticks",
                self.event_lsq_operation_float_store_latency_max_ticks,
            ),
            (
                "event.lsq_operation.vector_load_latency_max_ticks",
                self.event_lsq_operation_vector_load_latency_max_ticks,
            ),
            (
                "event.lsq_operation.vector_store_latency_max_ticks",
                self.event_lsq_operation_vector_store_latency_max_ticks,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Tick",
                value,
            });
        }
        for (suffix, total, samples) in [
            (
                "event.lsq_operation.load_latency_avg_ticks",
                self.event_lsq_operation_load_latency_ticks,
                self.event_lsq_operation_load,
            ),
            (
                "event.lsq_operation.store_latency_avg_ticks",
                self.event_lsq_operation_store_latency_ticks,
                self.event_lsq_operation_store,
            ),
            (
                "event.lsq_operation.load_reserved_latency_avg_ticks",
                self.event_lsq_operation_load_reserved_latency_ticks,
                self.event_lsq_operation_load_reserved,
            ),
            (
                "event.lsq_operation.store_conditional_latency_avg_ticks",
                self.event_lsq_operation_store_conditional_latency_ticks,
                self.event_lsq_operation_store_conditional,
            ),
            (
                "event.lsq_operation.atomic_latency_avg_ticks",
                self.event_lsq_operation_atomic_latency_ticks,
                self.event_lsq_operation_atomic,
            ),
            (
                "event.lsq_operation.float_load_latency_avg_ticks",
                self.event_lsq_operation_float_load_latency_ticks,
                self.event_lsq_operation_float_load,
            ),
            (
                "event.lsq_operation.float_store_latency_avg_ticks",
                self.event_lsq_operation_float_store_latency_ticks,
                self.event_lsq_operation_float_store,
            ),
            (
                "event.lsq_operation.vector_load_latency_avg_ticks",
                self.event_lsq_operation_vector_load_latency_ticks,
                self.event_lsq_operation_vector_load,
            ),
            (
                "event.lsq_operation.vector_store_latency_avg_ticks",
                self.event_lsq_operation_vector_store_latency_ticks,
                self.event_lsq_operation_vector_store,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Tick",
                value: average_ticks(total, samples),
            });
        }
        for (suffix, value) in [
            (
                "event.lsq_operation.load_latency_min_ticks",
                self.event_lsq_operation_load_latency_min_ticks,
            ),
            (
                "event.lsq_operation.store_latency_min_ticks",
                self.event_lsq_operation_store_latency_min_ticks,
            ),
            (
                "event.lsq_operation.load_reserved_latency_min_ticks",
                self.event_lsq_operation_load_reserved_latency_min_ticks,
            ),
            (
                "event.lsq_operation.store_conditional_latency_min_ticks",
                self.event_lsq_operation_store_conditional_latency_min_ticks,
            ),
            (
                "event.lsq_operation.atomic_latency_min_ticks",
                self.event_lsq_operation_atomic_latency_min_ticks,
            ),
            (
                "event.lsq_operation.float_load_latency_min_ticks",
                self.event_lsq_operation_float_load_latency_min_ticks,
            ),
            (
                "event.lsq_operation.float_store_latency_min_ticks",
                self.event_lsq_operation_float_store_latency_min_ticks,
            ),
            (
                "event.lsq_operation.vector_load_latency_min_ticks",
                self.event_lsq_operation_vector_load_latency_min_ticks,
            ),
            (
                "event.lsq_operation.vector_store_latency_min_ticks",
                self.event_lsq_operation_vector_store_latency_min_ticks,
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Tick",
                value: value.unwrap_or(0),
            });
        }
        for (suffix, value) in [
            (
                "fu_integer_mul_latency_cycles",
                self.fu_integer_mul_latency_cycles,
            ),
            (
                "fu_integer_div_latency_cycles",
                self.fu_integer_div_latency_cycles,
            ),
            ("event.fu_latency_cycles", self.event_fu_latency_cycles),
            (
                "event.fu_latency_max_cycles",
                self.event_fu_latency_max_cycles,
            ),
            (
                "event.fu_latency_min_cycles",
                self.event_fu_latency_min_cycles.unwrap_or(0),
            ),
            (
                "event.fu_latency_avg_cycles",
                average_ticks(
                    self.event_fu_latency_cycles,
                    self.event_fu_latency_instructions,
                ),
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Cycle",
                value,
            });
        }
        for class_stats in REM6_O3_FU_LATENCY_CLASS_STATS {
            let totals = self.event_fu_latency_classes[class_stats.class.index()];
            for (suffix, value) in [
                (class_stats.cycles, totals.cycles),
                (class_stats.max_cycles, totals.max_cycles),
                (class_stats.min_cycles, totals.min_cycles_value()),
                (class_stats.avg_cycles, totals.avg_cycles()),
            ] {
                stats.push(Rem6O3TraceStat {
                    suffix,
                    unit: "Cycle",
                    value,
                });
            }
        }
        stats
    }
}
