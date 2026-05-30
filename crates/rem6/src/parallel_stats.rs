use std::collections::BTreeMap;

use rem6_stats::{StatResetPolicy, StatsRegistry};

use super::{
    increment_stat, Rem6CliError, Rem6ExecutionSummary, Rem6ParallelFrontierSummary,
    Rem6ParallelReadyPartitionSummary,
};

pub(super) fn emit_scheduler_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6ExecutionSummary,
) -> Result<(), Rem6CliError> {
    increment_stat(
        stats,
        "sim.parallel.scheduler.epochs",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_epochs,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.max_workers",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_max_workers,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.dispatches",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_dispatches,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.batches",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_batches,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.total_workers",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_total_workers,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.active_partitions",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_active_partitions,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.remote_sends",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_remote_sends,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.frontiers",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_frontiers.len() as u64,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.final_frontiers",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_final_frontiers.len() as u64,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.ready_partitions",
        "Count",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_ready_partitions.len() as u64,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.batch.worker_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_batch_worker_ticks,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.batch.worker_capacity_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_batch_worker_capacity_ticks,
    )?;
    increment_stat(
        stats,
        "sim.parallel.scheduler.batch.idle_worker_ticks",
        "Tick",
        StatResetPolicy::Monotonic,
        execution.parallel_scheduler_batch_idle_worker_ticks,
    )?;
    for (index, frontier) in execution.parallel_scheduler_frontiers.iter().enumerate() {
        emit_frontier_record_stats(
            stats,
            &format!("sim.parallel.scheduler.frontier{index}"),
            frontier,
        )?;
    }
    for (index, frontier) in execution
        .parallel_scheduler_final_frontiers
        .iter()
        .enumerate()
    {
        emit_frontier_record_stats(
            stats,
            &format!("sim.parallel.scheduler.final_frontier{index}"),
            frontier,
        )?;
    }
    for (index, ready) in execution
        .parallel_scheduler_ready_partitions
        .iter()
        .enumerate()
    {
        emit_ready_partition_record_stats(stats, index, ready)?;
    }
    emit_partition_frontier_stats(stats, execution)?;
    emit_worker_stats(stats, execution)?;
    emit_partition_stats(stats, execution)?;
    Ok(())
}

fn emit_partition_frontier_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6ExecutionSummary,
) -> Result<(), Rem6CliError> {
    let mut initial_frontiers = BTreeMap::new();
    for frontier in &execution.parallel_scheduler_frontiers {
        initial_frontiers
            .entry(frontier.partition)
            .or_insert(frontier);
    }
    for frontier in initial_frontiers.values() {
        emit_frontier_stats(stats, frontier, false)?;
    }
    let mut final_frontiers = BTreeMap::new();
    for frontier in &execution.parallel_scheduler_final_frontiers {
        final_frontiers.insert(frontier.partition, frontier);
    }
    for frontier in final_frontiers.values() {
        emit_frontier_stats(stats, frontier, true)?;
    }
    Ok(())
}

fn emit_worker_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6ExecutionSummary,
) -> Result<(), Rem6CliError> {
    for slot in &execution.parallel_scheduler_worker_slots {
        increment_stat(
            stats,
            &format!("sim.parallel.scheduler.worker{}.active_ticks", slot.slot),
            "Tick",
            StatResetPolicy::Monotonic,
            slot.active_ticks,
        )?;
        increment_stat(
            stats,
            &format!("sim.parallel.scheduler.worker{}.idle_ticks", slot.slot),
            "Tick",
            StatResetPolicy::Monotonic,
            slot.idle_ticks,
        )?;
    }
    for lane in &execution.parallel_scheduler_worker_lanes {
        increment_stat(
            stats,
            &format!(
                "sim.parallel.scheduler.worker{}.partition{}.active_ticks",
                lane.lane, lane.partition
            ),
            "Tick",
            StatResetPolicy::Monotonic,
            lane.active_ticks,
        )?;
    }
    Ok(())
}

fn emit_partition_stats(
    stats: &mut StatsRegistry,
    execution: &Rem6ExecutionSummary,
) -> Result<(), Rem6CliError> {
    for partition in &execution.parallel_scheduler_partitions {
        increment_stat(
            stats,
            &format!(
                "sim.parallel.partition{}.scheduler.workers",
                partition.partition
            ),
            "Count",
            StatResetPolicy::Monotonic,
            partition.workers,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.parallel.partition{}.scheduler.dispatches",
                partition.partition
            ),
            "Count",
            StatResetPolicy::Monotonic,
            partition.dispatches,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.parallel.partition{}.scheduler.remote_sends",
                partition.partition
            ),
            "Count",
            StatResetPolicy::Monotonic,
            partition.remote_sends,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.parallel.partition{}.scheduler.remote_receives",
                partition.partition
            ),
            "Count",
            StatResetPolicy::Monotonic,
            partition.remote_receives,
        )?;
        increment_stat(
            stats,
            &format!(
                "sim.parallel.partition{}.scheduler.max_pending_events",
                partition.partition
            ),
            "Count",
            StatResetPolicy::Monotonic,
            partition.max_pending_events,
        )?;
    }
    Ok(())
}

fn emit_frontier_stats(
    stats: &mut StatsRegistry,
    frontier: &Rem6ParallelFrontierSummary,
    final_frontier: bool,
) -> Result<(), Rem6CliError> {
    let prefix = format!("sim.parallel.partition{}.frontier", frontier.partition);
    for (field, value, unit) in if final_frontier {
        [
            ("final_now", frontier.now, "Tick"),
            ("final_safe_until", frontier.safe_until, "Tick"),
            ("final_pending_events", frontier.pending_events, "Count"),
        ]
    } else {
        [
            ("now", frontier.now, "Tick"),
            ("safe_until", frontier.safe_until, "Tick"),
            ("pending_events", frontier.pending_events, "Count"),
        ]
    } {
        increment_stat(
            stats,
            &format!("{prefix}.{field}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    if let Some(next_tick) = frontier.next_tick {
        let field = if final_frontier {
            "final_next_tick"
        } else {
            "next_tick"
        };
        increment_stat(
            stats,
            &format!("{prefix}.{field}"),
            "Tick",
            StatResetPolicy::Monotonic,
            next_tick,
        )?;
    }
    Ok(())
}

fn emit_frontier_record_stats(
    stats: &mut StatsRegistry,
    prefix: &str,
    frontier: &Rem6ParallelFrontierSummary,
) -> Result<(), Rem6CliError> {
    for (field, value, unit) in [
        ("partition", frontier.partition as u64, "Count"),
        ("now", frontier.now, "Tick"),
        ("safe_until", frontier.safe_until, "Tick"),
        ("pending_events", frontier.pending_events, "Count"),
        (
            "next_tick_present",
            u64::from(frontier.next_tick.is_some()),
            "Count",
        ),
    ] {
        increment_stat(
            stats,
            &format!("{prefix}.{field}"),
            unit,
            StatResetPolicy::Monotonic,
            value,
        )?;
    }
    if let Some(next_tick) = frontier.next_tick {
        increment_stat(
            stats,
            &format!("{prefix}.next_tick"),
            "Tick",
            StatResetPolicy::Monotonic,
            next_tick,
        )?;
    }
    Ok(())
}

fn emit_ready_partition_record_stats(
    stats: &mut StatsRegistry,
    index: usize,
    ready: &Rem6ParallelReadyPartitionSummary,
) -> Result<(), Rem6CliError> {
    let prefix = format!("sim.parallel.scheduler.ready_partition{index}");
    increment_stat(
        stats,
        &format!("{prefix}.partition"),
        "Count",
        StatResetPolicy::Monotonic,
        ready.partition as u64,
    )?;
    increment_stat(
        stats,
        &format!("{prefix}.next_tick"),
        "Tick",
        StatResetPolicy::Monotonic,
        ready.next_tick,
    )?;
    Ok(())
}
