pub(crate) struct O3BranchMismatchScalarAlias {
    pub(crate) source_suffix: &'static str,
    pub(crate) alias_suffix: &'static str,
    pub(crate) bucket_alias: &'static str,
}

pub(crate) const O3_BRANCH_MISMATCH_SCALAR_ALIASES: &[O3BranchMismatchScalarAlias] = &[
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_direction_mismatch.mismatches",
        alias_suffix: "branchDirectionMismatches",
        bucket_alias: "branchDirectionMismatch",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_direction_mismatch.without_link_writes",
        alias_suffix: "branchDirectionMismatchWithoutLinkWrites",
        bucket_alias: "branchDirectionMismatchWithoutLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_direction_mismatch.squashed_targets",
        alias_suffix: "branchDirectionMismatchSquashedTargets",
        bucket_alias: "branchDirectionMismatchSquashedTargets",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_direction_mismatch.squashed_target_without_link_writes",
        alias_suffix: "branchDirectionMismatchSquashedTargetWithoutLinkWrites",
        bucket_alias: "branchDirectionMismatchSquashedTargetWithoutLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_direction_mismatch.squashed_target_link_writes",
        alias_suffix: "branchDirectionMismatchSquashedTargetLinkWrites",
        bucket_alias: "branchDirectionMismatchSquashedTargetLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.targetless_mismatches",
        alias_suffix: "branchTargetlessMismatches",
        bucket_alias: "branchTargetlessMismatch",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.targetless_mismatch_without_link_writes",
        alias_suffix: "branchTargetlessMismatchWithoutLinkWrites",
        bucket_alias: "branchTargetlessMismatchWithoutLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.targetless_mismatch_squashed_targets",
        alias_suffix: "branchTargetlessMismatchSquashedTargets",
        bucket_alias: "branchTargetlessMismatchSquashedTargets",
    },
    O3BranchMismatchScalarAlias {
        source_suffix:
            "branch_target_mismatch.targetless_mismatch_squashed_target_without_link_writes",
        alias_suffix: "branchTargetlessMismatchSquashedTargetWithoutLinkWrites",
        bucket_alias: "branchTargetlessMismatchSquashedTargetWithoutLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.wrong_targets",
        alias_suffix: "branchWrongTargets",
        bucket_alias: "branchWrongTarget",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.wrong_target_squashed_targets",
        alias_suffix: "branchWrongTargetSquashedTargets",
        bucket_alias: "branchWrongTargetSquashedTargets",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.wrong_target_squashed_target_without_link_writes",
        alias_suffix: "branchWrongTargetSquashedTargetWithoutLinkWrites",
        bucket_alias: "branchWrongTargetSquashedTargetWithoutLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.wrong_target_squashed_target_link_writes",
        alias_suffix: "branchWrongTargetSquashedTargetLinkWrites",
        bucket_alias: "branchWrongTargetSquashedTargetLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.wrong_target_link_writes",
        alias_suffix: "branchWrongTargetLinkWrites",
        bucket_alias: "branchWrongTargetLinkWrites",
    },
    O3BranchMismatchScalarAlias {
        source_suffix: "branch_target_mismatch.wrong_target_without_link_writes",
        alias_suffix: "branchWrongTargetWithoutLinkWrites",
        bucket_alias: "branchWrongTargetWithoutLinkWrites",
    },
];

pub(crate) struct O3BranchMismatchKindAlias {
    pub(crate) source_family: &'static str,
    pub(crate) alias_family: &'static str,
}

pub(crate) const O3_BRANCH_MISMATCH_KIND_ALIASES: &[O3BranchMismatchKindAlias] = &[
    O3BranchMismatchKindAlias {
        source_family: "branch_direction_mismatch.kind",
        alias_family: "branchDirectionMismatch",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_direction_mismatch.link_write_kind",
        alias_family: "branchDirectionMismatchLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_direction_mismatch.without_link_write_kind",
        alias_family: "branchDirectionMismatchWithoutLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_direction_mismatch.squashed_target_kind",
        alias_family: "branchDirectionMismatchSquashedTargets",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_direction_mismatch.squashed_target_link_write_kind",
        alias_family: "branchDirectionMismatchSquashedTargetLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_direction_mismatch.squashed_target_without_link_write_kind",
        alias_family: "branchDirectionMismatchSquashedTargetWithoutLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.targetless_mismatch_kind",
        alias_family: "branchTargetlessMismatch",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.targetless_mismatch_without_link_write_kind",
        alias_family: "branchTargetlessMismatchWithoutLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.targetless_mismatch_squashed_target_kind",
        alias_family: "branchTargetlessMismatchSquashedTargets",
    },
    O3BranchMismatchKindAlias {
        source_family:
            "branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind",
        alias_family: "branchTargetlessMismatchSquashedTargetWithoutLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.wrong_target_kind",
        alias_family: "branchWrongTarget",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.wrong_target_squashed_target_kind",
        alias_family: "branchWrongTargetSquashedTargets",
    },
    O3BranchMismatchKindAlias {
        source_family:
            "branch_target_mismatch.wrong_target_squashed_target_without_link_write_kind",
        alias_family: "branchWrongTargetSquashedTargetWithoutLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.wrong_target_squashed_target_link_write_kind",
        alias_family: "branchWrongTargetSquashedTargetLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.wrong_target_link_write_kind",
        alias_family: "branchWrongTargetLinkWrites",
    },
    O3BranchMismatchKindAlias {
        source_family: "branch_target_mismatch.wrong_target_without_link_write_kind",
        alias_family: "branchWrongTargetWithoutLinkWrites",
    },
];
