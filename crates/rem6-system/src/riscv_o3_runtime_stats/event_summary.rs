use std::collections::BTreeMap;

use rem6_cpu::{
    BranchTargetKind, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeTraceRecord,
};
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::groups::{
    RiscvO3RuntimeBranchDirectionMismatchStats, RiscvO3RuntimeBranchEventKindStats,
    RiscvO3RuntimeBranchRepairStats, RiscvO3RuntimeBranchTargetMismatchStats,
    RiscvO3RuntimeFuLatencyClassStats, RiscvO3RuntimeLsqLatencyStats,
};
use super::helpers::{
    ratio_ppm, register_o3_branch_event_kind_counters,
    register_o3_commit_fu_latency_class_counters, register_o3_counter,
    register_o3_iq_fu_latency_class_counters, register_o3_lsq_operation_counters,
    register_o3_lsq_operation_nested_counters, register_o3_lsq_operation_nested_latency_counters,
    register_o3_lsq_ordering_counters, register_o3_nested_fu_latency_class_counters,
    set_o3_lsq_latency_counters,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct LsqLatencySummary {
    samples: u64,
    ticks: u64,
    max_ticks: u64,
    min_ticks: u64,
}

impl LsqLatencySummary {
    fn observe(&mut self, ticks: u64) {
        if self.samples == 0 {
            self.min_ticks = ticks;
        } else {
            self.min_ticks = self.min_ticks.min(ticks);
        }
        self.samples = self.samples.saturating_add(1);
        self.ticks = self.ticks.saturating_add(ticks);
        self.max_ticks = self.max_ticks.max(ticks);
    }

    fn min_ticks(self) -> u64 {
        if self.samples == 0 {
            0
        } else {
            self.min_ticks
        }
    }

    fn avg_ticks(self) -> u64 {
        if self.samples == 0 {
            0
        } else {
            self.ticks / self.samples
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct FuLatencySummary {
    instructions: u64,
    cycles: u64,
    max_cycles: u64,
    min_cycles: u64,
}

impl FuLatencySummary {
    fn observe(&mut self, cycles: u64) {
        if self.instructions == 0 {
            self.min_cycles = cycles;
        } else {
            self.min_cycles = self.min_cycles.min(cycles);
        }
        self.instructions = self.instructions.saturating_add(1);
        self.cycles = self.cycles.saturating_add(cycles);
        self.max_cycles = self.max_cycles.max(cycles);
    }

    fn min_cycles(self) -> u64 {
        if self.instructions == 0 {
            0
        } else {
            self.min_cycles
        }
    }

    fn avg_cycles(self) -> u64 {
        if self.instructions == 0 {
            0
        } else {
            self.cycles / self.instructions
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeEventSummarySnapshot {
    events: BTreeMap<u64, O3RuntimeTraceRecord>,
}

impl RiscvO3RuntimeEventSummarySnapshot {
    pub(super) fn observe(&mut self, event: &O3RuntimeTraceRecord) {
        self.events.insert(event.sequence(), *event);
    }

    fn records(&self) -> u64 {
        self.events.len() as u64
    }

    fn span_ticks(&self) -> u64 {
        let first = self.events.values().next().map_or(0, |event| event.tick());
        let last = self
            .events
            .values()
            .next_back()
            .map_or(0, |event| event.tick());
        last.saturating_sub(first)
    }

    fn rob_allocations(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::rob_allocated)
    }

    fn rob_commits(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::rob_committed)
    }

    fn rob_max_occupancy(&self) -> u64 {
        self.events
            .values()
            .copied()
            .map(|event| event.rob_occupancy())
            .max()
            .unwrap_or(0)
    }

    fn rob_commit_blocked_events(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::rob_commit_blocked)
    }

    fn rob_max_commits_at_tick(&self) -> u64 {
        self.events
            .values()
            .copied()
            .map(|event| event.rob_commits_at_tick())
            .max()
            .unwrap_or(0)
    }

    fn rename_writes(&self) -> u64 {
        self.sum(O3RuntimeTraceRecord::rename_writes)
    }

    fn branch_event_mispredictions(&self) -> u64 {
        self.events
            .values()
            .filter(|event| event.branch_event() && event.branch_mispredicted())
            .count() as u64
    }

    fn branch_event_branches(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::branch_event)
    }

    fn branch_event_taken(&self) -> u64 {
        self.count(|event| event.branch_event() && event.branch_resolved_taken())
    }

    fn branch_event_not_taken(&self) -> u64 {
        self.count(|event| event.branch_event() && !event.branch_resolved_taken())
    }

    fn branch_event_predicted_taken(&self) -> u64 {
        self.count(|event| event.branch_event() && event.branch_predicted_taken())
    }

    fn branch_event_predicted_not_taken(&self) -> u64 {
        self.count(|event| event.branch_event() && !event.branch_predicted_taken())
    }

    fn branch_event_predicted_targets(&self) -> u64 {
        self.count(|event| event.branch_event() && event.branch_predicted_target().is_some())
    }

    fn branch_event_predicted_target_matches(&self) -> u64 {
        self.count(branch_predicted_target_match)
    }

    fn branch_event_predicted_target_mismatches(&self) -> u64 {
        self.count(branch_predicted_target_mismatch)
    }

    fn branch_event_resolved_targets(&self) -> u64 {
        self.count(|event| event.branch_event() && event.branch_resolved_target().is_some())
    }

    fn branch_event_link_writes(&self) -> u64 {
        self.count(|event| event.branch_event() && event.branch_link_register_write())
    }

    fn branch_event_without_link_writes(&self) -> u64 {
        self.count(|event| event.branch_event() && !event.branch_link_register_write())
    }

    fn branch_event_squashes(&self) -> u64 {
        self.count(|event| event.branch_event() && event.branch_squash())
    }

    fn branch_event_squashed_targets(&self) -> u64 {
        self.count(|event| event.branch_event() && event.branch_squashed_target().is_some())
    }

    fn branch_event_squashed_targets_with_link_writes(&self) -> u64 {
        self.count(|event| {
            event.branch_event()
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        })
    }

    fn branch_event_squashed_targets_without_link_writes(&self) -> u64 {
        self.count(|event| {
            event.branch_event()
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        })
    }

    fn branch_event_kind<F>(&self, kind: BranchTargetKind, matches: F) -> u64
    where
        F: Fn(O3RuntimeTraceRecord) -> bool,
    {
        self.count(|event| event.branch_kind() == kind && matches(event))
    }

    fn branch_repair_targetless_mismatches(&self) -> u64 {
        self.count(|event| branch_targetless_mismatch(&event))
    }

    fn branch_repair_wrong_targets(&self) -> u64 {
        self.count(|event| branch_wrong_target(&event))
    }

    fn branch_repair_direction_only_mismatches(&self) -> u64 {
        self.events
            .values()
            .filter(|event| {
                event.branch_event()
                    && !branch_targetless_mismatch(event)
                    && !branch_wrong_target(event)
                    && event.branch_predicted_taken() != event.branch_resolved_taken()
            })
            .count() as u64
    }

    fn branch_repair_targetless_mismatch_kind(&self, kind: BranchTargetKind) -> u64 {
        self.count(|event| event.branch_kind() == kind && branch_targetless_mismatch(&event))
    }

    fn branch_repair_wrong_target_kind(&self, kind: BranchTargetKind) -> u64 {
        self.count(|event| event.branch_kind() == kind && branch_wrong_target(&event))
    }

    fn branch_repair_direction_only_kind(&self, kind: BranchTargetKind) -> u64 {
        self.count(|event| {
            event.branch_kind() == kind
                && event.branch_event()
                && !branch_targetless_mismatch(&event)
                && !branch_wrong_target(&event)
                && event.branch_predicted_taken() != event.branch_resolved_taken()
        })
    }

    fn iew_dispatched_insts(&self) -> u64 {
        self.records()
    }

    fn iew_insts_to_commit(&self) -> u64 {
        self.rob_commits()
    }

    fn iew_writeback_count(&self) -> u64 {
        self.records()
    }

    fn iew_writeback_rate_ppm(&self) -> u64 {
        ratio_ppm(self.iew_writeback_count(), self.span_ticks())
    }

    fn iew_dependency_producers(&self) -> u64 {
        self.sum(O3RuntimeTraceRecord::iew_dependency_producers)
    }

    fn iew_dependency_consumers(&self) -> u64 {
        self.sum(O3RuntimeTraceRecord::iew_dependency_consumers)
    }

    fn iew_producer_consumer_fanout_ppm(&self) -> u64 {
        ratio_ppm(
            self.iew_dependency_producers(),
            self.iew_dependency_consumers(),
        )
    }

    fn iew_predicted_taken_incorrect(&self) -> u64 {
        self.count(|event| event.branch_mispredicted() && event.branch_predicted_taken())
    }

    fn iew_predicted_not_taken_incorrect(&self) -> u64 {
        self.count(|event| event.branch_mispredicted() && !event.branch_predicted_taken())
    }

    fn iew_branch_mispredicts(&self) -> u64 {
        self.iew_predicted_taken_incorrect()
            .saturating_add(self.iew_predicted_not_taken_incorrect())
    }

    fn iq_insts_issued(&self) -> u64 {
        self.records()
    }

    fn iq_mem_insts_issued(&self) -> u64 {
        self.lsq_operation_count(O3RuntimeLsqOperation::Load)
            .saturating_add(self.lsq_operation_count(O3RuntimeLsqOperation::Store))
    }

    fn iq_branch_insts_issued(&self) -> u64 {
        self.branch_event_branches()
    }

    fn fu_latency(&self) -> FuLatencySummary {
        self.fu_latency_summary(|event| event.fu_latency_cycles() != 0)
    }

    fn fu_latency_class(&self, class: O3RuntimeFuLatencyClass) -> FuLatencySummary {
        self.fu_latency_summary(|event| event.fu_latency_class() == Some(class))
    }

    fn fu_latency_summary<F>(&self, matches: F) -> FuLatencySummary
    where
        F: Fn(O3RuntimeTraceRecord) -> bool,
    {
        let mut latency = FuLatencySummary::default();
        for event in self
            .events
            .values()
            .copied()
            .filter(|event| matches(*event) && event.fu_latency_cycles() != 0)
        {
            latency.observe(event.fu_latency_cycles());
        }
        latency
    }

    fn lsq_load_bytes(&self) -> u64 {
        self.sum(O3RuntimeTraceRecord::lsq_load_bytes)
    }

    fn lsq_store_bytes(&self) -> u64 {
        self.sum(O3RuntimeTraceRecord::lsq_store_bytes)
    }

    fn lsq_store_conditional_failures(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::lsq_store_conditional_failed)
    }

    fn store_load_forwarding_candidates(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::store_load_forwarding_candidate)
    }

    fn store_load_forwarding_matches(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::store_load_forwarding_match)
    }

    fn store_load_forwarding_suppressed(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::store_load_forwarding_suppressed)
    }

    fn store_load_forwarding_address_mismatches(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::store_load_forwarding_address_mismatch)
    }

    fn store_load_forwarding_byte_mismatches(&self) -> u64 {
        self.count(O3RuntimeTraceRecord::store_load_forwarding_byte_mismatch)
    }

    fn lsq_operation_count(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.count(|event| event.lsq_operation() == operation)
    }

    fn lsq_operation_load_bytes(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.events
            .values()
            .copied()
            .filter(|event| event.lsq_operation() == operation)
            .map(O3RuntimeTraceRecord::lsq_load_bytes)
            .sum()
    }

    fn lsq_operation_store_bytes(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.events
            .values()
            .copied()
            .filter(|event| event.lsq_operation() == operation)
            .map(O3RuntimeTraceRecord::lsq_store_bytes)
            .sum()
    }

    fn lsq_operation_store_conditional_failures(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.count(|event| {
            event.lsq_operation() == operation && event.lsq_store_conditional_failed()
        })
    }

    fn lsq_operation_forwarding_candidates(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.count(|event| {
            event.lsq_operation() == operation && event.store_load_forwarding_candidate()
        })
    }

    fn lsq_operation_forwarding_matches(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.count(|event| {
            event.lsq_operation() == operation && event.store_load_forwarding_match()
        })
    }

    fn lsq_operation_forwarding_suppressed(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.count(|event| {
            event.lsq_operation() == operation && event.store_load_forwarding_suppressed()
        })
    }

    fn lsq_operation_forwarding_address_mismatches(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.count(|event| {
            event.lsq_operation() == operation && event.store_load_forwarding_address_mismatch()
        })
    }

    fn lsq_operation_forwarding_byte_mismatches(&self, operation: O3RuntimeLsqOperation) -> u64 {
        self.count(|event| {
            event.lsq_operation() == operation && event.store_load_forwarding_byte_mismatch()
        })
    }

    fn lsq_ordering_count(&self, ordering: O3RuntimeLsqOrdering) -> u64 {
        self.count(|event| event.lsq_ordering() == ordering)
    }

    fn lsq_data_latency(&self) -> LsqLatencySummary {
        self.lsq_latency(|event| event.lsq_operation() != O3RuntimeLsqOperation::None)
    }

    fn lsq_operation_latency(&self, operation: O3RuntimeLsqOperation) -> LsqLatencySummary {
        self.lsq_latency(|event| event.lsq_operation() == operation)
    }

    fn lsq_latency<F>(&self, matches: F) -> LsqLatencySummary
    where
        F: Fn(O3RuntimeTraceRecord) -> bool,
    {
        let mut latency = LsqLatencySummary::default();
        for event in self
            .events
            .values()
            .copied()
            .filter(|event| matches(*event))
        {
            latency.observe(event.lsq_data_latency_ticks());
        }
        latency
    }

    fn count<F>(&self, matches: F) -> u64
    where
        F: Fn(O3RuntimeTraceRecord) -> bool,
    {
        self.events
            .values()
            .copied()
            .filter(|event| matches(*event))
            .count() as u64
    }

    fn sum<F>(&self, value: F) -> u64
    where
        F: Fn(O3RuntimeTraceRecord) -> u64,
    {
        self.events.values().copied().map(value).sum()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeEventSummaryStats {
    records: StatId,
    span_ticks: StatId,
    rob_allocations: StatId,
    rob_commits: StatId,
    rob_max_occupancy: StatId,
    rob_commit_blocked_events: StatId,
    rob_max_commits_at_tick: StatId,
    rename_writes: StatId,
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
    branch_repair_targetless_mismatches: StatId,
    branch_repair_wrong_targets: StatId,
    branch_repair_direction_only_mismatches: StatId,
    branch_repair_kinds: [RiscvO3RuntimeBranchRepairStats; BranchTargetKind::COUNT],
    branch_direction_mismatch: RiscvO3RuntimeBranchDirectionMismatchStats,
    branch_target_mismatch: RiscvO3RuntimeBranchTargetMismatchStats,
    iq_insts_issued: StatId,
    iq_mem_insts_issued: StatId,
    iq_branch_insts_issued: StatId,
    iq_issued_inst_type_mem_read: StatId,
    iq_issued_inst_type_mem_write: StatId,
    iq_issued_inst_type_fu_classes: [StatId; O3RuntimeFuLatencyClass::COUNT],
    iew_dispatched_insts: StatId,
    iew_insts_to_commit: StatId,
    iew_writeback_count: StatId,
    iew_writeback_rate_ppm: StatId,
    iew_producer_inst: StatId,
    iew_consumer_inst: StatId,
    iew_producer_consumer_fanout_ppm: StatId,
    iew_predicted_taken_incorrect: StatId,
    iew_predicted_not_taken_incorrect: StatId,
    iew_branch_mispredicts: StatId,
    iew_dependency_producer: StatId,
    iew_dependency_consumer: StatId,
    commit_branch_mispredicts: StatId,
    commit_committed_inst_type_mem_read: StatId,
    commit_committed_inst_type_mem_write: StatId,
    commit_committed_inst_type_fu_classes: [StatId; O3RuntimeFuLatencyClass::COUNT],
    fu_latency_instructions: StatId,
    fu_latency_cycles: StatId,
    fu_latency_max_cycles: StatId,
    fu_latency_min_cycles: StatId,
    fu_latency_avg_cycles: StatId,
    fu_latency_classes: [RiscvO3RuntimeFuLatencyClassStats; O3RuntimeFuLatencyClass::COUNT],
    lsq_load_bytes: StatId,
    lsq_store_bytes: StatId,
    lsq_store_conditional_failures: StatId,
    store_load_forwarding_candidates: StatId,
    store_load_forwarding_matches: StatId,
    store_load_forwarding_suppressed: StatId,
    store_load_forwarding_address_mismatches: StatId,
    store_load_forwarding_byte_mismatches: StatId,
    lsq_data_latency: RiscvO3RuntimeLsqLatencyStats,
    lsq_operation_counts: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_load_bytes: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_store_bytes: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_store_conditional_failures: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_candidates: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_matches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_suppressed: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_address_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_forwarding_byte_mismatches: [StatId; O3RuntimeLsqOperation::COUNT],
    lsq_operation_latency: [RiscvO3RuntimeLsqLatencyStats; O3RuntimeLsqOperation::COUNT],
    lsq_ordering_counts: [StatId; O3RuntimeLsqOrdering::COUNT],
}

impl RiscvO3RuntimeEventSummaryStats {
    pub(super) fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        let prefix = format!("{prefix}.event_summary");
        Ok(Self {
            records: register_o3_counter(registry, &prefix, "records", "Count")?,
            span_ticks: register_o3_counter(registry, &prefix, "span_ticks", "Tick")?,
            rob_allocations: register_o3_counter(registry, &prefix, "rob.allocations", "Count")?,
            rob_commits: register_o3_counter(registry, &prefix, "rob.commits", "Count")?,
            rob_max_occupancy: register_o3_counter(
                registry,
                &prefix,
                "rob.max_occupancy",
                "Count",
            )?,
            rob_commit_blocked_events: register_o3_counter(
                registry,
                &prefix,
                "rob.commit_blocked_events",
                "Count",
            )?,
            rob_max_commits_at_tick: register_o3_counter(
                registry,
                &prefix,
                "rob.max_commits_at_tick",
                "Count",
            )?,
            rename_writes: register_o3_counter(registry, &prefix, "rename.writes", "Count")?,
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
            branch_repair_targetless_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_repair.targetless_mismatches",
                "Count",
            )?,
            branch_repair_wrong_targets: register_o3_counter(
                registry,
                &prefix,
                "branch_repair.wrong_targets",
                "Count",
            )?,
            branch_repair_direction_only_mismatches: register_o3_counter(
                registry,
                &prefix,
                "branch_repair.direction_only_mismatches",
                "Count",
            )?,
            branch_repair_kinds: register_event_summary_branch_repair_kind_counters(
                registry, &prefix,
            )?,
            branch_direction_mismatch: RiscvO3RuntimeBranchDirectionMismatchStats::register(
                registry, &prefix,
            )?,
            branch_target_mismatch: RiscvO3RuntimeBranchTargetMismatchStats::register(
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
            iew_writeback_rate_ppm: register_o3_counter(
                registry,
                &prefix,
                "iew.writeback_rate_ppm",
                "Ppm",
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
            iew_dependency_producer: register_o3_counter(
                registry,
                &prefix,
                "iew.dependency.producer",
                "Count",
            )?,
            iew_dependency_consumer: register_o3_counter(
                registry,
                &prefix,
                "iew.dependency.consumer",
                "Count",
            )?,
            commit_branch_mispredicts: register_o3_counter(
                registry,
                &prefix,
                "commit.branch_mispredicts",
                "Count",
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
            fu_latency_instructions: register_o3_counter(
                registry,
                &prefix,
                "fu_latency.instructions",
                "Count",
            )?,
            fu_latency_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_latency.cycles",
                "Cycle",
            )?,
            fu_latency_max_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_latency.max_cycles",
                "Cycle",
            )?,
            fu_latency_min_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_latency.min_cycles",
                "Cycle",
            )?,
            fu_latency_avg_cycles: register_o3_counter(
                registry,
                &prefix,
                "fu_latency.avg_cycles",
                "Cycle",
            )?,
            fu_latency_classes: register_o3_nested_fu_latency_class_counters(registry, &prefix)?,
            lsq_load_bytes: register_o3_counter(registry, &prefix, "lsq_load_bytes", "Byte")?,
            lsq_store_bytes: register_o3_counter(registry, &prefix, "lsq_store_bytes", "Byte")?,
            lsq_store_conditional_failures: register_o3_counter(
                registry,
                &prefix,
                "lsq_store_conditional_failures",
                "Count",
            )?,
            store_load_forwarding_candidates: register_o3_counter(
                registry,
                &prefix,
                "store_load_forwarding_candidates",
                "Count",
            )?,
            store_load_forwarding_matches: register_o3_counter(
                registry,
                &prefix,
                "store_load_forwarding_matches",
                "Count",
            )?,
            store_load_forwarding_suppressed: register_o3_counter(
                registry,
                &prefix,
                "store_load_forwarding_suppressed",
                "Count",
            )?,
            store_load_forwarding_address_mismatches: register_o3_counter(
                registry,
                &prefix,
                "store_load_forwarding_address_mismatches",
                "Count",
            )?,
            store_load_forwarding_byte_mismatches: register_o3_counter(
                registry,
                &prefix,
                "store_load_forwarding_byte_mismatches",
                "Count",
            )?,
            lsq_data_latency: register_event_summary_lsq_latency_counters(
                registry,
                &prefix,
                "lsq_data_latency",
            )?,
            lsq_operation_counts: register_o3_lsq_operation_counters(registry, &prefix)?,
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
            lsq_operation_store_conditional_failures: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "store_conditional_failures",
                "Count",
            )?,
            lsq_operation_forwarding_candidates: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_candidates",
                "Count",
            )?,
            lsq_operation_forwarding_matches: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_matches",
                "Count",
            )?,
            lsq_operation_forwarding_suppressed: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_suppressed",
                "Count",
            )?,
            lsq_operation_forwarding_address_mismatches: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_address_mismatches",
                "Count",
            )?,
            lsq_operation_forwarding_byte_mismatches: register_o3_lsq_operation_nested_counters(
                registry,
                &prefix,
                "forwarding_byte_mismatches",
                "Count",
            )?,
            lsq_operation_latency: register_o3_lsq_operation_nested_latency_counters(
                registry, &prefix,
            )?,
            lsq_ordering_counts: register_o3_lsq_ordering_counters(registry, &prefix)?,
        })
    }

    pub(super) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: &RiscvO3RuntimeEventSummarySnapshot,
    ) -> Result<(), StatsError> {
        let branch_mispredicts = snapshot.branch_event_mispredictions();
        let fu_latency = snapshot.fu_latency();
        let iew_producer_inst = snapshot.iew_dependency_producers();
        let iew_consumer_inst = snapshot.iew_dependency_consumers();
        let iew_branch_mispredicts = snapshot.iew_branch_mispredicts();
        let lsq_operation_loads = snapshot.lsq_operation_count(O3RuntimeLsqOperation::Load);
        let lsq_operation_stores = snapshot.lsq_operation_count(O3RuntimeLsqOperation::Store);
        for (stat, value) in [
            (self.records, snapshot.records()),
            (self.span_ticks, snapshot.span_ticks()),
            (self.rob_allocations, snapshot.rob_allocations()),
            (self.rob_commits, snapshot.rob_commits()),
            (self.rob_max_occupancy, snapshot.rob_max_occupancy()),
            (
                self.rob_commit_blocked_events,
                snapshot.rob_commit_blocked_events(),
            ),
            (
                self.rob_max_commits_at_tick,
                snapshot.rob_max_commits_at_tick(),
            ),
            (self.rename_writes, snapshot.rename_writes()),
            (self.branch_event_branches, snapshot.branch_event_branches()),
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
            (self.branch_event_mispredictions, branch_mispredicts),
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
            (self.iq_insts_issued, snapshot.iq_insts_issued()),
            (self.iq_mem_insts_issued, snapshot.iq_mem_insts_issued()),
            (
                self.iq_branch_insts_issued,
                snapshot.iq_branch_insts_issued(),
            ),
            (self.iq_issued_inst_type_mem_read, lsq_operation_loads),
            (self.iq_issued_inst_type_mem_write, lsq_operation_stores),
            (self.iew_dispatched_insts, snapshot.iew_dispatched_insts()),
            (self.iew_insts_to_commit, snapshot.iew_insts_to_commit()),
            (self.iew_writeback_count, snapshot.iew_writeback_count()),
            (
                self.iew_writeback_rate_ppm,
                snapshot.iew_writeback_rate_ppm(),
            ),
            (self.iew_producer_inst, iew_producer_inst),
            (self.iew_consumer_inst, iew_consumer_inst),
            (
                self.iew_producer_consumer_fanout_ppm,
                snapshot.iew_producer_consumer_fanout_ppm(),
            ),
            (
                self.iew_predicted_taken_incorrect,
                snapshot.iew_predicted_taken_incorrect(),
            ),
            (
                self.iew_predicted_not_taken_incorrect,
                snapshot.iew_predicted_not_taken_incorrect(),
            ),
            (self.iew_branch_mispredicts, iew_branch_mispredicts),
            (self.iew_dependency_producer, iew_producer_inst),
            (self.iew_dependency_consumer, iew_consumer_inst),
            (self.commit_branch_mispredicts, iew_branch_mispredicts),
            (
                self.commit_committed_inst_type_mem_read,
                lsq_operation_loads,
            ),
            (
                self.commit_committed_inst_type_mem_write,
                lsq_operation_stores,
            ),
            (self.fu_latency_instructions, fu_latency.instructions),
            (self.fu_latency_cycles, fu_latency.cycles),
            (self.fu_latency_max_cycles, fu_latency.max_cycles),
            (self.fu_latency_min_cycles, fu_latency.min_cycles()),
            (self.fu_latency_avg_cycles, fu_latency.avg_cycles()),
            (self.lsq_load_bytes, snapshot.lsq_load_bytes()),
            (self.lsq_store_bytes, snapshot.lsq_store_bytes()),
            (
                self.lsq_store_conditional_failures,
                snapshot.lsq_store_conditional_failures(),
            ),
            (
                self.store_load_forwarding_candidates,
                snapshot.store_load_forwarding_candidates(),
            ),
            (
                self.store_load_forwarding_matches,
                snapshot.store_load_forwarding_matches(),
            ),
            (
                self.store_load_forwarding_suppressed,
                snapshot.store_load_forwarding_suppressed(),
            ),
            (
                self.store_load_forwarding_address_mismatches,
                snapshot.store_load_forwarding_address_mismatches(),
            ),
            (
                self.store_load_forwarding_byte_mismatches,
                snapshot.store_load_forwarding_byte_mismatches(),
            ),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        set_event_summary_branch_direction_mismatch_stats(
            registry,
            self.branch_direction_mismatch,
            snapshot,
        )?;
        set_event_summary_branch_target_mismatch_stats(
            registry,
            self.branch_target_mismatch,
            snapshot,
        )?;
        for kind in BranchTargetKind::ALL {
            let branch_event_stats = self.branch_event_kinds[kind.index()];
            for (stat, value) in [
                (
                    branch_event_stats.kind,
                    snapshot.branch_event_kind(kind, O3RuntimeTraceRecord::branch_event),
                ),
                (
                    branch_event_stats.taken,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_resolved_taken()
                    }),
                ),
                (
                    branch_event_stats.not_taken,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && !event.branch_resolved_taken()
                    }),
                ),
                (
                    branch_event_stats.predicted_taken,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_predicted_taken()
                    }),
                ),
                (
                    branch_event_stats.predicted_not_taken,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && !event.branch_predicted_taken()
                    }),
                ),
                (
                    branch_event_stats.predicted_target,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_predicted_target().is_some()
                    }),
                ),
                (
                    branch_event_stats.predicted_target_match,
                    snapshot.branch_event_kind(kind, branch_predicted_target_match),
                ),
                (
                    branch_event_stats.predicted_target_mismatch,
                    snapshot.branch_event_kind(kind, branch_predicted_target_mismatch),
                ),
                (
                    branch_event_stats.resolved_target,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_resolved_target().is_some()
                    }),
                ),
                (
                    branch_event_stats.misprediction,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_mispredicted()
                    }),
                ),
                (
                    branch_event_stats.link_write,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_link_register_write()
                    }),
                ),
                (
                    branch_event_stats.without_link_write,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && !event.branch_link_register_write()
                    }),
                ),
                (
                    branch_event_stats.squash,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_squash()
                    }),
                ),
                (
                    branch_event_stats.squashed_target,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event() && event.branch_squashed_target().is_some()
                    }),
                ),
                (
                    branch_event_stats.squashed_target_link_write,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event()
                            && event.branch_squashed_target().is_some()
                            && event.branch_link_register_write()
                    }),
                ),
                (
                    branch_event_stats.squashed_target_without_link_write,
                    snapshot.branch_event_kind(kind, |event| {
                        event.branch_event()
                            && event.branch_squashed_target().is_some()
                            && !event.branch_link_register_write()
                    }),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
            let branch_repair_stats = self.branch_repair_kinds[kind.index()];
            for (stat, value) in [
                (
                    branch_repair_stats.targetless_mismatch,
                    snapshot.branch_repair_targetless_mismatch_kind(kind),
                ),
                (
                    branch_repair_stats.wrong_target,
                    snapshot.branch_repair_wrong_target_kind(kind),
                ),
                (
                    branch_repair_stats.direction_only,
                    snapshot.branch_repair_direction_only_kind(kind),
                ),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        for class in O3RuntimeFuLatencyClass::ALL {
            let class_stats = self.fu_latency_classes[class.index()];
            let class_latency = snapshot.fu_latency_class(class);
            registry.set_resettable_counter(
                self.iq_issued_inst_type_fu_classes[class.index()],
                class_latency.instructions,
            )?;
            registry.set_resettable_counter(
                self.commit_committed_inst_type_fu_classes[class.index()],
                class_latency.instructions,
            )?;
            for (stat, value) in [
                (class_stats.instructions, class_latency.instructions),
                (class_stats.latency_cycles, class_latency.cycles),
                (class_stats.latency_max_cycles, class_latency.max_cycles),
                (class_stats.latency_min_cycles, class_latency.min_cycles()),
                (class_stats.latency_avg_cycles, class_latency.avg_cycles()),
            ] {
                registry.set_resettable_counter(stat, value)?;
            }
        }
        set_lsq_latency_summary(registry, self.lsq_data_latency, snapshot.lsq_data_latency())?;
        for operation in O3RuntimeLsqOperation::TRACKED {
            registry.set_resettable_counter(
                self.lsq_operation_counts[operation.index()],
                snapshot.lsq_operation_count(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_load_bytes[operation.index()],
                snapshot.lsq_operation_load_bytes(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_store_bytes[operation.index()],
                snapshot.lsq_operation_store_bytes(operation),
            )?;
            registry.set_resettable_counter(
                self.lsq_operation_store_conditional_failures[operation.index()],
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
            set_lsq_latency_summary(
                registry,
                self.lsq_operation_latency[operation.index()],
                snapshot.lsq_operation_latency(operation),
            )?;
        }
        for ordering in O3RuntimeLsqOrdering::TRACKED {
            registry.set_resettable_counter(
                self.lsq_ordering_counts[ordering.index()],
                snapshot.lsq_ordering_count(ordering),
            )?;
        }
        Ok(())
    }
}

fn register_event_summary_lsq_latency_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    stem: &str,
) -> Result<RiscvO3RuntimeLsqLatencyStats, StatsError> {
    Ok(RiscvO3RuntimeLsqLatencyStats {
        samples: register_o3_counter(registry, prefix, &format!("{stem}.samples"), "Count")?,
        ticks: register_o3_counter(registry, prefix, &format!("{stem}.ticks"), "Tick")?,
        max_ticks: register_o3_counter(registry, prefix, &format!("{stem}.max_ticks"), "Tick")?,
        min_ticks: register_o3_counter(registry, prefix, &format!("{stem}.min_ticks"), "Tick")?,
        avg_ticks: register_o3_counter(registry, prefix, &format!("{stem}.avg_ticks"), "Tick")?,
    })
}

fn register_event_summary_branch_repair_kind_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
) -> Result<[RiscvO3RuntimeBranchRepairStats; BranchTargetKind::COUNT], StatsError> {
    let mut stats = [RiscvO3RuntimeBranchRepairStats {
        targetless_mismatch: StatId::new(0),
        wrong_target: StatId::new(0),
        direction_only: StatId::new(0),
    }; BranchTargetKind::COUNT];
    for kind in BranchTargetKind::ALL {
        let stat_name = kind.canonical_stat_name();
        stats[kind.index()] = RiscvO3RuntimeBranchRepairStats {
            targetless_mismatch: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair.targetless_mismatch_kind.{stat_name}"),
                "Count",
            )?,
            wrong_target: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair.wrong_target_kind.{stat_name}"),
                "Count",
            )?,
            direction_only: register_o3_counter(
                registry,
                prefix,
                &format!("branch_repair.direction_only_kind.{stat_name}"),
                "Count",
            )?,
        };
    }
    Ok(stats)
}

fn set_event_summary_branch_direction_mismatch_stats(
    registry: &mut StatsRegistry,
    stats: RiscvO3RuntimeBranchDirectionMismatchStats,
    snapshot: &RiscvO3RuntimeEventSummarySnapshot,
) -> Result<(), StatsError> {
    for (stat, value) in [
        (stats.mismatches, snapshot.count(branch_direction_mismatch)),
        (
            stats.without_link_writes,
            snapshot.count(|event| {
                branch_direction_mismatch(event) && !event.branch_link_register_write()
            }),
        ),
        (
            stats.squashed_targets,
            snapshot.count(|event| {
                branch_direction_mismatch(event) && event.branch_squashed_target().is_some()
            }),
        ),
        (
            stats.squashed_target_without_link_writes,
            snapshot.count(|event| {
                branch_direction_mismatch(event)
                    && event.branch_squashed_target().is_some()
                    && !event.branch_link_register_write()
            }),
        ),
        (
            stats.squashed_target_link_writes,
            snapshot.count(|event| {
                branch_direction_mismatch(event)
                    && event.branch_squashed_target().is_some()
                    && event.branch_link_register_write()
            }),
        ),
    ] {
        registry.set_resettable_counter(stat, value)?;
    }
    set_event_summary_branch_kind_counters(
        registry,
        stats.kinds,
        snapshot,
        branch_direction_mismatch,
    )?;
    set_event_summary_branch_kind_counters(registry, stats.link_write_kinds, snapshot, |event| {
        branch_direction_mismatch(event) && event.branch_link_register_write()
    })?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.without_link_write_kinds,
        snapshot,
        |event| branch_direction_mismatch(event) && !event.branch_link_register_write(),
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.squashed_target_kinds,
        snapshot,
        |event| branch_direction_mismatch(event) && event.branch_squashed_target().is_some(),
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.squashed_target_link_write_kinds,
        snapshot,
        |event| {
            branch_direction_mismatch(event)
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        },
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.squashed_target_without_link_write_kinds,
        snapshot,
        |event| {
            branch_direction_mismatch(event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )
}

fn set_event_summary_branch_target_mismatch_stats(
    registry: &mut StatsRegistry,
    stats: RiscvO3RuntimeBranchTargetMismatchStats,
    snapshot: &RiscvO3RuntimeEventSummarySnapshot,
) -> Result<(), StatsError> {
    for (stat, value) in [
        (
            stats.targetless_mismatches,
            snapshot.count(|event| branch_targetless_mismatch(&event)),
        ),
        (
            stats.targetless_without_link_writes,
            snapshot.count(|event| {
                branch_targetless_mismatch(&event) && !event.branch_link_register_write()
            }),
        ),
        (
            stats.targetless_squashed_targets,
            snapshot.count(|event| {
                branch_targetless_mismatch(&event) && event.branch_squashed_target().is_some()
            }),
        ),
        (
            stats.targetless_squashed_target_without_link_writes,
            snapshot.count(|event| {
                branch_targetless_mismatch(&event)
                    && event.branch_squashed_target().is_some()
                    && !event.branch_link_register_write()
            }),
        ),
        (
            stats.wrong_targets,
            snapshot.count(|event| branch_wrong_target(&event)),
        ),
        (
            stats.wrong_target_squashed_targets,
            snapshot.count(|event| {
                branch_wrong_target(&event) && event.branch_squashed_target().is_some()
            }),
        ),
        (
            stats.wrong_target_squashed_target_without_link_writes,
            snapshot.count(|event| {
                branch_wrong_target(&event)
                    && event.branch_squashed_target().is_some()
                    && !event.branch_link_register_write()
            }),
        ),
        (
            stats.wrong_target_squashed_target_link_writes,
            snapshot.count(|event| {
                branch_wrong_target(&event)
                    && event.branch_squashed_target().is_some()
                    && event.branch_link_register_write()
            }),
        ),
        (
            stats.wrong_target_link_writes,
            snapshot
                .count(|event| branch_wrong_target(&event) && event.branch_link_register_write()),
        ),
        (
            stats.wrong_target_without_link_writes,
            snapshot
                .count(|event| branch_wrong_target(&event) && !event.branch_link_register_write()),
        ),
    ] {
        registry.set_resettable_counter(stat, value)?;
    }
    set_event_summary_branch_kind_counters(registry, stats.targetless_kinds, snapshot, |event| {
        branch_targetless_mismatch(&event)
    })?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.targetless_without_link_write_kinds,
        snapshot,
        |event| branch_targetless_mismatch(&event) && !event.branch_link_register_write(),
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.targetless_squashed_target_kinds,
        snapshot,
        |event| branch_targetless_mismatch(&event) && event.branch_squashed_target().is_some(),
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.targetless_squashed_target_without_link_write_kinds,
        snapshot,
        |event| {
            branch_targetless_mismatch(&event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.wrong_target_kinds,
        snapshot,
        |event| branch_wrong_target(&event),
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.wrong_target_squashed_target_kinds,
        snapshot,
        |event| branch_wrong_target(&event) && event.branch_squashed_target().is_some(),
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.wrong_target_squashed_target_without_link_write_kinds,
        snapshot,
        |event| {
            branch_wrong_target(&event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.wrong_target_squashed_target_link_write_kinds,
        snapshot,
        |event| {
            branch_wrong_target(&event)
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        },
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.wrong_target_link_write_kinds,
        snapshot,
        |event| branch_wrong_target(&event) && event.branch_link_register_write(),
    )?;
    set_event_summary_branch_kind_counters(
        registry,
        stats.wrong_target_without_link_write_kinds,
        snapshot,
        |event| branch_wrong_target(&event) && !event.branch_link_register_write(),
    )
}

fn set_event_summary_branch_kind_counters<F>(
    registry: &mut StatsRegistry,
    stats: [StatId; BranchTargetKind::COUNT],
    snapshot: &RiscvO3RuntimeEventSummarySnapshot,
    matches: F,
) -> Result<(), StatsError>
where
    F: Fn(O3RuntimeTraceRecord) -> bool + Copy,
{
    for kind in BranchTargetKind::ALL {
        registry.set_resettable_counter(
            stats[kind.index()],
            snapshot.count(|event| event.branch_kind() == kind && matches(event)),
        )?;
    }
    Ok(())
}

fn set_lsq_latency_summary(
    registry: &mut StatsRegistry,
    stats: RiscvO3RuntimeLsqLatencyStats,
    summary: LsqLatencySummary,
) -> Result<(), StatsError> {
    set_o3_lsq_latency_counters(
        registry,
        stats,
        summary.samples,
        summary.ticks,
        summary.max_ticks,
        summary.min_ticks(),
        summary.avg_ticks(),
    )
}

fn branch_predicted_target_match(event: O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) == event.branch_resolved_target())
}

fn branch_predicted_target_mismatch(event: O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) != event.branch_resolved_target())
}

fn branch_direction_mismatch(event: O3RuntimeTraceRecord) -> bool {
    event.branch_event() && event.branch_predicted_taken() != event.branch_resolved_taken()
}

fn branch_targetless_mismatch(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event.branch_predicted_target().is_some()
        && event.branch_resolved_target().is_none()
}

fn branch_wrong_target(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event
            .branch_predicted_target()
            .zip(event.branch_resolved_target())
            .is_some_and(|(predicted, resolved)| predicted != resolved)
}
