use std::collections::BTreeMap;

use rem6_kernel::{PartitionFrontier, ReadyPartition};
use rem6_stats::{StatResetPolicy, StatsRegistry};
use rem6_system::RiscvSystemRun;

use super::{
    stats_output::increment_stat, Rem6CliError, Rem6ExecutionSummary, Rem6ParallelFrontierSummary,
    Rem6ParallelPartitionSummary, Rem6ParallelReadyPartitionSummary, Rem6ParallelWorkerLaneSummary,
    Rem6ParallelWorkerSlotSummary,
};

pub(super) fn parallel_worker_slot_summaries(
    run: &RiscvSystemRun,
) -> Vec<Rem6ParallelWorkerSlotSummary> {
    run.parallel_scheduler_batch_worker_slot_tick_summaries()
        .into_iter()
        .map(
            |(slot, active_ticks, idle_ticks)| Rem6ParallelWorkerSlotSummary {
                slot,
                active_ticks,
                idle_ticks,
            },
        )
        .collect()
}

pub(super) fn parallel_worker_lane_summaries(
    run: &RiscvSystemRun,
) -> Vec<Rem6ParallelWorkerLaneSummary> {
    let mut summaries = BTreeMap::<(usize, u32), u64>::new();
    for lane in run.parallel_scheduler_worker_lanes() {
        let key = (lane.lane(), lane.partition().index());
        let ticks = summaries.entry(key).or_default();
        *ticks = ticks.saturating_add(lane.duration_ticks());
    }
    summaries
        .into_iter()
        .map(
            |((lane, partition), active_ticks)| Rem6ParallelWorkerLaneSummary {
                lane,
                partition,
                active_ticks,
            },
        )
        .collect()
}

pub(super) fn parallel_partition_summaries(
    run: &RiscvSystemRun,
) -> Vec<Rem6ParallelPartitionSummary> {
    run.parallel_scheduler_partition_activities()
        .into_iter()
        .map(|(partition, activity)| Rem6ParallelPartitionSummary {
            partition: partition.index(),
            workers: activity.worker_count() as u64,
            dispatches: activity.dispatch_count() as u64,
            remote_sends: activity.remote_send_count() as u64,
            remote_receives: activity.remote_receive_count() as u64,
            max_pending_events: activity.max_pending_events() as u64,
        })
        .collect()
}

pub(super) fn parallel_frontier_summaries(
    frontiers: Vec<PartitionFrontier>,
) -> Vec<Rem6ParallelFrontierSummary> {
    frontiers
        .into_iter()
        .map(|frontier| Rem6ParallelFrontierSummary {
            partition: frontier.partition().index(),
            now: frontier.now(),
            safe_until: frontier.safe_until(),
            next_tick: frontier.next_tick(),
            pending_events: frontier.pending_events() as u64,
        })
        .collect()
}

pub(super) fn parallel_ready_partition_summaries(
    ready_partitions: Vec<ReadyPartition>,
) -> Vec<Rem6ParallelReadyPartitionSummary> {
    ready_partitions
        .into_iter()
        .map(|ready| Rem6ParallelReadyPartitionSummary {
            partition: ready.partition.index(),
            next_tick: ready.next_tick,
        })
        .collect()
}

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
