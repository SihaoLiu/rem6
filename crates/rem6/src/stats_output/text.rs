use std::collections::BTreeMap;

use rem6_cpu::{BranchTargetKind, BranchTargetProvider, O3RuntimeFuLatencyClass};
use rem6_stats::StatSnapshot;

pub(super) fn stats_snapshot_text(snapshot: &StatSnapshot) -> String {
    let mut output = "---------- Begin Simulation Statistics ----------\n".to_string();
    append_gem5_derived_text_stats(&mut output, snapshot);
    for sample in snapshot.samples() {
        output.push_str(&format!(
            "{:<64} {:>20} # kind={} unit={} reset_policy={}\n",
            sample.path(),
            sample.value(),
            sample.kind(),
            sample.unit(),
            sample.reset_policy()
        ));
        for bucket in sample.histogram_buckets() {
            output.push_str(&format!(
                "{:<64} {:>20} # histogram_bucket={} unit={} reset_policy={}\n",
                format!("{}.bucket", sample.path()),
                bucket.count(),
                bucket.bucket(),
                sample.unit(),
                sample.reset_policy()
            ));
        }
    }
    output.push_str("\n---------- End Simulation Statistics   ----------\n");
    output
}

fn append_gem5_derived_text_stats(output: &mut String, snapshot: &StatSnapshot) {
    if let (Some(final_tick), Some(sim_freq)) = (
        snapshot_value(snapshot, "finalTick"),
        snapshot_value(snapshot, "simFreq"),
    ) {
        if sim_freq != 0 {
            output.push_str(&format!(
                "{:<64} {:>20} # kind=derived unit=Second reset_policy=constant\n",
                "simSeconds",
                format_sim_seconds(final_tick, sim_freq)
            ));
        }
    }
    append_gem5_mem_ctrl_bandwidth_alias_stats(output, snapshot);
    append_gem5_dram_interface_ratio_stats(output, snapshot);
    append_gem5_dram_interface_latency_stats(output, snapshot);
    append_gem5_cpu_ratio_stats(output, snapshot);
    append_gem5_work_item_alias_stats(output, snapshot);
    append_gem5_work_item_duration_alias_stats(output, snapshot);
    append_gem5_in_order_pipeline_alias_stats(output, snapshot);
    append_gem5_branch_prediction_alias_stats(output, snapshot);
    append_gem5_o3_iq_alias_stats(output, snapshot);
    append_gem5_l1_cache_alias_stats(output, snapshot);
    append_gem5_l1_prefetcher_formula_alias_stats(output, snapshot);
    append_gem5_ruby_network_alias_stats(output, snapshot);
    append_gem5_shared_cache_alias_stats(
        output,
        snapshot,
        "system.l2",
        "sim.instruction_cache.l2",
        "sim.data_cache.l2",
    );
    append_gem5_shared_cache_alias_stats(
        output,
        snapshot,
        "system.l3",
        "sim.instruction_cache.l3",
        "sim.data_cache.l3",
    );
}

fn append_gem5_branch_prediction_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let Some(core_count) = snapshot_value(snapshot, "sim.cores") else {
        return;
    };
    for cpu in 0..core_count {
        let alias_prefix = gem5_cpu_alias_prefix(core_count, cpu);
        if let Some(predictions) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_predictions"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.condPredicted"),
                predictions,
            );
        }
        if let Some(predicted_taken) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_predicted_taken"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.condPredictedTaken"),
                predicted_taken,
            );
        }
        if let Some(mispredictions) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.pipeline.in_order.conditional_branch_mispredictions"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.condIncorrect"),
                mispredictions,
            );
        }
        let btb_lookups = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.btb.lookups"),
        );
        let btb_hits = snapshot_value(snapshot, &format!("sim.cpu{cpu}.branch_predictor.btb.hits"));
        if let Some(lookups) = btb_lookups {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.BTBLookups"),
                lookups,
            );
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.btb.lookups::total"),
                lookups,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(lookups) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.btb.lookups.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.btb.lookups::{}",
                        kind.gem5_branch_type_name()
                    ),
                    lookups,
                );
            }
        }
        if let Some(hits) = btb_hits {
            append_derived_count_stat(output, &format!("{alias_prefix}.branchPred.BTBHits"), hits);
        }
        if let Some(misses) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.btb.misses"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.btb.misses::total"),
                misses,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(misses) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.btb.misses.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.btb.misses::{}",
                        kind.gem5_branch_type_name()
                    ),
                    misses,
                );
            }
        }
        if let Some(updates) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.btb.updates"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.BTBUpdates"),
                updates,
            );
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.btb.updates::total"),
                updates,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(updates) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.btb.updates.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.btb.updates::{}",
                        kind.gem5_branch_type_name()
                    ),
                    updates,
                );
            }
        }
        if let Some(evictions) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.btb.evictions"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.btb.evictions"),
                evictions,
            );
        }
        if let Some(mispredictions) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.btb.mispredictions"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.BTBMispredicted"),
                mispredictions,
            );
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.btb.mispredict::total"),
                mispredictions,
            );
        }
        if let Some(predicted_taken_misses) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.btb.predicted_taken_misses"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.predTakenBTBMiss"),
                predicted_taken_misses,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(mispredictions) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.btb.mispredict_due_to_btb_miss.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.mispredictDueToBTBMiss_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    mispredictions,
                );
            }
        }
        if let Some(mispredictions) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.btb.mispredict_due_to_btb_miss.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.mispredictDueToBTBMiss_0::total"),
                mispredictions,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(lookups) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.lookups.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.lookups_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    lookups,
                );
            }
        }
        if let Some(lookups) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.lookups.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.lookups_0::total"),
                lookups,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(squashes) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.squashes.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.squashes_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    squashes,
                );
            }
        }
        if let Some(squashes) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.squashes.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.squashes_0::total"),
                squashes,
            );
        }
        if let Some(lookups) = gem5_indirect_branch_lookups(snapshot, cpu) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.indirectLookups"),
                lookups,
            );
            if let Some(hits) = snapshot_value(
                snapshot,
                &format!("sim.cpu{cpu}.branch_predictor.indirect_hits"),
            ) {
                append_derived_count_stat(
                    output,
                    &format!("{alias_prefix}.branchPred.indirectHits"),
                    hits,
                );
                if let Some(misses) = lookups.checked_sub(hits) {
                    append_derived_count_stat(
                        output,
                        &format!("{alias_prefix}.branchPred.indirectMisses"),
                        misses,
                    );
                }
            }
        }
        if let Some(mispredicted) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.indirect_mispredicted"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.indirectMispredicted"),
                mispredicted,
            );
        }
        for name in ["pushes", "pops", "squashes", "used", "correct", "incorrect"] {
            if let Some(value) = snapshot_value(
                snapshot,
                &format!("sim.cpu{cpu}.branch_predictor.ras.{name}"),
            ) {
                append_derived_count_stat(
                    output,
                    &format!("{alias_prefix}.branchPred.ras.{name}"),
                    value,
                );
            }
        }
        for provider in BranchTargetProvider::ALL {
            if let Some(target_provider) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.target_provider.{}",
                    provider.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.targetProvider_0::{}",
                        provider.gem5_target_provider_name()
                    ),
                    target_provider,
                );
            }
        }
        if let Some(target_provider) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.target_provider.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.targetProvider_0::total"),
                target_provider,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(committed) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.committed.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.committed_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    committed,
                );
            }
        }
        if let Some(committed) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.committed.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.committed_0::total"),
                committed,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(mispredicted) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.mispredicted.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.mispredicted_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    mispredicted,
                );
            }
        }
        if let Some(mispredicted) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.mispredicted.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.mispredicted_0::total"),
                mispredicted,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(corrected) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.corrected.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.corrected_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    corrected,
                );
            }
        }
        if let Some(corrected) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.corrected.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.corrected_0::total"),
                corrected,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(target_wrong) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.target_wrong.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.targetWrong_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    target_wrong,
                );
            }
        }
        if let Some(target_wrong) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.target_wrong.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.targetWrong_0::total"),
                target_wrong,
            );
        }
        for kind in BranchTargetKind::ALL {
            if let Some(mispredictions) = snapshot_value(
                snapshot,
                &format!(
                    "sim.cpu{cpu}.branch_predictor.mispredict_due_to_predictor.{}",
                    kind.canonical_stat_name()
                ),
            ) {
                append_derived_count_stat(
                    output,
                    &format!(
                        "{alias_prefix}.branchPred.mispredictDueToPredictor_0::{}",
                        kind.gem5_branch_type_name()
                    ),
                    mispredictions,
                );
            }
        }
        if let Some(mispredictions) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.branch_predictor.mispredict_due_to_predictor.total"),
        ) {
            append_derived_count_stat(
                output,
                &format!("{alias_prefix}.branchPred.mispredictDueToPredictor_0::total"),
                mispredictions,
            );
        }
        append_gem5_branch_predictor_family_alias_stats(output, snapshot, cpu, &alias_prefix);
        if let (Some(hits), Some(lookups)) = (btb_hits, btb_lookups) {
            if lookups != 0 {
                append_derived_ratio_stat(
                    output,
                    &format!("{alias_prefix}.branchPred.BTBHitRatio"),
                    hits,
                    lookups,
                );
            }
        }
    }
}

fn append_gem5_branch_predictor_family_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    cpu: u64,
    alias_prefix: &str,
) {
    for (family, fields) in [
        (
            "gshare",
            &["lookups", "history_updates", "updates", "squashes"][..],
        ),
        (
            "bimode",
            &["lookups", "history_updates", "updates", "squashes"][..],
        ),
        (
            "tournament",
            &[
                "lookups",
                "history_updates",
                "updates",
                "squashes",
                "local_predictions",
                "global_predictions",
            ][..],
        ),
        (
            "tage_sc_l",
            &[
                "lookups",
                "history_updates",
                "updates",
                "repairs",
                "selected_rollbacks",
            ][..],
        ),
        (
            "multiperspective_perceptron",
            &["lookups", "updates", "selected_rollbacks"][..],
        ),
    ] {
        for field in fields {
            append_derived_stat_from_snapshot(
                output,
                snapshot,
                &format!("sim.cpu{cpu}.branch_predictor.{family}.{field}"),
                &format!("{alias_prefix}.branchPred.{family}.{field}"),
                "Count",
            );
        }
    }
}

fn append_gem5_o3_iq_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let Some(core_count) = snapshot_value(snapshot, "sim.cores") else {
        return;
    };
    for cpu in 0..core_count {
        let alias_prefix = gem5_cpu_alias_prefix(core_count, cpu);
        if let Some(instructions) =
            snapshot_value(snapshot, &format!("sim.cpu{cpu}.o3.instructions"))
        {
            append_derived_count_stat_if_absent(
                output,
                snapshot,
                &format!("{alias_prefix}.iq.instsIssued"),
                instructions,
            );
        }
        if let (Some(loads), Some(stores)) = (
            snapshot_value(snapshot, &format!("sim.cpu{cpu}.o3.lsq_loads")),
            snapshot_value(snapshot, &format!("sim.cpu{cpu}.o3.lsq_stores")),
        ) {
            append_derived_count_stat_if_absent(
                output,
                snapshot,
                &format!("{alias_prefix}.iq.memInstsIssued"),
                loads.saturating_add(stores),
            );
        }
        append_derived_stat_from_snapshot_if_absent(
            output,
            snapshot,
            &format!("sim.cpu{cpu}.o3.iq.branch_insts_issued"),
            &format!("{alias_prefix}.iq.branchInstsIssued"),
            "Count",
        );
        append_derived_stat_from_snapshot_if_absent(
            output,
            snapshot,
            &format!("sim.cpu{cpu}.o3.lsq_store_to_load_forwarding_matches"),
            &format!("{alias_prefix}.lsq0.forwLoads"),
            "Count",
        );
        let targetless_mismatches = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.o3.branch_repair_targetless_mismatches"),
        );
        let wrong_targets = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.o3.branch_repair_wrong_targets"),
        );
        let direction_only_mismatches = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.o3.branch_repair_direction_only_mismatches"),
        );
        let branch_mispredict_stats = [
            targetless_mismatches,
            wrong_targets,
            direction_only_mismatches,
        ];
        if branch_mispredict_stats.iter().any(Option::is_some) {
            let branch_mispredicts = branch_mispredict_stats
                .into_iter()
                .flatten()
                .fold(0_u64, u64::saturating_add);
            for (alias_name, value) in [
                ("TargetlessMismatch", targetless_mismatches.unwrap_or(0)),
                ("WrongTarget", wrong_targets.unwrap_or(0)),
                ("DirectionOnly", direction_only_mismatches.unwrap_or(0)),
                ("total", branch_mispredicts),
            ] {
                append_derived_count_stat_if_absent(
                    output,
                    snapshot,
                    &format!("{alias_prefix}.iew.branchRepair_0::{alias_name}"),
                    value,
                );
            }
            for alias_name in ["iew.branchMispredicts", "commit.branchMispredicts"] {
                let alias_path = format!("{alias_prefix}.{alias_name}");
                if snapshot_value(snapshot, &alias_path).is_none() {
                    append_derived_count_stat(output, &alias_path, branch_mispredicts);
                }
            }
        }
        for (op_class, source_name) in [("MemRead", "lsq_loads"), ("MemWrite", "lsq_stores")] {
            let source_path = format!("sim.cpu{cpu}.o3.{source_name}");
            if let Some(value) = snapshot_value(snapshot, &source_path) {
                append_derived_count_stat_if_absent(
                    output,
                    snapshot,
                    &format!("{alias_prefix}.iq.issuedInstType_0::{op_class}"),
                    value,
                );
            }
        }
        for (op_class, class) in [
            ("IntMult", O3RuntimeFuLatencyClass::ScalarIntegerMul),
            ("IntDiv", O3RuntimeFuLatencyClass::ScalarIntegerDiv),
            ("FloatAdd", O3RuntimeFuLatencyClass::ScalarFloatAdd),
            ("FloatCmp", O3RuntimeFuLatencyClass::ScalarFloatCompare),
            ("FloatMisc", O3RuntimeFuLatencyClass::ScalarFloatMisc),
            ("FloatMult", O3RuntimeFuLatencyClass::ScalarFloatMul),
            ("FloatMultAcc", O3RuntimeFuLatencyClass::ScalarFloatFma),
            ("FloatDiv", O3RuntimeFuLatencyClass::ScalarFloatDiv),
            ("FloatSqrt", O3RuntimeFuLatencyClass::ScalarFloatSqrt),
            ("SimdMult", O3RuntimeFuLatencyClass::VectorIntegerMul),
            ("SimdDiv", O3RuntimeFuLatencyClass::VectorIntegerDiv),
            ("SimdFloatAdd", O3RuntimeFuLatencyClass::VectorFloatAdd),
            ("SimdFloatCmp", O3RuntimeFuLatencyClass::VectorFloatCompare),
            ("SimdFloatMisc", O3RuntimeFuLatencyClass::VectorFloatMisc),
            ("SimdFloatMult", O3RuntimeFuLatencyClass::VectorFloatMul),
            ("SimdFloatMultAcc", O3RuntimeFuLatencyClass::VectorFloatFma),
            ("SimdFloatDiv", O3RuntimeFuLatencyClass::VectorFloatDiv),
            ("SimdFloatSqrt", O3RuntimeFuLatencyClass::VectorFloatSqrt),
        ] {
            let source_path = format!("sim.cpu{cpu}.o3.fu_{}_instructions", class.stat_stem());
            if let Some(value) = snapshot_value(snapshot, &source_path) {
                append_derived_count_stat_if_absent(
                    output,
                    snapshot,
                    &format!("{alias_prefix}.iq.issuedInstType_0::{op_class}"),
                    value,
                );
            }
        }
        for (op_class, source_name) in [
            ("MemRead", "mem_read"),
            ("MemWrite", "mem_write"),
            ("IntMult", "int_mul"),
            ("IntDiv", "int_div"),
            ("FloatAdd", "float_add"),
            ("FloatCmp", "float_compare"),
            ("FloatMisc", "float_misc"),
            ("FloatMult", "float_mul"),
            ("FloatMultAcc", "float_fma"),
            ("FloatDiv", "float_div"),
            ("FloatSqrt", "float_sqrt"),
            ("SimdMult", "vector_integer_mul"),
            ("SimdDiv", "vector_integer_div"),
            ("SimdFloatAdd", "vector_float_add"),
            ("SimdFloatCmp", "vector_float_compare"),
            ("SimdFloatMisc", "vector_float_misc"),
            ("SimdFloatMult", "vector_float_mul"),
            ("SimdFloatMultAcc", "vector_float_fma"),
            ("SimdFloatDiv", "vector_float_div"),
            ("SimdFloatSqrt", "vector_float_sqrt"),
        ] {
            append_derived_stat_from_snapshot_if_absent(
                output,
                snapshot,
                &format!("sim.cpu{cpu}.o3.commit.committed_inst_type.{source_name}"),
                &format!("{alias_prefix}.commit.committedInstType_0::{op_class}"),
                "Count",
            );
        }
        if let Some(insts_to_commit) =
            snapshot_value(snapshot, &format!("sim.cpu{cpu}.o3.iew.insts_to_commit"))
        {
            append_derived_count_stat_if_absent(
                output,
                snapshot,
                &format!("{alias_prefix}.iew.instsToCommit::total"),
                insts_to_commit,
            );
        }
        if let Some(writeback_count) =
            snapshot_value(snapshot, &format!("sim.cpu{cpu}.o3.iew.writeback_count"))
        {
            append_derived_count_stat_if_absent(
                output,
                snapshot,
                &format!("{alias_prefix}.iew.writebackCount::total"),
                writeback_count,
            );
            if let Some(cycles) = snapshot_value(snapshot, &format!("{alias_prefix}.numCycles")) {
                append_derived_count_per_cycle_stat(
                    output,
                    &format!("{alias_prefix}.iew.wbRate"),
                    writeback_count,
                    cycles,
                );
            }
        }
        let producer_inst = snapshot_value(snapshot, &format!("sim.cpu{cpu}.o3.iew.producer_inst"));
        let consumer_inst = snapshot_value(snapshot, &format!("sim.cpu{cpu}.o3.iew.consumer_inst"));
        if let Some(producer_inst) = producer_inst {
            append_derived_count_stat_if_absent(
                output,
                snapshot,
                &format!("{alias_prefix}.iew.producerInst::total"),
                producer_inst,
            );
        }
        if let Some(consumer_inst) = consumer_inst {
            append_derived_count_stat_if_absent(
                output,
                snapshot,
                &format!("{alias_prefix}.iew.consumerInst::total"),
                consumer_inst,
            );
        }
        if let (Some(producer_inst), Some(consumer_inst)) = (producer_inst, consumer_inst) {
            append_derived_count_per_count_stat(
                output,
                &format!("{alias_prefix}.iew.wbFanout"),
                producer_inst,
                consumer_inst,
            );
        }
    }
}

fn gem5_indirect_branch_lookups(snapshot: &StatSnapshot, cpu: u64) -> Option<u64> {
    let mut lookups = 0_u64;
    for kind in [
        BranchTargetKind::IndirectConditional,
        BranchTargetKind::IndirectUnconditional,
        BranchTargetKind::CallIndirect,
    ] {
        let value = snapshot_value(
            snapshot,
            &format!(
                "sim.cpu{cpu}.branch_predictor.lookups.{}",
                kind.canonical_stat_name()
            ),
        )?;
        lookups = lookups.saturating_add(value);
    }
    Some(lookups)
}

fn append_gem5_work_item_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let Some(core_count) = snapshot_value(snapshot, "sim.cores") else {
        return;
    };
    if core_count != 1 {
        return;
    }
    append_derived_stat_from_snapshot(
        output,
        snapshot,
        "sim.host_actions.roi_begin",
        "system.cpu.numWorkItemsStarted",
        "Count",
    );
    append_derived_stat_from_snapshot(
        output,
        snapshot,
        "sim.host_actions.roi_end",
        "system.cpu.numWorkItemsCompleted",
        "Count",
    );
}

fn append_gem5_work_item_duration_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    for sample in snapshot.samples() {
        let Some(work_id) = gem5_work_item_type_from_host_action_duration_path(sample.path())
        else {
            continue;
        };
        let alias_path = format!("system.work_item_type{work_id}");
        output.push_str(&format!(
            "{alias_path:<64} {:>20} # kind=histogram unit={} reset_policy={}\n",
            sample.value(),
            sample.unit(),
            sample.reset_policy()
        ));
        for bucket in sample.histogram_buckets() {
            output.push_str(&format!(
                "{:<64} {:>20} # histogram_bucket={} unit={} reset_policy={}\n",
                format!("{alias_path}.bucket"),
                bucket.count(),
                bucket.bucket(),
                sample.unit(),
                sample.reset_policy()
            ));
        }
    }
}

fn gem5_work_item_type_from_host_action_duration_path(path: &str) -> Option<&str> {
    let work_id = path
        .strip_prefix("sim.host_actions.roi_work_item_type")?
        .strip_suffix(".duration_ticks")?;
    (!work_id.is_empty() && work_id.bytes().all(|byte| byte.is_ascii_digit())).then_some(work_id)
}

fn append_gem5_ruby_network_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let mut total_flits = 0_u64;
    for sample in snapshot.samples() {
        let Some(vnet) = gem5_ruby_network_vnet_from_flit_path(sample.path()) else {
            continue;
        };
        let value = sample.value();
        total_flits = total_flits.saturating_add(value);
        append_derived_count_stat(
            output,
            &format!("system.ruby.network.flits_injected::vnet-{vnet}"),
            value,
        );
        append_derived_count_stat(
            output,
            &format!("system.ruby.network.flits_received::vnet-{vnet}"),
            value,
        );
    }
    if total_flits != 0 {
        append_derived_count_stat(
            output,
            "system.ruby.network.flits_injected::total",
            total_flits,
        );
        append_derived_count_stat(
            output,
            "system.ruby.network.flits_received::total",
            total_flits,
        );
    }
}

fn gem5_ruby_network_vnet_from_flit_path(path: &str) -> Option<&str> {
    let vnet = path
        .strip_prefix("sim.memory.fabric.vn")?
        .strip_suffix(".flits")?;
    (!vnet.is_empty() && vnet.bytes().all(|byte| byte.is_ascii_digit())).then_some(vnet)
}

fn append_gem5_in_order_pipeline_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let Some(core_count) = snapshot_value(snapshot, "sim.cores") else {
        return;
    };
    for cpu in 0..core_count {
        let alias_prefix = gem5_cpu_alias_prefix(core_count, cpu);
        let pipeline_alias_prefix = format!("{alias_prefix}.pipeline.inOrder");
        #[rustfmt::skip]
        const PIPELINE_ALIAS_STATS: [(&str, &str, &str); 23] = [
            ("advanced", "advanced", "Count"),
            ("flushed", "flushed", "Count"),
            ("flush_cycles", "flushCycles", "Cycle"),
            ("resource_blocked", "resourceBlocked", "Count"),
            ("ordering_blocked", "orderingBlocked", "Count"),
            ("stall_cycles", "stallCycles", "Cycle"),
            ("fetch_wait_cycles", "fetchWaitCycles", "Cycle"),
            ("data_wait_cycles", "dataWaitCycles", "Cycle"),
            ("execute_wait_cycles", "executeWaitCycles", "Cycle"),
            ("branch_prediction_flushes", "branchPredictionFlushes", "Count"),
            ("branch_prediction_flush_cycles", "branchPredictionFlushCycles", "Cycle"),
            ("redirects", "redirects", "Count"),
            ("branch_prediction_redirects", "branchPredictionRedirects", "Count"),
            ("interrupt_redirects", "interruptRedirects", "Count"),
            ("interrupt_redirect_flushes", "interruptRedirectFlushes", "Count"),
            ("interrupt_redirect_flush_cycles", "interruptRedirectFlushCycles", "Cycle"),
            ("trap_redirects", "trapRedirects", "Count"),
            ("trap_redirect_flushes", "trapRedirectFlushes", "Count"),
            ("trap_redirect_flush_cycles", "trapRedirectFlushCycles", "Cycle"),
            ("branch_speculation_predictions", "branchSpeculationPredictions", "Count"),
            ("branch_speculation_repairs", "branchSpeculationRepairs", "Count"),
            ("branch_speculation_removed_youngers", "branchSpeculationRemovedYoungers", "Count"),
            ("branch_speculation_max_pending", "branchSpeculationMaxPending", "Count"),
        ];
        for (source_name, alias_name, unit) in PIPELINE_ALIAS_STATS {
            append_derived_stat_from_snapshot(
                output,
                snapshot,
                &format!("sim.cpu{cpu}.pipeline.in_order.{source_name}"),
                &format!("{pipeline_alias_prefix}.{alias_name}"),
                unit,
            );
        }
        #[rustfmt::skip]
        const STAGE_ALIAS_STATS: [(&str, &str, &str); 17] = [
            ("occupied_cycles", "occupiedCycles", "Cycle"),
            ("advanced", "advanced", "Count"),
            ("advanced_cycles", "advancedCycles", "Cycle"),
            ("retired", "retired", "Count"),
            ("retired_cycles", "retiredCycles", "Cycle"),
            ("resource_blocked", "resourceBlocked", "Count"),
            ("resource_blocked_cycles", "resourceBlockedCycles", "Cycle"),
            ("ordering_blocked", "orderingBlocked", "Count"),
            ("ordering_blocked_cycles", "orderingBlockedCycles", "Cycle"),
            ("flushed", "flushed", "Count"),
            ("flushed_cycles", "flushedCycles", "Cycle"),
            ("branch_prediction_flushed", "branchPredictionFlushed", "Count"),
            ("branch_prediction_flushed_cycles", "branchPredictionFlushedCycles", "Cycle"),
            ("interrupt_redirect_flushed", "interruptRedirectFlushed", "Count"),
            ("interrupt_redirect_flushed_cycles", "interruptRedirectFlushedCycles", "Cycle"),
            ("trap_redirect_flushed", "trapRedirectFlushed", "Count"),
            ("trap_redirect_flushed_cycles", "trapRedirectFlushedCycles", "Cycle"),
        ];
        for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
            for (source_name, alias_name, unit) in STAGE_ALIAS_STATS {
                append_derived_stat_from_snapshot(
                    output,
                    snapshot,
                    &format!("sim.cpu{cpu}.pipeline.in_order.stage.{stage}.{source_name}"),
                    &format!("{pipeline_alias_prefix}.stage.{stage}.{alias_name}"),
                    unit,
                );
            }
        }
        for (source_cause, alias_cause) in [
            ("fetch_wait", "fetchWait"),
            ("data_wait", "dataWait"),
            ("execute_wait", "executeWait"),
        ] {
            for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
                for (source_name, alias_name, unit) in [
                    ("resource_blocked", "resourceBlocked", "Count"),
                    ("resource_blocked_cycles", "resourceBlockedCycles", "Cycle"),
                    ("ordering_blocked", "orderingBlocked", "Count"),
                    ("ordering_blocked_cycles", "orderingBlockedCycles", "Cycle"),
                ] {
                    append_derived_stat_from_snapshot(
                        output,
                        snapshot,
                        &format!(
                            "sim.cpu{cpu}.pipeline.in_order.stall_cause.{source_cause}.stage.{stage}.{source_name}"
                        ),
                        &format!(
                            "{pipeline_alias_prefix}.stallCause.{alias_cause}.stage.{stage}.{alias_name}"
                        ),
                        unit,
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
                for stage in ["fetch1", "fetch2", "decode", "execute", "commit"] {
                    for (source_name, alias_name, unit) in [
                        ("flushed", "flushed", "Count"),
                        ("flushed_cycles", "flushedCycles", "Cycle"),
                    ] {
                        append_derived_stat_from_snapshot(
                            output,
                            snapshot,
                            &format!(
                                "sim.cpu{cpu}.pipeline.in_order.{source_family}.{source_cause}.stage.{stage}.{source_name}"
                            ),
                            &format!(
                                "{pipeline_alias_prefix}.{alias_family}.{alias_cause}.stage.{stage}.{alias_name}"
                            ),
                            unit,
                        );
                    }
                }
            }
        }
    }
}

fn format_sim_seconds(final_tick: u64, sim_freq: u64) -> String {
    let whole = final_tick / sim_freq;
    let remainder = final_tick % sim_freq;
    let fractional = (u128::from(remainder) * 1_000_000_000_000_u128) / u128::from(sim_freq);
    format!("{whole}.{fractional:012}")
}

fn snapshot_value(snapshot: &StatSnapshot, path: &str) -> Option<u64> {
    snapshot
        .samples()
        .iter()
        .find(|sample| sample.path() == path)
        .map(|sample| sample.value())
}

fn gem5_cpu_alias_prefix(core_count: u64, cpu: u64) -> String {
    if core_count == 1 {
        "system.cpu".to_string()
    } else {
        format!("system.cpu{cpu}")
    }
}

fn append_gem5_l1_cache_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    if snapshot_value(snapshot, "sim.cores") != Some(1) {
        return;
    }
    append_gem5_l1_cache_alias_stats_for(
        output,
        snapshot,
        "system.cpu.icache",
        "sim.instruction_cache",
    );
    append_gem5_l1_cache_alias_stats_for(output, snapshot, "system.cpu.dcache", "sim.data_cache");
}

fn append_gem5_l1_cache_alias_stats_for(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_prefix: &str,
    source_prefix: &str,
) {
    let Some(inputs) = gem5_cache_hit_miss_inputs(snapshot, source_prefix) else {
        return;
    };
    if can_emit_gem5_l1_cache_demand_alias_stats(snapshot, source_prefix) {
        append_gem5_cache_hit_miss_alias_stats(
            output,
            alias_prefix,
            "demand",
            inputs.hits,
            inputs.misses,
        );
        append_gem5_cache_mshr_alias_stats(
            output,
            alias_prefix,
            "demand",
            inputs.mshr_hits,
            inputs.mshr_misses,
            inputs.accesses(),
        );
    }
    append_gem5_cache_hit_miss_alias_stats(
        output,
        alias_prefix,
        "overall",
        inputs.hits,
        inputs.misses,
    );
    append_gem5_cache_mshr_alias_stats(
        output,
        alias_prefix,
        "overall",
        inputs.mshr_hits,
        inputs.mshr_misses,
        inputs.accesses(),
    );
}

fn can_emit_gem5_l1_cache_demand_alias_stats(snapshot: &StatSnapshot, source_prefix: &str) -> bool {
    snapshot_value(snapshot, &format!("{source_prefix}.prefetch.issued")) == Some(0)
        && snapshot_value(snapshot, &format!("{source_prefix}.prefetch.queue.issued")) == Some(0)
}

#[derive(Clone, Copy, Debug)]
struct CacheHitMissInputs {
    hits: u64,
    misses: u64,
    mshr_hits: u64,
    mshr_misses: u64,
}

impl CacheHitMissInputs {
    fn accesses(self) -> u64 {
        self.hits.saturating_add(self.misses)
    }

    fn saturating_add(self, other: Self) -> Self {
        Self {
            hits: self.hits.saturating_add(other.hits),
            misses: self.misses.saturating_add(other.misses),
            mshr_hits: self.mshr_hits.saturating_add(other.mshr_hits),
            mshr_misses: self.mshr_misses.saturating_add(other.mshr_misses),
        }
    }
}

fn gem5_cache_hit_miss_inputs(
    snapshot: &StatSnapshot,
    source_prefix: &str,
) -> Option<CacheHitMissInputs> {
    let (Some(hits), Some(scheduled_misses), Some(coalesced_misses)) = (
        snapshot_value(snapshot, &format!("{source_prefix}.bank.immediate_hits")),
        snapshot_value(snapshot, &format!("{source_prefix}.bank.scheduled_misses")),
        snapshot_value(snapshot, &format!("{source_prefix}.bank.coalesced_misses")),
    ) else {
        return None;
    };
    Some(CacheHitMissInputs {
        hits,
        misses: scheduled_misses.saturating_add(coalesced_misses),
        mshr_hits: coalesced_misses,
        mshr_misses: scheduled_misses,
    })
}

fn append_gem5_shared_cache_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_prefix: &str,
    instruction_source_prefix: &str,
    data_source_prefix: &str,
) {
    let instruction = gem5_cache_hit_miss_inputs(snapshot, instruction_source_prefix);
    let data = gem5_cache_hit_miss_inputs(snapshot, data_source_prefix);
    let inputs = match (instruction, data) {
        (Some(instruction), Some(data)) => Some(instruction.saturating_add(data)),
        (Some(inputs), None) | (None, Some(inputs)) => Some(inputs),
        (None, None) => None,
    };
    let Some(inputs) = inputs else {
        return;
    };
    if inputs.accesses() == 0 {
        return;
    }
    append_gem5_cache_hit_miss_alias_stats(
        output,
        alias_prefix,
        "overall",
        inputs.hits,
        inputs.misses,
    );
    append_gem5_cache_mshr_alias_stats(
        output,
        alias_prefix,
        "overall",
        inputs.mshr_hits,
        inputs.mshr_misses,
        inputs.accesses(),
    );
}

fn append_gem5_cache_hit_miss_alias_stats(
    output: &mut String,
    alias_prefix: &str,
    alias_kind: &str,
    hits: u64,
    misses: u64,
) {
    append_derived_count_stat(output, &format!("{alias_prefix}.{alias_kind}Hits"), hits);
    append_derived_count_stat(
        output,
        &format!("{alias_prefix}.{alias_kind}Misses"),
        misses,
    );
    let accesses = hits.saturating_add(misses);
    append_derived_count_stat(
        output,
        &format!("{alias_prefix}.{alias_kind}Accesses"),
        accesses,
    );
    if accesses != 0 {
        append_derived_ratio_stat(
            output,
            &format!("{alias_prefix}.{alias_kind}MissRate"),
            misses,
            accesses,
        );
    }
}

fn append_gem5_cache_mshr_alias_stats(
    output: &mut String,
    alias_prefix: &str,
    alias_kind: &str,
    mshr_hits: u64,
    mshr_misses: u64,
    accesses: u64,
) {
    append_derived_count_stat(
        output,
        &format!("{alias_prefix}.{alias_kind}MshrHits"),
        mshr_hits,
    );
    append_derived_count_stat(
        output,
        &format!("{alias_prefix}.{alias_kind}MshrMisses"),
        mshr_misses,
    );
    if accesses != 0 {
        append_derived_ratio_stat(
            output,
            &format!("{alias_prefix}.{alias_kind}MshrMissRate"),
            mshr_misses,
            accesses,
        );
    }
}

fn append_gem5_l1_prefetcher_formula_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    append_gem5_l1_prefetcher_formula_alias_stats_for(output, snapshot, "system.cpu.icache");
    append_gem5_l1_prefetcher_formula_alias_stats_for(output, snapshot, "system.cpu.dcache");
}

fn append_gem5_l1_prefetcher_formula_alias_stats_for(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_prefix: &str,
) {
    let useful = snapshot_value(snapshot, &format!("{alias_prefix}.prefetcher.pfUseful"));
    let issued = snapshot_value(snapshot, &format!("{alias_prefix}.prefetcher.pfIssued"));
    if let (Some(useful), Some(issued)) = (useful, issued) {
        if issued != 0 {
            append_derived_ratio_stat(
                output,
                &format!("{alias_prefix}.prefetcher.accuracy"),
                useful,
                issued,
            );
        }
    }

    let demand_mshr_misses = snapshot_value(
        snapshot,
        &format!("{alias_prefix}.prefetcher.demandMshrMisses"),
    );
    if let (Some(useful), Some(demand_mshr_misses)) = (useful, demand_mshr_misses) {
        let denominator = useful.saturating_add(demand_mshr_misses);
        if denominator != 0 {
            append_derived_ratio_stat(
                output,
                &format!("{alias_prefix}.prefetcher.coverage"),
                useful,
                denominator,
            );
        }
    }
}

fn append_derived_count_stat(output: &mut String, path: &str, value: u64) {
    append_derived_unit_stat(output, path, value, "Count");
}

fn append_derived_count_stat_if_absent(
    output: &mut String,
    snapshot: &StatSnapshot,
    path: &str,
    value: u64,
) {
    if snapshot_value(snapshot, path).is_none() {
        append_derived_count_stat(output, path, value);
    }
}

fn append_derived_cycle_stat(output: &mut String, path: &str, value: u64) {
    append_derived_unit_stat(output, path, value, "Cycle");
}

fn append_derived_unit_stat(output: &mut String, path: &str, value: u64, unit: &str) {
    output.push_str(&format!(
        "{path:<64} {value:>20} # kind=derived unit={unit} reset_policy=monotonic\n"
    ));
}

fn append_derived_stat_from_snapshot(
    output: &mut String,
    snapshot: &StatSnapshot,
    source_path: &str,
    alias_path: &str,
    unit: &str,
) {
    let Some(value) = snapshot_value(snapshot, source_path) else {
        return;
    };
    match unit {
        "Count" => append_derived_count_stat(output, alias_path, value),
        "Cycle" => append_derived_cycle_stat(output, alias_path, value),
        _ => append_derived_unit_stat(output, alias_path, value, unit),
    }
}

fn append_derived_stat_from_snapshot_if_absent(
    output: &mut String,
    snapshot: &StatSnapshot,
    source_path: &str,
    alias_path: &str,
    unit: &str,
) {
    if snapshot_value(snapshot, alias_path).is_none() {
        append_derived_stat_from_snapshot(output, snapshot, source_path, alias_path, unit);
    }
}

fn append_derived_ratio_stat(output: &mut String, path: &str, numerator: u64, denominator: u64) {
    output.push_str(&format!(
        "{path:<64} {:>20} # kind=derived unit=Ratio reset_policy=monotonic\n",
        format_fixed_ratio(numerator, denominator)
    ));
}

fn append_derived_count_per_cycle_stat(output: &mut String, path: &str, count: u64, cycles: u64) {
    if cycles == 0 {
        return;
    }
    output.push_str(&format!(
        "{path:<64} {:>20} # kind=derived unit=(Count/Cycle) reset_policy=monotonic\n",
        format_fixed_ratio(count, cycles)
    ));
}

fn append_derived_count_per_count_stat(
    output: &mut String,
    path: &str,
    numerator: u64,
    denominator: u64,
) {
    if denominator == 0 {
        return;
    }
    output.push_str(&format!(
        "{path:<64} {:>20} # kind=derived unit=(Count/Count) reset_policy=monotonic\n",
        format_fixed_ratio(numerator, denominator)
    ));
}

fn append_gem5_mem_ctrl_bandwidth_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
    let (Some(final_tick), Some(sim_freq)) = (
        snapshot_value(snapshot, "finalTick"),
        snapshot_value(snapshot, "simFreq"),
    ) else {
        return;
    };
    if final_tick == 0 || sim_freq == 0 {
        return;
    }
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.avgRdBWSys",
        "system.mem_ctrl.bytesReadSys",
        sim_freq,
        final_tick,
        1,
        Some(8),
    );
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.avgWrBWSys",
        "system.mem_ctrl.bytesWrittenSys",
        sim_freq,
        final_tick,
        1,
        Some(8),
    );
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.avgRdBW",
        "system.mem_ctrl.dram.dramBytesRead",
        sim_freq,
        final_tick,
        1_000_000,
        None,
    );
    append_gem5_mem_ctrl_bandwidth_alias_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.avgWrBW",
        "system.mem_ctrl.dram.dramBytesWritten",
        sim_freq,
        final_tick,
        1_000_000,
        None,
    );
}

fn append_gem5_mem_ctrl_bandwidth_alias_stat(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_path: &str,
    bytes_path: &str,
    sim_freq: u64,
    final_tick: u64,
    denominator_scale: u64,
    precision: Option<usize>,
) {
    let Some(bytes) = snapshot_value(snapshot, bytes_path) else {
        return;
    };
    output.push_str(&format!(
        "{alias_path:<64} {:>20} # kind=derived unit=(Byte/Second) reset_policy=monotonic\n",
        format_scaled_ratio(bytes, sim_freq, final_tick, denominator_scale, precision)
    ));
}

fn append_gem5_dram_interface_ratio_stats(output: &mut String, snapshot: &StatSnapshot) {
    append_gem5_dram_interface_row_hit_rate_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.readRowHitRate",
        "system.mem_ctrl.dram.readRowHits",
        "system.mem_ctrl.dram.readBursts",
    );
    append_gem5_dram_interface_row_hit_rate_stat(
        output,
        snapshot,
        "system.mem_ctrl.dram.writeRowHitRate",
        "system.mem_ctrl.dram.writeRowHits",
        "system.mem_ctrl.dram.writeBursts",
    );

    let (Some(row_hits), Some(read_bursts), Some(write_bursts)) = (
        snapshot_value(snapshot, "sim.memory.dram.row_hits"),
        snapshot_value(snapshot, "system.mem_ctrl.dram.readBursts"),
        snapshot_value(snapshot, "system.mem_ctrl.dram.writeBursts"),
    ) else {
        return;
    };
    let bursts = read_bursts.saturating_add(write_bursts);
    if bursts == 0 {
        return;
    }
    append_gem5_dram_interface_percent_ratio_stat(
        output,
        "system.mem_ctrl.dram.pageHitRate",
        row_hits,
        bursts,
    );
}

fn append_gem5_dram_interface_row_hit_rate_stat(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_path: &str,
    row_hits_path: &str,
    bursts_path: &str,
) {
    let (Some(row_hits), Some(bursts)) = (
        snapshot_value(snapshot, row_hits_path),
        snapshot_value(snapshot, bursts_path),
    ) else {
        return;
    };
    if bursts == 0 {
        return;
    }
    append_gem5_dram_interface_percent_ratio_stat(output, alias_path, row_hits, bursts);
}

fn append_gem5_dram_interface_percent_ratio_stat(
    output: &mut String,
    alias_path: &str,
    numerator: u64,
    denominator: u64,
) {
    output.push_str(&format!(
        "{alias_path:<64} {:>20} # kind=derived unit=Ratio reset_policy=monotonic\n",
        format_scaled_ratio(numerator, 100, denominator, 1, Some(2))
    ));
}

fn append_gem5_dram_interface_latency_stats(output: &mut String, snapshot: &StatSnapshot) {
    let (Some(total_latency), Some(read_bursts)) = (
        snapshot_value(snapshot, "system.mem_ctrl.dram.totMemAccLat"),
        snapshot_value(snapshot, "system.mem_ctrl.dram.readBursts"),
    ) else {
        return;
    };
    if read_bursts == 0 {
        return;
    }
    output.push_str(&format!(
        "{:<64} {:>20} # kind=derived unit=(Tick/Count) reset_policy=monotonic\n",
        "system.mem_ctrl.dram.avgMemAccLat",
        format_scaled_ratio(total_latency, 1, read_bursts, 1, Some(2))
    ));
}

#[derive(Clone, Copy, Debug, Default)]
struct CpuRatioInputs {
    instructions: Option<u64>,
    cycles: Option<u64>,
}

fn append_gem5_cpu_ratio_stats(output: &mut String, snapshot: &StatSnapshot) {
    let mut cpus = BTreeMap::<String, CpuRatioInputs>::new();
    let mut commit_stats0_instructions = BTreeMap::<String, u64>::new();
    for sample in snapshot.samples() {
        if let Some(prefix) = sample.path().strip_suffix(".numInsts") {
            if is_gem5_cpu_prefix(prefix) {
                cpus.entry(prefix.to_string()).or_default().instructions = Some(sample.value());
            }
        }
        if let Some(prefix) = sample.path().strip_suffix(".numCycles") {
            if is_gem5_cpu_prefix(prefix) {
                cpus.entry(prefix.to_string()).or_default().cycles = Some(sample.value());
            }
        }
        if let Some(prefix) = sample.path().strip_suffix(".commitStats0.numInsts") {
            if is_gem5_cpu_prefix(prefix) {
                commit_stats0_instructions.insert(prefix.to_string(), sample.value());
            }
        }
    }
    for (prefix, inputs) in &cpus {
        let (Some(instructions), Some(cycles)) = (inputs.instructions, inputs.cycles) else {
            continue;
        };
        append_gem5_cpu_ratio_stat_pair(output, prefix, instructions, cycles);
    }
    for (prefix, instructions) in commit_stats0_instructions {
        let Some(cycles) = cpus.get(&prefix).and_then(|inputs| inputs.cycles) else {
            continue;
        };
        append_gem5_cpu_ratio_stat_pair(
            output,
            &format!("{prefix}.commitStats0"),
            instructions,
            cycles,
        );
    }
}

fn append_gem5_cpu_ratio_stat_pair(
    output: &mut String,
    prefix: &str,
    instructions: u64,
    cycles: u64,
) {
    if instructions == 0 || cycles == 0 {
        return;
    }
    output.push_str(&format!(
        "{:<64} {:>20} # kind=derived unit=(Count/Cycle) reset_policy=monotonic\n",
        format!("{prefix}.ipc"),
        format_fixed_ratio(instructions, cycles)
    ));
    output.push_str(&format!(
        "{:<64} {:>20} # kind=derived unit=(Cycle/Count) reset_policy=monotonic\n",
        format!("{prefix}.cpi"),
        format_fixed_ratio(cycles, instructions)
    ));
}

fn is_gem5_cpu_prefix(prefix: &str) -> bool {
    prefix == "system.cpu"
        || prefix.strip_prefix("system.cpu").is_some_and(|suffix| {
            !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
        })
}

fn format_fixed_ratio(numerator: u64, denominator: u64) -> String {
    format!("{:.6}", numerator as f64 / denominator as f64)
}

fn format_scaled_ratio(
    value: u64,
    multiplier: u64,
    denominator: u64,
    denominator_scale: u64,
    precision: Option<usize>,
) -> String {
    let value = (value as f64 * multiplier as f64) / denominator as f64 / denominator_scale as f64;
    match precision {
        Some(precision) => format!("{value:.precision$}"),
        None if value == value.round() => format!("{value:.0}"),
        None => format!("{value:.6}"),
    }
}

#[cfg(test)]
mod tests {
    use rem6_stats::StatsRegistry;

    use super::stats_snapshot_text;

    macro_rules! counter {
        ($stats:expr, $path:expr, $unit:expr) => {
            $stats.register_counter($path, $unit).unwrap()
        };
    }

    #[test]
    fn stats_output_renders_gem5_sim_seconds_without_float_rounding() {
        let mut stats = StatsRegistry::new();
        let ticks = stats.register_counter("finalTick", "Tick").unwrap();
        let frequency = stats.register_counter("simFreq", "Hz").unwrap();
        stats.increment(ticks, 9_007_199_254_740_993).unwrap();
        stats.increment(frequency, 1_000_000_000_000).unwrap();

        let text = stats_snapshot_text(&stats.snapshot(0));

        assert!(text.contains("simSeconds"));
        assert!(text.contains("9007.199254740993"));
    }

    #[test]
    fn stats_output_renders_only_valid_gem5_cpu_ratio_prefixes() {
        let mut stats = StatsRegistry::new();
        let cpu0_insts = counter!(&mut stats, "system.cpu0.numInsts", "Count");
        let cpu0_cycles = counter!(&mut stats, "system.cpu0.numCycles", "Cycle");
        let cpu0_commit_insts = counter!(&mut stats, "system.cpu0.commitStats0.numInsts", "Count");
        let cpu_named_insts = counter!(&mut stats, "system.cpu.main.numInsts", "Count");
        let cpu_named_cycles = counter!(&mut stats, "system.cpu.main.numCycles", "Cycle");
        let cpu_named_commit_insts =
            counter!(&mut stats, "system.cpu.main.commitStats0.numInsts", "Count");
        let cpu1_insts = counter!(&mut stats, "system.cpu1.numInsts", "Count");
        let cpu1_cycles = counter!(&mut stats, "system.cpu1.numCycles", "Cycle");
        let cpu1_commit_insts = counter!(&mut stats, "system.cpu1.commitStats0.numInsts", "Count");
        stats.increment(cpu0_insts, 3).unwrap();
        stats.increment(cpu0_cycles, 12).unwrap();
        stats.increment(cpu0_commit_insts, 3).unwrap();
        stats.increment(cpu_named_insts, 7).unwrap();
        stats.increment(cpu_named_cycles, 14).unwrap();
        stats.increment(cpu_named_commit_insts, 7).unwrap();
        stats.increment(cpu1_insts, 5).unwrap();
        stats.increment(cpu1_cycles, 0).unwrap();
        stats.increment(cpu1_commit_insts, 5).unwrap();

        let text = stats_snapshot_text(&stats.snapshot(0));

        assert!(text.contains("system.cpu0.ipc"));
        assert!(text.contains("0.250000"));
        assert!(text.contains("system.cpu0.cpi"));
        assert!(text.contains("4.000000"));
        assert!(text.contains("system.cpu0.commitStats0.ipc"));
        assert!(text.contains("system.cpu0.commitStats0.cpi"));
        assert!(!text.contains("system.cpu.main.ipc"));
        assert!(!text.contains("system.cpu.main.commitStats0.ipc"));
        assert!(!text.contains("system.cpu1.ipc"));
        assert!(!text.contains("system.cpu1.commitStats0.ipc"));
    }

    #[test]
    fn stats_output_renders_zero_o3_wb_rate_when_writeback_stat_is_present() {
        let mut stats = StatsRegistry::new();
        let cores = counter!(&mut stats, "sim.cores", "Count");
        let cycles = counter!(&mut stats, "system.cpu.numCycles", "Cycle");
        counter!(&mut stats, "sim.cpu0.o3.iew.writeback_count", "Count");
        stats.increment(cores, 1).unwrap();
        stats.increment(cycles, 12).unwrap();

        let text = stats_snapshot_text(&stats.snapshot(0));

        assert!(text.contains("system.cpu.iew.writebackCount::total"));
        assert!(text.contains("system.cpu.iew.wbRate"));
        assert!(text.contains("0.000000 # kind=derived unit=(Count/Cycle)"));
    }

    #[test]
    fn stats_output_saturates_o3_branch_mispredict_alias_total() {
        let mut stats = StatsRegistry::new();
        let cores = counter!(&mut stats, "sim.cores", "Count");
        let targetless = counter!(
            &mut stats,
            "sim.cpu0.o3.branch_repair_targetless_mismatches",
            "Count"
        );
        let wrong_targets = counter!(
            &mut stats,
            "sim.cpu0.o3.branch_repair_wrong_targets",
            "Count"
        );
        let direction_only = counter!(
            &mut stats,
            "sim.cpu0.o3.branch_repair_direction_only_mismatches",
            "Count"
        );
        stats.increment(cores, 1).unwrap();
        stats.increment(targetless, u64::MAX).unwrap();
        stats.increment(wrong_targets, 1).unwrap();
        stats.increment(direction_only, 1).unwrap();

        let text = stats_snapshot_text(&stats.snapshot(0));

        assert!(text.contains("system.cpu.iew.branchMispredicts"));
        assert!(text.contains("system.cpu.commit.branchMispredicts"));
        assert!(text.contains("18446744073709551615 # kind=derived unit=Count"));
    }
}
