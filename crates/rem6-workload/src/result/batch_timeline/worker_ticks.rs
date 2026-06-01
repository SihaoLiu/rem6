use std::collections::BTreeMap;

use rem6_kernel::Tick;

use crate::parallel_batch::{
    parallel_batch_longest_tick_streak_at_or_above, WorkloadParallelBatchTimelineRecord,
};

pub(super) fn planned_batch_worker_ticks(records: &[WorkloadParallelBatchTimelineRecord]) -> Tick {
    records
        .iter()
        .filter(|record| record.has_record_shape())
        .map(|record| {
            record
                .duration_ticks()
                .saturating_mul(record.worker_count() as Tick)
        })
        .fold(0, Tick::saturating_add)
}

pub(super) fn planned_batch_worker_slot_tick_summaries(
    records: &[WorkloadParallelBatchTimelineRecord],
    worker_capacity_ticks: Tick,
) -> Vec<(usize, Tick, Tick)> {
    let records = records
        .iter()
        .filter(|record| record.has_record_shape())
        .collect::<Vec<_>>();
    let total_duration_ticks = records
        .iter()
        .map(|record| record.duration_ticks())
        .fold(0, Tick::saturating_add);
    if worker_capacity_ticks == 0
        || total_duration_ticks == 0
        || !worker_capacity_ticks.is_multiple_of(total_duration_ticks)
    {
        return Vec::new();
    }
    let worker_capacity = worker_capacity_ticks / total_duration_ticks;
    let Some(worker_capacity) = usize::try_from(worker_capacity).ok() else {
        return Vec::new();
    };
    let max_worker_count = records
        .iter()
        .map(|record| record.worker_count())
        .max()
        .unwrap_or(0);
    if worker_capacity < max_worker_count {
        return Vec::new();
    }

    let mut summaries: Vec<(Tick, Tick)> = vec![(0, 0); worker_capacity];
    for record in records {
        let duration_ticks = record.duration_ticks();
        for (worker_slot, summary) in summaries.iter_mut().enumerate() {
            if worker_slot < record.worker_count() {
                summary.0 = summary.0.saturating_add(duration_ticks);
            } else {
                summary.1 = summary.1.saturating_add(duration_ticks);
            }
        }
    }
    summaries
        .into_iter()
        .enumerate()
        .filter(|(_, (active_ticks, idle_ticks))| *active_ticks != 0 || *idle_ticks != 0)
        .map(|(worker_slot, (active_ticks, idle_ticks))| (worker_slot, active_ticks, idle_ticks))
        .collect()
}

pub(super) fn recorded_batch_worker_ticks(
    records: &[WorkloadParallelBatchTimelineRecord],
    slot_summaries: &[(usize, Tick, Tick)],
) -> Tick {
    let slot_active_ticks = slot_summaries
        .iter()
        .map(|(_, active_ticks, _)| *active_ticks)
        .fold(0, Tick::saturating_add);
    if slot_active_ticks != 0 {
        return slot_active_ticks;
    }
    records
        .iter()
        .filter(|record| record.has_record_shape())
        .map(|record| {
            record
                .duration_ticks()
                .saturating_mul(record.worker_count() as Tick)
        })
        .fold(0, Tick::saturating_add)
}

pub(super) fn collect_batch_worker_slot_tick_summaries(
    summaries: impl IntoIterator<Item = (usize, Tick, Tick)>,
) -> Vec<(usize, Tick, Tick)> {
    let mut by_worker_slot = BTreeMap::<usize, (Tick, Tick)>::new();
    for (worker_slot, active_ticks, idle_ticks) in summaries {
        if active_ticks == 0 && idle_ticks == 0 {
            continue;
        }
        let summary = by_worker_slot.entry(worker_slot).or_default();
        summary.0 = summary.0.saturating_add(active_ticks);
        summary.1 = summary.1.saturating_add(idle_ticks);
    }
    by_worker_slot
        .into_iter()
        .map(|(worker_slot, (active_ticks, idle_ticks))| (worker_slot, active_ticks, idle_ticks))
        .collect()
}

pub(super) fn collect_strongest_batch_worker_slot_tick_summaries(
    left: &[(usize, Tick, Tick)],
    right: &[(usize, Tick, Tick)],
) -> Vec<(usize, Tick, Tick)> {
    let mut by_worker_slot = BTreeMap::<usize, (Tick, Tick)>::new();
    for (worker_slot, active_ticks, idle_ticks) in left.iter().chain(right.iter()).copied() {
        if active_ticks == 0 && idle_ticks == 0 {
            continue;
        }
        let summary = by_worker_slot.entry(worker_slot).or_default();
        summary.0 = summary.0.max(active_ticks);
        summary.1 = summary.1.max(idle_ticks);
    }
    by_worker_slot
        .into_iter()
        .map(|(worker_slot, (active_ticks, idle_ticks))| (worker_slot, active_ticks, idle_ticks))
        .collect()
}

pub(super) fn collect_batch_worker_count_tick_summaries(
    summaries: impl IntoIterator<Item = (usize, Tick)>,
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in summaries {
        if worker_count < 2 || ticks == 0 {
            continue;
        }
        let stored = by_worker_count.entry(worker_count).or_default();
        *stored = stored.saturating_add(ticks);
    }
    by_worker_count.into_iter().collect()
}

pub(super) fn collect_batch_worker_tick_streak_summaries(
    summaries: impl IntoIterator<Item = (usize, Tick)>,
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in summaries {
        if worker_count < 2 || ticks == 0 {
            continue;
        }
        by_worker_count
            .entry(worker_count)
            .and_modify(|stored| *stored = (*stored).max(ticks))
            .or_insert(ticks);
    }
    by_worker_count.into_iter().collect()
}

pub(super) fn collect_strongest_batch_worker_count_tick_summaries(
    left: &[(usize, Tick)],
    right: &[(usize, Tick)],
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in left.iter().chain(right.iter()).copied() {
        if worker_count < 2 || ticks == 0 {
            continue;
        }
        by_worker_count
            .entry(worker_count)
            .and_modify(|stored| *stored = (*stored).max(ticks))
            .or_insert(ticks);
    }
    by_worker_count.into_iter().collect()
}

pub(super) fn collect_parallel_batch_worker_tick_streak_summaries(
    timeline: &[WorkloadParallelBatchTimelineRecord],
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for record in timeline {
        if !record.is_parallel_evidence() {
            continue;
        }
        by_worker_count.entry(record.worker_count()).or_default();
    }
    by_worker_count
        .into_keys()
        .filter_map(|worker_count| {
            let ticks = parallel_batch_longest_tick_streak_at_or_above(timeline, worker_count);
            (ticks != 0).then_some((worker_count, ticks))
        })
        .collect()
}

pub(super) fn collect_strongest_batch_worker_tick_streak_summaries(
    left: &[(usize, Tick)],
    right: &[(usize, Tick)],
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in left.iter().chain(right.iter()).copied() {
        if worker_count < 2 || ticks == 0 {
            continue;
        }
        by_worker_count
            .entry(worker_count)
            .and_modify(|stored| *stored = (*stored).max(ticks))
            .or_insert(ticks);
    }
    by_worker_count.into_iter().collect()
}

pub(super) fn batch_ticks_for_worker_count(
    summaries: &[(usize, Tick)],
    worker_count: usize,
) -> Tick {
    summaries
        .iter()
        .filter(|(count, _)| *count == worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_ticks_at_or_above(
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    summaries
        .iter()
        .filter(|(count, _)| *count >= minimum_worker_count)
        .map(|(_, ticks)| *ticks)
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_worker_ticks(summaries: &[(usize, Tick)]) -> Tick {
    summaries
        .iter()
        .map(|(count, ticks)| ticks.saturating_mul(*count as Tick))
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_worker_ticks_at_or_above(
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    summaries
        .iter()
        .filter(|(count, _)| *count >= minimum_worker_count)
        .map(|(count, ticks)| ticks.saturating_mul(*count as Tick))
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_worker_tick_streak_at_or_above(
    summaries: &[(usize, Tick)],
    minimum_worker_count: usize,
) -> Tick {
    summaries
        .iter()
        .filter(|(worker_count, _)| *worker_count >= minimum_worker_count)
        .map(|(_, ticks)| *ticks)
        .max()
        .unwrap_or(0)
}
