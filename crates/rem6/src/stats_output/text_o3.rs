use rem6_cpu::BranchTargetKind;
use rem6_stats::StatSnapshot;

use super::text::{append_derived_count_stat_if_absent, snapshot_value};

pub(super) fn append_gem5_o3_ftq_alias_stats(
    output: &mut String,
    snapshot: &StatSnapshot,
    cpu: u64,
    alias_prefix: &str,
) {
    for kind in BranchTargetKind::ALL {
        let source_path = format!(
            "sim.cpu{cpu}.o3.branch_event.squashed_target_kind.{}",
            kind.canonical_stat_name()
        );
        let alias_path = format!(
            "{alias_prefix}.ftq.squashedTargets_0::{}",
            kind.gem5_branch_type_name()
        );
        if let Some(value) = snapshot_value(snapshot, &source_path) {
            append_derived_count_stat_if_absent(output, snapshot, &alias_path, value);
        }
    }
    if let Some(value) = snapshot_value(
        snapshot,
        &format!("sim.cpu{cpu}.o3.branch_event.squashed_targets"),
    ) {
        append_derived_count_stat_if_absent(
            output,
            snapshot,
            &format!("{alias_prefix}.ftq.squashedTargets_0::total"),
            value,
        );
    }
}
