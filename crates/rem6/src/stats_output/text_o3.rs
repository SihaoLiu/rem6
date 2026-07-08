use rem6_cpu::{BranchTargetKind, O3RuntimeFuLatencyClass};
use rem6_stats::StatSnapshot;

use super::text::{
    append_derived_count_per_count_stat, append_derived_count_per_cycle_stat,
    append_derived_count_stat, append_derived_count_stat_if_absent,
    append_derived_stat_from_snapshot_if_absent, gem5_cpu_alias_prefix, snapshot_value,
};

pub(super) fn append_gem5_o3_iq_alias_stats(output: &mut String, snapshot: &StatSnapshot) {
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
        append_gem5_o3_branch_event_alias_stats(output, snapshot, cpu, &alias_prefix);
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
                append_gem5_o3_iq_inst_type_alias_stats(
                    output,
                    snapshot,
                    &alias_prefix,
                    op_class,
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
                append_gem5_o3_iq_inst_type_alias_stats(
                    output,
                    snapshot,
                    &alias_prefix,
                    op_class,
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
            let source_path = format!("sim.cpu{cpu}.o3.commit.committed_inst_type.{source_name}");
            if let Some(value) = snapshot_value(snapshot, &source_path) {
                append_gem5_o3_commit_inst_type_alias_stats(
                    output,
                    snapshot,
                    &alias_prefix,
                    op_class,
                    value,
                );
            }
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

fn append_gem5_o3_iq_inst_type_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_prefix: &str,
    op_class: &str,
    value: u64,
) {
    for alias_stem in ["issuedInstType.", "issuedInstType_0::"] {
        append_derived_count_stat_if_absent(
            output,
            snapshot,
            &format!("{alias_prefix}.iq.{alias_stem}{op_class}"),
            value,
        );
    }
}

fn append_gem5_o3_commit_inst_type_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    alias_prefix: &str,
    op_class: &str,
    value: u64,
) {
    for alias_stem in ["committedInstType.", "committedInstType_0::"] {
        append_derived_count_stat_if_absent(
            output,
            snapshot,
            &format!("{alias_prefix}.commit.{alias_stem}{op_class}"),
            value,
        );
    }
}

fn append_gem5_o3_branch_event_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    cpu: u64,
    alias_prefix: &str,
) {
    append_gem5_o3_branch_prediction_alias_stats(output, snapshot, cpu, alias_prefix);
    append_gem5_o3_ftq_alias_stats(output, snapshot, cpu, alias_prefix);
}

fn append_gem5_o3_branch_prediction_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    cpu: u64,
    alias_prefix: &str,
) {
    for (source_suffix, alias_suffix) in [
        ("predicted_taken", "fetch.predictedBranches"),
        ("mispredictions", "bac.branchMisspredict"),
    ] {
        let source_path = format!("sim.cpu{cpu}.o3.branch_event.{source_suffix}");
        let alias_path = format!("{alias_prefix}.{alias_suffix}");
        if let Some(value) = snapshot_value(snapshot, &source_path) {
            append_derived_count_stat_if_absent(output, snapshot, &alias_path, value);
        }
    }
}

fn append_gem5_o3_ftq_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
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
            let source_path = format!(
                "sim.cpu{cpu}.o3.branch_event.{source_family}.{}",
                kind.canonical_stat_name()
            );
            let alias_path = format!(
                "{alias_prefix}.ftq.{alias_family}_0::{}",
                kind.gem5_branch_type_name()
            );
            if let Some(value) = snapshot_value(snapshot, &source_path) {
                append_derived_count_stat_if_absent(output, snapshot, &alias_path, value);
            }
        }
        if let Some(value) = snapshot_value(
            snapshot,
            &format!("sim.cpu{cpu}.o3.branch_event.{source_total}"),
        ) {
            append_derived_count_stat_if_absent(
                output,
                snapshot,
                &format!("{alias_prefix}.ftq.{alias_family}_0::total"),
                value,
            );
        }
    }
}
