use rem6_cpu::{
    BranchTargetKind, CpuId, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeSnapshot, O3RuntimeStats,
};
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::event_summary::{RiscvO3RuntimeEventSummarySnapshot, RiscvO3RuntimeEventSummaryStats};
use super::event_window::{RiscvO3RuntimeEventWindowSnapshot, RiscvO3RuntimeEventWindowStats};
use super::groups::*;
use super::helpers::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeCpuStats {
    instructions: StatId,
    rob_allocations: StatId,
    rob_commits: StatId,
    rename_writes: StatId,
    lsq_loads: StatId,
    lsq_stores: StatId,
    lsq_load_bytes: StatId,
    lsq_store_bytes: StatId,
    lsq_store_to_load_forwarding_candidates: StatId,
    lsq_store_to_load_forwarding_matches: StatId,
    lsq_store_to_load_forwarding_suppressed: StatId,
    lsq_store_to_load_forwarding_address_mismatches: StatId,
    lsq_store_to_load_forwarding_byte_mismatches: StatId,
    structural_aliases: RiscvO3RuntimeStructuralAliasStats,
    lsq_operation_counts: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_alias_counts: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_alias_total: StatId,
    lsq_operation_load_bytes: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_store_bytes: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_load_byte_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_store_byte_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_store_conditional_failures: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_candidates: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_matches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_suppressed: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_address_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_byte_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_nested_store_conditional_failures: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_nested_forwarding_candidates: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_nested_forwarding_matches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_nested_forwarding_suppressed: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_nested_forwarding_address_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_nested_forwarding_byte_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_candidate_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_match_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_suppressed_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_address_mismatch_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_byte_mismatch_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_store_conditional_failure_aliases: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_data_latency: RiscvO3RuntimeLsqLatencyStats,
    lsq_operation_latency: [RiscvO3RuntimeLsqLatencyStats; O3RuntimeLsqOperation::COUNT],
    lsq_operation_nested_latency: [RiscvO3RuntimeLsqLatencyStats; O3RuntimeLsqOperation::COUNT],
    lsq_ordering_counts: [StatId; O3RuntimeLsqOrdering::COUNT],
    lsq_ordering_alias_counts: [StatId; O3RuntimeLsqOrdering::COUNT],
    lsq_ordering_alias_total: StatId,
    lsq_store_conditional_failures: StatId,
    branch_repair_targetless_mismatches: StatId,
    branch_repair_wrong_targets: StatId,
    branch_repair_direction_only_mismatches: StatId,
    branch_repair_kinds: [RiscvO3RuntimeBranchRepairStats; BranchTargetKind::COUNT],
    branch_direction_mismatch: RiscvO3RuntimeBranchDirectionMismatchStats,
    branch_target_mismatch: RiscvO3RuntimeBranchTargetMismatchStats,
    branch_event_branches: StatId,
    branch_event_taken: StatId,
    branch_event_not_taken: StatId,
    branch_event_predicted_taken: StatId,
    branch_event_predicted_not_taken: StatId,
    branch_event_predicted_targets: StatId,
    branch_event_predicted_target_matches: StatId,
    branch_event_predicted_target_mismatches: StatId,
    branch_event_resolved_targets: StatId,
    branch_event_mispredictions: StatId,
    branch_event_link_writes: StatId,
    branch_event_without_link_writes: StatId,
    branch_event_squashes: StatId,
    branch_event_squashed_targets: StatId,
    branch_event_squashed_targets_with_link_writes: StatId,
    branch_event_squashed_targets_without_link_writes: StatId,
    branch_event_kinds: [RiscvO3RuntimeBranchEventKindStats; BranchTargetKind::COUNT],
    branch_aliases: RiscvO3RuntimeBranchAliasStats,
    fu_latency_instructions: StatId,
    fu_latency_cycles: StatId,
    fu_latency_classes: [RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT],
    nested_fu_latency_classes: [RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT],
    iq_insts_issued: StatId,
    iq_mem_insts_issued: StatId,
    iq_branch_insts_issued: StatId,
    iq_issued_inst_type_mem_read: StatId,
    iq_issued_inst_type_mem_write: StatId,
    iq_issued_inst_type_fu_classes: [StatId; O3RuntimeFuLatencyClass::COUNT],
    iq_issued_inst_type_fu_aliases: [StatId; O3RuntimeFuLatencyClass::COUNT],
    commit_committed_inst_type_mem_read: StatId,
    commit_committed_inst_type_mem_write: StatId,
    commit_committed_inst_type_fu_classes: [StatId; O3RuntimeFuLatencyClass::COUNT],
    commit_committed_inst_type_fu_aliases: [StatId; O3RuntimeFuLatencyClass::COUNT],
    iew_dispatched_insts: StatId,
    iew_insts_to_commit: StatId,
    iew_writeback_count: StatId,
    iew_producer_inst: StatId,
    iew_consumer_inst: StatId,
    iew_writeback_rate_ppm: StatId,
    iew_producer_consumer_fanout_ppm: StatId,
    iew_predicted_taken_incorrect: StatId,
    iew_predicted_not_taken_incorrect: StatId,
    iew_branch_mispredicts: StatId,
    commit_branch_mispredicts: StatId,
    max_rob_occupancy: StatId,
    max_lsq_occupancy: StatId,
    rename_map_entries: StatId,
    snapshot_rob_count: StatId,
    snapshot_lsq_count: StatId,
    snapshot_rename_map_count: StatId,
    snapshot_rob_entries: StatId,
    snapshot_lsq_entries: StatId,
    snapshot_rename_map_entries: StatId,
    event_window: Option<RiscvO3RuntimeEventWindowStats>,
    event_summary: Option<RiscvO3RuntimeEventSummaryStats>,
}

impl RiscvO3RuntimeCpuStats {
    pub(super) fn register(
        registry: &mut StatsRegistry,
        cpu: CpuId,
        single_cpu_run: bool,
        trace_enabled: bool,
    ) -> Result<Self, StatsError> {
        let prefix = format!("sim.host_actions.stats_dump.cpu{}.o3", cpu.get());
        let gem5_cpu_alias_prefix = if single_cpu_run {
            "system.cpu".to_string()
        } else {
            format!("system.cpu{}", cpu.get())
        };
        Ok(Self {
            instructions: register_o3_counter(registry, &prefix, "instructions", "Count")?,
            rob_allocations: register_o3_counter(registry, &prefix, "rob_allocations", "Count")?,
            rob_commits: register_o3_counter(registry, &prefix, "rob_commits", "Count")?,
            rename_writes: register_o3_counter(registry, &prefix, "rename_writes", "Count")?,
            lsq_loads: register_o3_counter(registry, &prefix, "lsq_loads", "Count")?,
            lsq_stores: register_o3_counter(registry, &prefix, "lsq_stores", "Count")?,
            lsq_load_bytes: register_o3_counter(registry, &prefix, "lsq_load_bytes", "Byte")?,
            lsq_store_bytes: register_o3_counter(registry, &prefix, "lsq_store_bytes", "Byte")?,
            lsq_store_to_load_forwarding_candidates: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_candidates",
                "Count",
            )?,
            lsq_store_to_load_forwarding_matches: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_matches",
                "Count",
            )?,
            lsq_store_to_load_forwarding_suppressed: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_suppressed",
                "Count",
            )?,
            lsq_store_to_load_forwarding_address_mismatches: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_address_mismatches",
                "Count",
            )?,
            lsq_store_to_load_forwarding_byte_mismatches: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_to_load_forwarding_byte_mismatches",
                "Count",
            )?,
            structural_aliases: RiscvO3RuntimeStructuralAliasStats::register(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            lsq_operation_counts: register_o3_lsq_operation_counters(registry, &prefix)?,
            lsq_operation_alias_counts: register_o3_lsq_operation_alias_counters(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            lsq_operation_alias_total: register_o3_counter(
                registry,
                &gem5_cpu_alias_prefix,
                "lsq0.operation.total",
                "Count",
            )?,
            lsq_operation_load_bytes: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "load_bytes",
                "Byte",
            )?,
            lsq_operation_store_bytes: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "store_bytes",
                "Byte",
            )?,
            lsq_operation_load_byte_aliases: register_o3_lsq_operation_alias_suffix_unit_counters(
                registry,
                &gem5_cpu_alias_prefix,
                "loadBytes",
                "Byte",
            )?,
            lsq_operation_store_byte_aliases: register_o3_lsq_operation_alias_suffix_unit_counters(
                registry,
                &gem5_cpu_alias_prefix,
                "storeBytes",
                "Byte",
            )?,
            lsq_operation_store_conditional_failures: register_o3_lsq_operation_suffix_counters(
                registry,
                &prefix,
                "store_conditional_failures",
            )?,
            lsq_operation_forwarding_candidates: register_o3_lsq_operation_suffix_counters(
                registry,
                &prefix,
                "forwarding_candidates",
            )?,
            lsq_operation_forwarding_matches: register_o3_lsq_operation_suffix_counters(
                registry,
                &prefix,
                "forwarding_matches",
            )?,
            lsq_operation_forwarding_suppressed: register_o3_lsq_operation_suffix_counters(
                registry,
                &prefix,
                "forwarding_suppressed",
            )?,
            lsq_operation_forwarding_address_mismatches: register_o3_lsq_operation_suffix_counters(
                registry,
                &prefix,
                "forwarding_address_mismatches",
            )?,
            lsq_operation_forwarding_byte_mismatches: register_o3_lsq_operation_suffix_counters(
                registry,
                &prefix,
                "forwarding_byte_mismatches",
            )?,
            lsq_operation_nested_store_conditional_failures:
                register_o3_lsq_operation_nested_counters(
                    registry,
                    &prefix,
                    "store_conditional_failures",
                    "Count",
                )?,
            lsq_operation_nested_forwarding_candidates: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_candidates",
                "Count",
            )?,
            lsq_operation_nested_forwarding_matches: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_matches",
                "Count",
            )?,
            lsq_operation_nested_forwarding_suppressed: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_suppressed",
                "Count",
            )?,
            lsq_operation_nested_forwarding_address_mismatches:
                register_o3_lsq_operation_nested_counters(
                    registry,
                    &prefix,
                    "forwarding_address_mismatches",
                    "Count",
                )?,
            lsq_operation_nested_forwarding_byte_mismatches:
                register_o3_lsq_operation_nested_counters(
                    registry,
                    &prefix,
                    "forwarding_byte_mismatches",
                    "Count",
                )?,
            lsq_operation_forwarding_candidate_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingCandidates",
                )?,
            lsq_operation_forwarding_match_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingMatches",
                )?,
            lsq_operation_forwarding_suppressed_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingSuppressed",
                )?,
            lsq_operation_forwarding_address_mismatch_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingAddressMismatches",
                )?,
            lsq_operation_forwarding_byte_mismatch_aliases:
                register_o3_lsq_operation_forwarding_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeLoadForwardingByteMismatches",
                )?,
            lsq_operation_store_conditional_failure_aliases:
                register_o3_lsq_operation_alias_suffix_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                    "storeConditionalFailures",
                )?,
            lsq_data_latency: register_o3_lsq_latency_counters(
                registry,
                &prefix,
                "lsq_data_latency",
            )?,
            lsq_operation_latency: register_o3_lsq_operation_latency_counters(registry, &prefix)?,
            lsq_operation_nested_latency: register_o3_lsq_operation_nested_latency_counters(
                registry, &prefix,
            )?,
            lsq_ordering_counts: register_o3_lsq_ordering_counters(registry, &prefix)?,
            lsq_ordering_alias_counts: register_o3_lsq_ordering_alias_counters(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            lsq_ordering_alias_total: register_o3_counter(
                registry,
                &gem5_cpu_alias_prefix,
                "lsq0.ordering.total",
                "Count",
            )?,
            lsq_store_conditional_failures: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_conditional_failures",
                "Count",
            )?,
            branch_repair_targetless_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_repair_targetless_mismatches",
                "Count",
            )?,
            branch_repair_wrong_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_repair_wrong_targets",
                "Count",
            )?,
            branch_repair_direction_only_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_repair_direction_only_mismatches",
                "Count",
            )?,
            branch_repair_kinds: register_o3_branch_repair_kind_counters(registry, &prefix)?,
            branch_direction_mismatch: RiscvO3RuntimeBranchDirectionMismatchStats::register(
                registry, &prefix,
            )?,
            branch_target_mismatch: RiscvO3RuntimeBranchTargetMismatchStats::register(
                registry, &prefix,
            )?,
            branch_event_branches: register_o3_counter(
                registry,
                &prefix,
                "branch_event.branches",
                "Count",
            )?,
            branch_event_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.taken",
                "Count",
            )?,
            branch_event_not_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.not_taken",
                "Count",
            )?,
            branch_event_predicted_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_taken",
                "Count",
            )?,
            branch_event_predicted_not_taken: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_not_taken",
                "Count",
            )?,
            branch_event_predicted_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_targets",
                "Count",
            )?,
            branch_event_predicted_target_matches: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_target_matches",
                "Count",
            )?,
            branch_event_predicted_target_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_event.predicted_target_mismatches",
                "Count",
            )?,
            branch_event_resolved_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_event.resolved_targets",
                "Count",
            )?,
            branch_event_mispredictions: register_o3_counter(
                registry,
                &prefix,
                "branch_event.mispredictions",
                "Count",
            )?,
            branch_event_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.link_writes",
                "Count",
            )?,
            branch_event_without_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.without_link_writes",
                "Count",
            )?,
            branch_event_squashes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashes",
                "Count",
            )?,
            branch_event_squashed_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashed_targets",
                "Count",
            )?,
            branch_event_squashed_targets_with_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashed_targets_with_link_writes",
                "Count",
            )?,
            branch_event_squashed_targets_without_link_writes: register_o3_counter(
                registry,
                &prefix,
                "branch_event.squashed_targets_without_link_writes",
                "Count",
            )?,
            branch_event_kinds: register_o3_branch_event_kind_counters(registry, &prefix)?,
            branch_aliases: RiscvO3RuntimeBranchAliasStats::register(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            fu_latency_instructions: register_o3_counter(
                registry,
                &prefix,
                "fu_latency_instructions",
                "Count",
            )?,
            fu_latency_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_latency_cycles",
                "Cycle",
            )?,
            fu_latency_classes: register_o3_fu_latency_class_counters(registry, &prefix)?,
            nested_fu_latency_classes: register_o3_nested_fu_latency_class_counters(
                registry, &prefix,
            )?,
            iq_insts_issued: register_o3_counter(registry, &prefix, "iq.insts_issued", "Count")?,
            iq_mem_insts_issued: register_o3_counter(
                registry,
                &prefix,
                "iq.mem_insts_issued",
                "Count",
            )?,
            iq_branch_insts_issued: register_o3_counter(
                registry,
                &prefix,
                "iq.branch_insts_issued",
                "Count",
            )?,
            iq_issued_inst_type_mem_read: register_o3_counter(
                registry,
                &prefix,
                "iq.issued_inst_type.mem_read",
                "Count",
            )?,
            iq_issued_inst_type_mem_write: register_o3_counter(
                registry,
                &prefix,
                "iq.issued_inst_type.mem_write",
                "Count",
            )?,
            iq_issued_inst_type_fu_classes: register_o3_iq_fu_latency_class_counters(
                registry, &prefix,
            )?,
            iq_issued_inst_type_fu_aliases: register_o3_iq_fu_latency_class_alias_counters(
                registry,
                &gem5_cpu_alias_prefix,
            )?,
            commit_committed_inst_type_mem_read: register_o3_counter(
                registry,
                &prefix,
                "commit.committed_inst_type.mem_read",
                "Count",
            )?,
            commit_committed_inst_type_mem_write: register_o3_counter(
                registry,
                &prefix,
                "commit.committed_inst_type.mem_write",
                "Count",
            )?,
            commit_committed_inst_type_fu_classes: register_o3_commit_fu_latency_class_counters(
                registry, &prefix,
            )?,
            commit_committed_inst_type_fu_aliases:
                register_o3_commit_fu_latency_class_alias_counters(
                    registry,
                    &gem5_cpu_alias_prefix,
                )?,
            iew_dispatched_insts: register_o3_counter(
                registry,
                &prefix,
                "iew.dispatched_insts",
                "Count",
            )?,
            iew_insts_to_commit: register_o3_counter(
                registry,
                &prefix,
                "iew.insts_to_commit",
                "Count",
            )?,
            iew_writeback_count: register_o3_counter(
                registry,
                &prefix,
                "iew.writeback_count",
                "Count",
            )?,
            iew_producer_inst: register_o3_counter(
                registry,
                &prefix,
                "iew.producer_inst",
                "Count",
            )?,
            iew_consumer_inst: register_o3_counter(
                registry,
                &prefix,
                "iew.consumer_inst",
                "Count",
            )?,
            iew_writeback_rate_ppm: register_o3_counter(
                registry,
                &prefix,
                "iew.writeback_rate_ppm",
                "Ppm",
            )?,
            iew_producer_consumer_fanout_ppm: register_o3_counter(
                registry,
                &prefix,
                "iew.producer_consumer_fanout_ppm",
                "Ppm",
            )?,
            iew_predicted_taken_incorrect: register_o3_counter(
                registry,
                &prefix,
                "iew.predicted_taken_incorrect",
                "Count",
            )?,
            iew_predicted_not_taken_incorrect: register_o3_counter(
                registry,
                &prefix,
                "iew.predicted_not_taken_incorrect",
                "Count",
            )?,
            iew_branch_mispredicts: register_o3_counter(
                registry,
                &prefix,
                "iew.branch_mispredicts",
                "Count",
            )?,
            commit_branch_mispredicts: register_o3_counter(
                registry,
                &prefix,
                "commit.branch_mispredicts",
                "Count",
            )?,
            max_rob_occupancy: register_o3_counter(
                registry,
                &prefix,
                "max_rob_occupancy",
                "Count",
            )?,
            max_lsq_occupancy: register_o3_counter(
                registry,
                &prefix,
                "max_lsq_occupancy",
                "Count",
            )?,
            rename_map_entries: register_o3_counter(
                registry,
                &prefix,
                "rename_map_entries",
                "Count",
            )?,
            snapshot_rob_count: register_o3_counter(
                registry,
                &prefix,
                "snapshot.rob.count",
                "Count",
            )?,
            snapshot_lsq_count: register_o3_counter(
                registry,
                &prefix,
                "snapshot.lsq.count",
                "Count",
            )?,
            snapshot_rename_map_count: register_o3_counter(
                registry,
                &prefix,
                "snapshot.rename_map.count",
                "Count",
            )?,
            snapshot_rob_entries: register_o3_counter(
                registry,
                &prefix,
                "snapshot.rob.entries",
                "Count",
            )?,
            snapshot_lsq_entries: register_o3_counter(
                registry,
                &prefix,
                "snapshot.lsq.entries",
                "Count",
            )?,
            snapshot_rename_map_entries: register_o3_counter(
                registry,
                &prefix,
                "snapshot.rename_map.entries",
                "Count",
            )?,
            event_window: if trace_enabled {
                Some(RiscvO3RuntimeEventWindowStats::register(registry, &prefix)?)
            } else {
                None
            },
            event_summary: if trace_enabled {
                Some(RiscvO3RuntimeEventSummaryStats::register(
                    registry, &prefix,
                )?)
            } else {
                None
            },
        })
    }

    pub(super) fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
        runtime_snapshot: &O3RuntimeSnapshot,
        in_order_pipeline_cycles: u64,
    ) -> Result<(), StatsError> {
        for (stat, previous, current) in [
            (
                self.instructions,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.rob_allocations,
                previous.rob_allocations(),
                current.rob_allocations(),
            ),
            (
                self.rob_commits,
                previous.rob_commits(),
                current.rob_commits(),
            ),
            (
                self.rename_writes,
                previous.rename_writes(),
                current.rename_writes(),
            ),
            (self.lsq_loads, previous.lsq_loads(), current.lsq_loads()),
            (self.lsq_stores, previous.lsq_stores(), current.lsq_stores()),
            (
                self.lsq_load_bytes,
                previous.lsq_load_bytes(),
                current.lsq_load_bytes(),
            ),
            (
                self.lsq_store_bytes,
                previous.lsq_store_bytes(),
                current.lsq_store_bytes(),
            ),
            (
                self.lsq_store_to_load_forwarding_candidates,
                previous.lsq_store_to_load_forwarding_candidates(),
                current.lsq_store_to_load_forwarding_candidates(),
            ),
            (
                self.lsq_store_to_load_forwarding_matches,
                previous.lsq_store_to_load_forwarding_matches(),
                current.lsq_store_to_load_forwarding_matches(),
            ),
            (
                self.lsq_store_to_load_forwarding_suppressed,
                previous.lsq_store_to_load_forwarding_suppressed(),
                current.lsq_store_to_load_forwarding_suppressed(),
            ),
            (
                self.lsq_store_to_load_forwarding_address_mismatches,
                previous.lsq_store_to_load_forwarding_address_mismatches(),
                current.lsq_store_to_load_forwarding_address_mismatches(),
            ),
            (
                self.lsq_store_to_load_forwarding_byte_mismatches,
                previous.lsq_store_to_load_forwarding_byte_mismatches(),
                current.lsq_store_to_load_forwarding_byte_mismatches(),
            ),
            (
                self.lsq_store_conditional_failures,
                previous.lsq_store_conditional_failures(),
                current.lsq_store_conditional_failures(),
            ),
            (
                self.branch_repair_targetless_mismatches,
                previous.branch_repair_targetless_mismatches(),
                current.branch_repair_targetless_mismatches(),
            ),
            (
                self.branch_repair_wrong_targets,
                previous.branch_repair_wrong_targets(),
                current.branch_repair_wrong_targets(),
            ),
            (
                self.branch_repair_direction_only_mismatches,
                previous.branch_repair_direction_only_mismatches(),
                current.branch_repair_direction_only_mismatches(),
            ),
            (
                self.branch_event_branches,
                previous.branch_events(),
                current.branch_events(),
            ),
            (
                self.branch_event_taken,
                previous.branch_event_taken(),
                current.branch_event_taken(),
            ),
            (
                self.branch_event_not_taken,
                previous.branch_event_not_taken(),
                current.branch_event_not_taken(),
            ),
            (
                self.branch_event_predicted_taken,
                previous.branch_event_predicted_taken(),
                current.branch_event_predicted_taken(),
            ),
            (
                self.branch_event_predicted_not_taken,
                previous.branch_event_predicted_not_taken(),
                current.branch_event_predicted_not_taken(),
            ),
            (
                self.branch_event_predicted_targets,
                previous.branch_event_predicted_targets(),
                current.branch_event_predicted_targets(),
            ),
            (
                self.branch_event_predicted_target_matches,
                previous.branch_event_predicted_target_matches(),
                current.branch_event_predicted_target_matches(),
            ),
            (
                self.branch_event_predicted_target_mismatches,
                previous.branch_event_predicted_target_mismatches(),
                current.branch_event_predicted_target_mismatches(),
            ),
            (
                self.branch_event_resolved_targets,
                previous.branch_event_resolved_targets(),
                current.branch_event_resolved_targets(),
            ),
            (
                self.branch_event_mispredictions,
                previous.branch_event_mispredictions(),
                current.branch_event_mispredictions(),
            ),
            (
                self.branch_event_link_writes,
                previous.branch_event_link_writes(),
                current.branch_event_link_writes(),
            ),
            (
                self.branch_event_without_link_writes,
                previous.branch_event_without_link_writes(),
                current.branch_event_without_link_writes(),
            ),
            (
                self.branch_event_squashes,
                previous.branch_event_squashes(),
                current.branch_event_squashes(),
            ),
            (
                self.branch_event_squashed_targets,
                previous.branch_event_squashed_targets(),
                current.branch_event_squashed_targets(),
            ),
            (
                self.branch_event_squashed_targets_with_link_writes,
                previous.branch_event_squashed_targets_with_link_writes(),
                current.branch_event_squashed_targets_with_link_writes(),
            ),
            (
                self.branch_event_squashed_targets_without_link_writes,
                previous.branch_event_squashed_targets_without_link_writes(),
                current.branch_event_squashed_targets_without_link_writes(),
            ),
            (
                self.fu_latency_instructions,
                previous.fu_latency_instructions(),
                current.fu_latency_instructions(),
            ),
            (
                self.fu_latency_cycles,
                previous.fu_latency_cycles(),
                current.fu_latency_cycles(),
            ),
            (
                self.iq_insts_issued,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.iq_mem_insts_issued,
                previous.lsq_loads().saturating_add(previous.lsq_stores()),
                current.lsq_loads().saturating_add(current.lsq_stores()),
            ),
            (
                self.iq_branch_insts_issued,
                previous.iq_branch_insts_issued(),
                current.iq_branch_insts_issued(),
            ),
            (
                self.iq_issued_inst_type_mem_read,
                previous.lsq_loads(),
                current.lsq_loads(),
            ),
            (
                self.iq_issued_inst_type_mem_write,
                previous.lsq_stores(),
                current.lsq_stores(),
            ),
            (
                self.commit_committed_inst_type_mem_read,
                previous.lsq_loads(),
                current.lsq_loads(),
            ),
            (
                self.commit_committed_inst_type_mem_write,
                previous.lsq_stores(),
                current.lsq_stores(),
            ),
            (
                self.iew_dispatched_insts,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.iew_insts_to_commit,
                previous.rob_commits(),
                current.rob_commits(),
            ),
            (
                self.iew_writeback_count,
                previous.instructions(),
                current.instructions(),
            ),
            (
                self.iew_producer_inst,
                previous.iew_producer_insts(),
                current.iew_producer_insts(),
            ),
            (
                self.iew_consumer_inst,
                previous.iew_consumer_insts(),
                current.iew_consumer_insts(),
            ),
            (
                self.iew_predicted_taken_incorrect,
                previous.iew_predicted_taken_incorrect(),
                current.iew_predicted_taken_incorrect(),
            ),
            (
                self.iew_predicted_not_taken_incorrect,
                previous.iew_predicted_not_taken_incorrect(),
                current.iew_predicted_not_taken_incorrect(),
            ),
            (
                self.iew_branch_mispredicts,
                o3_branch_mispredicts(previous),
                o3_branch_mispredicts(current),
            ),
            (
                self.commit_branch_mispredicts,
                o3_branch_mispredicts(previous),
                o3_branch_mispredicts(current),
            ),
            (
                self.max_rob_occupancy,
                previous.max_rob_occupancy(),
                current.max_rob_occupancy(),
            ),
            (
                self.max_lsq_occupancy,
                previous.max_lsq_occupancy(),
                current.max_lsq_occupancy(),
            ),
            (
                self.rename_map_entries,
                previous.rename_map_entries(),
                current.rename_map_entries(),
            ),
        ] {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        self.set_runtime_snapshot_counts(registry, runtime_snapshot)?;
        self.structural_aliases
            .increment_delta(registry, previous, current)?;
        self.branch_aliases
            .increment_delta(registry, previous, current)?;
        self.branch_direction_mismatch
            .increment_delta(registry, previous, current)?;
        self.branch_target_mismatch
            .increment_delta(registry, previous, current)?;
        self.set_iew_rate_snapshots(registry, current, in_order_pipeline_cycles)?;
        for kind in BranchTargetKind::ALL {
            let repair_stats = self.branch_repair_kinds[kind.index()];
            for (stat, previous, current) in [
                (
                    repair_stats.targetless_mismatch,
                    previous.branch_repair_targetless_mismatch_kind(kind),
                    current.branch_repair_targetless_mismatch_kind(kind),
                ),
                (
                    repair_stats.wrong_target,
                    previous.branch_repair_wrong_target_kind(kind),
                    current.branch_repair_wrong_target_kind(kind),
                ),
                (
                    repair_stats.direction_only,
                    previous.branch_repair_direction_only_kind(kind),
                    current.branch_repair_direction_only_kind(kind),
                ),
            ] {
                let delta = current.saturating_sub(previous);
                if delta != 0 {
                    registry.increment(stat, delta)?;
                }
            }
        }
        for kind in BranchTargetKind::ALL {
            let event_stats = self.branch_event_kinds[kind.index()];
            for (stat, previous, current) in [
                (
                    event_stats.kind,
                    previous.branch_event_kind(kind),
                    current.branch_event_kind(kind),
                ),
                (
                    event_stats.taken,
                    previous.branch_event_taken_kind(kind),
                    current.branch_event_taken_kind(kind),
                ),
                (
                    event_stats.not_taken,
                    previous.branch_event_not_taken_kind(kind),
                    current.branch_event_not_taken_kind(kind),
                ),
                (
                    event_stats.predicted_taken,
                    previous.branch_event_predicted_taken_kind(kind),
                    current.branch_event_predicted_taken_kind(kind),
                ),
                (
                    event_stats.predicted_not_taken,
                    previous.branch_event_predicted_not_taken_kind(kind),
                    current.branch_event_predicted_not_taken_kind(kind),
                ),
                (
                    event_stats.predicted_target,
                    previous.branch_event_predicted_target_kind(kind),
                    current.branch_event_predicted_target_kind(kind),
                ),
                (
                    event_stats.predicted_target_match,
                    previous.branch_event_predicted_target_match_kind(kind),
                    current.branch_event_predicted_target_match_kind(kind),
                ),
                (
                    event_stats.predicted_target_mismatch,
                    previous.branch_event_predicted_target_mismatch_kind(kind),
                    current.branch_event_predicted_target_mismatch_kind(kind),
                ),
                (
                    event_stats.resolved_target,
                    previous.branch_event_resolved_target_kind(kind),
                    current.branch_event_resolved_target_kind(kind),
                ),
                (
                    event_stats.misprediction,
                    previous.branch_event_misprediction_kind(kind),
                    current.branch_event_misprediction_kind(kind),
                ),
                (
                    event_stats.link_write,
                    previous.branch_event_link_write_kind(kind),
                    current.branch_event_link_write_kind(kind),
                ),
                (
                    event_stats.without_link_write,
                    previous.branch_event_without_link_write_kind(kind),
                    current.branch_event_without_link_write_kind(kind),
                ),
                (
                    event_stats.squash,
                    previous.branch_event_squash_kind(kind),
                    current.branch_event_squash_kind(kind),
                ),
                (
                    event_stats.squashed_target,
                    previous.branch_event_squashed_target_kind(kind),
                    current.branch_event_squashed_target_kind(kind),
                ),
                (
                    event_stats.squashed_target_link_write,
                    previous.branch_event_squashed_target_link_write_kind(kind),
                    current.branch_event_squashed_target_link_write_kind(kind),
                ),
                (
                    event_stats.squashed_target_without_link_write,
                    previous.branch_event_squashed_target_without_link_write_kind(kind),
                    current.branch_event_squashed_target_without_link_write_kind(kind),
                ),
            ] {
                let delta = current.saturating_sub(previous);
                if delta != 0 {
                    registry.increment(stat, delta)?;
                }
            }
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            let delta = current
                .fu_latency_class_instructions(class)
                .saturating_sub(previous.fu_latency_class_instructions(class));
            if delta != 0 {
                registry.increment(self.iq_issued_inst_type_fu_classes[class.index()], delta)?;
            }
            if delta != 0 {
                registry.increment(self.iq_issued_inst_type_fu_aliases[class.index()], delta)?;
            }
            if delta != 0 {
                registry.increment(
                    self.commit_committed_inst_type_fu_classes[class.index()],
                    delta,
                )?;
            }
            if delta != 0 {
                registry.increment(
                    self.commit_committed_inst_type_fu_aliases[class.index()],
                    delta,
                )?;
            }
        }
        let mut lsq_operation_delta_total = 0_u64;
        for operation in O3RuntimeLsqOperation::TRACKED {
            let delta = current
                .lsq_operation_count(operation)
                .saturating_sub(previous.lsq_operation_count(operation));
            lsq_operation_delta_total = lsq_operation_delta_total.saturating_add(delta);
            if delta != 0 {
                registry.increment(self.lsq_operation_counts[operation.index()], delta)?;
                registry.increment(self.lsq_operation_alias_counts[operation.index()], delta)?;
            }
        }
        if lsq_operation_delta_total != 0 {
            registry.increment(self.lsq_operation_alias_total, lsq_operation_delta_total)?;
        }
        for operation in O3RuntimeLsqOperation::TRACKED {
            for (stat, previous, current) in [
                (
                    self.lsq_operation_load_bytes[operation.index()],
                    previous.lsq_operation_load_bytes(operation),
                    current.lsq_operation_load_bytes(operation),
                ),
                (
                    self.lsq_operation_load_byte_aliases[operation.index()],
                    previous.lsq_operation_load_bytes(operation),
                    current.lsq_operation_load_bytes(operation),
                ),
                (
                    self.lsq_operation_store_bytes[operation.index()],
                    previous.lsq_operation_store_bytes(operation),
                    current.lsq_operation_store_bytes(operation),
                ),
                (
                    self.lsq_operation_store_byte_aliases[operation.index()],
                    previous.lsq_operation_store_bytes(operation),
                    current.lsq_operation_store_bytes(operation),
                ),
                (
                    self.lsq_operation_store_conditional_failures[operation.index()],
                    previous.lsq_operation_store_conditional_failures(operation),
                    current.lsq_operation_store_conditional_failures(operation),
                ),
                (
                    self.lsq_operation_nested_store_conditional_failures[operation.index()],
                    previous.lsq_operation_store_conditional_failures(operation),
                    current.lsq_operation_store_conditional_failures(operation),
                ),
                (
                    self.lsq_operation_store_conditional_failure_aliases[operation.index()],
                    previous.lsq_operation_store_conditional_failures(operation),
                    current.lsq_operation_store_conditional_failures(operation),
                ),
            ] {
                let delta = current.saturating_sub(previous);
                if delta != 0 {
                    registry.increment(stat, delta)?;
                }
            }
        }
        for operation in O3RuntimeLsqOperation::TRACKED {
            let candidate_delta = current
                .lsq_operation_forwarding_candidates(operation)
                .saturating_sub(previous.lsq_operation_forwarding_candidates(operation));
            if candidate_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_candidates[operation.index()],
                    candidate_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_candidate_aliases[operation.index()],
                    candidate_delta,
                )?;
                registry.increment(
                    self.lsq_operation_nested_forwarding_candidates[operation.index()],
                    candidate_delta,
                )?;
            }
            let match_delta = current
                .lsq_operation_forwarding_matches(operation)
                .saturating_sub(previous.lsq_operation_forwarding_matches(operation));
            if match_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_matches[operation.index()],
                    match_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_match_aliases[operation.index()],
                    match_delta,
                )?;
                registry.increment(
                    self.lsq_operation_nested_forwarding_matches[operation.index()],
                    match_delta,
                )?;
            }
            let suppressed_delta = current
                .lsq_operation_forwarding_suppressed(operation)
                .saturating_sub(previous.lsq_operation_forwarding_suppressed(operation));
            if suppressed_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_suppressed[operation.index()],
                    suppressed_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_suppressed_aliases[operation.index()],
                    suppressed_delta,
                )?;
                registry.increment(
                    self.lsq_operation_nested_forwarding_suppressed[operation.index()],
                    suppressed_delta,
                )?;
            }
            let address_mismatch_delta = current
                .lsq_operation_forwarding_address_mismatches(operation)
                .saturating_sub(previous.lsq_operation_forwarding_address_mismatches(operation));
            if address_mismatch_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_address_mismatches[operation.index()],
                    address_mismatch_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_address_mismatch_aliases[operation.index()],
                    address_mismatch_delta,
                )?;
                registry.increment(
                    self.lsq_operation_nested_forwarding_address_mismatches[operation.index()],
                    address_mismatch_delta,
                )?;
            }
            let byte_mismatch_delta = current
                .lsq_operation_forwarding_byte_mismatches(operation)
                .saturating_sub(previous.lsq_operation_forwarding_byte_mismatches(operation));
            if byte_mismatch_delta != 0 {
                registry.increment(
                    self.lsq_operation_forwarding_byte_mismatches[operation.index()],
                    byte_mismatch_delta,
                )?;
                registry.increment(
                    self.lsq_operation_forwarding_byte_mismatch_aliases[operation.index()],
                    byte_mismatch_delta,
                )?;
                registry.increment(
                    self.lsq_operation_nested_forwarding_byte_mismatches[operation.index()],
                    byte_mismatch_delta,
                )?;
            }
        }
        self.set_lsq_latency_snapshot(registry, current)?;
        let mut lsq_ordering_delta_total = 0_u64;
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            let delta = current
                .lsq_ordering_count(ordering)
                .saturating_sub(previous.lsq_ordering_count(ordering));
            lsq_ordering_delta_total = lsq_ordering_delta_total.saturating_add(delta);
            if delta != 0 {
                registry.increment(self.lsq_ordering_counts[ordering.index()], delta)?;
                registry.increment(self.lsq_ordering_alias_counts[ordering.index()], delta)?;
            }
        }
        if lsq_ordering_delta_total != 0 {
            registry.increment(self.lsq_ordering_alias_total, lsq_ordering_delta_total)?;
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            let class_stats = self.fu_latency_classes[class.index()];
            let nested_class_stats = self.nested_fu_latency_classes[class.index()];
            for (stat, previous, current) in [
                (
                    class_stats.instructions,
                    previous.fu_latency_class_instructions(class),
                    current.fu_latency_class_instructions(class),
                ),
                (
                    class_stats.latency_cycles,
                    previous.fu_latency_class_cycles(class),
                    current.fu_latency_class_cycles(class),
                ),
                (
                    nested_class_stats.instructions,
                    previous.fu_latency_class_instructions(class),
                    current.fu_latency_class_instructions(class),
                ),
                (
                    nested_class_stats.latency_cycles,
                    previous.fu_latency_class_cycles(class),
                    current.fu_latency_class_cycles(class),
                ),
            ] {
                let delta = current.saturating_sub(previous);
                if delta != 0 {
                    registry.increment(stat, delta)?;
                }
            }
        }
        self.set_fu_latency_class_extrema_snapshot(registry, current)?;
        Ok(())
    }

    pub(super) fn set_event_window_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: RiscvO3RuntimeEventWindowSnapshot,
    ) -> Result<(), StatsError> {
        if let Some(event_window) = self.event_window {
            event_window.set_snapshot(registry, snapshot)?;
        }
        Ok(())
    }

    pub(super) fn set_event_summary_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: &RiscvO3RuntimeEventSummarySnapshot,
    ) -> Result<(), StatsError> {
        if let Some(event_summary) = self.event_summary {
            event_summary.set_snapshot(registry, snapshot)?;
        }
        Ok(())
    }

    pub(super) fn set_snapshot(
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

    fn set_runtime_snapshot_counts(
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

    fn set_fu_latency_class_extrema_snapshot(
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

    fn set_iew_rate_snapshots(
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

    fn set_lsq_latency_snapshot(
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
