use std::collections::BTreeMap;

use rem6_cpu::{
    BranchTargetKind, O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeTraceRecord,
};
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::groups::{
    RiscvO3RuntimeBranchEventKindStats, RiscvO3RuntimeBranchRepairStats,
    RiscvO3RuntimeLsqLatencyStats,
};
use super::helpers::{
    register_o3_branch_event_kind_counters, register_o3_counter,
    register_o3_lsq_operation_counters, register_o3_lsq_operation_nested_counters,
    register_o3_lsq_operation_nested_latency_counters, register_o3_lsq_ordering_counters,
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

    fn fu_latency_instructions(&self) -> u64 {
        self.events
            .values()
            .filter(|event| event.fu_latency_cycles() != 0)
            .count() as u64
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
    iew_branch_mispredicts: StatId,
    fu_latency_instructions: StatId,
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
            iew_branch_mispredicts: register_o3_counter(
                registry,
                &prefix,
                "iew.branch_mispredicts",
                "Count",
            )?,
            fu_latency_instructions: register_o3_counter(
                registry,
                &prefix,
                "fu_latency.instructions",
                "Count",
            )?,
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
        for (stat, value) in [
            (self.records, snapshot.records()),
            (self.span_ticks, snapshot.span_ticks()),
            (self.rob_allocations, snapshot.rob_allocations()),
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
            (self.iew_branch_mispredicts, branch_mispredicts),
            (
                self.fu_latency_instructions,
                snapshot.fu_latency_instructions(),
            ),
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
