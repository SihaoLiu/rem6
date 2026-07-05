use rem6_cpu::BranchTargetKind;

use super::Rem6O3TraceStat;

pub(super) fn push_o3_branch_kind_count_stats(
    stats: &mut Vec<Rem6O3TraceStat>,
    suffix: fn(BranchTargetKind) -> &'static str,
    value: impl Fn(BranchTargetKind) -> u64,
) {
    for kind in BranchTargetKind::ALL {
        if matches!(kind, BranchTargetKind::NoBranch) {
            continue;
        }
        stats.push(Rem6O3TraceStat {
            suffix: suffix(kind),
            unit: "Count",
            value: value(kind),
        });
    }
}

pub(super) fn o3_branch_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_kind.no_branch",
        BranchTargetKind::Return => "event.branch_kind.return",
        BranchTargetKind::CallDirect => "event.branch_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => "event.branch_kind.indirect_unconditional",
    }
}

pub(super) fn o3_branch_taken_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_taken_kind.no_branch",
        BranchTargetKind::Return => "event.branch_taken_kind.return",
        BranchTargetKind::CallDirect => "event.branch_taken_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_taken_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_taken_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_taken_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_taken_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => "event.branch_taken_kind.indirect_unconditional",
    }
}

pub(super) fn o3_branch_not_taken_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_not_taken_kind.no_branch",
        BranchTargetKind::Return => "event.branch_not_taken_kind.return",
        BranchTargetKind::CallDirect => "event.branch_not_taken_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_not_taken_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_not_taken_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_not_taken_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_not_taken_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_not_taken_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_predicted_taken_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_taken_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_taken_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_taken_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_predicted_taken_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_taken_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_taken_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_taken_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_taken_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_predicted_not_taken_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_not_taken_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_not_taken_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_not_taken_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_predicted_not_taken_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_not_taken_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_not_taken_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_not_taken_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_not_taken_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_predicted_target_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_target_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_target_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_target_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_predicted_target_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_target_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_target_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_predicted_target_match_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_target_match_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_target_match_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_target_match_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_predicted_target_match_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_target_match_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_target_match_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_target_match_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_target_match_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_predicted_target_mismatch_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_predicted_target_mismatch_kind.no_branch",
        BranchTargetKind::Return => "event.branch_predicted_target_mismatch_kind.return",
        BranchTargetKind::CallDirect => "event.branch_predicted_target_mismatch_kind.call_direct",
        BranchTargetKind::CallIndirect => {
            "event.branch_predicted_target_mismatch_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_predicted_target_mismatch_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_predicted_target_mismatch_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_predicted_target_mismatch_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_predicted_target_mismatch_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_targetless_mismatch_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_targetless_mismatch_kind.no_branch",
        BranchTargetKind::Return => "event.branch_targetless_mismatch_kind.return",
        BranchTargetKind::CallDirect => "event.branch_targetless_mismatch_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_targetless_mismatch_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_targetless_mismatch_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_targetless_mismatch_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_targetless_mismatch_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_targetless_mismatch_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_targetless_mismatch_squashed_target_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => {
            "event.branch_targetless_mismatch_squashed_target_kind.no_branch"
        }
        BranchTargetKind::Return => "event.branch_targetless_mismatch_squashed_target_kind.return",
        BranchTargetKind::CallDirect => {
            "event.branch_targetless_mismatch_squashed_target_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_targetless_mismatch_squashed_target_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_targetless_mismatch_squashed_target_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_targetless_mismatch_squashed_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_targetless_mismatch_squashed_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_targetless_mismatch_squashed_target_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_wrong_target_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_wrong_target_kind.no_branch",
        BranchTargetKind::Return => "event.branch_wrong_target_kind.return",
        BranchTargetKind::CallDirect => "event.branch_wrong_target_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_wrong_target_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_wrong_target_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => {
            "event.branch_wrong_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_wrong_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_wrong_target_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_wrong_target_squashed_target_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_wrong_target_squashed_target_kind.no_branch",
        BranchTargetKind::Return => "event.branch_wrong_target_squashed_target_kind.return",
        BranchTargetKind::CallDirect => {
            "event.branch_wrong_target_squashed_target_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_wrong_target_squashed_target_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_wrong_target_squashed_target_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_wrong_target_squashed_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_wrong_target_squashed_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_wrong_target_squashed_target_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_wrong_target_squashed_target_without_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.no_branch"
        }
        BranchTargetKind::Return => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.return"
        }
        BranchTargetKind::CallDirect => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_wrong_target_squashed_target_without_link_write_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_wrong_target_squashed_target_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => {
            "event.branch_wrong_target_squashed_target_link_write_kind.no_branch"
        }
        BranchTargetKind::Return => {
            "event.branch_wrong_target_squashed_target_link_write_kind.return"
        }
        BranchTargetKind::CallDirect => {
            "event.branch_wrong_target_squashed_target_link_write_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_wrong_target_squashed_target_link_write_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_wrong_target_squashed_target_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_wrong_target_squashed_target_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_wrong_target_squashed_target_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_wrong_target_squashed_target_link_write_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_wrong_target_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_wrong_target_link_write_kind.no_branch",
        BranchTargetKind::Return => "event.branch_wrong_target_link_write_kind.return",
        BranchTargetKind::CallDirect => "event.branch_wrong_target_link_write_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_wrong_target_link_write_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_wrong_target_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_wrong_target_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_wrong_target_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_wrong_target_link_write_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_wrong_target_without_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_wrong_target_without_link_write_kind.no_branch",
        BranchTargetKind::Return => "event.branch_wrong_target_without_link_write_kind.return",
        BranchTargetKind::CallDirect => {
            "event.branch_wrong_target_without_link_write_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_wrong_target_without_link_write_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_wrong_target_without_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_wrong_target_without_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_wrong_target_without_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_wrong_target_without_link_write_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_resolved_target_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_resolved_target_kind.no_branch",
        BranchTargetKind::Return => "event.branch_resolved_target_kind.return",
        BranchTargetKind::CallDirect => "event.branch_resolved_target_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_resolved_target_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_resolved_target_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_resolved_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_resolved_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_resolved_target_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_link_write_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_link_write_kind.no_branch",
        BranchTargetKind::Return => "event.branch_link_write_kind.return",
        BranchTargetKind::CallDirect => "event.branch_link_write_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_link_write_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_link_write_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => {
            "event.branch_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_link_write_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_misprediction_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_misprediction_kind.no_branch",
        BranchTargetKind::Return => "event.branch_misprediction_kind.return",
        BranchTargetKind::CallDirect => "event.branch_misprediction_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_misprediction_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_misprediction_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => {
            "event.branch_misprediction_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_misprediction_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_misprediction_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_squash_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_squash_kind.no_branch",
        BranchTargetKind::Return => "event.branch_squash_kind.return",
        BranchTargetKind::CallDirect => "event.branch_squash_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_squash_kind.call_indirect",
        BranchTargetKind::DirectConditional => "event.branch_squash_kind.direct_conditional",
        BranchTargetKind::DirectUnconditional => "event.branch_squash_kind.direct_unconditional",
        BranchTargetKind::IndirectConditional => "event.branch_squash_kind.indirect_conditional",
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_squash_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_squashed_target_kind_stat_suffix(kind: BranchTargetKind) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_squashed_target_kind.no_branch",
        BranchTargetKind::Return => "event.branch_squashed_target_kind.return",
        BranchTargetKind::CallDirect => "event.branch_squashed_target_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_squashed_target_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_squashed_target_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_squashed_target_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_squashed_target_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_squashed_target_kind.indirect_unconditional"
        }
    }
}
