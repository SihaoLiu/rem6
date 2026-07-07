use rem6_cpu::{BranchTargetKind, O3RuntimeStats};

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

fn o3_branch_event_kind_json<F>(count: F) -> String
where
    F: Fn(BranchTargetKind) -> u64,
{
    let fields = BranchTargetKind::ALL
        .into_iter()
        .map(|kind| format!("\"{}\":{}", kind.canonical_stat_name(), count(kind)))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{fields}}}")
}

pub(super) fn o3_branch_event_json(stats: O3RuntimeStats) -> String {
    let kind = o3_branch_event_kind_json(|branch_kind| stats.branch_event_kind(branch_kind));
    let taken_kind =
        o3_branch_event_kind_json(|branch_kind| stats.branch_event_taken_kind(branch_kind));
    let not_taken_kind =
        o3_branch_event_kind_json(|branch_kind| stats.branch_event_not_taken_kind(branch_kind));
    let predicted_taken_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_predicted_taken_kind(branch_kind)
    });
    let predicted_not_taken_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_predicted_not_taken_kind(branch_kind)
    });
    let predicted_target_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_predicted_target_kind(branch_kind)
    });
    let predicted_target_match_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_predicted_target_match_kind(branch_kind)
    });
    let predicted_target_mismatch_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_predicted_target_mismatch_kind(branch_kind)
    });
    let resolved_target_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_resolved_target_kind(branch_kind)
    });
    let misprediction_kind =
        o3_branch_event_kind_json(|branch_kind| stats.branch_event_misprediction_kind(branch_kind));
    let link_write_kind =
        o3_branch_event_kind_json(|branch_kind| stats.branch_event_link_write_kind(branch_kind));
    let squash_kind =
        o3_branch_event_kind_json(|branch_kind| stats.branch_event_squash_kind(branch_kind));
    let squashed_target_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_squashed_target_kind(branch_kind)
    });
    let squashed_target_link_write_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_squashed_target_link_write_kind(branch_kind)
    });
    let squashed_target_without_link_write_kind = o3_branch_event_kind_json(|branch_kind| {
        stats.branch_event_squashed_target_without_link_write_kind(branch_kind)
    });
    format!(
        "{{\"branches\":{},\"taken\":{},\"not_taken\":{},\"predicted_taken\":{},\"predicted_not_taken\":{},\"predicted_targets\":{},\"predicted_target_matches\":{},\"predicted_target_mismatches\":{},\"resolved_targets\":{},\"mispredictions\":{},\"kind\":{kind},\"taken_kind\":{taken_kind},\"not_taken_kind\":{not_taken_kind},\"predicted_taken_kind\":{predicted_taken_kind},\"predicted_not_taken_kind\":{predicted_not_taken_kind},\"predicted_target_kind\":{predicted_target_kind},\"predicted_target_match_kind\":{predicted_target_match_kind},\"predicted_target_mismatch_kind\":{predicted_target_mismatch_kind},\"resolved_target_kind\":{resolved_target_kind},\"misprediction_kind\":{misprediction_kind},\"link_writes\":{},\"without_link_writes\":{},\"link_write_kind\":{link_write_kind},\"squashes\":{},\"squashed_targets\":{},\"squashed_targets_with_link_writes\":{},\"squashed_targets_without_link_writes\":{},\"squash_kind\":{squash_kind},\"squashed_target_kind\":{squashed_target_kind},\"squashed_target_link_write_kind\":{squashed_target_link_write_kind},\"squashed_target_without_link_write_kind\":{squashed_target_without_link_write_kind}}}",
        stats.branch_events(),
        stats.branch_event_taken(),
        stats.branch_event_not_taken(),
        stats.branch_event_predicted_taken(),
        stats.branch_event_predicted_not_taken(),
        stats.branch_event_predicted_targets(),
        stats.branch_event_predicted_target_matches(),
        stats.branch_event_predicted_target_mismatches(),
        stats.branch_event_resolved_targets(),
        stats.branch_event_mispredictions(),
        stats.branch_event_link_writes(),
        stats.branch_event_without_link_writes(),
        stats.branch_event_squashes(),
        stats.branch_event_squashed_targets(),
        stats.branch_event_squashed_targets_with_link_writes(),
        stats.branch_event_squashed_targets_without_link_writes(),
    )
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

pub(super) fn o3_branch_direction_mismatch_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_direction_mismatch_kind.no_branch",
        BranchTargetKind::Return => "event.branch_direction_mismatch_kind.return",
        BranchTargetKind::CallDirect => "event.branch_direction_mismatch_kind.call_direct",
        BranchTargetKind::CallIndirect => "event.branch_direction_mismatch_kind.call_indirect",
        BranchTargetKind::DirectConditional => {
            "event.branch_direction_mismatch_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_direction_mismatch_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_direction_mismatch_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_direction_mismatch_kind.indirect_unconditional"
        }
    }
}

macro_rules! o3_branch_direction_mismatch_suffix_fn {
    ($name:ident, $prefix:literal) => {
        pub(super) fn $name(kind: BranchTargetKind) -> &'static str {
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

o3_branch_direction_mismatch_suffix_fn!(
    o3_branch_direction_mismatch_link_write_kind_stat_suffix,
    "event.branch_direction_mismatch_link_write_kind"
);
o3_branch_direction_mismatch_suffix_fn!(
    o3_branch_direction_mismatch_without_link_write_kind_stat_suffix,
    "event.branch_direction_mismatch_without_link_write_kind"
);
o3_branch_direction_mismatch_suffix_fn!(
    o3_branch_direction_mismatch_squashed_target_kind_stat_suffix,
    "event.branch_direction_mismatch_squashed_target_kind"
);
o3_branch_direction_mismatch_suffix_fn!(
    o3_branch_direction_mismatch_squashed_target_link_write_kind_stat_suffix,
    "event.branch_direction_mismatch_squashed_target_link_write_kind"
);
o3_branch_direction_mismatch_suffix_fn!(
    o3_branch_direction_mismatch_squashed_target_without_link_write_kind_stat_suffix,
    "event.branch_direction_mismatch_squashed_target_without_link_write_kind"
);

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

pub(super) fn o3_branch_targetless_mismatch_without_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => {
            "event.branch_targetless_mismatch_without_link_write_kind.no_branch"
        }
        BranchTargetKind::Return => {
            "event.branch_targetless_mismatch_without_link_write_kind.return"
        }
        BranchTargetKind::CallDirect => {
            "event.branch_targetless_mismatch_without_link_write_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_targetless_mismatch_without_link_write_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_targetless_mismatch_without_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_targetless_mismatch_without_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_targetless_mismatch_without_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_targetless_mismatch_without_link_write_kind.indirect_unconditional"
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

pub(super) fn o3_branch_targetless_mismatch_squashed_target_without_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.no_branch"
        }
        BranchTargetKind::Return => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.return"
        }
        BranchTargetKind::CallDirect => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_targetless_mismatch_squashed_target_without_link_write_kind.indirect_unconditional"
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

pub(super) fn o3_branch_squashed_target_without_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => {
            "event.branch_squashed_target_without_link_write_kind.no_branch"
        }
        BranchTargetKind::Return => "event.branch_squashed_target_without_link_write_kind.return",
        BranchTargetKind::CallDirect => {
            "event.branch_squashed_target_without_link_write_kind.call_direct"
        }
        BranchTargetKind::CallIndirect => {
            "event.branch_squashed_target_without_link_write_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_squashed_target_without_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_squashed_target_without_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_squashed_target_without_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_squashed_target_without_link_write_kind.indirect_unconditional"
        }
    }
}

pub(super) fn o3_branch_squashed_target_link_write_kind_stat_suffix(
    kind: BranchTargetKind,
) -> &'static str {
    match kind {
        BranchTargetKind::NoBranch => "event.branch_squashed_target_link_write_kind.no_branch",
        BranchTargetKind::Return => "event.branch_squashed_target_link_write_kind.return",
        BranchTargetKind::CallDirect => "event.branch_squashed_target_link_write_kind.call_direct",
        BranchTargetKind::CallIndirect => {
            "event.branch_squashed_target_link_write_kind.call_indirect"
        }
        BranchTargetKind::DirectConditional => {
            "event.branch_squashed_target_link_write_kind.direct_conditional"
        }
        BranchTargetKind::DirectUnconditional => {
            "event.branch_squashed_target_link_write_kind.direct_unconditional"
        }
        BranchTargetKind::IndirectConditional => {
            "event.branch_squashed_target_link_write_kind.indirect_conditional"
        }
        BranchTargetKind::IndirectUnconditional => {
            "event.branch_squashed_target_link_write_kind.indirect_unconditional"
        }
    }
}
