use rem6_cpu::{BranchTargetKind, O3RuntimeTraceRecord};
use rem6_stats::StatsRegistry;

use super::{
    count_o3_event_summary_records, increment_count_stat, o3_event_branch_direction_mismatch,
    o3_event_branch_targetless_mismatch, o3_event_branch_wrong_target,
};
use crate::Rem6CliError;

fn emit_o3_runtime_branch_kind_stats<F>(
    stats: &mut StatsRegistry,
    cpu: u32,
    family_prefix: &str,
    events: &[O3RuntimeTraceRecord],
    family: &str,
    matches: F,
) -> Result<(), Rem6CliError>
where
    F: Fn(&O3RuntimeTraceRecord) -> bool + Copy,
{
    for kind in BranchTargetKind::ALL {
        let value = count_o3_event_summary_records(events, |event| {
            matches(event) && event.branch_kind() == kind
        });
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{cpu}.o3.{family_prefix}{family}.{}",
                kind.canonical_stat_name()
            ),
            value,
        )?;
    }
    Ok(())
}

fn emit_o3_runtime_branch_mismatch_stats_with_prefix(
    stats: &mut StatsRegistry,
    cpu: u32,
    family_prefix: &str,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    if events.is_empty() {
        return Ok(());
    }

    for (name, value) in [
        (
            "branch_direction_mismatch.mismatches",
            count_o3_event_summary_records(events, o3_event_branch_direction_mismatch),
        ),
        (
            "branch_direction_mismatch.without_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_direction_mismatch(event) && !event.branch_link_register_write()
            }),
        ),
        (
            "branch_direction_mismatch.squashed_targets",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_direction_mismatch(event)
                    && event.branch_squashed_target().is_some()
            }),
        ),
        (
            "branch_direction_mismatch.squashed_target_without_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_direction_mismatch(event)
                    && event.branch_squashed_target().is_some()
                    && !event.branch_link_register_write()
            }),
        ),
        (
            "branch_direction_mismatch.squashed_target_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_direction_mismatch(event)
                    && event.branch_squashed_target().is_some()
                    && event.branch_link_register_write()
            }),
        ),
        (
            "branch_target_mismatch.targetless_mismatches",
            count_o3_event_summary_records(events, o3_event_branch_targetless_mismatch),
        ),
        (
            "branch_target_mismatch.targetless_mismatch_without_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_targetless_mismatch(event) && !event.branch_link_register_write()
            }),
        ),
        (
            "branch_target_mismatch.targetless_mismatch_squashed_targets",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_targetless_mismatch(event)
                    && event.branch_squashed_target().is_some()
            }),
        ),
        (
            "branch_target_mismatch.targetless_mismatch_squashed_target_without_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_targetless_mismatch(event)
                    && event.branch_squashed_target().is_some()
                    && !event.branch_link_register_write()
            }),
        ),
        (
            "branch_target_mismatch.wrong_targets",
            count_o3_event_summary_records(events, o3_event_branch_wrong_target),
        ),
        (
            "branch_target_mismatch.wrong_target_squashed_targets",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_wrong_target(event) && event.branch_squashed_target().is_some()
            }),
        ),
        (
            "branch_target_mismatch.wrong_target_squashed_target_without_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_wrong_target(event)
                    && event.branch_squashed_target().is_some()
                    && !event.branch_link_register_write()
            }),
        ),
        (
            "branch_target_mismatch.wrong_target_squashed_target_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_wrong_target(event)
                    && event.branch_squashed_target().is_some()
                    && event.branch_link_register_write()
            }),
        ),
        (
            "branch_target_mismatch.wrong_target_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_wrong_target(event) && event.branch_link_register_write()
            }),
        ),
        (
            "branch_target_mismatch.wrong_target_without_link_writes",
            count_o3_event_summary_records(events, |event| {
                o3_event_branch_wrong_target(event) && !event.branch_link_register_write()
            }),
        ),
    ] {
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.{family_prefix}{name}"),
            value,
        )?;
    }

    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_direction_mismatch.kind",
        o3_event_branch_direction_mismatch,
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_direction_mismatch.link_write_kind",
        |event| o3_event_branch_direction_mismatch(event) && event.branch_link_register_write(),
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_direction_mismatch.without_link_write_kind",
        |event| o3_event_branch_direction_mismatch(event) && !event.branch_link_register_write(),
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_direction_mismatch.squashed_target_kind",
        |event| {
            o3_event_branch_direction_mismatch(event) && event.branch_squashed_target().is_some()
        },
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_direction_mismatch.squashed_target_link_write_kind",
        |event| {
            o3_event_branch_direction_mismatch(event)
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_direction_mismatch.squashed_target_without_link_write_kind",
        |event| {
            o3_event_branch_direction_mismatch(event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.targetless_mismatch_kind",
        o3_event_branch_targetless_mismatch,
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.targetless_mismatch_without_link_write_kind",
        |event| o3_event_branch_targetless_mismatch(event) && !event.branch_link_register_write(),
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.targetless_mismatch_squashed_target_kind",
        |event| {
            o3_event_branch_targetless_mismatch(event) && event.branch_squashed_target().is_some()
        },
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind",
        |event| {
            o3_event_branch_targetless_mismatch(event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.wrong_target_kind",
        o3_event_branch_wrong_target,
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.wrong_target_squashed_target_kind",
        |event| o3_event_branch_wrong_target(event) && event.branch_squashed_target().is_some(),
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.wrong_target_squashed_target_without_link_write_kind",
        |event| {
            o3_event_branch_wrong_target(event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.wrong_target_squashed_target_link_write_kind",
        |event| {
            o3_event_branch_wrong_target(event)
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.wrong_target_link_write_kind",
        |event| o3_event_branch_wrong_target(event) && event.branch_link_register_write(),
    )?;
    emit_o3_runtime_branch_kind_stats(
        stats,
        cpu,
        family_prefix,
        events,
        "branch_target_mismatch.wrong_target_without_link_write_kind",
        |event| o3_event_branch_wrong_target(event) && !event.branch_link_register_write(),
    )?;
    Ok(())
}

pub(super) fn emit_o3_runtime_branch_mismatch_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    emit_o3_runtime_branch_mismatch_stats_with_prefix(stats, cpu, "", events)
}

pub(super) fn emit_o3_runtime_event_summary_branch_mismatch_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    emit_o3_runtime_branch_mismatch_stats_with_prefix(stats, cpu, "event_summary.", events)
}
