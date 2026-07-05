use rem6_cpu::{BranchTargetKind, O3RuntimeTraceRecord};

use super::{
    o3_branch_stats::{
        o3_branch_direction_mismatch_kind_stat_suffix,
        o3_branch_direction_mismatch_link_write_kind_stat_suffix,
        o3_branch_direction_mismatch_squashed_target_kind_stat_suffix,
        o3_branch_direction_mismatch_squashed_target_link_write_kind_stat_suffix,
        o3_branch_direction_mismatch_squashed_target_without_link_write_kind_stat_suffix,
        o3_branch_direction_mismatch_without_link_write_kind_stat_suffix,
        push_o3_branch_kind_count_stats,
    },
    Rem6O3TraceStat,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3BranchDirectionMismatchTotals {
    mismatches: u64,
    without_link_writes: u64,
    squashed_targets: u64,
    squashed_target_without_link_writes: u64,
    kinds: [u64; BranchTargetKind::COUNT],
    link_write_kinds: [u64; BranchTargetKind::COUNT],
    without_link_write_kinds: [u64; BranchTargetKind::COUNT],
    squashed_target_kinds: [u64; BranchTargetKind::COUNT],
    squashed_target_link_write_kinds: [u64; BranchTargetKind::COUNT],
    squashed_target_without_link_write_kinds: [u64; BranchTargetKind::COUNT],
}

impl Rem6O3BranchDirectionMismatchTotals {
    pub(super) fn add_event(
        &mut self,
        event: &O3RuntimeTraceRecord,
        direction_mismatch: bool,
        squashed_target: bool,
    ) {
        if !direction_mismatch {
            return;
        }

        let link_write = event.branch_link_register_write();
        let without_link_write = !link_write;
        let squashed_target_without_link_write = squashed_target && without_link_write;
        let squashed_target_link_write = squashed_target && link_write;

        self.mismatches = self.mismatches.saturating_add(1);
        self.without_link_writes = self
            .without_link_writes
            .saturating_add(u64::from(without_link_write));
        self.squashed_targets = self
            .squashed_targets
            .saturating_add(u64::from(squashed_target));
        self.squashed_target_without_link_writes = self
            .squashed_target_without_link_writes
            .saturating_add(u64::from(squashed_target_without_link_write));

        let index = event.branch_kind().index();
        self.kinds[index] = self.kinds[index].saturating_add(1);
        if link_write {
            self.link_write_kinds[index] = self.link_write_kinds[index].saturating_add(1);
        } else {
            self.without_link_write_kinds[index] =
                self.without_link_write_kinds[index].saturating_add(1);
        }
        if squashed_target {
            self.squashed_target_kinds[index] = self.squashed_target_kinds[index].saturating_add(1);
        }
        if squashed_target_link_write {
            self.squashed_target_link_write_kinds[index] =
                self.squashed_target_link_write_kinds[index].saturating_add(1);
        }
        if squashed_target_without_link_write {
            self.squashed_target_without_link_write_kinds[index] =
                self.squashed_target_without_link_write_kinds[index].saturating_add(1);
        }
    }

    pub(super) fn push_stats(self, stats: &mut Vec<Rem6O3TraceStat>) {
        for (suffix, value) in [
            ("event.branch_direction_mismatches", self.mismatches),
            (
                "event.branch_direction_mismatch_without_link_writes",
                self.without_link_writes,
            ),
            (
                "event.branch_direction_mismatch_squashed_targets",
                self.squashed_targets,
            ),
            (
                "event.branch_direction_mismatch_squashed_target_without_link_writes",
                self.squashed_target_without_link_writes,
            ),
            (
                "event.branch_direction_mismatch_squashed_target_link_writes",
                self.squashed_targets
                    .saturating_sub(self.squashed_target_without_link_writes),
            ),
        ] {
            stats.push(Rem6O3TraceStat {
                suffix,
                unit: "Count",
                value,
            });
        }

        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_direction_mismatch_kind_stat_suffix,
            |kind| self.kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_direction_mismatch_link_write_kind_stat_suffix,
            |kind| self.link_write_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_direction_mismatch_without_link_write_kind_stat_suffix,
            |kind| self.without_link_write_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_direction_mismatch_squashed_target_kind_stat_suffix,
            |kind| self.squashed_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_direction_mismatch_squashed_target_link_write_kind_stat_suffix,
            |kind| self.squashed_target_link_write_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_direction_mismatch_squashed_target_without_link_write_kind_stat_suffix,
            |kind| self.squashed_target_without_link_write_kinds[kind.index()],
        );
    }
}
