use std::collections::{BTreeMap, BTreeSet};

use crate::scheduler::PartitionId;
use crate::Tick;

use super::ParallelEpochBatchRecord;

pub(super) fn collect_batch_worker_count_summaries<'a, I>(batches: I) -> Vec<(usize, usize)>
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    let mut summaries = BTreeMap::<usize, usize>::new();
    for batch in batches {
        let worker_count = batch.worker_count();
        if worker_count != 0 {
            *summaries.entry(worker_count).or_default() += 1;
        }
    }
    summaries.into_iter().collect()
}

pub(super) fn batch_count_for_worker_count<'a, I>(batches: I, worker_count: usize) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() == worker_count)
        .count()
}

pub(super) fn batch_count_at_or_above<'a, I>(batches: I, minimum_worker_count: usize) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .count()
}

pub(super) fn collect_batch_partition_set_summaries<'a, I>(
    batches: I,
) -> Vec<(Vec<PartitionId>, usize)>
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    let mut summaries = BTreeMap::<Vec<PartitionId>, usize>::new();
    for batch in batches {
        let partitions = batch.partition_set();
        if !partitions.is_empty() {
            *summaries.entry(partitions).or_default() += 1;
        }
    }
    summaries.into_iter().collect()
}

pub(super) fn batch_count_for_partition_set<'a, I>(
    batches: I,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    let expected = normalize_partition_set(partitions);
    batches
        .into_iter()
        .filter(|batch| batch.partition_set() == expected)
        .count()
}

pub(super) fn collect_batch_partition_streak_summaries<'a, I>(
    batches: I,
) -> Vec<(Vec<PartitionId>, usize)>
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    let mut summaries = BTreeMap::<Vec<PartitionId>, usize>::new();
    let mut current: Option<(Vec<PartitionId>, usize)> = None;
    for batch in batches {
        let partitions = batch.partition_set();
        if partitions.is_empty() {
            continue;
        }
        match current.as_mut() {
            Some((current_partitions, count)) if current_partitions == &partitions => {
                *count += 1;
            }
            Some(_) => {
                flush_partition_streak(&mut summaries, current.take());
                current = Some((partitions, 1));
            }
            None => {
                current = Some((partitions, 1));
            }
        }
    }
    flush_partition_streak(&mut summaries, current);
    summaries.into_iter().collect()
}

pub(super) fn max_consecutive_batch_count_for_partition_set(
    streaks: impl IntoIterator<Item = (Vec<PartitionId>, usize)>,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let expected = normalize_partition_set(partitions);
    streaks
        .into_iter()
        .find_map(|(partitions, count)| (partitions == expected).then_some(count))
        .unwrap_or(0)
}

fn flush_partition_streak(
    summaries: &mut BTreeMap<Vec<PartitionId>, usize>,
    streak: Option<(Vec<PartitionId>, usize)>,
) {
    if let Some((partitions, count)) = streak {
        summaries
            .entry(partitions)
            .and_modify(|stored| *stored = (*stored).max(count))
            .or_insert(count);
    }
}

pub(super) fn collect_batch_worker_count_tick_summaries<'a, I>(batches: I) -> Vec<(usize, Tick)>
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    let mut summaries = BTreeMap::<usize, Tick>::new();
    for batch in batches {
        let worker_count = batch.worker_count();
        if worker_count != 0 {
            let duration_ticks = batch_duration_ticks(batch);
            let summary_ticks = summaries.entry(worker_count).or_default();
            *summary_ticks = summary_ticks.saturating_add(duration_ticks);
        }
    }
    summaries.into_iter().collect()
}

pub(super) fn batch_ticks_for_worker_count<'a, I>(batches: I, worker_count: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() == worker_count)
        .map(batch_duration_ticks)
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_ticks_at_or_above<'a, I>(batches: I, minimum_worker_count: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .map(batch_duration_ticks)
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_worker_ticks<'a, I>(batches: I) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .map(batch_worker_tick_count)
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_worker_ticks_at_or_above<'a, I>(batches: I, minimum_worker_count: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .map(batch_worker_tick_count)
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_worker_capacity_ticks<'a, I>(batches: I, worker_capacity: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    if worker_capacity == usize::MAX {
        return batch_worker_ticks(batches);
    }
    batches
        .into_iter()
        .map(|batch| batch.worker_capacity_ticks(worker_capacity))
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_idle_worker_ticks<'a, I>(batches: I, worker_capacity: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochBatchRecord>,
{
    if worker_capacity == usize::MAX {
        return 0;
    }
    batches
        .into_iter()
        .map(|batch| batch.idle_worker_ticks(worker_capacity))
        .fold(0, Tick::saturating_add)
}

pub(super) fn batch_worker_slot_tick_summaries(
    batches: &[ParallelEpochBatchRecord],
    worker_capacity: usize,
) -> Vec<(usize, Tick, Tick)> {
    if batches.is_empty() {
        return Vec::new();
    }
    if worker_capacity == usize::MAX {
        return active_batch_worker_slot_tick_summaries(batches);
    }
    let capacity = batches
        .iter()
        .map(ParallelEpochBatchRecord::worker_count)
        .fold(worker_capacity.max(1), usize::max);
    let mut summaries: Vec<(Tick, Tick)> = vec![(0, 0); capacity];
    for batch in batches {
        let duration = batch.duration_ticks();
        if duration == 0 {
            continue;
        }
        for (worker_slot, summary) in summaries.iter_mut().enumerate() {
            if worker_slot < batch.worker_count() {
                summary.0 = summary.0.saturating_add(duration);
            } else {
                summary.1 = summary.1.saturating_add(duration);
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

fn active_batch_worker_slot_tick_summaries(
    batches: &[ParallelEpochBatchRecord],
) -> Vec<(usize, Tick, Tick)> {
    let mut summaries = BTreeMap::<usize, Tick>::new();
    for batch in batches {
        let duration = batch.duration_ticks();
        if duration == 0 {
            continue;
        }
        for worker_slot in 0..batch.worker_count() {
            let active_ticks = summaries.entry(worker_slot).or_default();
            *active_ticks = active_ticks.saturating_add(duration);
        }
    }
    summaries
        .into_iter()
        .map(|(worker_slot, active_ticks)| (worker_slot, active_ticks, 0))
        .collect()
}

fn batch_worker_tick_count(batch: &ParallelEpochBatchRecord) -> Tick {
    batch.worker_ticks()
}

fn batch_duration_ticks(batch: &ParallelEpochBatchRecord) -> Tick {
    batch.duration_ticks()
}

pub(super) fn normalize_partition_set(
    partitions: impl IntoIterator<Item = PartitionId>,
) -> Vec<PartitionId> {
    partitions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
