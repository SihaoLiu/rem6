use rem6_cpu::{BranchTargetKind, BranchTargetProvider, O3_RUNTIME_INST_TYPE_DESCRIPTORS};
use rem6_stats::{StatResetPolicy, StatSnapshot};

use super::{json_record_for_derived_counter, snapshot_sample, snapshot_sample_value};
use crate::o3_branch_mismatch_aliases::{
    O3_BRANCH_MISMATCH_KIND_ALIASES, O3_BRANCH_MISMATCH_SCALAR_ALIASES,
};
use crate::o3_iew_aliases::{
    O3IewGem5Alias, O3_IEW_GEM5_PHASE_ALIASES, O3_IEW_GEM5_RATE_ALIASES, O3_IEW_GEM5_TOTAL_ALIASES,
};
use crate::o3_lsq_aliases::{
    O3_LSQ_OPERATION_GEM5_ALIASES, O3_LSQ_ORDERING_GEM5_ALIASES, O3_LSQ_TOTAL_ALIAS,
};

pub(super) fn append_gem5_json_alias_stats(snapshot: &StatSnapshot, records: &mut Vec<String>) {
    let mut next_id = snapshot
        .samples()
        .iter()
        .map(|sample| sample.id().get())
        .max()
        .unwrap_or(0)
        .saturating_add(1);

    append_gem5_o3_json_alias_stats(snapshot, records, &mut next_id);
    append_gem5_in_order_pipeline_json_alias_stats(snapshot, records, &mut next_id);
    append_gem5_branch_predictor_json_alias_stats(snapshot, records, &mut next_id);
}

fn append_gem5_branch_predictor_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
) {
    let Some(core_count) = snapshot_sample_value(snapshot, "sim.cores") else {
        return;
    };

    for cpu in 0..core_count {
        let alias_prefix = gem5_json_cpu_alias_prefix(core_count, cpu);
        for (source_path, alias_path) in [
            (
                format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_predictions"),
                format!("{alias_prefix}.branchPred.condPredicted"),
            ),
            (
                format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_predicted_taken"),
                format!("{alias_prefix}.branchPred.condPredictedTaken"),
            ),
            (
                format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_mispredictions"),
                format!("{alias_prefix}.branchPred.condIncorrect"),
            ),
        ] {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &source_path,
                &alias_path,
            );
        }

        for (source_suffix, alias_suffix) in [
            ("lookups", "BTBLookups"),
            ("lookups", "btb.lookups::total"),
            ("hits", "BTBHits"),
            ("misses", "btb.misses::total"),
            ("updates", "BTBUpdates"),
            ("updates", "btb.updates::total"),
            ("evictions", "btb.evictions"),
            ("mispredictions", "BTBMispredicted"),
            ("mispredictions", "btb.mispredict::total"),
            ("predicted_taken_misses", "predTakenBTBMiss"),
        ] {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!("sim.cpu{cpu}.branch_predictor.btb.{source_suffix}"),
                &format!("{alias_prefix}.branchPred.{alias_suffix}"),
            );
        }

        for kind in BranchTargetKind::ALL {
            for (source_family, alias_family) in [
                ("lookups", "lookups"),
                ("misses", "misses"),
                ("updates", "updates"),
            ] {
                append_gem5_json_alias_from_paths(
                    snapshot,
                    records,
                    next_id,
                    &format!(
                        "sim.cpu{cpu}.branch_predictor.btb.{source_family}.{}",
                        kind.canonical_stat_name()
                    ),
                    &format!(
                        "{alias_prefix}.branchPred.btb.{alias_family}::{}",
                        kind.gem5_branch_type_name()
                    ),
                );
            }
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.btb.mispredict_due_to_btb_miss.{}",
                    kind.canonical_stat_name()
                ),
                &format!(
                    "{alias_prefix}.branchPred.mispredictDueToBTBMiss_0::{}",
                    kind.gem5_branch_type_name()
                ),
            );
        }
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("sim.cpu{cpu}.branch_predictor.btb.mispredict_due_to_btb_miss.total"),
            &format!("{alias_prefix}.branchPred.mispredictDueToBTBMiss_0::total"),
        );

        for kind in BranchTargetKind::ALL {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.squashes.{}",
                    kind.canonical_stat_name()
                ),
                &format!(
                    "{alias_prefix}.branchPred.squashes_0::{}",
                    kind.gem5_branch_type_name()
                ),
            );
        }
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("sim.cpu{cpu}.branch_predictor.squashes.total"),
            &format!("{alias_prefix}.branchPred.squashes_0::total"),
        );

        if let Some((indirect_lookups, unit, reset_policy)) =
            gem5_indirect_branch_lookups(snapshot, cpu)
        {
            append_gem5_json_alias_from_value(
                snapshot,
                records,
                next_id,
                &format!("{alias_prefix}.branchPred.indirectLookups"),
                unit,
                indirect_lookups,
                reset_policy,
            );
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_hits"),
                &format!("{alias_prefix}.branchPred.indirectHits"),
            );
            if let Some(indirect_hits) = snapshot_sample_value(
                snapshot,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_hits"),
            ) {
                if let Some(indirect_misses) = indirect_lookups.checked_sub(indirect_hits) {
                    append_gem5_json_alias_from_value(
                        snapshot,
                        records,
                        next_id,
                        &format!("{alias_prefix}.branchPred.indirectMisses"),
                        unit,
                        indirect_misses,
                        reset_policy,
                    );
                }
            }
        }

        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("sim.cpu{cpu}.branch_predictor.indirect_mispredicted"),
            &format!("{alias_prefix}.branchPred.indirectMispredicted"),
        );
        for name in ["pushes", "pops", "squashes", "used", "correct", "incorrect"] {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!("sim.cpu{cpu}.branch_predictor.ras.{name}"),
                &format!("{alias_prefix}.branchPred.ras.{name}"),
            );
        }
        for kind in BranchTargetKind::ALL {
            for (source_family, alias_family) in [
                ("lookups", "lookups_0"),
                ("committed", "committed_0"),
                ("mispredicted", "mispredicted_0"),
                ("corrected", "corrected_0"),
                ("target_wrong", "targetWrong_0"),
                ("mispredict_due_to_predictor", "mispredictDueToPredictor_0"),
            ] {
                append_gem5_json_alias_from_paths(
                    snapshot,
                    records,
                    next_id,
                    &format!(
                        "sim.cpu{cpu}.branch_predictor.{source_family}.{}",
                        kind.canonical_stat_name()
                    ),
                    &format!(
                        "{alias_prefix}.branchPred.{alias_family}::{}",
                        kind.gem5_branch_type_name()
                    ),
                );
            }
        }
        for (source_family, alias_family) in [
            ("lookups", "lookups_0"),
            ("committed", "committed_0"),
            ("mispredicted", "mispredicted_0"),
            ("corrected", "corrected_0"),
            ("target_wrong", "targetWrong_0"),
            ("mispredict_due_to_predictor", "mispredictDueToPredictor_0"),
        ] {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!("sim.cpu{cpu}.branch_predictor.{source_family}.total"),
                &format!("{alias_prefix}.branchPred.{alias_family}::total"),
            );
        }
        for provider in BranchTargetProvider::ALL {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.target_provider.{}",
                    provider.canonical_stat_name()
                ),
                &format!(
                    "{alias_prefix}.branchPred.targetProvider_0::{}",
                    provider.gem5_target_provider_name()
                ),
            );
        }
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("sim.cpu{cpu}.branch_predictor.target_provider.total"),
            &format!("{alias_prefix}.branchPred.targetProvider_0::total"),
        );
    }
}

fn append_gem5_o3_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
) {
    let Some(core_count) = snapshot_sample_value(snapshot, "sim.cores") else {
        return;
    };

    for cpu in 0..core_count {
        let alias_prefix = gem5_json_cpu_alias_prefix(core_count, cpu);
        append_gem5_o3_op_class_json_alias_stats(
            snapshot,
            records,
            next_id,
            cpu,
            &alias_prefix,
            core_count == 1,
        );
        append_gem5_o3_lsq_count_bucket_json_alias_stats(snapshot, records, next_id, &alias_prefix);
        for aliases in [
            O3_IEW_GEM5_TOTAL_ALIASES,
            O3_IEW_GEM5_RATE_ALIASES,
            O3_IEW_GEM5_PHASE_ALIASES,
        ] {
            append_gem5_o3_iew_json_alias_stats(
                snapshot,
                records,
                next_id,
                cpu,
                &alias_prefix,
                aliases,
            );
        }
        append_gem5_o3_branch_repair_json_alias_stats(snapshot, records, next_id, &alias_prefix);
        append_gem5_o3_branch_mismatch_json_alias_stats(
            snapshot,
            records,
            next_id,
            cpu,
            &alias_prefix,
        );
        append_gem5_o3_branch_prediction_json_alias_stats(
            snapshot,
            records,
            next_id,
            cpu,
            &alias_prefix,
        );
        append_gem5_o3_ftq_json_alias_stats(snapshot, records, next_id, cpu, &alias_prefix);
    }
}

fn append_gem5_o3_iew_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    alias_prefix: &str,
    aliases: &[O3IewGem5Alias],
) {
    for alias in aliases {
        let source_path = format!("sim.cpu{cpu}.o3.{}", alias.source_suffix());
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &source_path,
            &format!("{alias_prefix}.{}", alias.alias_suffix()),
        );
        if let Some(bucket_alias_suffix) = alias.bucket_alias_suffix() {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &source_path,
                &format!("{alias_prefix}.{bucket_alias_suffix}"),
            );
        }
    }
}

fn append_gem5_o3_lsq_count_bucket_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    alias_prefix: &str,
) {
    for alias in O3_LSQ_OPERATION_GEM5_ALIASES {
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("{alias_prefix}.lsq0.operation.{}", alias.alias()),
            &format!("{alias_prefix}.lsq0.operation_0::{}", alias.bucket_alias()),
        );
    }
    append_gem5_json_alias_from_paths(
        snapshot,
        records,
        next_id,
        &format!("{alias_prefix}.lsq0.operation.{O3_LSQ_TOTAL_ALIAS}"),
        &format!("{alias_prefix}.lsq0.operation_0::{O3_LSQ_TOTAL_ALIAS}"),
    );
    for alias in O3_LSQ_ORDERING_GEM5_ALIASES {
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("{alias_prefix}.lsq0.ordering.{}", alias.alias()),
            &format!("{alias_prefix}.lsq0.ordering_0::{}", alias.bucket_alias()),
        );
    }
    append_gem5_json_alias_from_paths(
        snapshot,
        records,
        next_id,
        &format!("{alias_prefix}.lsq0.ordering.{O3_LSQ_TOTAL_ALIAS}"),
        &format!("{alias_prefix}.lsq0.ordering_0::{O3_LSQ_TOTAL_ALIAS}"),
    );
}

fn append_gem5_o3_branch_mismatch_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    alias_prefix: &str,
) {
    for alias in O3_BRANCH_MISMATCH_SCALAR_ALIASES {
        let source_path = format!("sim.cpu{cpu}.o3.{}", alias.source_suffix);
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &source_path,
            &format!("{alias_prefix}.iew.{}", alias.alias_suffix),
        );
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &source_path,
            &format!("{alias_prefix}.iew.{}_0::total", alias.bucket_alias),
        );
    }

    for alias in O3_BRANCH_MISMATCH_KIND_ALIASES {
        let source_family = alias.source_family;
        let alias_family = alias.alias_family;
        for kind in BranchTargetKind::ALL {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!(
                    "sim.cpu{cpu}.o3.{source_family}.{}",
                    kind.canonical_stat_name()
                ),
                &format!(
                    "{alias_prefix}.iew.{alias_family}_0::{}",
                    kind.gem5_branch_type_name()
                ),
            );
        }
    }
}

fn append_gem5_o3_branch_prediction_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    alias_prefix: &str,
) {
    for (source_suffix, alias_suffix) in [
        ("predicted_taken", "fetch.predictedBranches"),
        ("mispredictions", "bac.branchMisspredict"),
    ] {
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("sim.cpu{cpu}.o3.branch_event.{source_suffix}"),
            &format!("{alias_prefix}.{alias_suffix}"),
        );
    }
}

fn append_gem5_o3_ftq_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    alias_prefix: &str,
) {
    for (source_family, source_total, alias_family) in [
        ("squash_kind", "squashes", "squashes"),
        (
            "squashed_target_kind",
            "squashed_targets",
            "squashedTargets",
        ),
        (
            "squashed_target_link_write_kind",
            "squashed_targets_with_link_writes",
            "squashedTargetsWithLinkWrites",
        ),
        (
            "squashed_target_without_link_write_kind",
            "squashed_targets_without_link_writes",
            "squashedTargetsWithoutLinkWrites",
        ),
    ] {
        for kind in BranchTargetKind::ALL {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!(
                    "sim.cpu{cpu}.o3.branch_event.{source_family}.{}",
                    kind.canonical_stat_name()
                ),
                &format!(
                    "{alias_prefix}.ftq.{alias_family}_0::{}",
                    kind.gem5_branch_type_name()
                ),
            );
        }
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("sim.cpu{cpu}.o3.branch_event.{source_total}"),
            &format!("{alias_prefix}.ftq.{alias_family}_0::total"),
        );
    }
}

fn append_gem5_o3_branch_repair_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    alias_prefix: &str,
) {
    for (source_suffix, alias_suffix) in [
        (
            "iew.branchRepair.targetlessMismatch",
            "iew.branchRepair_0::TargetlessMismatch",
        ),
        (
            "iew.branchRepair.directionOnly",
            "iew.branchRepair_0::DirectionOnly",
        ),
        (
            "iew.branchRepair.wrongTarget",
            "iew.branchRepair_0::WrongTarget",
        ),
        ("iew.branchRepair.total", "iew.branchRepair_0::total"),
    ] {
        append_gem5_json_alias_from_paths(
            snapshot,
            records,
            next_id,
            &format!("{alias_prefix}.{source_suffix}"),
            &format!("{alias_prefix}.{alias_suffix}"),
        );
    }
}

fn append_gem5_in_order_pipeline_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
) {
    let Some(core_count) = snapshot_sample_value(snapshot, "sim.cores") else {
        return;
    };
    for cpu in 0..core_count {
        let pipeline_alias_prefix = format!(
            "{}.pipeline.inOrder",
            gem5_json_cpu_alias_prefix(core_count, cpu)
        );
        for (source_cause, alias_cause) in [
            ("fetch_wait", "fetchWait"),
            ("data_wait", "dataWait"),
            ("execute_wait", "executeWait"),
        ] {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!("sim.cpu{cpu}.pipeline.in_order.stall_cause.{source_cause}.records"),
                &format!("{pipeline_alias_prefix}.stallCause.{alias_cause}.records"),
            );
            for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
                for (source_name, alias_name) in [
                    ("records", "records"),
                    ("resource_blocked", "resourceBlocked"),
                    ("resource_blocked_cycles", "resourceBlockedCycles"),
                    ("ordering_blocked", "orderingBlocked"),
                    ("ordering_blocked_cycles", "orderingBlockedCycles"),
                ] {
                    append_gem5_json_alias_from_paths(
                        snapshot,
                        records,
                        next_id,
                        &format!(
                            "sim.cpu{cpu}.pipeline.in_order.stall_cause.{source_cause}.stage.{stage}.{source_name}"
                        ),
                        &format!(
                            "{pipeline_alias_prefix}.stallCause.{alias_cause}.stage.{stage}.{alias_name}"
                        ),
                    );
                }
            }
        }
        for (source_family, alias_family) in [
            ("flush_cause", "flushCause"),
            ("redirect_cause", "redirectCause"),
        ] {
            for (source_cause, alias_cause) in [
                ("branch_prediction", "branchPrediction"),
                ("interrupt_redirect", "interruptRedirect"),
                ("trap_redirect", "trapRedirect"),
            ] {
                for (source_name, alias_name) in [
                    ("records", "records"),
                    ("flushed", "flushed"),
                    ("flushed_cycles", "flushedCycles"),
                ] {
                    append_gem5_json_alias_from_paths(
                        snapshot,
                        records,
                        next_id,
                        &format!(
                            "sim.cpu{cpu}.pipeline.in_order.{source_family}.{source_cause}.{source_name}"
                        ),
                        &format!(
                            "{pipeline_alias_prefix}.{alias_family}.{alias_cause}.{alias_name}"
                        ),
                    );
                }
                for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
                    for (source_name, alias_name) in [
                        ("records", "records"),
                        ("flushed", "flushed"),
                        ("flushed_cycles", "flushedCycles"),
                    ] {
                        append_gem5_json_alias_from_paths(
                            snapshot,
                            records,
                            next_id,
                            &format!(
                                "sim.cpu{cpu}.pipeline.in_order.{source_family}.{source_cause}.stage.{stage}.{source_name}"
                            ),
                            &format!(
                                "{pipeline_alias_prefix}.{alias_family}.{alias_cause}.stage.{stage}.{alias_name}"
                            ),
                        );
                    }
                }
            }
        }
    }
    for cpu in 0..core_count {
        let pipeline_alias_prefix = format!(
            "{}.pipeline.inOrder",
            gem5_json_cpu_alias_prefix(core_count, cpu)
        );
        for (source_name, alias_name) in [
            ("advanced", "advanced"),
            ("flushed", "flushed"),
            ("flush_cycles", "flushCycles"),
            ("resource_blocked", "resourceBlocked"),
            ("ordering_blocked", "orderingBlocked"),
            ("stall_cycles", "stallCycles"),
            ("fetch_wait_cycles", "fetchWaitCycles"),
            ("data_wait_cycles", "dataWaitCycles"),
            ("execute_wait_cycles", "executeWaitCycles"),
            ("branch_prediction_flushes", "branchPredictionFlushes"),
            (
                "branch_prediction_flush_cycles",
                "branchPredictionFlushCycles",
            ),
            ("redirects", "redirects"),
            ("branch_prediction_redirects", "branchPredictionRedirects"),
            ("interrupt_redirects", "interruptRedirects"),
            ("interrupt_redirect_flushes", "interruptRedirectFlushes"),
            (
                "interrupt_redirect_flush_cycles",
                "interruptRedirectFlushCycles",
            ),
            ("trap_redirects", "trapRedirects"),
            ("trap_redirect_flushes", "trapRedirectFlushes"),
            ("trap_redirect_flush_cycles", "trapRedirectFlushCycles"),
            (
                "branch_speculation_predictions",
                "branchSpeculationPredictions",
            ),
            ("branch_speculation_repairs", "branchSpeculationRepairs"),
            (
                "branch_speculation_removed_youngers",
                "branchSpeculationRemovedYoungers",
            ),
            (
                "branch_speculation_max_pending",
                "branchSpeculationMaxPending",
            ),
        ] {
            append_gem5_json_alias_from_paths(
                snapshot,
                records,
                next_id,
                &format!("sim.cpu{cpu}.pipeline.in_order.{source_name}"),
                &format!("{pipeline_alias_prefix}.{alias_name}"),
            );
        }
    }
    for cpu in 0..core_count {
        let pipeline_alias_prefix = format!(
            "{}.pipeline.inOrder",
            gem5_json_cpu_alias_prefix(core_count, cpu)
        );
        for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
            for (source_name, alias_name) in [
                ("width", "width"),
                ("in_flight", "inFlight"),
                ("max_in_flight", "maxInFlight"),
                ("occupied_cycles", "occupiedCycles"),
                ("advanced", "advanced"),
                ("advanced_cycles", "advancedCycles"),
                ("retired", "retired"),
                ("retired_cycles", "retiredCycles"),
                ("resource_blocked", "resourceBlocked"),
                ("resource_blocked_cycles", "resourceBlockedCycles"),
                ("ordering_blocked", "orderingBlocked"),
                ("ordering_blocked_cycles", "orderingBlockedCycles"),
                ("flushed", "flushed"),
                ("flushed_cycles", "flushedCycles"),
                ("branch_prediction_flushed", "branchPredictionFlushed"),
                (
                    "branch_prediction_flushed_cycles",
                    "branchPredictionFlushedCycles",
                ),
                ("interrupt_redirect_flushed", "interruptRedirectFlushed"),
                (
                    "interrupt_redirect_flushed_cycles",
                    "interruptRedirectFlushedCycles",
                ),
                ("trap_redirect_flushed", "trapRedirectFlushed"),
                ("trap_redirect_flushed_cycles", "trapRedirectFlushedCycles"),
            ] {
                append_gem5_json_alias_from_paths(
                    snapshot,
                    records,
                    next_id,
                    &format!("sim.cpu{cpu}.pipeline.in_order.stage.{stage}.{source_name}"),
                    &format!("{pipeline_alias_prefix}.stage.{stage}.{alias_name}"),
                );
            }
        }
    }
}

fn append_gem5_o3_op_class_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    alias_prefix: &str,
    include_zero_extended_aliases: bool,
) {
    append_gem5_o3_memory_inst_type_json_alias_stats(
        snapshot,
        records,
        next_id,
        cpu,
        alias_prefix,
        "iq.issued_inst_type",
        "iq.issuedInstType",
    );
    append_gem5_o3_descriptor_inst_type_json_alias_stats(
        snapshot,
        records,
        next_id,
        cpu,
        alias_prefix,
        "iq.issued_inst_type",
        "iq.issuedInstType",
        false,
        include_zero_extended_aliases,
    );
    append_gem5_o3_memory_inst_type_json_alias_stats(
        snapshot,
        records,
        next_id,
        cpu,
        alias_prefix,
        "commit.committed_inst_type",
        "commit.committedInstType",
    );
    append_gem5_o3_descriptor_inst_type_json_alias_stats(
        snapshot,
        records,
        next_id,
        cpu,
        alias_prefix,
        "commit.committed_inst_type",
        "commit.committedInstType",
        false,
        include_zero_extended_aliases,
    );
    append_gem5_o3_descriptor_inst_type_json_alias_stats(
        snapshot,
        records,
        next_id,
        cpu,
        alias_prefix,
        "iq.issued_inst_type",
        "iq.issuedInstType",
        true,
        include_zero_extended_aliases,
    );
    append_gem5_o3_descriptor_inst_type_json_alias_stats(
        snapshot,
        records,
        next_id,
        cpu,
        alias_prefix,
        "commit.committed_inst_type",
        "commit.committedInstType",
        true,
        include_zero_extended_aliases,
    );
}

fn append_gem5_o3_memory_inst_type_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    alias_prefix: &str,
    source_family: &str,
    alias_family: &str,
) {
    for (source_name, alias_name) in [("mem_read", "MemRead"), ("mem_write", "MemWrite")] {
        append_gem5_o3_json_alias_from_sample_with_policy(
            snapshot,
            records,
            next_id,
            cpu,
            &format!("{source_family}.{source_name}"),
            alias_prefix,
            &format!("{alias_family}.{alias_name}"),
            true,
        );
    }
}

fn append_gem5_o3_descriptor_inst_type_json_alias_stats(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    alias_prefix: &str,
    source_family: &str,
    alias_family: &str,
    zero_extended_alias: bool,
    include_zero_extended_aliases: bool,
) {
    for descriptor in O3_RUNTIME_INST_TYPE_DESCRIPTORS.iter() {
        if descriptor.zero_extended_alias() != zero_extended_alias {
            continue;
        }
        append_gem5_o3_json_alias_from_sample_with_policy(
            snapshot,
            records,
            next_id,
            cpu,
            &format!("{source_family}.{}", descriptor.source_stem()),
            alias_prefix,
            &format!("{alias_family}.{}", descriptor.gem5_alias()),
            !descriptor.zero_extended_alias() || include_zero_extended_aliases,
        );
    }
}

fn append_gem5_o3_json_alias_from_sample_with_policy(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    cpu: u64,
    source_suffix: &str,
    alias_prefix: &str,
    alias_suffix: &str,
    include_zero_values: bool,
) {
    let source_path = format!("sim.cpu{cpu}.o3.{source_suffix}");
    if !include_zero_values
        && snapshot_sample(snapshot, &source_path).is_some_and(|source| source.value() == 0)
    {
        return;
    }
    let alias_path = format!("{alias_prefix}.{alias_suffix}");
    append_gem5_json_alias_from_paths(snapshot, records, next_id, &source_path, &alias_path);
    let Some(bucket_alias_suffix) = gem5_o3_bucket_alias_suffix(alias_suffix) else {
        return;
    };
    let bucket_alias_path = format!("{alias_prefix}.{bucket_alias_suffix}");
    append_gem5_json_alias_from_paths(snapshot, records, next_id, &source_path, &bucket_alias_path);
}

fn gem5_o3_bucket_alias_suffix(alias_suffix: &str) -> Option<String> {
    alias_suffix
        .strip_prefix("iq.issuedInstType.")
        .map(|op_class| format!("iq.issuedInstType_0::{op_class}"))
        .or_else(|| {
            alias_suffix
                .strip_prefix("commit.committedInstType.")
                .map(|op_class| format!("commit.committedInstType_0::{op_class}"))
        })
}

fn append_gem5_json_alias_from_paths(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    source_path: &str,
    alias_path: &str,
) {
    let Some(source) = snapshot_sample(snapshot, source_path) else {
        return;
    };
    if snapshot_sample(snapshot, alias_path).is_none() {
        records.push(json_record_for_derived_counter(
            *next_id,
            alias_path,
            source.unit(),
            source.value(),
            source.reset_policy(),
        ));
        *next_id = next_id.saturating_add(1);
    }
}

fn append_gem5_json_alias_from_value(
    snapshot: &StatSnapshot,
    records: &mut Vec<String>,
    next_id: &mut u64,
    alias_path: &str,
    unit: &str,
    value: u64,
    reset_policy: StatResetPolicy,
) {
    if snapshot_sample(snapshot, alias_path).is_none() {
        records.push(json_record_for_derived_counter(
            *next_id,
            alias_path,
            unit,
            value,
            reset_policy,
        ));
        *next_id = next_id.saturating_add(1);
    }
}

fn gem5_indirect_branch_lookups(
    snapshot: &StatSnapshot,
    cpu: u64,
) -> Option<(u64, &str, StatResetPolicy)> {
    let mut lookups = 0_u64;
    let mut unit = None;
    let mut reset_policy = None;
    for kind in [
        BranchTargetKind::IndirectConditional,
        BranchTargetKind::IndirectUnconditional,
        BranchTargetKind::CallIndirect,
    ] {
        let source_path = format!(
            "sim.cpu{cpu}.branch_predictor.lookups.{}",
            kind.canonical_stat_name()
        );
        let source = snapshot_sample(snapshot, &source_path)?;
        lookups = lookups.saturating_add(source.value());
        unit.get_or_insert(source.unit());
        reset_policy.get_or_insert(source.reset_policy());
    }
    Some((lookups, unit?, reset_policy?))
}

fn gem5_json_cpu_alias_prefix(core_count: u64, cpu: u64) -> String {
    if core_count == 1 {
        "system.cpu".to_string()
    } else {
        format!("system.cpu{cpu}")
    }
}
