use rem6_cpu::BranchTargetKind;
use rem6_stats::StatSnapshot;

use super::text::{append_derived_count_stat_if_absent, snapshot_value};

pub(super) fn append_gem5_o3_branch_event_alias_stats(
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
