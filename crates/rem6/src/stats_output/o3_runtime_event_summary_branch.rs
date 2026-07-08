use rem6_cpu::O3RuntimeTraceRecord;
use rem6_stats::StatsRegistry;

use crate::Rem6CliError;

use super::{
    count_o3_event_summary_records, emit_o3_runtime_event_summary_branch_kind_stats,
    increment_count_stat,
};

fn o3_event_branch_predicted_target_match(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) == event.branch_resolved_target())
}

fn o3_event_branch_predicted_target_mismatch(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event
            .branch_predicted_target()
            .is_some_and(|target| Some(target) != event.branch_resolved_target())
}

pub(super) fn emit_o3_runtime_event_summary_branch_event_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    for (name, value) in [
        (
            "branch_event.branches",
            count_o3_event_summary_records(events, |event| event.branch_event()),
        ),
        (
            "branch_event.taken",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_resolved_taken()
            }),
        ),
        (
            "branch_event.not_taken",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && !event.branch_resolved_taken()
            }),
        ),
        (
            "branch_event.predicted_taken",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_predicted_taken()
            }),
        ),
        (
            "branch_event.predicted_not_taken",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && !event.branch_predicted_taken()
            }),
        ),
        (
            "branch_event.predicted_targets",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_predicted_target().is_some()
            }),
        ),
        (
            "branch_event.predicted_target_matches",
            count_o3_event_summary_records(events, o3_event_branch_predicted_target_match),
        ),
        (
            "branch_event.predicted_target_mismatches",
            count_o3_event_summary_records(events, o3_event_branch_predicted_target_mismatch),
        ),
        (
            "branch_event.resolved_targets",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_resolved_target().is_some()
            }),
        ),
        (
            "branch_event.mispredictions",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_mispredicted()
            }),
        ),
        (
            "branch_event.link_writes",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_link_register_write()
            }),
        ),
        (
            "branch_event.without_link_writes",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && !event.branch_link_register_write()
            }),
        ),
        (
            "branch_event.squashes",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_squash()
            }),
        ),
        (
            "branch_event.squashed_targets",
            count_o3_event_summary_records(events, |event| {
                event.branch_event() && event.branch_squashed_target().is_some()
            }),
        ),
        (
            "branch_event.squashed_targets_with_link_writes",
            count_o3_event_summary_records(events, |event| {
                event.branch_event()
                    && event.branch_squashed_target().is_some()
                    && event.branch_link_register_write()
            }),
        ),
        (
            "branch_event.squashed_targets_without_link_writes",
            count_o3_event_summary_records(events, |event| {
                event.branch_event()
                    && event.branch_squashed_target().is_some()
                    && !event.branch_link_register_write()
            }),
        ),
    ] {
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            value,
        )?;
    }
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.kind",
        |event| event.branch_event(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.taken_kind",
        |event| event.branch_event() && event.branch_resolved_taken(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.not_taken_kind",
        |event| event.branch_event() && !event.branch_resolved_taken(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.predicted_taken_kind",
        |event| event.branch_event() && event.branch_predicted_taken(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.predicted_not_taken_kind",
        |event| event.branch_event() && !event.branch_predicted_taken(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.predicted_target_kind",
        |event| event.branch_event() && event.branch_predicted_target().is_some(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.predicted_target_match_kind",
        o3_event_branch_predicted_target_match,
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.predicted_target_mismatch_kind",
        o3_event_branch_predicted_target_mismatch,
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.resolved_target_kind",
        |event| event.branch_event() && event.branch_resolved_target().is_some(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.misprediction_kind",
        |event| event.branch_event() && event.branch_mispredicted(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.link_write_kind",
        |event| event.branch_event() && event.branch_link_register_write(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.without_link_write_kind",
        |event| event.branch_event() && !event.branch_link_register_write(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.squash_kind",
        |event| event.branch_event() && event.branch_squash(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.squashed_target_kind",
        |event| event.branch_event() && event.branch_squashed_target().is_some(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.squashed_target_link_write_kind",
        |event| {
            event.branch_event()
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_event.squashed_target_without_link_write_kind",
        |event| {
            event.branch_event()
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    Ok(())
}
