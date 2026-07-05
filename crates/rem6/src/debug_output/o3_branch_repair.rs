use rem6_cpu::{BranchTargetKind, O3RuntimeTraceRecord};

use super::{o3_branch_stats::push_o3_branch_kind_count_stats, Rem6O3TraceStat};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Rem6O3BranchRepairKind {
    None,
    DirectionOnly,
    TargetlessMismatch,
    WrongTarget,
}

impl Rem6O3BranchRepairKind {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::DirectionOnly => "direction_only",
            Self::TargetlessMismatch => "targetless_mismatch",
            Self::WrongTarget => "wrong_target",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(super) struct Rem6O3BranchRepairTotals {
    targetless_mismatches: u64,
    wrong_targets: u64,
    direction_only_mismatches: u64,
    targetless_mismatch_kinds: [u64; BranchTargetKind::COUNT],
    wrong_target_kinds: [u64; BranchTargetKind::COUNT],
    direction_only_kinds: [u64; BranchTargetKind::COUNT],
}

impl Rem6O3BranchRepairTotals {
    pub(super) fn add_event(
        &mut self,
        event: &O3RuntimeTraceRecord,
        repair: Rem6O3BranchRepairKind,
    ) {
        let index = event.branch_kind().index();
        match repair {
            Rem6O3BranchRepairKind::None => {}
            Rem6O3BranchRepairKind::DirectionOnly => {
                self.direction_only_mismatches = self.direction_only_mismatches.saturating_add(1);
                self.direction_only_kinds[index] =
                    self.direction_only_kinds[index].saturating_add(1);
            }
            Rem6O3BranchRepairKind::TargetlessMismatch => {
                self.targetless_mismatches = self.targetless_mismatches.saturating_add(1);
                self.targetless_mismatch_kinds[index] =
                    self.targetless_mismatch_kinds[index].saturating_add(1);
            }
            Rem6O3BranchRepairKind::WrongTarget => {
                self.wrong_targets = self.wrong_targets.saturating_add(1);
                self.wrong_target_kinds[index] = self.wrong_target_kinds[index].saturating_add(1);
            }
        }
    }

    pub(super) fn push_stats(self, stats: &mut Vec<Rem6O3TraceStat>) {
        for (suffix, value) in [
            (
                "event.branch_repair_targetless_mismatches",
                self.targetless_mismatches,
            ),
            ("event.branch_repair_wrong_targets", self.wrong_targets),
            (
                "event.branch_repair_direction_only_mismatches",
                self.direction_only_mismatches,
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
            o3_branch_repair_targetless_mismatch_kind_stat_suffix,
            |kind| self.targetless_mismatch_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_repair_wrong_target_kind_stat_suffix,
            |kind| self.wrong_target_kinds[kind.index()],
        );
        push_o3_branch_kind_count_stats(
            stats,
            o3_branch_repair_direction_only_kind_stat_suffix,
            |kind| self.direction_only_kinds[kind.index()],
        );
    }
}

pub(super) fn o3_branch_repair_kind(event: &O3RuntimeTraceRecord) -> Rem6O3BranchRepairKind {
    if !event.branch_event() {
        return Rem6O3BranchRepairKind::None;
    }
    if o3_branch_targetless_mismatch(event) {
        return Rem6O3BranchRepairKind::TargetlessMismatch;
    }
    if o3_branch_wrong_target(event) {
        return Rem6O3BranchRepairKind::WrongTarget;
    }
    if event.branch_predicted_taken() != event.branch_resolved_taken() {
        return Rem6O3BranchRepairKind::DirectionOnly;
    }
    Rem6O3BranchRepairKind::None
}

pub(super) fn o3_branch_wrong_target(event: &O3RuntimeTraceRecord) -> bool {
    event
        .branch_predicted_target()
        .zip(event.branch_resolved_target())
        .is_some_and(|(predicted, resolved)| predicted != resolved)
}

pub(super) fn o3_branch_targetless_mismatch(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_predicted_target().is_some() && event.branch_resolved_target().is_none()
}

macro_rules! branch_repair_kind_suffix_fn {
    ($name:ident, $prefix:literal) => {
        fn $name(kind: BranchTargetKind) -> &'static str {
            match kind {
                BranchTargetKind::NoBranch => concat!($prefix, ".no_branch"),
                BranchTargetKind::Return => concat!($prefix, ".return"),
                BranchTargetKind::CallDirect => concat!($prefix, ".call_direct"),
                BranchTargetKind::CallIndirect => concat!($prefix, ".call_indirect"),
                BranchTargetKind::DirectConditional => concat!($prefix, ".direct_conditional"),
                BranchTargetKind::DirectUnconditional => concat!($prefix, ".direct_unconditional"),
                BranchTargetKind::IndirectConditional => concat!($prefix, ".indirect_conditional"),
                BranchTargetKind::IndirectUnconditional => {
                    concat!($prefix, ".indirect_unconditional")
                }
            }
        }
    };
}

branch_repair_kind_suffix_fn!(
    o3_branch_repair_targetless_mismatch_kind_stat_suffix,
    "event.branch_repair_targetless_mismatch_kind"
);
branch_repair_kind_suffix_fn!(
    o3_branch_repair_wrong_target_kind_stat_suffix,
    "event.branch_repair_wrong_target_kind"
);
branch_repair_kind_suffix_fn!(
    o3_branch_repair_direction_only_kind_stat_suffix,
    "event.branch_repair_direction_only_kind"
);
