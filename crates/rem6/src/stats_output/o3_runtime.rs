use rem6_cpu::{
    O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering, O3RuntimeStats,
};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::increment_stat;
use crate::Rem6CliError;

pub(super) fn emit_o3_branch_event_aggregate_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    o3: O3RuntimeStats,
) -> Result<(), Rem6CliError> {
    for (name, value) in [
        ("branch_event.branches", o3.branch_events()),
        ("branch_event.taken", o3.branch_event_taken()),
        ("branch_event.not_taken", o3.branch_event_not_taken()),
        (
            "branch_event.predicted_taken",
            o3.branch_event_predicted_taken(),
        ),
        (
            "branch_event.predicted_not_taken",
            o3.branch_event_predicted_not_taken(),
        ),
        (
            "branch_event.predicted_targets",
            o3.branch_event_predicted_targets(),
        ),
        (
            "branch_event.predicted_target_matches",
            o3.branch_event_predicted_target_matches(),
        ),
        (
            "branch_event.predicted_target_mismatches",
            o3.branch_event_predicted_target_mismatches(),
        ),
        (
            "branch_event.resolved_targets",
            o3.branch_event_resolved_targets(),
        ),
        ("branch_event.link_writes", o3.branch_event_link_writes()),
        (
            "branch_event.without_link_writes",
            o3.branch_event_without_link_writes(),
        ),
        ("branch_event.squashes", o3.branch_event_squashes()),
        (
            "branch_event.squashed_targets",
            o3.branch_event_squashed_targets(),
        ),
        (
            "branch_event.squashed_targets_with_link_writes",
            o3.branch_event_squashed_targets_with_link_writes(),
        ),
        (
            "branch_event.squashed_targets_without_link_writes",
            o3.branch_event_squashed_targets_without_link_writes(),
        ),
    ] {
        increment_count_stat(stats, format!("sim.cpu{cpu}.o3.{name}"), value)?;
    }
    Ok(())
}

pub(super) fn increment_count_stat(
    stats: &mut StatsRegistry,
    name: String,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(stats, &name, "Count", StatResetPolicy::Monotonic, value)
}

pub(super) fn o3_lsq_operation_alias(operation: O3RuntimeLsqOperation) -> &'static str {
    match operation {
        O3RuntimeLsqOperation::None => "none",
        O3RuntimeLsqOperation::Load => "load",
        O3RuntimeLsqOperation::Store => "store",
        O3RuntimeLsqOperation::LoadReserved => "loadReserved",
        O3RuntimeLsqOperation::StoreConditional => "storeConditional",
        O3RuntimeLsqOperation::Atomic => "atomic",
        O3RuntimeLsqOperation::FloatLoad => "floatLoad",
        O3RuntimeLsqOperation::FloatStore => "floatStore",
        O3RuntimeLsqOperation::VectorLoad => "vectorLoad",
        O3RuntimeLsqOperation::VectorStore => "vectorStore",
    }
}

pub(super) fn o3_lsq_ordering_alias(ordering: O3RuntimeLsqOrdering) -> &'static str {
    match ordering {
        O3RuntimeLsqOrdering::None => "none",
        O3RuntimeLsqOrdering::Acquire => "acquire",
        O3RuntimeLsqOrdering::Release => "release",
        O3RuntimeLsqOrdering::AcquireRelease => "acquireRelease",
    }
}

pub(super) fn o3_fu_latency_class_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

pub(super) fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let ppm = u128::from(numerator).saturating_mul(1_000_000) / u128::from(denominator);
    ppm.min(u128::from(u64::MAX)) as u64
}
