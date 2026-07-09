use rem6_cpu::{BranchTargetKind, O3RuntimeStats};
use rem6_stats::{StatId, StatsError, StatsRegistry};

use super::helpers::{o3_branch_mispredicts, register_o3_counter};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeFuLatencyClassStats {
    pub(super) instructions: StatId,
    pub(super) latency_cycles: StatId,
    pub(super) latency_max_cycles: StatId,
    pub(super) latency_min_cycles: StatId,
    pub(super) latency_avg_cycles: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeBranchRepairStats {
    pub(super) targetless_mismatch: StatId,
    pub(super) wrong_target: StatId,
    pub(super) direction_only: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeBranchEventKindStats {
    pub(super) kind: StatId,
    pub(super) taken: StatId,
    pub(super) not_taken: StatId,
    pub(super) predicted_taken: StatId,
    pub(super) predicted_not_taken: StatId,
    pub(super) predicted_target: StatId,
    pub(super) predicted_target_match: StatId,
    pub(super) predicted_target_mismatch: StatId,
    pub(super) resolved_target: StatId,
    pub(super) misprediction: StatId,
    pub(super) link_write: StatId,
    pub(super) without_link_write: StatId,
    pub(super) squash: StatId,
    pub(super) squashed_target: StatId,
    pub(super) squashed_target_link_write: StatId,
    pub(super) squashed_target_without_link_write: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeBranchDirectionMismatchStats {
    mismatches: StatId,
    without_link_writes: StatId,
    squashed_targets: StatId,
    squashed_target_without_link_writes: StatId,
    squashed_target_link_writes: StatId,
    kinds: [StatId; BranchTargetKind::COUNT],
    link_write_kinds: [StatId; BranchTargetKind::COUNT],
    without_link_write_kinds: [StatId; BranchTargetKind::COUNT],
    squashed_target_kinds: [StatId; BranchTargetKind::COUNT],
    squashed_target_link_write_kinds: [StatId; BranchTargetKind::COUNT],
    squashed_target_without_link_write_kinds: [StatId; BranchTargetKind::COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeBranchTargetMismatchStats {
    targetless_mismatches: StatId,
    targetless_without_link_writes: StatId,
    targetless_squashed_targets: StatId,
    targetless_squashed_target_without_link_writes: StatId,
    wrong_targets: StatId,
    wrong_target_squashed_targets: StatId,
    wrong_target_squashed_target_without_link_writes: StatId,
    wrong_target_squashed_target_link_writes: StatId,
    wrong_target_link_writes: StatId,
    wrong_target_without_link_writes: StatId,
    targetless_kinds: [StatId; BranchTargetKind::COUNT],
    targetless_without_link_write_kinds: [StatId; BranchTargetKind::COUNT],
    targetless_squashed_target_kinds: [StatId; BranchTargetKind::COUNT],
    targetless_squashed_target_without_link_write_kinds: [StatId; BranchTargetKind::COUNT],
    wrong_target_kinds: [StatId; BranchTargetKind::COUNT],
    wrong_target_squashed_target_kinds: [StatId; BranchTargetKind::COUNT],
    wrong_target_squashed_target_without_link_write_kinds: [StatId; BranchTargetKind::COUNT],
    wrong_target_squashed_target_link_write_kinds: [StatId; BranchTargetKind::COUNT],
    wrong_target_link_write_kinds: [StatId; BranchTargetKind::COUNT],
    wrong_target_without_link_write_kinds: [StatId; BranchTargetKind::COUNT],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeLsqLatencyStats {
    pub(super) samples: StatId,
    pub(super) ticks: StatId,
    pub(super) max_ticks: StatId,
    pub(super) min_ticks: StatId,
    pub(super) avg_ticks: StatId,
}

fn register_branch_kind_counters(
    registry: &mut StatsRegistry,
    prefix: &str,
    family: &str,
) -> Result<[StatId; BranchTargetKind::COUNT], StatsError> {
    let mut stats = [StatId::new(0); BranchTargetKind::COUNT];
    for kind in BranchTargetKind::ALL {
        stats[kind.index()] = register_o3_counter(
            registry,
            prefix,
            &format!("{family}.{}", kind.canonical_stat_name()),
            "Count",
        )?;
    }
    Ok(stats)
}

fn increment_delta_counter(
    registry: &mut StatsRegistry,
    stat: StatId,
    previous: u64,
    current: u64,
) -> Result<(), StatsError> {
    let delta = current.saturating_sub(previous);
    if delta != 0 {
        registry.increment(stat, delta)?;
    }
    Ok(())
}

fn increment_branch_kind_delta<F>(
    registry: &mut StatsRegistry,
    stats: [StatId; BranchTargetKind::COUNT],
    previous: O3RuntimeStats,
    current: O3RuntimeStats,
    count: F,
) -> Result<(), StatsError>
where
    F: Fn(O3RuntimeStats, BranchTargetKind) -> u64,
{
    for kind in BranchTargetKind::ALL {
        increment_delta_counter(
            registry,
            stats[kind.index()],
            count(previous, kind),
            count(current, kind),
        )?;
    }
    Ok(())
}

fn set_branch_kind_snapshot<F>(
    registry: &mut StatsRegistry,
    stats: [StatId; BranchTargetKind::COUNT],
    snapshot: O3RuntimeStats,
    count: F,
) -> Result<(), StatsError>
where
    F: Fn(O3RuntimeStats, BranchTargetKind) -> u64,
{
    for kind in BranchTargetKind::ALL {
        registry.set_resettable_counter(stats[kind.index()], count(snapshot, kind))?;
    }
    Ok(())
}

impl RiscvO3RuntimeBranchDirectionMismatchStats {
    pub(super) fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        Ok(Self {
            mismatches: register_o3_counter(
                registry,
                prefix,
                "branch_direction_mismatch.mismatches",
                "Count",
            )?,
            without_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_direction_mismatch.without_link_writes",
                "Count",
            )?,
            squashed_targets: register_o3_counter(
                registry,
                prefix,
                "branch_direction_mismatch.squashed_targets",
                "Count",
            )?,
            squashed_target_without_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_direction_mismatch.squashed_target_without_link_writes",
                "Count",
            )?,
            squashed_target_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_direction_mismatch.squashed_target_link_writes",
                "Count",
            )?,
            kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_direction_mismatch.kind",
            )?,
            link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_direction_mismatch.link_write_kind",
            )?,
            without_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_direction_mismatch.without_link_write_kind",
            )?,
            squashed_target_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_direction_mismatch.squashed_target_kind",
            )?,
            squashed_target_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_direction_mismatch.squashed_target_link_write_kind",
            )?,
            squashed_target_without_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_direction_mismatch.squashed_target_without_link_write_kind",
            )?,
        })
    }

    pub(super) fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, previous, current) in [
            (
                self.mismatches,
                previous.branch_direction_mismatches(),
                current.branch_direction_mismatches(),
            ),
            (
                self.without_link_writes,
                previous.branch_direction_mismatch_without_link_writes(),
                current.branch_direction_mismatch_without_link_writes(),
            ),
            (
                self.squashed_targets,
                previous.branch_direction_mismatch_squashed_targets(),
                current.branch_direction_mismatch_squashed_targets(),
            ),
            (
                self.squashed_target_without_link_writes,
                previous.branch_direction_mismatch_squashed_target_without_link_writes(),
                current.branch_direction_mismatch_squashed_target_without_link_writes(),
            ),
            (
                self.squashed_target_link_writes,
                previous.branch_direction_mismatch_squashed_target_link_writes(),
                current.branch_direction_mismatch_squashed_target_link_writes(),
            ),
        ] {
            increment_delta_counter(registry, stat, previous, current)?;
        }
        increment_branch_kind_delta(
            registry,
            self.kinds,
            previous,
            current,
            O3RuntimeStats::branch_direction_mismatch_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_direction_mismatch_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.without_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_direction_mismatch_without_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.squashed_target_kinds,
            previous,
            current,
            O3RuntimeStats::branch_direction_mismatch_squashed_target_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.squashed_target_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_direction_mismatch_squashed_target_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.squashed_target_without_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_direction_mismatch_squashed_target_without_link_write_kind,
        )?;
        Ok(())
    }

    pub(super) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, value) in [
            (self.mismatches, snapshot.branch_direction_mismatches()),
            (
                self.without_link_writes,
                snapshot.branch_direction_mismatch_without_link_writes(),
            ),
            (
                self.squashed_targets,
                snapshot.branch_direction_mismatch_squashed_targets(),
            ),
            (
                self.squashed_target_without_link_writes,
                snapshot.branch_direction_mismatch_squashed_target_without_link_writes(),
            ),
            (
                self.squashed_target_link_writes,
                snapshot.branch_direction_mismatch_squashed_target_link_writes(),
            ),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        set_branch_kind_snapshot(
            registry,
            self.kinds,
            snapshot,
            O3RuntimeStats::branch_direction_mismatch_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_direction_mismatch_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.without_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_direction_mismatch_without_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.squashed_target_kinds,
            snapshot,
            O3RuntimeStats::branch_direction_mismatch_squashed_target_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.squashed_target_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_direction_mismatch_squashed_target_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.squashed_target_without_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_direction_mismatch_squashed_target_without_link_write_kind,
        )?;
        Ok(())
    }
}

impl RiscvO3RuntimeBranchTargetMismatchStats {
    pub(super) fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        Ok(Self {
            targetless_mismatches: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatches",
                "Count",
            )?,
            targetless_without_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatch_without_link_writes",
                "Count",
            )?,
            targetless_squashed_targets: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatch_squashed_targets",
                "Count",
            )?,
            targetless_squashed_target_without_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatch_squashed_target_without_link_writes",
                "Count",
            )?,
            wrong_targets: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.wrong_targets",
                "Count",
            )?,
            wrong_target_squashed_targets: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_squashed_targets",
                "Count",
            )?,
            wrong_target_squashed_target_without_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_squashed_target_without_link_writes",
                "Count",
            )?,
            wrong_target_squashed_target_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_squashed_target_link_writes",
                "Count",
            )?,
            wrong_target_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_link_writes",
                "Count",
            )?,
            wrong_target_without_link_writes: register_o3_counter(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_without_link_writes",
                "Count",
            )?,
            targetless_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatch_kind",
            )?,
            targetless_without_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatch_without_link_write_kind",
            )?,
            targetless_squashed_target_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatch_squashed_target_kind",
            )?,
            targetless_squashed_target_without_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind",
            )?,
            wrong_target_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_kind",
            )?,
            wrong_target_squashed_target_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_squashed_target_kind",
            )?,
            wrong_target_squashed_target_without_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_squashed_target_without_link_write_kind",
            )?,
            wrong_target_squashed_target_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_squashed_target_link_write_kind",
            )?,
            wrong_target_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_link_write_kind",
            )?,
            wrong_target_without_link_write_kinds: register_branch_kind_counters(
                registry,
                prefix,
                "branch_target_mismatch.wrong_target_without_link_write_kind",
            )?,
        })
    }

    pub(super) fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, previous, current) in [
            (
                self.targetless_mismatches,
                previous.branch_target_mismatch_targetless_mismatches(),
                current.branch_target_mismatch_targetless_mismatches(),
            ),
            (
                self.targetless_without_link_writes,
                previous.branch_target_mismatch_targetless_without_link_writes(),
                current.branch_target_mismatch_targetless_without_link_writes(),
            ),
            (
                self.targetless_squashed_targets,
                previous.branch_target_mismatch_targetless_squashed_targets(),
                current.branch_target_mismatch_targetless_squashed_targets(),
            ),
            (
                self.targetless_squashed_target_without_link_writes,
                previous.branch_target_mismatch_targetless_squashed_target_without_link_writes(),
                current.branch_target_mismatch_targetless_squashed_target_without_link_writes(),
            ),
            (
                self.wrong_targets,
                previous.branch_target_mismatch_wrong_targets(),
                current.branch_target_mismatch_wrong_targets(),
            ),
            (
                self.wrong_target_squashed_targets,
                previous.branch_target_mismatch_wrong_target_squashed_targets(),
                current.branch_target_mismatch_wrong_target_squashed_targets(),
            ),
            (
                self.wrong_target_squashed_target_without_link_writes,
                previous.branch_target_mismatch_wrong_target_squashed_target_without_link_writes(),
                current.branch_target_mismatch_wrong_target_squashed_target_without_link_writes(),
            ),
            (
                self.wrong_target_squashed_target_link_writes,
                previous.branch_target_mismatch_wrong_target_squashed_target_link_writes(),
                current.branch_target_mismatch_wrong_target_squashed_target_link_writes(),
            ),
            (
                self.wrong_target_link_writes,
                previous.branch_target_mismatch_wrong_target_link_writes(),
                current.branch_target_mismatch_wrong_target_link_writes(),
            ),
            (
                self.wrong_target_without_link_writes,
                previous.branch_target_mismatch_wrong_target_without_link_writes(),
                current.branch_target_mismatch_wrong_target_without_link_writes(),
            ),
        ] {
            increment_delta_counter(registry, stat, previous, current)?;
        }
        increment_branch_kind_delta(
            registry,
            self.targetless_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_targetless_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.targetless_without_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_targetless_without_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.targetless_squashed_target_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_targetless_squashed_target_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.targetless_squashed_target_without_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_targetless_squashed_target_without_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.wrong_target_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_wrong_target_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.wrong_target_squashed_target_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.wrong_target_squashed_target_without_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_without_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.wrong_target_squashed_target_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.wrong_target_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_wrong_target_link_write_kind,
        )?;
        increment_branch_kind_delta(
            registry,
            self.wrong_target_without_link_write_kinds,
            previous,
            current,
            O3RuntimeStats::branch_target_mismatch_wrong_target_without_link_write_kind,
        )?;
        Ok(())
    }

    pub(super) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, value) in [
            (
                self.targetless_mismatches,
                snapshot.branch_target_mismatch_targetless_mismatches(),
            ),
            (
                self.targetless_without_link_writes,
                snapshot.branch_target_mismatch_targetless_without_link_writes(),
            ),
            (
                self.targetless_squashed_targets,
                snapshot.branch_target_mismatch_targetless_squashed_targets(),
            ),
            (
                self.targetless_squashed_target_without_link_writes,
                snapshot.branch_target_mismatch_targetless_squashed_target_without_link_writes(),
            ),
            (
                self.wrong_targets,
                snapshot.branch_target_mismatch_wrong_targets(),
            ),
            (
                self.wrong_target_squashed_targets,
                snapshot.branch_target_mismatch_wrong_target_squashed_targets(),
            ),
            (
                self.wrong_target_squashed_target_without_link_writes,
                snapshot.branch_target_mismatch_wrong_target_squashed_target_without_link_writes(),
            ),
            (
                self.wrong_target_squashed_target_link_writes,
                snapshot.branch_target_mismatch_wrong_target_squashed_target_link_writes(),
            ),
            (
                self.wrong_target_link_writes,
                snapshot.branch_target_mismatch_wrong_target_link_writes(),
            ),
            (
                self.wrong_target_without_link_writes,
                snapshot.branch_target_mismatch_wrong_target_without_link_writes(),
            ),
        ] {
            registry.set_resettable_counter(stat, value)?;
        }
        set_branch_kind_snapshot(
            registry,
            self.targetless_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_targetless_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.targetless_without_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_targetless_without_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.targetless_squashed_target_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_targetless_squashed_target_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.targetless_squashed_target_without_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_targetless_squashed_target_without_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.wrong_target_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_wrong_target_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.wrong_target_squashed_target_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.wrong_target_squashed_target_without_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_without_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.wrong_target_squashed_target_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_wrong_target_squashed_target_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.wrong_target_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_wrong_target_link_write_kind,
        )?;
        set_branch_kind_snapshot(
            registry,
            self.wrong_target_without_link_write_kinds,
            snapshot,
            O3RuntimeStats::branch_target_mismatch_wrong_target_without_link_write_kind,
        )?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeStructuralAliasStats {
    pub(super) rob_writes: StatId,
    pub(super) rob_reads: StatId,
    pub(super) rob_max_occupancy: StatId,
    pub(super) rename_renamed_insts: StatId,
    pub(super) rename_renamed_operands: StatId,
    pub(super) iew_dispatched_insts: StatId,
    pub(super) iew_disp_load_insts: StatId,
    pub(super) iew_disp_store_insts: StatId,
    pub(super) iew_insts_to_commit_total: StatId,
    pub(super) iew_writeback_count_total: StatId,
    pub(super) iew_producer_inst_total: StatId,
    pub(super) iew_consumer_inst_total: StatId,
    pub(super) lsq_added_loads_and_stores: StatId,
    pub(super) lsq_store_load_forwarding_candidates: StatId,
    pub(super) lsq_store_load_forwarding_matches: StatId,
    pub(super) lsq_store_load_forwarding_suppressed: StatId,
    pub(super) lsq_store_load_forwarding_address_mismatches: StatId,
    pub(super) lsq_store_load_forwarding_byte_mismatches: StatId,
    pub(super) lsq_forw_loads: StatId,
    pub(super) lsq_max_occupancy: StatId,
    pub(super) iq_insts_issued: StatId,
    pub(super) iq_mem_insts_issued: StatId,
    pub(super) iq_issued_inst_type_mem_read: StatId,
    pub(super) iq_issued_inst_type_mem_write: StatId,
    pub(super) commit_committed_inst_type_mem_read: StatId,
    pub(super) commit_committed_inst_type_mem_write: StatId,
    pub(super) lsq_load_bytes: StatId,
    pub(super) lsq_store_bytes: StatId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct RiscvO3RuntimeBranchAliasStats {
    pub(super) branch_repair_targetless_mismatch: StatId,
    pub(super) branch_repair_direction_only: StatId,
    pub(super) branch_repair_wrong_target: StatId,
    pub(super) branch_repair_total: StatId,
    pub(super) iew_predicted_taken_incorrect: StatId,
    pub(super) iew_predicted_not_taken_incorrect: StatId,
    pub(super) iew_branch_mispredicts: StatId,
    pub(super) commit_branch_mispredicts: StatId,
    pub(super) iq_branch_insts_issued: StatId,
}

impl RiscvO3RuntimeBranchAliasStats {
    pub(super) fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        Ok(Self {
            branch_repair_targetless_mismatch: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.targetlessMismatch",
                "Count",
            )?,
            branch_repair_direction_only: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.directionOnly",
                "Count",
            )?,
            branch_repair_wrong_target: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.wrongTarget",
                "Count",
            )?,
            branch_repair_total: register_o3_counter(
                registry,
                prefix,
                "iew.branchRepair.total",
                "Count",
            )?,
            iew_predicted_taken_incorrect: register_o3_counter(
                registry,
                prefix,
                "iew.predictedTakenIncorrect",
                "Count",
            )?,
            iew_predicted_not_taken_incorrect: register_o3_counter(
                registry,
                prefix,
                "iew.predictedNotTakenIncorrect",
                "Count",
            )?,
            iew_branch_mispredicts: register_o3_counter(
                registry,
                prefix,
                "iew.branchMispredicts",
                "Count",
            )?,
            commit_branch_mispredicts: register_o3_counter(
                registry,
                prefix,
                "commit.branchMispredicts",
                "Count",
            )?,
            iq_branch_insts_issued: register_o3_counter(
                registry,
                prefix,
                "iq.branchInstsIssued",
                "Count",
            )?,
        })
    }

    pub(super) fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for ((stat, previous), (_, current)) in self
            .count_values(previous)
            .into_iter()
            .zip(self.count_values(current))
        {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        Ok(())
    }

    pub(super) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, value) in self.count_values(snapshot) {
            registry.set_resettable_counter(stat, value)?;
        }
        Ok(())
    }

    fn count_values(self, stats: O3RuntimeStats) -> [(StatId, u64); 9] {
        let branch_mispredicts = o3_branch_mispredicts(stats);
        [
            (
                self.branch_repair_targetless_mismatch,
                stats.branch_repair_targetless_mismatches(),
            ),
            (
                self.branch_repair_direction_only,
                stats.branch_repair_direction_only_mismatches(),
            ),
            (
                self.branch_repair_wrong_target,
                stats.branch_repair_wrong_targets(),
            ),
            (self.branch_repair_total, branch_mispredicts),
            (
                self.iew_predicted_taken_incorrect,
                stats.iew_predicted_taken_incorrect(),
            ),
            (
                self.iew_predicted_not_taken_incorrect,
                stats.iew_predicted_not_taken_incorrect(),
            ),
            (self.iew_branch_mispredicts, branch_mispredicts),
            (self.commit_branch_mispredicts, branch_mispredicts),
            (self.iq_branch_insts_issued, stats.iq_branch_insts_issued()),
        ]
    }
}

impl RiscvO3RuntimeStructuralAliasStats {
    pub(super) fn register(registry: &mut StatsRegistry, prefix: &str) -> Result<Self, StatsError> {
        Ok(Self {
            rob_writes: register_o3_counter(registry, prefix, "rob.writes", "Count")?,
            rob_reads: register_o3_counter(registry, prefix, "rob.reads", "Count")?,
            rob_max_occupancy: register_o3_counter(registry, prefix, "rob.maxOccupancy", "Count")?,
            rename_renamed_insts: register_o3_counter(
                registry,
                prefix,
                "rename.renamedInsts",
                "Count",
            )?,
            rename_renamed_operands: register_o3_counter(
                registry,
                prefix,
                "rename.renamedOperands",
                "Count",
            )?,
            iew_dispatched_insts: register_o3_counter(
                registry,
                prefix,
                "iew.dispatchedInsts",
                "Count",
            )?,
            iew_disp_load_insts: register_o3_counter(
                registry,
                prefix,
                "iew.dispLoadInsts",
                "Count",
            )?,
            iew_disp_store_insts: register_o3_counter(
                registry,
                prefix,
                "iew.dispStoreInsts",
                "Count",
            )?,
            iew_insts_to_commit_total: register_o3_counter(
                registry,
                prefix,
                "iew.instsToCommit.total",
                "Count",
            )?,
            iew_writeback_count_total: register_o3_counter(
                registry,
                prefix,
                "iew.writebackCount.total",
                "Count",
            )?,
            iew_producer_inst_total: register_o3_counter(
                registry,
                prefix,
                "iew.producerInst.total",
                "Count",
            )?,
            iew_consumer_inst_total: register_o3_counter(
                registry,
                prefix,
                "iew.consumerInst.total",
                "Count",
            )?,
            lsq_added_loads_and_stores: register_o3_counter(
                registry,
                prefix,
                "lsq0.addedLoadsAndStores",
                "Count",
            )?,
            lsq_store_load_forwarding_candidates: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingCandidates",
                "Count",
            )?,
            lsq_store_load_forwarding_matches: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingMatches",
                "Count",
            )?,
            lsq_store_load_forwarding_suppressed: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingSuppressed",
                "Count",
            )?,
            lsq_store_load_forwarding_address_mismatches: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingAddressMismatches",
                "Count",
            )?,
            lsq_store_load_forwarding_byte_mismatches: register_o3_counter(
                registry,
                prefix,
                "lsq0.storeLoadForwardingByteMismatches",
                "Count",
            )?,
            lsq_forw_loads: register_o3_counter(registry, prefix, "lsq0.forwLoads", "Count")?,
            lsq_max_occupancy: register_o3_counter(registry, prefix, "lsq0.maxOccupancy", "Count")?,
            iq_insts_issued: register_o3_counter(registry, prefix, "iq.instsIssued", "Count")?,
            iq_mem_insts_issued: register_o3_counter(
                registry,
                prefix,
                "iq.memInstsIssued",
                "Count",
            )?,
            iq_issued_inst_type_mem_read: register_o3_counter(
                registry,
                prefix,
                "iq.issuedInstType.MemRead",
                "Count",
            )?,
            iq_issued_inst_type_mem_write: register_o3_counter(
                registry,
                prefix,
                "iq.issuedInstType.MemWrite",
                "Count",
            )?,
            commit_committed_inst_type_mem_read: register_o3_counter(
                registry,
                prefix,
                "commit.committedInstType.MemRead",
                "Count",
            )?,
            commit_committed_inst_type_mem_write: register_o3_counter(
                registry,
                prefix,
                "commit.committedInstType.MemWrite",
                "Count",
            )?,
            lsq_load_bytes: register_o3_counter(registry, prefix, "lsq0.loadBytes", "Byte")?,
            lsq_store_bytes: register_o3_counter(registry, prefix, "lsq0.storeBytes", "Byte")?,
        })
    }

    pub(super) fn increment_delta(
        self,
        registry: &mut StatsRegistry,
        previous: O3RuntimeStats,
        current: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for ((stat, previous), (_, current)) in self
            .count_values(previous)
            .into_iter()
            .zip(self.count_values(current))
        {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        for ((stat, previous), (_, current)) in self
            .byte_values(previous)
            .into_iter()
            .zip(self.byte_values(current))
        {
            let delta = current.saturating_sub(previous);
            if delta != 0 {
                registry.increment(stat, delta)?;
            }
        }
        Ok(())
    }

    pub(super) fn set_snapshot(
        self,
        registry: &mut StatsRegistry,
        snapshot: O3RuntimeStats,
    ) -> Result<(), StatsError> {
        for (stat, value) in self.count_values(snapshot) {
            registry.set_resettable_counter(stat, value)?;
        }
        for (stat, value) in self.byte_values(snapshot) {
            registry.set_resettable_counter(stat, value)?;
        }
        Ok(())
    }

    fn count_values(self, stats: O3RuntimeStats) -> [(StatId, u64); 26] {
        [
            (self.rob_writes, stats.rob_allocations()),
            (self.rob_reads, stats.rob_commits()),
            (self.rob_max_occupancy, stats.max_rob_occupancy()),
            (self.rename_renamed_insts, stats.instructions()),
            (self.rename_renamed_operands, stats.rename_writes()),
            (self.iew_dispatched_insts, stats.instructions()),
            (self.iew_disp_load_insts, stats.lsq_loads()),
            (self.iew_disp_store_insts, stats.lsq_stores()),
            (self.iew_insts_to_commit_total, stats.rob_commits()),
            (self.iew_writeback_count_total, stats.instructions()),
            (self.iew_producer_inst_total, stats.iew_producer_insts()),
            (self.iew_consumer_inst_total, stats.iew_consumer_insts()),
            (
                self.lsq_added_loads_and_stores,
                stats.lsq_loads().saturating_add(stats.lsq_stores()),
            ),
            (
                self.lsq_store_load_forwarding_candidates,
                stats.lsq_store_to_load_forwarding_candidates(),
            ),
            (
                self.lsq_store_load_forwarding_matches,
                stats.lsq_store_to_load_forwarding_matches(),
            ),
            (
                self.lsq_store_load_forwarding_suppressed,
                stats.lsq_store_to_load_forwarding_suppressed(),
            ),
            (
                self.lsq_store_load_forwarding_address_mismatches,
                stats.lsq_store_to_load_forwarding_address_mismatches(),
            ),
            (
                self.lsq_store_load_forwarding_byte_mismatches,
                stats.lsq_store_to_load_forwarding_byte_mismatches(),
            ),
            (
                self.lsq_forw_loads,
                stats.lsq_store_to_load_forwarding_matches(),
            ),
            (self.lsq_max_occupancy, stats.max_lsq_occupancy()),
            (self.iq_insts_issued, stats.instructions()),
            (
                self.iq_mem_insts_issued,
                stats.lsq_loads().saturating_add(stats.lsq_stores()),
            ),
            (self.iq_issued_inst_type_mem_read, stats.lsq_loads()),
            (self.iq_issued_inst_type_mem_write, stats.lsq_stores()),
            (self.commit_committed_inst_type_mem_read, stats.lsq_loads()),
            (
                self.commit_committed_inst_type_mem_write,
                stats.lsq_stores(),
            ),
        ]
    }

    fn byte_values(self, stats: O3RuntimeStats) -> [(StatId, u64); 2] {
        [
            (self.lsq_load_bytes, stats.lsq_load_bytes()),
            (self.lsq_store_bytes, stats.lsq_store_bytes()),
        ]
    }
}
