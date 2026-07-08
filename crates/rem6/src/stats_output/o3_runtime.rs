use std::collections::{BTreeMap, BTreeSet};

use rem6_cpu::{
    BranchTargetKind, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeStats, O3RuntimeTraceRecord,
};
use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{increment_stat, stat_path_segment};
use crate::{Rem6CliError, Rem6CoreSummary};

const EXECUTION_MODE_STAT_LANES: [&str; 3] = ["functional", "timing", "detailed"];

fn emit_o3_branch_event_aggregate_stats(
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
        (
            "branch_event.mispredictions",
            o3.branch_event_mispredictions(),
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

fn count_o3_event_summary_records<F>(events: &[O3RuntimeTraceRecord], matches: F) -> u64
where
    F: Fn(&O3RuntimeTraceRecord) -> bool,
{
    events.iter().filter(|event| matches(event)).count() as u64
}

fn o3_event_branch_direction_mismatch(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event() && event.branch_predicted_taken() != event.branch_resolved_taken()
}

fn o3_event_branch_targetless_mismatch(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event.branch_predicted_target().is_some()
        && event.branch_resolved_target().is_none()
}

fn o3_event_branch_wrong_target(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && event
            .branch_predicted_target()
            .zip(event.branch_resolved_target())
            .is_some_and(|(predicted, resolved)| predicted != resolved)
}

fn emit_o3_runtime_event_summary_branch_kind_stats<F>(
    stats: &mut StatsRegistry,
    cpu: u32,
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
                "sim.cpu{cpu}.o3.event_summary.{family}.{}",
                kind.canonical_stat_name()
            ),
            value,
        )?;
    }
    Ok(())
}

fn emit_o3_runtime_event_summary_branch_mismatch_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
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
            format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            value,
        )?;
    }

    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_direction_mismatch.kind",
        o3_event_branch_direction_mismatch,
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_direction_mismatch.link_write_kind",
        |event| o3_event_branch_direction_mismatch(event) && event.branch_link_register_write(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_direction_mismatch.without_link_write_kind",
        |event| o3_event_branch_direction_mismatch(event) && !event.branch_link_register_write(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_direction_mismatch.squashed_target_kind",
        |event| {
            o3_event_branch_direction_mismatch(event) && event.branch_squashed_target().is_some()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_direction_mismatch.squashed_target_link_write_kind",
        |event| {
            o3_event_branch_direction_mismatch(event)
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_direction_mismatch.squashed_target_without_link_write_kind",
        |event| {
            o3_event_branch_direction_mismatch(event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.targetless_mismatch_kind",
        o3_event_branch_targetless_mismatch,
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.targetless_mismatch_without_link_write_kind",
        |event| o3_event_branch_targetless_mismatch(event) && !event.branch_link_register_write(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.targetless_mismatch_squashed_target_kind",
        |event| {
            o3_event_branch_targetless_mismatch(event) && event.branch_squashed_target().is_some()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.targetless_mismatch_squashed_target_without_link_write_kind",
        |event| {
            o3_event_branch_targetless_mismatch(event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.wrong_target_kind",
        o3_event_branch_wrong_target,
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.wrong_target_squashed_target_kind",
        |event| o3_event_branch_wrong_target(event) && event.branch_squashed_target().is_some(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.wrong_target_squashed_target_without_link_write_kind",
        |event| {
            o3_event_branch_wrong_target(event)
                && event.branch_squashed_target().is_some()
                && !event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.wrong_target_squashed_target_link_write_kind",
        |event| {
            o3_event_branch_wrong_target(event)
                && event.branch_squashed_target().is_some()
                && event.branch_link_register_write()
        },
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.wrong_target_link_write_kind",
        |event| o3_event_branch_wrong_target(event) && event.branch_link_register_write(),
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_target_mismatch.wrong_target_without_link_write_kind",
        |event| o3_event_branch_wrong_target(event) && !event.branch_link_register_write(),
    )?;
    Ok(())
}

fn emit_o3_runtime_event_summary_window_row_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    row: &str,
    event: Option<&O3RuntimeTraceRecord>,
) -> Result<(), Rem6CliError> {
    let sequence = event.map_or(0, |event| event.sequence());
    let tick = event.map_or(0, |event| event.tick());
    let rob_occupancy = event.map_or(0, |event| event.rob_occupancy());
    let lsq_occupancy = event.map_or(0, |event| event.lsq_occupancy());
    let rename_map_entries = event.map_or(0, |event| event.rename_map_entries());
    for (name, unit, value) in [
        ("sequence", "Count", sequence),
        ("tick", "Tick", tick),
        ("rob_occupancy", "Count", rob_occupancy),
        ("lsq_occupancy", "Count", lsq_occupancy),
        ("rename_map_entries", "Count", rename_map_entries),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.event_summary.event_window.{row}.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

fn emit_o3_runtime_event_summary_window_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    emit_o3_runtime_event_summary_window_row_stats(stats, cpu, "first", events.first())?;
    emit_o3_runtime_event_summary_window_row_stats(stats, cpu, "last", events.last())?;
    emit_o3_runtime_event_summary_window_row_stats(
        stats,
        cpu,
        "max_rob_occupancy",
        events.iter().max_by_key(|event| event.rob_occupancy()),
    )?;
    emit_o3_runtime_event_summary_window_row_stats(
        stats,
        cpu,
        "max_lsq_occupancy",
        events.iter().max_by_key(|event| event.lsq_occupancy()),
    )?;
    emit_o3_runtime_event_summary_window_row_stats(
        stats,
        cpu,
        "max_rename_map_entries",
        events.iter().max_by_key(|event| event.rename_map_entries()),
    )?;
    Ok(())
}

fn emit_o3_runtime_event_summary_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    if events.is_empty() {
        return Ok(());
    }

    let records = events.len() as u64;
    let first_tick = events.first().map_or(0, |event| event.tick());
    let last_tick = events.last().map_or(0, |event| event.tick());
    let span_ticks = last_tick.saturating_sub(first_tick);
    let rob_allocations = events.iter().filter(|event| event.rob_allocated()).count() as u64;
    let rob_commits = events.iter().filter(|event| event.rob_committed()).count() as u64;
    let rename_writes = events
        .iter()
        .map(|event| event.rename_writes())
        .sum::<u64>();
    let max_rob_occupancy = events
        .iter()
        .map(|event| event.rob_occupancy())
        .max()
        .unwrap_or(0);
    let max_lsq_occupancy = events
        .iter()
        .map(|event| event.lsq_occupancy())
        .max()
        .unwrap_or(0);
    let max_rename_map_entries = events
        .iter()
        .map(|event| event.rename_map_entries())
        .max()
        .unwrap_or(0);
    let mut lsq_latency_samples = 0_u64;
    let mut lsq_latency_ticks = 0_u64;
    let mut lsq_latency_max_ticks = 0_u64;
    let mut lsq_latency_min_ticks: Option<u64> = None;
    for event in events
        .iter()
        .filter(|event| event.lsq_operation() != O3RuntimeLsqOperation::None)
    {
        let ticks = event.lsq_data_latency_ticks();
        lsq_latency_samples = lsq_latency_samples.saturating_add(1);
        lsq_latency_ticks = lsq_latency_ticks.saturating_add(ticks);
        lsq_latency_max_ticks = lsq_latency_max_ticks.max(ticks);
        lsq_latency_min_ticks = Some(lsq_latency_min_ticks.map_or(ticks, |min| min.min(ticks)));
    }
    let lsq_latency_avg_ticks = if lsq_latency_samples == 0 {
        0
    } else {
        lsq_latency_ticks / lsq_latency_samples
    };

    for (name, value) in [
        ("records", records),
        ("max_rob_occupancy", max_rob_occupancy),
        ("max_lsq_occupancy", max_lsq_occupancy),
        ("max_rename_map_entries", max_rename_map_entries),
        (
            "system_events",
            events.iter().filter(|event| event.system_event()).count() as u64,
        ),
        ("rob.allocations", rob_allocations),
        ("rob.commits", rob_commits),
        ("rename.writes", rename_writes),
        ("iq.insts_issued", records),
        (
            "iq.mem_insts_issued",
            events
                .iter()
                .map(|event| event.lsq_loads().saturating_add(event.lsq_stores()))
                .sum::<u64>(),
        ),
        (
            "iq.branch_insts_issued",
            events.iter().filter(|event| event.branch_event()).count() as u64,
        ),
        ("iew.dispatched_insts", records),
        ("iew.insts_to_commit", rob_commits),
        ("iew.writeback_count", records),
        (
            "iew.producer_inst",
            events
                .iter()
                .map(|event| event.iew_dependency_producers())
                .sum::<u64>(),
        ),
        (
            "iew.consumer_inst",
            events
                .iter()
                .map(|event| event.iew_dependency_consumers())
                .sum::<u64>(),
        ),
        (
            "iew.branch_mispredicts",
            events
                .iter()
                .filter(|event| event.branch_mispredicted())
                .count() as u64,
        ),
        (
            "commit.branch_mispredicts",
            events
                .iter()
                .filter(|event| event.branch_mispredicted())
                .count() as u64,
        ),
    ] {
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            value,
        )?;
    }
    for (name, value) in [
        ("first_tick", first_tick),
        ("last_tick", last_tick),
        ("span_ticks", span_ticks),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            "Tick",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    emit_o3_runtime_event_summary_window_stats(stats, cpu, events)?;
    for operation in O3RuntimeLsqOperation::TRACKED {
        let count = events
            .iter()
            .filter(|event| event.lsq_operation() == operation)
            .count() as u64;
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{cpu}.o3.event_summary.lsq_operation.{}",
                operation.as_str()
            ),
            count,
        )?;
    }
    for (name, unit, value) in [
        ("samples", "Count", lsq_latency_samples),
        ("ticks", "Tick", lsq_latency_ticks),
        ("max_ticks", "Tick", lsq_latency_max_ticks),
        ("min_ticks", "Tick", lsq_latency_min_ticks.unwrap_or(0)),
        ("avg_ticks", "Tick", lsq_latency_avg_ticks),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.event_summary.lsq_data_latency.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        let instructions = events
            .iter()
            .filter(|event| event.fu_latency_class() == Some(class))
            .count() as u64;
        let cycles = events
            .iter()
            .filter(|event| event.fu_latency_class() == Some(class))
            .map(|event| event.fu_latency_cycles())
            .sum::<u64>();
        let inst_type = o3_fu_latency_class_inst_type_stem(class);
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.iq.issued_inst_type.{inst_type}"),
            instructions,
        )?;
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.commit.committed_inst_type.{inst_type}"),
            instructions,
        )?;
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{cpu}.o3.event_summary.fu_latency_class.{}.instructions",
                class.stat_stem()
            ),
            instructions,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.cpu{cpu}.o3.event_summary.fu_latency_class.{}.cycles",
                class.stat_stem()
            ),
            "Cycle",
            StatResetPolicy::Monotonic,
            cycles,
        )?;
    }
    for (name, value) in [
        (
            "iq.issued_inst_type.mem_read",
            events.iter().map(|event| event.lsq_loads()).sum::<u64>(),
        ),
        (
            "iq.issued_inst_type.mem_write",
            events.iter().map(|event| event.lsq_stores()).sum::<u64>(),
        ),
        (
            "commit.committed_inst_type.mem_read",
            events.iter().map(|event| event.lsq_loads()).sum::<u64>(),
        ),
        (
            "commit.committed_inst_type.mem_write",
            events.iter().map(|event| event.lsq_stores()).sum::<u64>(),
        ),
    ] {
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            value,
        )?;
    }
    emit_o3_runtime_event_summary_branch_mismatch_stats(stats, cpu, events)?;

    Ok(())
}

fn emit_o3_runtime_snapshot_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
) -> Result<(), Rem6CliError> {
    let snapshot = &core.o3_runtime_snapshot;
    for (name, value) in [
        ("snapshot.rob.count", snapshot.reorder_buffer().len() as u64),
        (
            "snapshot.lsq.count",
            snapshot.load_store_queue().len() as u64,
        ),
        (
            "snapshot.rename_map.count",
            snapshot.rename_map().len() as u64,
        ),
    ] {
        increment_count_stat(stats, format!("sim.cpu{}.o3.{name}", core.cpu), value)?;
    }
    Ok(())
}

#[derive(Default)]
struct O3RestoreComponentStats {
    components: u64,
    chunks: u64,
    payload_bytes: u64,
}

#[derive(Default)]
struct O3RestoreChunkStats {
    chunks: u64,
    payload_bytes: u64,
    payload_checksum_accumulator: u64,
}

fn emit_o3_restore_component_stat_set(
    stats: &mut StatsRegistry,
    prefix: &str,
    component_stats: O3RestoreComponentStats,
) -> Result<(), Rem6CliError> {
    for (suffix, unit, value) in [
        ("components", "Count", component_stats.components),
        ("chunks", "Count", component_stats.chunks),
        ("payload_bytes", "Byte", component_stats.payload_bytes),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

fn emit_o3_restore_chunk_stat_set(
    stats: &mut StatsRegistry,
    prefix: &str,
    chunk_stats: O3RestoreChunkStats,
) -> Result<(), Rem6CliError> {
    for (suffix, unit, value) in [
        ("chunks", "Count", chunk_stats.chunks),
        ("payload_bytes", "Byte", chunk_stats.payload_bytes),
        (
            "payload_checksum_accumulator",
            "Unspecified",
            chunk_stats.payload_checksum_accumulator,
        ),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{suffix}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

fn emit_o3_runtime_checkpoint_restore_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
) -> Result<(), Rem6CliError> {
    let Some(restore) = &core.o3_runtime_checkpoint_restore else {
        return Ok(());
    };
    for (name, unit, value) in [
        ("tick", "Tick", restore.tick),
        ("manifest_tick", "Tick", restore.manifest_tick),
        ("component_count", "Count", restore.component_count),
        ("chunk_count", "Count", restore.chunk_count),
        ("payload_bytes", "Byte", restore.payload_bytes),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{}.o3.checkpoint_restore.{name}", core.cpu),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }

    let authority_prefix = format!(
        "sim.cpu{}.o3.checkpoint_restore.execution_mode_authority",
        core.cpu
    );
    for (name, value) in [
        (
            "manifests",
            if restore.execution_mode_authority_present {
                1
            } else {
                0
            },
        ),
        (
            "cleared_manifests",
            if restore.execution_mode_authority_cleared {
                1
            } else {
                0
            },
        ),
        (
            "decode_errors",
            if restore.execution_mode_authority_decode_error {
                1
            } else {
                0
            },
        ),
        ("targets", restore.execution_modes.len() as u64),
    ] {
        increment_count_stat(stats, format!("{authority_prefix}.{name}"), value)?;
    }

    let mut authority_modes = BTreeMap::<&'static str, u64>::new();
    let mut authority_targets_seen = BTreeSet::<String>::new();
    let mut authority_target_modes = BTreeMap::<(String, &'static str), u64>::new();
    for authority in &restore.execution_modes {
        let target = stat_path_segment(&authority.target);
        authority_targets_seen.insert(target.clone());
        *authority_modes.entry(authority.mode).or_default() += 1;
        *authority_target_modes
            .entry((target, authority.mode))
            .or_default() += 1;
    }
    for mode in EXECUTION_MODE_STAT_LANES {
        increment_count_stat(
            stats,
            format!("{authority_prefix}.mode.{mode}"),
            authority_modes.get(mode).copied().unwrap_or_default(),
        )?;
    }
    for target in authority_targets_seen {
        for mode in EXECUTION_MODE_STAT_LANES {
            increment_count_stat(
                stats,
                format!("{authority_prefix}.target.{target}.mode.{mode}"),
                authority_target_modes
                    .get(&(target.clone(), mode))
                    .copied()
                    .unwrap_or_default(),
            )?;
        }
    }
    for ((target, mode), count) in authority_target_modes {
        if EXECUTION_MODE_STAT_LANES.contains(&mode) {
            continue;
        }
        increment_count_stat(
            stats,
            format!("{authority_prefix}.target.{target}.mode.{mode}"),
            count,
        )?;
    }

    let restore_targets = restore
        .execution_modes
        .iter()
        .map(|mode| stat_path_segment(&mode.target))
        .collect::<Vec<_>>();
    let mut component_stats = BTreeMap::<String, O3RestoreComponentStats>::new();
    let mut chunk_stats = BTreeMap::<(String, String), O3RestoreChunkStats>::new();
    let mut target_stats = BTreeMap::<String, O3RestoreComponentStats>::new();
    let mut target_component_stats = BTreeMap::<(String, String), O3RestoreComponentStats>::new();
    let mut target_chunk_stats = BTreeMap::<(String, String, String), O3RestoreChunkStats>::new();
    for component in &restore.components {
        let component_path = stat_path_segment(&component.component);
        let component_entry = component_stats.entry(component_path.clone()).or_default();
        component_entry.components = component_entry.components.saturating_add(1);
        component_entry.chunks = component_entry.chunks.saturating_add(component.chunk_count);
        component_entry.payload_bytes = component_entry
            .payload_bytes
            .saturating_add(component.payload_bytes);
        let is_target_component = restore_targets
            .iter()
            .any(|target| target.as_str() == component_path.as_str());
        if is_target_component {
            let target_entry = target_stats.entry(component_path.clone()).or_default();
            target_entry.components = target_entry.components.saturating_add(1);
            target_entry.chunks = target_entry.chunks.saturating_add(component.chunk_count);
            target_entry.payload_bytes = target_entry
                .payload_bytes
                .saturating_add(component.payload_bytes);
            let target_component_entry = target_component_stats
                .entry((component_path.clone(), component_path.clone()))
                .or_default();
            target_component_entry.components = target_component_entry.components.saturating_add(1);
            target_component_entry.chunks = target_component_entry
                .chunks
                .saturating_add(component.chunk_count);
            target_component_entry.payload_bytes = target_component_entry
                .payload_bytes
                .saturating_add(component.payload_bytes);
        }
        for chunk in &component.chunks {
            let chunk_path = stat_path_segment(&chunk.name);
            let chunk_entry = chunk_stats
                .entry((component_path.clone(), chunk_path.clone()))
                .or_default();
            chunk_entry.chunks = chunk_entry.chunks.saturating_add(1);
            chunk_entry.payload_bytes = chunk_entry
                .payload_bytes
                .saturating_add(chunk.payload_bytes);
            chunk_entry.payload_checksum_accumulator = chunk_entry
                .payload_checksum_accumulator
                .wrapping_add(chunk.payload_checksum);
            if is_target_component {
                let target_chunk_entry = target_chunk_stats
                    .entry((component_path.clone(), component_path.clone(), chunk_path))
                    .or_default();
                target_chunk_entry.chunks = target_chunk_entry.chunks.saturating_add(1);
                target_chunk_entry.payload_bytes = target_chunk_entry
                    .payload_bytes
                    .saturating_add(chunk.payload_bytes);
                target_chunk_entry.payload_checksum_accumulator = target_chunk_entry
                    .payload_checksum_accumulator
                    .wrapping_add(chunk.payload_checksum);
            }
        }
    }
    for (component_path, component_stats) in component_stats {
        emit_o3_restore_component_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.component.{component_path}",
                core.cpu
            ),
            component_stats,
        )?;
    }
    for ((component_path, chunk_path), chunk_stats) in chunk_stats {
        emit_o3_restore_chunk_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.component.{component_path}.chunk.{chunk_path}",
                core.cpu
            ),
            chunk_stats,
        )?;
    }
    for (target_path, target_stats) in target_stats {
        emit_o3_restore_component_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.target.{target_path}",
                core.cpu
            ),
            target_stats,
        )?;
    }
    for ((target_path, component_path), component_stats) in target_component_stats {
        emit_o3_restore_component_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.target.{target_path}.component.{component_path}",
                core.cpu
            ),
            component_stats,
        )?;
    }
    for ((target_path, component_path, chunk_path), chunk_stats) in target_chunk_stats {
        emit_o3_restore_chunk_stat_set(
            stats,
            &format!(
                "sim.cpu{}.o3.checkpoint_restore.target.{target_path}.component.{component_path}.chunk.{chunk_path}",
                core.cpu
            ),
            chunk_stats,
        )?;
    }
    Ok(())
}

pub(super) fn emit_o3_runtime_stats(
    stats: &mut StatsRegistry,
    core: &Rem6CoreSummary,
    gem5_cpu_alias_prefix: &str,
) -> Result<(), Rem6CliError> {
    let o3 = core.o3_runtime;
    if !o3.has_activity() {
        return Ok(());
    }

    increment_count_stat(
        stats,
        format!("sim.cpu{}.o3.stats_epoch", core.cpu),
        core.o3_runtime_stats_epoch,
    )?;
    increment_stat(
        stats,
        &format!("sim.cpu{}.o3.stats_reset_tick", core.cpu),
        "Tick",
        StatResetPolicy::Monotonic,
        core.o3_runtime_stats_reset_tick,
    )?;
    for mode in EXECUTION_MODE_STAT_LANES {
        increment_count_stat(
            stats,
            format!("sim.cpu{}.o3.execution_mode.{mode}", core.cpu),
            if core.o3_runtime_execution_mode == Some(mode) {
                1
            } else {
                0
            },
        )?;
    }
    if let Some(mode) = core.o3_runtime_execution_mode {
        if !EXECUTION_MODE_STAT_LANES.contains(&mode) {
            increment_count_stat(
                stats,
                format!(
                    "sim.cpu{}.o3.execution_mode.{}",
                    core.cpu,
                    stat_path_segment(mode)
                ),
                1,
            )?;
        }
    }

    for (name, value) in [
        ("instructions", o3.instructions()),
        ("rob_allocations", o3.rob_allocations()),
        ("rob_commits", o3.rob_commits()),
        ("rename_writes", o3.rename_writes()),
        ("lsq_loads", o3.lsq_loads()),
        ("lsq_stores", o3.lsq_stores()),
        (
            "lsq_store_to_load_forwarding_candidates",
            o3.lsq_store_to_load_forwarding_candidates(),
        ),
        (
            "lsq_store_to_load_forwarding_matches",
            o3.lsq_store_to_load_forwarding_matches(),
        ),
        (
            "lsq_store_to_load_forwarding_suppressed",
            o3.lsq_store_to_load_forwarding_suppressed(),
        ),
        (
            "lsq_store_to_load_forwarding_address_mismatches",
            o3.lsq_store_to_load_forwarding_address_mismatches(),
        ),
        (
            "lsq_store_to_load_forwarding_byte_mismatches",
            o3.lsq_store_to_load_forwarding_byte_mismatches(),
        ),
        (
            "branch_repair_targetless_mismatches",
            o3.branch_repair_targetless_mismatches(),
        ),
        (
            "branch_repair_wrong_targets",
            o3.branch_repair_wrong_targets(),
        ),
        (
            "branch_repair_direction_only_mismatches",
            o3.branch_repair_direction_only_mismatches(),
        ),
        ("fu_latency_instructions", o3.fu_latency_instructions()),
        ("max_rob_occupancy", o3.max_rob_occupancy()),
        ("max_lsq_occupancy", o3.max_lsq_occupancy()),
        ("rename_map_entries", o3.rename_map_entries()),
    ] {
        increment_count_stat(stats, format!("sim.cpu{}.o3.{name}", core.cpu), value)?;
    }
    emit_o3_runtime_snapshot_stats(stats, core)?;
    emit_o3_runtime_checkpoint_restore_stats(stats, core)?;
    emit_o3_runtime_event_summary_stats(stats, core.cpu, &core.o3_runtime_trace_records)?;
    emit_o3_branch_event_aggregate_stats(stats, core.cpu, o3)?;
    for kind in BranchTargetKind::ALL {
        let branch_event_resolved = o3.branch_event_resolved_target_kind(kind);
        let branch_event_link = o3.branch_event_link_write_kind(kind);
        let branch_event_no_link = o3.branch_event_without_link_write_kind(kind);
        let branch_event_squash = o3.branch_event_squash_kind(kind);
        let branch_event_squashed = o3.branch_event_squashed_target_kind(kind);
        let branch_event_squashed_link = o3.branch_event_squashed_target_link_write_kind(kind);
        let branch_event_squashed_no_link =
            o3.branch_event_squashed_target_without_link_write_kind(kind);
        for (name, value) in [
            ("branch_event.kind", o3.branch_event_kind(kind)),
            ("branch_event.taken_kind", o3.branch_event_taken_kind(kind)),
            (
                "branch_event.not_taken_kind",
                o3.branch_event_not_taken_kind(kind),
            ),
            (
                "branch_event.predicted_taken_kind",
                o3.branch_event_predicted_taken_kind(kind),
            ),
            (
                "branch_event.predicted_not_taken_kind",
                o3.branch_event_predicted_not_taken_kind(kind),
            ),
            (
                "branch_event.predicted_target_kind",
                o3.branch_event_predicted_target_kind(kind),
            ),
            (
                "branch_event.predicted_target_match_kind",
                o3.branch_event_predicted_target_match_kind(kind),
            ),
            (
                "branch_event.predicted_target_mismatch_kind",
                o3.branch_event_predicted_target_mismatch_kind(kind),
            ),
            ("branch_event.resolved_target_kind", branch_event_resolved),
            (
                "branch_event.misprediction_kind",
                o3.branch_event_misprediction_kind(kind),
            ),
            ("branch_event.link_write_kind", branch_event_link),
            ("branch_event.without_link_write_kind", branch_event_no_link),
            ("branch_event.squash_kind", branch_event_squash),
            ("branch_event.squashed_target_kind", branch_event_squashed),
            (
                "branch_event.squashed_target_link_write_kind",
                branch_event_squashed_link,
            ),
            (
                "branch_event.squashed_target_without_link_write_kind",
                branch_event_squashed_no_link,
            ),
            (
                "branch_repair_targetless_mismatch_kind",
                o3.branch_repair_targetless_mismatch_kind(kind),
            ),
            (
                "branch_repair_wrong_target_kind",
                o3.branch_repair_wrong_target_kind(kind),
            ),
            (
                "branch_repair_direction_only_kind",
                o3.branch_repair_direction_only_kind(kind),
            ),
        ] {
            increment_count_stat(
                stats,
                format!(
                    "sim.cpu{}.o3.{name}.{}",
                    core.cpu,
                    kind.canonical_stat_name()
                ),
                value,
            )?;
        }
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{}.o3.fu_{}_instructions",
                core.cpu,
                class.stat_stem()
            ),
            o3.fu_latency_class_instructions(class),
        )?;
    }
    for (name, unit, value) in [
        (
            "lsq_data_latency_samples",
            "Count",
            o3.lsq_data_latency_samples(),
        ),
        (
            "lsq_data_latency_ticks",
            "Tick",
            o3.lsq_data_latency_ticks(),
        ),
        (
            "lsq_data_latency_max_ticks",
            "Tick",
            o3.lsq_data_latency_max_ticks(),
        ),
        (
            "lsq_data_latency_min_ticks",
            "Tick",
            o3.lsq_data_latency_min_ticks(),
        ),
        (
            "lsq_data_latency_avg_ticks",
            "Tick",
            o3.lsq_data_latency_avg_ticks(),
        ),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{}.o3.{name}", core.cpu),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{}.o3.lsq_operation.{}",
                core.cpu,
                operation.as_str()
            ),
            o3.lsq_operation_count(operation),
        )?;
        for (suffix, value) in [
            (
                "forwarding_candidates",
                o3.lsq_operation_forwarding_candidates(operation),
            ),
            (
                "forwarding_matches",
                o3.lsq_operation_forwarding_matches(operation),
            ),
            (
                "forwarding_suppressed",
                o3.lsq_operation_forwarding_suppressed(operation),
            ),
            (
                "forwarding_address_mismatches",
                o3.lsq_operation_forwarding_address_mismatches(operation),
            ),
            (
                "forwarding_byte_mismatches",
                o3.lsq_operation_forwarding_byte_mismatches(operation),
            ),
        ] {
            increment_count_stat(
                stats,
                format!(
                    "sim.cpu{}.o3.lsq_operation.{}_{}",
                    core.cpu,
                    operation.as_str(),
                    suffix
                ),
                value,
            )?;
        }
        for (suffix, unit, value) in [
            (
                "latency_samples",
                "Count",
                o3.lsq_operation_latency_samples(operation),
            ),
            (
                "latency_ticks",
                "Tick",
                o3.lsq_operation_latency_ticks(operation),
            ),
            (
                "latency_max_ticks",
                "Tick",
                o3.lsq_operation_latency_max_ticks(operation),
            ),
            (
                "latency_min_ticks",
                "Tick",
                o3.lsq_operation_latency_min_ticks(operation),
            ),
            (
                "latency_avg_ticks",
                "Tick",
                o3.lsq_operation_latency_avg_ticks(operation),
            ),
        ] {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.o3.lsq_operation.{}_{}",
                    core.cpu,
                    operation.as_str(),
                    suffix
                ),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        increment_count_stat(
            stats,
            format!("sim.cpu{}.o3.lsq_ordering.{}", core.cpu, ordering.as_str()),
            o3.lsq_ordering_count(ordering),
        )?;
    }
    increment_count_stat(
        stats,
        format!("sim.cpu{}.o3.lsq_store_conditional_failures", core.cpu),
        o3.lsq_store_conditional_failures(),
    )?;
    increment_stat(
        stats,
        &format!("sim.cpu{}.o3.fu_latency_cycles", core.cpu),
        "Cycle",
        StatResetPolicy::Monotonic,
        o3.fu_latency_cycles(),
    )?;
    for class in O3RuntimeFuLatencyClass::ALL {
        increment_stat(
            stats,
            &format!(
                "sim.cpu{}.o3.fu_{}_latency_cycles",
                core.cpu,
                class.stat_stem()
            ),
            "Cycle",
            StatResetPolicy::Monotonic,
            o3.fu_latency_class_cycles(class),
        )?;
    }
    for (name, value) in [
        ("insts_issued", o3.instructions()),
        (
            "mem_insts_issued",
            o3.lsq_loads().saturating_add(o3.lsq_stores()),
        ),
        ("branch_insts_issued", o3.iq_branch_insts_issued()),
        ("issued_inst_type.mem_read", o3.lsq_loads()),
        ("issued_inst_type.mem_write", o3.lsq_stores()),
    ] {
        increment_count_stat(stats, format!("sim.cpu{}.o3.iq.{name}", core.cpu), value)?;
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{}.o3.iq.issued_inst_type.{}",
                core.cpu,
                o3_fu_latency_class_inst_type_stem(class)
            ),
            o3.fu_latency_class_instructions(class),
        )?;
    }
    for (name, value) in [("mem_read", o3.lsq_loads()), ("mem_write", o3.lsq_stores())] {
        increment_count_stat(
            stats,
            format!("sim.cpu{}.o3.commit.committed_inst_type.{name}", core.cpu),
            value,
        )?;
    }
    for class in O3RuntimeFuLatencyClass::ALL {
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{}.o3.commit.committed_inst_type.{}",
                core.cpu,
                o3_fu_latency_class_inst_type_stem(class)
            ),
            o3.fu_latency_class_instructions(class),
        )?;
    }
    let writeback_count = o3.instructions();
    for (name, value) in [
        ("dispatched_insts", o3.instructions()),
        ("insts_to_commit", o3.rob_commits()),
        ("writeback_count", writeback_count),
        ("producer_inst", o3.iew_producer_insts()),
        ("consumer_inst", o3.iew_consumer_insts()),
        (
            "predicted_taken_incorrect",
            o3.iew_predicted_taken_incorrect(),
        ),
        (
            "predicted_not_taken_incorrect",
            o3.iew_predicted_not_taken_incorrect(),
        ),
    ] {
        increment_count_stat(stats, format!("sim.cpu{}.o3.iew.{name}", core.cpu), value)?;
    }
    let iew_branch_mispredicts = o3
        .iew_predicted_taken_incorrect()
        .saturating_add(o3.iew_predicted_not_taken_incorrect());
    for name in ["iew.branch_mispredicts", "commit.branch_mispredicts"] {
        increment_count_stat(
            stats,
            format!("sim.cpu{}.o3.{name}", core.cpu),
            iew_branch_mispredicts,
        )?;
    }
    for (name, value) in [
        ("lsq_load_bytes", o3.lsq_load_bytes()),
        ("lsq_store_bytes", o3.lsq_store_bytes()),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{}.o3.{name}", core.cpu),
            "Byte",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for (name, value) in [
        ("rob.writes", o3.rob_allocations()),
        ("rob.reads", o3.rob_commits()),
        ("rob.maxOccupancy", o3.max_rob_occupancy()),
        ("rename.renamedInsts", o3.instructions()),
        ("rename.renamedOperands", o3.rename_writes()),
        ("iew.dispatchedInsts", o3.instructions()),
        ("iew.dispLoadInsts", o3.lsq_loads()),
        ("iew.dispStoreInsts", o3.lsq_stores()),
        (
            "iew.predictedTakenIncorrect",
            o3.iew_predicted_taken_incorrect(),
        ),
        (
            "iew.predictedNotTakenIncorrect",
            o3.iew_predicted_not_taken_incorrect(),
        ),
        ("ftq.squashes", o3.branch_event_squashes()),
        ("ftq.squashedTargets", o3.branch_event_squashed_targets()),
        (
            "ftq.squashedTargetsWithLinkWrites",
            o3.branch_event_squashed_targets_with_link_writes(),
        ),
        (
            "ftq.squashedTargetsWithoutLinkWrites",
            o3.branch_event_squashed_targets_without_link_writes(),
        ),
        (
            "lsq0.addedLoadsAndStores",
            o3.lsq_loads().saturating_add(o3.lsq_stores()),
        ),
        (
            "lsq0.storeLoadForwardingCandidates",
            o3.lsq_store_to_load_forwarding_candidates(),
        ),
        (
            "lsq0.storeLoadForwardingMatches",
            o3.lsq_store_to_load_forwarding_matches(),
        ),
        (
            "lsq0.storeLoadForwardingSuppressed",
            o3.lsq_store_to_load_forwarding_suppressed(),
        ),
        (
            "lsq0.storeLoadForwardingAddressMismatches",
            o3.lsq_store_to_load_forwarding_address_mismatches(),
        ),
        (
            "lsq0.storeLoadForwardingByteMismatches",
            o3.lsq_store_to_load_forwarding_byte_mismatches(),
        ),
        ("lsq0.forwLoads", o3.lsq_store_to_load_forwarding_matches()),
        ("lsq0.maxOccupancy", o3.max_lsq_occupancy()),
        ("iq.instsIssued", o3.instructions()),
        (
            "iq.memInstsIssued",
            o3.lsq_loads().saturating_add(o3.lsq_stores()),
        ),
        ("iq.branchInstsIssued", o3.iq_branch_insts_issued()),
    ] {
        increment_count_stat(stats, format!("{gem5_cpu_alias_prefix}.{name}"), value)?;
    }
    for (name, value) in [
        ("lsq0.loadBytes", o3.lsq_load_bytes()),
        ("lsq0.storeBytes", o3.lsq_store_bytes()),
    ] {
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.{name}"),
            "Byte",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    let mut lsq_operation_total = 0_u64;
    for operation in O3RuntimeLsqOperation::TRACKED {
        let value = o3.lsq_operation_count(operation);
        lsq_operation_total = lsq_operation_total.saturating_add(value);
        increment_count_stat(
            stats,
            format!(
                "{gem5_cpu_alias_prefix}.lsq0.operation.{}",
                o3_lsq_operation_alias(operation)
            ),
            value,
        )?;
        let operation_alias = o3_lsq_operation_alias(operation);
        for (name, value) in [
            (
                "storeLoadForwardingCandidates",
                o3.lsq_operation_forwarding_candidates(operation),
            ),
            (
                "storeLoadForwardingMatches",
                o3.lsq_operation_forwarding_matches(operation),
            ),
            (
                "storeLoadForwardingSuppressed",
                o3.lsq_operation_forwarding_suppressed(operation),
            ),
            (
                "storeLoadForwardingAddressMismatches",
                o3.lsq_operation_forwarding_address_mismatches(operation),
            ),
            (
                "storeLoadForwardingByteMismatches",
                o3.lsq_operation_forwarding_byte_mismatches(operation),
            ),
        ] {
            increment_count_stat(
                stats,
                format!("{gem5_cpu_alias_prefix}.lsq0.operation.{operation_alias}.{name}"),
                value,
            )?;
        }
    }
    increment_count_stat(
        stats,
        format!("{gem5_cpu_alias_prefix}.lsq0.operation.total"),
        lsq_operation_total,
    )?;
    let mut lsq_ordering_total = 0_u64;
    for ordering in O3RuntimeLsqOrdering::TRACKED {
        let value = o3.lsq_ordering_count(ordering);
        lsq_ordering_total = lsq_ordering_total.saturating_add(value);
        increment_count_stat(
            stats,
            format!(
                "{gem5_cpu_alias_prefix}.lsq0.ordering.{}",
                o3_lsq_ordering_alias(ordering)
            ),
            value,
        )?;
    }
    increment_count_stat(
        stats,
        format!("{gem5_cpu_alias_prefix}.lsq0.ordering.total"),
        lsq_ordering_total,
    )?;
    for (name, unit, value) in [
        ("samples", "Count", o3.lsq_data_latency_samples()),
        ("totalLatency", "Tick", o3.lsq_data_latency_ticks()),
        ("maxLatency", "Tick", o3.lsq_data_latency_max_ticks()),
        ("minLatency", "Tick", o3.lsq_data_latency_min_ticks()),
        ("avgLatency", "Tick", o3.lsq_data_latency_avg_ticks()),
    ] {
        increment_stat(
            stats,
            &format!("{gem5_cpu_alias_prefix}.lsq0.dataResponse.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for operation in O3RuntimeLsqOperation::TRACKED {
        let operation_alias = o3_lsq_operation_alias(operation);
        for (name, unit, value) in [
            (
                "samples",
                "Count",
                o3.lsq_operation_latency_samples(operation),
            ),
            (
                "totalLatency",
                "Tick",
                o3.lsq_operation_latency_ticks(operation),
            ),
            (
                "maxLatency",
                "Tick",
                o3.lsq_operation_latency_max_ticks(operation),
            ),
            (
                "minLatency",
                "Tick",
                o3.lsq_operation_latency_min_ticks(operation),
            ),
            (
                "avgLatency",
                "Tick",
                o3.lsq_operation_latency_avg_ticks(operation),
            ),
        ] {
            increment_stat(
                stats,
                &format!("{gem5_cpu_alias_prefix}.lsq0.dataResponse.{operation_alias}.{name}"),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    for (name, numerator, denominator) in [
        (
            "iew.writeback_rate_ppm",
            writeback_count,
            core.in_order_pipeline_cycles,
        ),
        (
            "iew.producer_consumer_fanout_ppm",
            o3.iew_producer_insts(),
            o3.iew_consumer_insts(),
        ),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{}.o3.{name}", core.cpu),
            "Ppm",
            StatResetPolicy::Monotonic,
            ratio_ppm(numerator, denominator),
        )?;
    }
    let branch_mispredicts = o3
        .branch_repair_targetless_mismatches()
        .saturating_add(o3.branch_repair_wrong_targets())
        .saturating_add(o3.branch_repair_direction_only_mismatches());
    for (name, value) in [
        (
            "iew.branchRepair.targetlessMismatch",
            o3.branch_repair_targetless_mismatches(),
        ),
        (
            "iew.branchRepair.wrongTarget",
            o3.branch_repair_wrong_targets(),
        ),
        (
            "iew.branchRepair.directionOnly",
            o3.branch_repair_direction_only_mismatches(),
        ),
        ("iew.branchRepair.total", branch_mispredicts),
    ] {
        increment_count_stat(stats, format!("{gem5_cpu_alias_prefix}.{name}"), value)?;
    }
    for name in ["iew.branchMispredicts", "commit.branchMispredicts"] {
        increment_count_stat(
            stats,
            format!("{gem5_cpu_alias_prefix}.{name}"),
            branch_mispredicts,
        )?;
    }
    Ok(())
}

fn increment_count_stat(
    stats: &mut StatsRegistry,
    name: String,
    value: u64,
) -> Result<(), Rem6CliError> {
    increment_stat(stats, &name, "Count", StatResetPolicy::Monotonic, value)
}

fn o3_lsq_operation_alias(operation: O3RuntimeLsqOperation) -> &'static str {
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

fn o3_lsq_ordering_alias(ordering: O3RuntimeLsqOrdering) -> &'static str {
    match ordering {
        O3RuntimeLsqOrdering::None => "none",
        O3RuntimeLsqOrdering::Acquire => "acquire",
        O3RuntimeLsqOrdering::Release => "release",
        O3RuntimeLsqOrdering::AcquireRelease => "acquireRelease",
    }
}

fn o3_fu_latency_class_inst_type_stem(class: O3RuntimeFuLatencyClass) -> &'static str {
    match class {
        O3RuntimeFuLatencyClass::ScalarIntegerMul => "int_mul",
        O3RuntimeFuLatencyClass::ScalarIntegerDiv => "int_div",
        _ => class.stat_stem(),
    }
}

fn ratio_ppm(numerator: u64, denominator: u64) -> u64 {
    if denominator == 0 {
        return 0;
    }
    let ppm = u128::from(numerator).saturating_mul(1_000_000) / u128::from(denominator);
    ppm.min(u128::from(u64::MAX)) as u64
}
