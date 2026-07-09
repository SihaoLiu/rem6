use rem6_cpu::{
    BranchTargetKind, O3RuntimeFuLatencyClass, O3RuntimeLsqOperation, O3RuntimeLsqOrdering,
    O3RuntimeStats, O3RuntimeTraceRecord,
};
use rem6_stats::{StatResetPolicy, StatsRegistry};

#[path = "o3_runtime_snapshot_restore.rs"]
mod o3_runtime_snapshot_restore;

#[path = "o3_runtime_event_summary_branch.rs"]
mod o3_runtime_event_summary_branch;

#[path = "o3_runtime_branch_mismatch.rs"]
mod o3_runtime_branch_mismatch;

#[path = "o3_runtime_gem5_lsq.rs"]
mod o3_runtime_gem5_lsq;

use super::{increment_stat, stat_path_segment};
use crate::{Rem6CliError, Rem6CoreSummary};
use o3_runtime_branch_mismatch::{
    emit_o3_runtime_branch_mismatch_stats, emit_o3_runtime_event_summary_branch_mismatch_stats,
};
use o3_runtime_event_summary_branch::emit_o3_runtime_event_summary_branch_event_stats;
use o3_runtime_gem5_lsq::emit_gem5_o3_lsq_alias_stats;
use o3_runtime_snapshot_restore::{
    emit_o3_runtime_checkpoint_restore_stats, emit_o3_runtime_snapshot_stats,
};

const EXECUTION_MODE_STAT_LANES: [&str; 3] = ["functional", "timing", "detailed"];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct O3EventSummaryFuLatencyStats {
    instructions: u64,
    cycles: u64,
    max_cycles: u64,
    min_cycles: u64,
}

impl O3EventSummaryFuLatencyStats {
    fn add(&mut self, cycles: u64) {
        self.instructions = self.instructions.saturating_add(1);
        self.cycles = self.cycles.saturating_add(cycles);
        self.max_cycles = self.max_cycles.max(cycles);
        if self.min_cycles == 0 || cycles < self.min_cycles {
            self.min_cycles = cycles;
        }
    }

    const fn avg_cycles(self) -> u64 {
        if self.instructions == 0 {
            0
        } else {
            self.cycles / self.instructions
        }
    }
}

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

fn o3_event_branch_repair_direction_only(event: &O3RuntimeTraceRecord) -> bool {
    event.branch_event()
        && !o3_event_branch_targetless_mismatch(event)
        && !o3_event_branch_wrong_target(event)
        && event.branch_predicted_taken() != event.branch_resolved_taken()
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

fn emit_o3_runtime_event_summary_branch_repair_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    for (name, value) in [
        (
            "branch_repair.targetless_mismatches",
            count_o3_event_summary_records(events, o3_event_branch_targetless_mismatch),
        ),
        (
            "branch_repair.wrong_targets",
            count_o3_event_summary_records(events, o3_event_branch_wrong_target),
        ),
        (
            "branch_repair.direction_only_mismatches",
            count_o3_event_summary_records(events, o3_event_branch_repair_direction_only),
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
        "branch_repair.targetless_mismatch_kind",
        o3_event_branch_targetless_mismatch,
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_repair.wrong_target_kind",
        o3_event_branch_wrong_target,
    )?;
    emit_o3_runtime_event_summary_branch_kind_stats(
        stats,
        cpu,
        events,
        "branch_repair.direction_only_kind",
        o3_event_branch_repair_direction_only,
    )?;
    Ok(())
}

fn emit_o3_runtime_window_row_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    family: &str,
    row: &str,
    event: Option<&O3RuntimeTraceRecord>,
) -> Result<(), Rem6CliError> {
    let sequence = event.map_or(0, |event| event.sequence());
    let tick = event.map_or(0, |event| event.tick());
    let issue_tick = event.map_or(0, |event| event.issue_tick());
    let writeback_tick = event.map_or(0, |event| event.writeback_tick());
    let commit_tick = event.map_or(0, |event| event.commit_tick());
    let pc = event.map_or(0, |event| event.pc().get());
    let rob_occupancy = event.map_or(0, |event| event.rob_occupancy());
    let lsq_occupancy = event.map_or(0, |event| event.lsq_occupancy());
    let rename_map_entries = event.map_or(0, |event| event.rename_map_entries());
    let lsq_data_latency_ticks = event.map_or(0, |event| event.lsq_data_latency_ticks());
    let fu_latency_cycles = event.map_or(0, |event| event.fu_latency_cycles());
    for (name, unit, value) in [
        ("sequence", "Count", sequence),
        ("tick", "Tick", tick),
        ("issue_tick", "Tick", issue_tick),
        ("writeback_tick", "Tick", writeback_tick),
        ("commit_tick", "Tick", commit_tick),
        ("pc", "Address", pc),
        ("rob_occupancy", "Count", rob_occupancy),
        ("lsq_occupancy", "Count", lsq_occupancy),
        ("rename_map_entries", "Count", rename_map_entries),
        ("lsq_data_latency_ticks", "Tick", lsq_data_latency_ticks),
        ("fu_latency_cycles", "Cycle", fu_latency_cycles),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.{family}.{row}.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    Ok(())
}

fn emit_o3_runtime_window_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    family: &str,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    if events.is_empty() {
        return Ok(());
    }
    let records = events.len() as u64;
    let first_tick = events.first().map_or(0, |event| event.tick());
    let last_tick = events.last().map_or(0, |event| event.tick());
    let span_ticks = last_tick.saturating_sub(first_tick);
    for (name, unit, value) in [
        ("records", "Count", records),
        ("span_ticks", "Tick", span_ticks),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.{family}.{name}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    emit_o3_runtime_window_row_stats(stats, cpu, family, "first", events.first())?;
    emit_o3_runtime_window_row_stats(stats, cpu, family, "last", events.last())?;
    emit_o3_runtime_window_row_stats(
        stats,
        cpu,
        family,
        "max_rob_occupancy",
        events.iter().max_by_key(|event| event.rob_occupancy()),
    )?;
    emit_o3_runtime_window_row_stats(
        stats,
        cpu,
        family,
        "max_lsq_occupancy",
        events.iter().max_by_key(|event| event.lsq_occupancy()),
    )?;
    emit_o3_runtime_window_row_stats(
        stats,
        cpu,
        family,
        "max_rename_map_entries",
        events.iter().max_by_key(|event| event.rename_map_entries()),
    )?;
    emit_o3_runtime_window_row_stats(
        stats,
        cpu,
        family,
        "max_structural_pressure",
        events
            .iter()
            .max_by_key(|event| event.structural_pressure_key()),
    )?;
    emit_o3_runtime_window_row_stats(
        stats,
        cpu,
        family,
        "max_lsq_data_latency",
        events
            .iter()
            .max_by_key(|event| event.lsq_data_latency_ticks()),
    )?;
    emit_o3_runtime_window_row_stats(
        stats,
        cpu,
        family,
        "max_fu_latency",
        events.iter().max_by_key(|event| event.fu_latency_cycles()),
    )?;
    Ok(())
}

fn emit_o3_runtime_event_summary_lsq_matrix_stats(
    stats: &mut StatsRegistry,
    cpu: u32,
    events: &[O3RuntimeTraceRecord],
) -> Result<(), Rem6CliError> {
    for (name, value) in [
        (
            "store_load_forwarding_candidates",
            count_o3_event_summary_records(events, |event| event.store_load_forwarding_candidate()),
        ),
        (
            "store_load_forwarding_matches",
            count_o3_event_summary_records(events, |event| event.store_load_forwarding_match()),
        ),
        (
            "store_load_forwarding_suppressed",
            count_o3_event_summary_records(events, |event| {
                event.store_load_forwarding_suppressed()
            }),
        ),
        (
            "store_load_forwarding_address_mismatches",
            count_o3_event_summary_records(events, |event| {
                event.store_load_forwarding_address_mismatch()
            }),
        ),
        (
            "store_load_forwarding_byte_mismatches",
            count_o3_event_summary_records(events, |event| {
                event.store_load_forwarding_byte_mismatch()
            }),
        ),
    ] {
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            value,
        )?;
    }

    for operation in O3RuntimeLsqOperation::TRACKED {
        let mut latency_samples = 0_u64;
        let mut latency_ticks = 0_u64;
        let mut latency_max_ticks = 0_u64;
        let mut latency_min_ticks: Option<u64> = None;
        let mut load_bytes = 0_u64;
        let mut store_bytes = 0_u64;
        for event in events
            .iter()
            .filter(|event| event.lsq_operation() == operation)
        {
            let ticks = event.lsq_data_latency_ticks();
            latency_samples = latency_samples.saturating_add(1);
            latency_ticks = latency_ticks.saturating_add(ticks);
            latency_max_ticks = latency_max_ticks.max(ticks);
            latency_min_ticks = Some(latency_min_ticks.map_or(ticks, |min| min.min(ticks)));
            load_bytes = load_bytes.saturating_add(event.lsq_load_bytes());
            store_bytes = store_bytes.saturating_add(event.lsq_store_bytes());
        }
        let latency_avg_ticks = if latency_samples == 0 {
            0
        } else {
            latency_ticks / latency_samples
        };
        for (name, value) in [
            (
                "store_conditional_failures",
                count_o3_event_summary_records(events, |event| {
                    event.lsq_operation() == operation && event.lsq_store_conditional_failed()
                }),
            ),
            (
                "forwarding_candidates",
                count_o3_event_summary_records(events, |event| {
                    event.lsq_operation() == operation && event.store_load_forwarding_candidate()
                }),
            ),
            (
                "forwarding_matches",
                count_o3_event_summary_records(events, |event| {
                    event.lsq_operation() == operation && event.store_load_forwarding_match()
                }),
            ),
            (
                "forwarding_suppressed",
                count_o3_event_summary_records(events, |event| {
                    event.lsq_operation() == operation && event.store_load_forwarding_suppressed()
                }),
            ),
            (
                "forwarding_address_mismatches",
                count_o3_event_summary_records(events, |event| {
                    event.lsq_operation() == operation
                        && event.store_load_forwarding_address_mismatch()
                }),
            ),
            (
                "forwarding_byte_mismatches",
                count_o3_event_summary_records(events, |event| {
                    event.lsq_operation() == operation
                        && event.store_load_forwarding_byte_mismatch()
                }),
            ),
        ] {
            increment_count_stat(
                stats,
                format!(
                    "sim.cpu{cpu}.o3.event_summary.lsq_operation.{}.{name}",
                    operation.as_str()
                ),
                value,
            )?;
        }
        for (name, value) in [("load_bytes", load_bytes), ("store_bytes", store_bytes)] {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{cpu}.o3.event_summary.lsq_operation.{}.{name}",
                    operation.as_str()
                ),
                "Byte",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        for (name, unit, value) in [
            ("samples", "Count", latency_samples),
            ("ticks", "Tick", latency_ticks),
            ("max_ticks", "Tick", latency_max_ticks),
            ("min_ticks", "Tick", latency_min_ticks.unwrap_or(0)),
            ("avg_ticks", "Tick", latency_avg_ticks),
        ] {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{cpu}.o3.event_summary.lsq_operation.{}.latency.{name}",
                    operation.as_str()
                ),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }

    for ordering in O3RuntimeLsqOrdering::TRACKED {
        let value =
            count_o3_event_summary_records(events, |event| event.lsq_ordering() == ordering);
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{cpu}.o3.event_summary.lsq_ordering.{}",
                ordering.as_str()
            ),
            value,
        )?;
    }
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
    let lsq_loads = events.iter().map(|event| event.lsq_loads()).sum::<u64>();
    let lsq_stores = events.iter().map(|event| event.lsq_stores()).sum::<u64>();
    let lsq_load_bytes = events
        .iter()
        .map(|event| event.lsq_load_bytes())
        .sum::<u64>();
    let lsq_store_bytes = events
        .iter()
        .map(|event| event.lsq_store_bytes())
        .sum::<u64>();
    let lsq_store_conditional_failures = events
        .iter()
        .filter(|event| event.lsq_store_conditional_failed())
        .count() as u64;
    let mem_insts_issued = events
        .iter()
        .map(|event| event.lsq_loads().saturating_add(event.lsq_stores()))
        .sum::<u64>();
    let lsq_operation_load = events
        .iter()
        .filter(|event| event.lsq_operation() == O3RuntimeLsqOperation::Load)
        .count() as u64;
    let lsq_operation_store = events
        .iter()
        .filter(|event| event.lsq_operation() == O3RuntimeLsqOperation::Store)
        .count() as u64;
    let iew_dependency_producers = events
        .iter()
        .map(|event| event.iew_dependency_producers())
        .sum::<u64>();
    let iew_dependency_consumers = events
        .iter()
        .map(|event| event.iew_dependency_consumers())
        .sum::<u64>();
    let iew_predicted_taken_incorrect = events
        .iter()
        .filter(|event| event.branch_mispredicted() && event.branch_predicted_taken())
        .count() as u64;
    let iew_predicted_not_taken_incorrect = events
        .iter()
        .filter(|event| event.branch_mispredicted() && !event.branch_predicted_taken())
        .count() as u64;
    let iew_branch_mispredicts =
        iew_predicted_taken_incorrect.saturating_add(iew_predicted_not_taken_incorrect);
    let max_rob_occupancy = events
        .iter()
        .map(|event| event.rob_occupancy())
        .max()
        .unwrap_or(0);
    let rob_commit_blocked_events = events
        .iter()
        .filter(|event| event.rob_commit_blocked())
        .count() as u64;
    let rob_max_commits_at_tick = events
        .iter()
        .map(|event| event.rob_commits_at_tick())
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
    let (fu_latency, fu_latency_classes) = o3_event_summary_fu_latency_stats(events);

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
        ("rob.max_occupancy", max_rob_occupancy),
        ("rob.commit_blocked_events", rob_commit_blocked_events),
        ("rob.max_commits_at_tick", rob_max_commits_at_tick),
        ("rename.writes", rename_writes),
        ("rename.map_entries", max_rename_map_entries),
        ("rob_allocations", rob_allocations),
        ("rob_commits", rob_commits),
        ("rename_writes", rename_writes),
        ("lsq_loads", lsq_loads),
        ("lsq_stores", lsq_stores),
        (
            "lsq_store_conditional_failures",
            lsq_store_conditional_failures,
        ),
        ("lsq_operation_load", lsq_operation_load),
        ("lsq_operation_store", lsq_operation_store),
        ("iq.insts_issued", records),
        ("iq.mem_insts_issued", mem_insts_issued),
        (
            "iq.branch_insts_issued",
            events.iter().filter(|event| event.branch_event()).count() as u64,
        ),
        ("iew.dispatched_insts", records),
        ("iew.insts_to_commit", rob_commits),
        ("iew.writeback_count", records),
        ("iew.producer_inst", iew_dependency_producers),
        ("iew.consumer_inst", iew_dependency_consumers),
        ("iew.dependency.producer", iew_dependency_producers),
        ("iew.dependency.consumer", iew_dependency_consumers),
        (
            "iew.predicted_taken_incorrect",
            iew_predicted_taken_incorrect,
        ),
        (
            "iew.predicted_not_taken_incorrect",
            iew_predicted_not_taken_incorrect,
        ),
        ("iew.branch_mispredicts", iew_branch_mispredicts),
        ("commit.branch_mispredicts", iew_branch_mispredicts),
    ] {
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            value,
        )?;
    }
    for (name, value) in [
        ("lsq_load_bytes", lsq_load_bytes),
        ("lsq_store_bytes", lsq_store_bytes),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            "Byte",
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    for (name, numerator, denominator) in [
        ("iew.writeback_rate_ppm", records, span_ticks),
        (
            "iew.producer_consumer_fanout_ppm",
            iew_dependency_producers,
            iew_dependency_consumers,
        ),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            "Ppm",
            StatResetPolicy::Monotonic,
            ratio_ppm(numerator, denominator),
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
    emit_o3_runtime_window_stats(stats, cpu, "event_summary.event_window", events)?;
    emit_o3_runtime_event_summary_branch_event_stats(stats, cpu, events)?;
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
    emit_o3_runtime_event_summary_lsq_matrix_stats(stats, cpu, events)?;
    for (name, unit, value) in [
        ("instructions", "Count", fu_latency.instructions),
        ("cycles", "Cycle", fu_latency.cycles),
        ("max_cycles", "Cycle", fu_latency.max_cycles),
        ("min_cycles", "Cycle", fu_latency.min_cycles),
        ("avg_cycles", "Cycle", fu_latency.avg_cycles()),
    ] {
        increment_stat(
            stats,
            &format!("sim.cpu{cpu}.o3.event_summary.fu_latency.{name}"),
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
        let latency = fu_latency_classes[class.index()];
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
            latency.instructions,
        )?;
        for (name, value) in [
            ("cycles", latency.cycles),
            ("max_cycles", latency.max_cycles),
            ("min_cycles", latency.min_cycles),
            ("avg_cycles", latency.avg_cycles()),
        ] {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{cpu}.o3.event_summary.fu_latency_class.{}.{name}",
                    class.stat_stem()
                ),
                "Cycle",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
    }
    for (name, value) in [
        ("iq.issued_inst_type.mem_read", lsq_loads),
        ("iq.issued_inst_type.mem_write", lsq_stores),
        ("commit.committed_inst_type.mem_read", lsq_loads),
        ("commit.committed_inst_type.mem_write", lsq_stores),
    ] {
        increment_count_stat(
            stats,
            format!("sim.cpu{cpu}.o3.event_summary.{name}"),
            value,
        )?;
    }
    emit_o3_runtime_event_summary_branch_repair_stats(stats, cpu, events)?;
    emit_o3_runtime_event_summary_branch_mismatch_stats(stats, cpu, events)?;

    Ok(())
}

fn o3_event_summary_fu_latency_stats(
    events: &[O3RuntimeTraceRecord],
) -> (
    O3EventSummaryFuLatencyStats,
    [O3EventSummaryFuLatencyStats; O3RuntimeFuLatencyClass::COUNT],
) {
    let mut summary = O3EventSummaryFuLatencyStats::default();
    let mut classes = [O3EventSummaryFuLatencyStats::default(); O3RuntimeFuLatencyClass::COUNT];
    for event in events {
        let cycles = event.fu_latency_cycles();
        if cycles == 0 {
            continue;
        }
        summary.add(cycles);
        if let Some(class) = event.fu_latency_class() {
            classes[class.index()].add(cycles);
        }
    }
    (summary, classes)
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
            "branch_repair.targetless_mismatches",
            o3.branch_repair_targetless_mismatches(),
        ),
        (
            "branch_repair_wrong_targets",
            o3.branch_repair_wrong_targets(),
        ),
        (
            "branch_repair.wrong_targets",
            o3.branch_repair_wrong_targets(),
        ),
        (
            "branch_repair_direction_only_mismatches",
            o3.branch_repair_direction_only_mismatches(),
        ),
        (
            "branch_repair.direction_only_mismatches",
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
    emit_o3_runtime_window_stats(
        stats,
        core.cpu,
        "event_window",
        &core.o3_runtime_trace_records,
    )?;
    if core.o3_runtime_stats_epoch != 0
        || o3.branch_direction_mismatches() != 0
        || o3.branch_target_mismatch_targetless_mismatches() != 0
        || o3.branch_target_mismatch_wrong_targets() != 0
    {
        emit_o3_runtime_branch_mismatch_stats(stats, core.cpu, o3)?;
    }
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
                "branch_repair.targetless_mismatch_kind",
                o3.branch_repair_targetless_mismatch_kind(kind),
            ),
            (
                "branch_repair_wrong_target_kind",
                o3.branch_repair_wrong_target_kind(kind),
            ),
            (
                "branch_repair.wrong_target_kind",
                o3.branch_repair_wrong_target_kind(kind),
            ),
            (
                "branch_repair_direction_only_kind",
                o3.branch_repair_direction_only_kind(kind),
            ),
            (
                "branch_repair.direction_only_kind",
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
        let instructions = o3.fu_latency_class_instructions(class);
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{}.o3.fu_{}_instructions",
                core.cpu,
                class.stat_stem()
            ),
            instructions,
        )?;
        increment_count_stat(
            stats,
            format!(
                "sim.cpu{}.o3.fu_latency_class.{}.instructions",
                core.cpu,
                class.stat_stem()
            ),
            instructions,
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
        let operation_name = operation.as_str();
        increment_count_stat(
            stats,
            format!("sim.cpu{}.o3.lsq_operation.{operation_name}", core.cpu),
            o3.lsq_operation_count(operation),
        )?;
        for (suffix, value) in [
            ("load_bytes", o3.lsq_operation_load_bytes(operation)),
            ("store_bytes", o3.lsq_operation_store_bytes(operation)),
        ] {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.o3.lsq_operation.{}_{}",
                    core.cpu, operation_name, suffix
                ),
                "Byte",
                StatResetPolicy::Monotonic,
                value,
            )?;
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.o3.lsq_operation.{operation_name}.{suffix}",
                    core.cpu
                ),
                "Byte",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
        for (suffix, value) in [
            (
                "store_conditional_failures",
                o3.lsq_operation_store_conditional_failures(operation),
            ),
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
                    core.cpu, operation_name, suffix
                ),
                value,
            )?;
            increment_count_stat(
                stats,
                format!(
                    "sim.cpu{}.o3.lsq_operation.{operation_name}.{suffix}",
                    core.cpu
                ),
                value,
            )?;
        }
        for (flat_suffix, nested_suffix, unit, value) in [
            (
                "latency_samples",
                "samples",
                "Count",
                o3.lsq_operation_latency_samples(operation),
            ),
            (
                "latency_ticks",
                "ticks",
                "Tick",
                o3.lsq_operation_latency_ticks(operation),
            ),
            (
                "latency_max_ticks",
                "max_ticks",
                "Tick",
                o3.lsq_operation_latency_max_ticks(operation),
            ),
            (
                "latency_min_ticks",
                "min_ticks",
                "Tick",
                o3.lsq_operation_latency_min_ticks(operation),
            ),
            (
                "latency_avg_ticks",
                "avg_ticks",
                "Tick",
                o3.lsq_operation_latency_avg_ticks(operation),
            ),
        ] {
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.o3.lsq_operation.{}_{}",
                    core.cpu, operation_name, flat_suffix
                ),
                unit,
                StatResetPolicy::Monotonic,
                value,
            )?;
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.o3.lsq_operation.{operation_name}.latency.{nested_suffix}",
                    core.cpu
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
        let cycles = o3.fu_latency_class_cycles(class);
        let stem = class.stat_stem();
        increment_stat(
            stats,
            &format!("sim.cpu{}.o3.fu_{stem}_latency_cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            cycles,
        )?;
        increment_stat(
            stats,
            &format!("sim.cpu{}.o3.fu_latency_class.{stem}.cycles", core.cpu),
            "Cycle",
            StatResetPolicy::Monotonic,
            cycles,
        )?;
        for (flat_suffix, nested_suffix, value) in [
            (
                "latency_max_cycles",
                "max_cycles",
                o3.fu_latency_class_max_cycles(class),
            ),
            (
                "latency_min_cycles",
                "min_cycles",
                o3.fu_latency_class_min_cycles(class),
            ),
            (
                "latency_avg_cycles",
                "avg_cycles",
                o3.fu_latency_class_avg_cycles(class),
            ),
        ] {
            increment_stat(
                stats,
                &format!("sim.cpu{}.o3.fu_{stem}_{flat_suffix}", core.cpu),
                "Cycle",
                StatResetPolicy::Monotonic,
                value,
            )?;
            increment_stat(
                stats,
                &format!(
                    "sim.cpu{}.o3.fu_latency_class.{stem}.{nested_suffix}",
                    core.cpu
                ),
                "Cycle",
                StatResetPolicy::Monotonic,
                value,
            )?;
        }
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
        ("rename.mapEntries", o3.rename_map_entries()),
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
        (
            "lsq0.storeConditionalFailures",
            o3.lsq_store_conditional_failures(),
        ),
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
    emit_gem5_o3_lsq_alias_stats(stats, gem5_cpu_alias_prefix, o3)?;
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
