use rem6_cpu::{
    BranchTargetKind, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeSnapshot, O3RuntimeStats,
};
use rem6_stats::{StatsError, StatsRegistry};

use super::super::event_summary::RiscvO3RuntimeEventSummarySnapshot;
use super::super::event_window::RiscvO3RuntimeEventWindowSnapshot;
use super::super::helpers::{o3_branch_mispredicts, ratio_ppm, set_o3_lsq_latency_counters};
use super::RiscvO3RuntimeCpuStats;

impl RiscvO3RuntimeCpuStats {
    pub(in crate::riscv_o3_runtime_stats) fn set_event_window_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: RiscvO3RuntimeEventWindowSnapshot,
    ) -> Result<(), StatsError> {
        if let Some(event_window) = self.event_window {
            event_window.set_snapshot(registry, snapshot)?;
        }
        Ok(())
    }

    pub(in crate::riscv_o3_runtime_stats) fn set_event_summary_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: &RiscvO3RuntimeEventSummarySnapshot,
    ) -> Result<(), StatsError> {
        if let Some(event_summary) = self.event_summary {
            event_summary.set_snapshot(registry, snapshot)?;
        }
        Ok(())
    }

    pub(in crate::riscv_o3_runtime_stats) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
        runtime_snapshot: &O3RuntimeSnapshot,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        for (stat, value) in [
            (self.instructions, snapshot.instructions()),
            (self.rob_allocations, snapshot.rob_allocations()),
            (self.rob_commits, snapshot.rob_commits()),
            (self.rename_writes, snapshot.rename_writes()),
            (self.lsq_loads, snapshot.lsq_loads()),
            (self.lsq_stores, snapshot.lsq_stores()),
            (self.lsq_load_bytes, snapshot.lsq_load_bytes()),
            (self.lsq_store_bytes, snapshot.lsq_store_bytes()),
            (
                self.lsq_store_to_load_forwarding_candidates,
                snapshot.lsq_store_to_load_forwarding_candidates(),
            ),
            (
                self.lsq_store_to_load_forwarding_matches,
                snapshot.lsq_store_to_load_forwarding_matches(),
            ),
            (
                self.lsq_store_to_load_forwarding_suppressed,
                snapshot.lsq_store_to_load_forwarding_suppressed(),
            ),
            (
                self.lsq_store_to_load_forwarding_address_mismatches,
                snapshot.lsq_store_to_load_forwarding_address_mismatches(),
            ),
            (
                self.lsq_store_to_load_forwarding_byte_mismatches,
                snapshot.lsq_store_to_load_forwarding_byte_mismatches(),
            ),
            (
                self.lsq_store_conditional_failures,
                snapshot.lsq_store_conditional_failures(),
            ),
            (
                self.branch_repair_targetless_mismatches,
                snapshot.branch_repair_targetless_mismatches(),
            ),
            (
                self.branch_repair_wrong_targets,
                snapshot.branch_repair_wrong_targets(),
            ),
            (
                self.branch_repair_direction_only_mismatches,
                snapshot.branch_repair_direction_only_mismatches(),
            ),
            (self.branch_event_branches, snapshot.branch_events()),
            (self.branch_event_taken, snapshot.branch_event_taken()),
            (
                self.branch_event_not_taken,
                snapshot.branch_event_not_taken(),
            ),
            (
                self.branch_event_predicted_taken,
                snapshot.branch_event_predicted_taken(),
            ),
            (
                self.branch_event_predicted_not_taken,
                snapshot.branch_event_predicted_not_taken(),
            ),
            (
                self.branch_event_predicted_targets,
                snapshot.branch_event_predicted_targets(),
            ),
            (
                self.branch_event_predicted_target_matches,
                snapshot.branch_event_predicted_target_matches(),
            ),
            (
                self.branch_event_predicted_target_mismatches,
                snapshot.branch_event_predicted_target_mismatches(),
            ),
            (
                self.branch_event_resolved_targets,
                snapshot.branch_event_resolved_targets(),
            ),
            (
                self.branch_event_mispredictions,
                snapshot.branch_event_mispredictions(),
            ),
            (
                self.branch_event_link_writes,
                snapshot.branch_event_link_writes(),
            ),
            (
                self.branch_event_without_link_writes,
                snapshot.branch_event_without_link_writes(),
            ),
            (self.branch_event_squashes, snapshot.branch_event_squashes()),
            (
                self.branch_event_squashed_targets,
                snapshot.branch_event_squashed_targets(),
            ),
            (
                self.branch_event_squashed_targets_with_link_writes,
                snapshot.branch_event_squashed_targets_with_link_writes(),
            ),
            (
                self.branch_event_squashed_targets_without_link_writes,
                snapshot.branch_event_squashed_targets_without_link_writes(),
            ),
            (
                self.fu_latency_instructions,
                snapshot.fu_latency_instructions(),
            ),
            (self.fu_latency_cycles, snapshot.fu_latency_cycles()),
            (self.iq_insts_issued, snapshot.instructions()),
            (
                self.iq_mem_insts_issued,
                snapshot.lsq_loads().saturating_add(snapshot.lsq_stores()),
            ),
            (
                self.iq_branch_insts_issued,
                snapshot.iq_branch_insts_issued(),
            ),
            (self.iq_issued_inst_type_mem_read, snapshot.lsq_loads()),
            (self.iq_issued_inst_type_mem_write, snapshot.lsq_stores()),
            (
                self.commit_committed_inst_type_mem_read,
                snapshot.lsq_loads(),
            ),
            (
                self.commit_committed_inst_type_mem_write,
                snapshot.lsq_stores(),
            ),
            (self.iew_dispatched_insts, snapshot.instructions()),
            (self.iew_insts_to_commit, snapshot.rob_commits()),
            (self.iew_writeback_count, snapshot.instructions()),
            (self.iew_producer_inst, snapshot.iew_producer_insts()),
            (self.iew_consumer_inst, snapshot.iew_consumer_insts()),
            (
                self.iew_predicted_taken_incorrect,
                snapshot.iew_predicted_taken_incorrect(),
            ),
            (
                self.iew_predicted_not_taken_incorrect,
                snapshot.iew_predicted_not_taken_incorrect(),
            ),
            (self.iew_branch_mispredicts, o3_branch_mispredicts(snapshot)),
            (
                self.commit_branch_mispredicts,
                o3_branch_mispredicts(snapshot),
            ),
            (self.max_rob_occupancy, snapshot.max_rob_occupancy()),
            (self.max_lsq_occupancy, snapshot.max_lsq_occupancy()),
            (self.rename_map_entries, snapshot.rename_map_entries()),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        self.set_runtime_snapshot_counts(registry, runtime_snapshot)?;
        self.structural_aliases.set_snapshot(registry, snapshot)?;
        self.branch_aliases.set_snapshot(registry, snapshot)?;
        self.branch_direction_mismatch
            .set_snapshot(registry, snapshot)?;
        self.branch_target_mismatch
            .set_snapshot(registry, snapshot)?;
        self.set_iew_rate_snapshots(registry, snapshot, in_order_pipeline_cycles)?;
        self.set_event_window_snapshot(registry, RiscvO3RuntimeEventWindowSnapshot::default())?;
        self.set_event_summary_snapshot(registry, &RiscvO3RuntimeEventSummarySnapshot::default())?;
        let mut lsq_operation_total = 0_u64;
        for operation in O3RuntimeLsqOperation::TRACKED {
            let value = snapshot.lsq_operation_count(operation);
            lsq_operation_total = lsq_operation_total.saturating_add(value);
            registry.set_resettable_counter(self.lsq_operation_counts[operation.index()], value)?;
            registry.set_resettable_counter(
                self.lsq_operation_alias_counts[operation.index()],
                value,
            )?;
        }
        registry.set_resettable_counter(self.lsq_operation_alias_total, lsq_operation_total)?;
        for operation in O3RuntimeLsqOperation::TRACKED {
            registry.set_resettable_counter(
                self.lsq_operation_load_bytes[operation.index()],
                snapshot.lsq_operation_load_bytes(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_load_byte_aliases[operation.index()],
                snapshot.lsq_operation_load_bytes(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_store_bytes[operation.index()],
                snapshot.lsq_operation_store_bytes(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_store_byte_aliases[operation.index()],
                snapshot.lsq_operation_store_bytes(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_store_conditional_failures[operation.index()],
                snapshot.lsq_operation_store_conditional_failures(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_nested_store_conditional_failures[operation.index()],
                snapshot.lsq_operation_store_conditional_failures(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_store_conditional_failure_aliases[operation.index()],
                snapshot.lsq_operation_store_conditional_failures(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_candidates[operation.index()],
                snapshot.lsq_operation_forwarding_candidates(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_matches[operation.index()],
                snapshot.lsq_operation_forwarding_matches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_suppressed[operation.index()],
                snapshot.lsq_operation_forwarding_suppressed(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_address_mismatches[operation.index()],
                snapshot.lsq_operation_forwarding_address_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_byte_mismatches[operation.index()],
                snapshot.lsq_operation_forwarding_byte_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_nested_forwarding_candidates[operation.index()],
                snapshot.lsq_operation_forwarding_candidates(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_nested_forwarding_matches[operation.index()],
                snapshot.lsq_operation_forwarding_matches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_nested_forwarding_suppressed[operation.index()],
                snapshot.lsq_operation_forwarding_suppressed(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_nested_forwarding_address_mismatches[operation.index()],
                snapshot.lsq_operation_forwarding_address_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_nested_forwarding_byte_mismatches[operation.index()],
                snapshot.lsq_operation_forwarding_byte_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_candidate_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_candidates(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_match_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_matches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_suppressed_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_suppressed(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_address_mismatch_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_address_mismatches(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_forwarding_byte_mismatch_aliases[operation.index()],
                snapshot.lsq_operation_forwarding_byte_mismatches(operation),
            )?;
        }
        self.set_lsq_latency_snapshot(registry, snapshot)?;
        let mut lsq_ordering_total = 0_u64;
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            let value = snapshot.lsq_ordering_count(ordering);
            lsq_ordering_total = lsq_ordering_total.saturating_add(value);
            registry.set_resettable_counter(self.lsq_ordering_counts[ordering.index()], value)?;
            registry
                .set_resettable_counter(self.lsq_ordering_alias_counts[ordering.index()], value)?;
        }
        registry.set_resettable_counter(self.lsq_ordering_alias_total, lsq_ordering_total)?;
        for kind in BranchTargetKind::ALL {
            let repair_stats = self.branch_repair_kinds[kind.index()];
            for (stat, value) in [
                (
                    repair_stats.targetless_mismatch,
                    snapshot.branch_repair_targetless_mismatch_kind(kind),
                ),
                (
                    repair_stats.wrong_target,
                    snapshot.branch_repair_wrong_target_kind(kind),
                ),
                (
                    repair_stats.direction_only,
                    snapshot.branch_repair_direction_only_kind(kind),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        for kind in BranchTargetKind::ALL {
            let event_stats = self.branch_event_kinds[kind.index()];
            for (stat, value) in [
                (event_stats.kind, snapshot.branch_event_kind(kind)),
                (event_stats.taken, snapshot.branch_event_taken_kind(kind)),
                (
                    event_stats.not_taken,
                    snapshot.branch_event_not_taken_kind(kind),
                ),
                (
                    event_stats.predicted_taken,
                    snapshot.branch_event_predicted_taken_kind(kind),
                ),
                (
                    event_stats.predicted_not_taken,
                    snapshot.branch_event_predicted_not_taken_kind(kind),
                ),
                (
                    event_stats.predicted_target,
                    snapshot.branch_event_predicted_target_kind(kind),
                ),
                (
                    event_stats.predicted_target_match,
                    snapshot.branch_event_predicted_target_match_kind(kind),
                ),
                (
                    event_stats.predicted_target_mismatch,
                    snapshot.branch_event_predicted_target_mismatch_kind(kind),
                ),
                (
                    event_stats.resolved_target,
                    snapshot.branch_event_resolved_target_kind(kind),
                ),
                (
                    event_stats.misprediction,
                    snapshot.branch_event_misprediction_kind(kind),
                ),
                (
                    event_stats.link_write,
                    snapshot.branch_event_link_write_kind(kind),
                ),
                (
                    event_stats.without_link_write,
                    snapshot.branch_event_without_link_write_kind(kind),
                ),
                (event_stats.squash, snapshot.branch_event_squash_kind(kind)),
                (
                    event_stats.squashed_target,
                    snapshot.branch_event_squashed_target_kind(kind),
                ),
                (
                    event_stats.squashed_target_link_write,
                    snapshot.branch_event_squashed_target_link_write_kind(kind),
                ),
                (
                    event_stats.squashed_target_without_link_write,
                    snapshot.branch_event_squashed_target_without_link_write_kind(kind),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            let class_stats = self.fu_latency_classes[class.index()];
            let nested_class_stats = self.nested_fu_latency_classes[class.index()];
            for (stat, value) in [
                (
                    class_stats.instructions,
                    snapshot.fu_latency_class_instructions(class),
                ),
                (
                    class_stats.latency_cycles,
                    snapshot.fu_latency_class_cycles(class),
                ),
                (
                    nested_class_stats.instructions,
                    snapshot.fu_latency_class_instructions(class),
                ),
                (
                    nested_class_stats.latency_cycles,
                    snapshot.fu_latency_class_cycles(class),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        self.set_fu_latency_class_extrema_snapshot(registry, snapshot)?;
        for class in O3RuntimeFuLatencyClass::ALL {
            registry.set_resettable_counter(
                self.iq_issued_inst_type_fu_classes[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
            registry.set_resettable_counter(
                self.iq_issued_inst_type_fu_aliases[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
            registry.set_resettable_counter(
                self.commit_committed_inst_type_fu_classes[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
            registry.set_resettable_counter(
                self.commit_committed_inst_type_fu_aliases[class.index()],
                snapshot.fu_latency_class_instructions(class),
            )?;
        }
        Ok(())
    }

    pub(super) fn set_runtime_snapshot_counts(
        self,
        registry: &mut StatsRegistry,
        snapshot: &O3RuntimeSnapshot,
    ) -> Result<(), StatsError> {
        let rob_entries = snapshot.reorder_buffer().len() as u64;
        let lsq_entries = snapshot.load_store_queue().len() as u64;
        let rename_map_entries = snapshot.rename_map().len() as u64;
        for (stat, value) in [
            (self.snapshot_rob_count, rob_entries),
            (self.snapshot_lsq_count, lsq_entries),
            (self.snapshot_rename_map_count, rename_map_entries),
            (self.snapshot_rob_entries, rob_entries),
            (self.snapshot_lsq_entries, lsq_entries),
            (self.snapshot_rename_map_entries, rename_map_entries),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        Ok(())
    }

    pub(super) fn set_fu_latency_class_extrema_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for class in O3RuntimeFuLatencyClass::ALL {
            let class_stats = self.fu_latency_classes[class.index()];
            let nested_class_stats = self.nested_fu_latency_classes[class.index()];
            for (stat, value) in [
                (
                    class_stats.latency_max_cycles,
                    snapshot.fu_latency_class_max_cycles(class),
                ),
                (
                    class_stats.latency_min_cycles,
                    snapshot.fu_latency_class_min_cycles(class),
                ),
                (
                    class_stats.latency_avg_cycles,
                    snapshot.fu_latency_class_avg_cycles(class),
                ),
                (
                    nested_class_stats.latency_max_cycles,
                    snapshot.fu_latency_class_max_cycles(class),
                ),
                (
                    nested_class_stats.latency_min_cycles,
                    snapshot.fu_latency_class_min_cycles(class),
                ),
                (
                    nested_class_stats.latency_avg_cycles,
                    snapshot.fu_latency_class_avg_cycles(class),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        Ok(())
    }

    pub(super) fn set_iew_rate_snapshots(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        let writeback_rate = ratio_ppm(snapshot.instructions(), in_order_pipeline_cycles);
        let producer_consumer_fanout =
            ratio_ppm(snapshot.iew_producer_insts(), snapshot.iew_consumer_insts());
        registry.set_resettable_counter(self.iew_writeback_rate_ppm, writeback_rate)?;
        registry.set_resettable_counter(
            self.iew_producer_consumer_fanout_ppm,
            producer_consumer_fanout,
        )?;
        Ok(())
    }

    pub(super) fn set_lsq_latency_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        set_o3_lsq_latency_counters(
            registry,
            self.lsq_data_latency,
            snapshot.lsq_data_latency_samples(),
            snapshot.lsq_data_latency_ticks(),
            snapshot.lsq_data_latency_max_ticks(),
            snapshot.lsq_data_latency_min_ticks(),
            snapshot.lsq_data_latency_avg_ticks(),
        )?;
        for operation in O3RuntimeLsqOperation::TRACKED {
            set_o3_lsq_latency_counters(
                registry,
                self.lsq_operation_latency[operation.index()],
                snapshot.lsq_operation_latency_samples(operation),
                snapshot.lsq_operation_latency_ticks(operation),
                snapshot.lsq_operation_latency_max_ticks(operation),
                snapshot.lsq_operation_latency_min_ticks(operation),
                snapshot.lsq_operation_latency_avg_ticks(operation),
            )?;
            set_o3_lsq_latency_counters(
                registry,
                self.lsq_operation_nested_latency[operation.index()],
                snapshot.lsq_operation_latency_samples(operation),
                snapshot.lsq_operation_latency_ticks(operation),
                snapshot.lsq_operation_latency_max_ticks(operation),
                snapshot.lsq_operation_latency_min_ticks(operation),
                snapshot.lsq_operation_latency_avg_ticks(operation),
            )?;
        }
        Ok(())
    }
}
