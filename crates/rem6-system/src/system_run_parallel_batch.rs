use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{ParallelEpochBatchRecord, PartitionId, Tick};

use crate::RiscvSystemRun;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum RiscvSystemParallelBatchScope {
    Scheduler,
    DataCacheScheduler,
}

impl RiscvSystemParallelBatchScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::DataCacheScheduler => "data-cache-scheduler",
        }
    }

    const fn sort_rank(self) -> u8 {
        match self {
            Self::Scheduler => 0,
            Self::DataCacheScheduler => 1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSystemParallelBatchTimelineRecord {
    scope: RiscvSystemParallelBatchScope,
    start_tick: Tick,
    horizon: Tick,
    partitions: Vec<PartitionId>,
    worker_count: usize,
}

impl RiscvSystemParallelBatchTimelineRecord {
    pub fn new(scope: RiscvSystemParallelBatchScope, batch: &ParallelEpochBatchRecord) -> Self {
        Self {
            scope,
            start_tick: batch_start_tick(batch),
            horizon: batch.horizon(),
            worker_count: batch.worker_count(),
            partitions: batch.partition_set(),
        }
    }

    pub const fn scope(&self) -> RiscvSystemParallelBatchScope {
        self.scope
    }

    pub const fn start_tick(&self) -> Tick {
        self.start_tick
    }

    pub const fn horizon(&self) -> Tick {
        self.horizon
    }

    pub const fn duration_ticks(&self) -> Tick {
        self.horizon.saturating_sub(self.start_tick)
    }

    pub fn partitions(&self) -> &[PartitionId] {
        &self.partitions
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }
}

impl RiscvSystemRun {
    pub fn parallel_scheduler_batch_timeline(&self) -> Vec<RiscvSystemParallelBatchTimelineRecord> {
        collect_batch_timeline(
            RiscvSystemParallelBatchScope::Scheduler,
            self.parallel_scheduler_batches(),
        )
    }

    pub fn data_cache_parallel_scheduler_batch_timeline(
        &self,
    ) -> Vec<RiscvSystemParallelBatchTimelineRecord> {
        collect_batch_timeline(
            RiscvSystemParallelBatchScope::DataCacheScheduler,
            self.data_cache_parallel_scheduler_batches(),
        )
    }

    pub fn full_system_parallel_scheduler_batch_timeline(
        &self,
    ) -> Vec<RiscvSystemParallelBatchTimelineRecord> {
        let mut timeline = self.parallel_scheduler_batch_timeline();
        timeline.extend(self.data_cache_parallel_scheduler_batch_timeline());
        sort_batch_timeline(&mut timeline);
        timeline
    }

    pub fn parallel_scheduler_batch_worker_count_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_worker_count_tick_summaries_from_summaries(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| epoch.batch_worker_count_tick_summaries()),
        )
    }

    pub fn data_cache_parallel_scheduler_batch_worker_count_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_batch_worker_count_tick_summaries(
            self.data_cache_parallel_scheduler_batch_timeline(),
        )
    }

    pub fn full_system_parallel_scheduler_batch_worker_count_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_worker_count_tick_summaries_from_summaries(
            self.parallel_scheduler_batch_worker_count_tick_summaries()
                .into_iter()
                .chain(self.data_cache_parallel_scheduler_batch_worker_count_tick_summaries()),
        )
    }

    pub fn parallel_scheduler_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_ticks_for_worker_count(worker_count))
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_ticks_at_or_above(minimum_worker_count))
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_worker_ticks())
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_worker_ticks_at_or_above(minimum_worker_count))
            .fold(0, Tick::saturating_add)
    }

    pub fn parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        longest_batch_tick_streak_at_or_above(
            self.parallel_scheduler_batch_timeline(),
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_ticks_for_worker_count(
        &self,
        worker_count: usize,
    ) -> Tick {
        batch_ticks_for_worker_count(
            self.data_cache_parallel_scheduler_batch_timeline(),
            worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        batch_ticks_at_or_above(
            self.data_cache_parallel_scheduler_batch_timeline(),
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        batch_worker_ticks(self.data_cache_parallel_scheduler_batch_timeline())
    }

    pub fn data_cache_parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        batch_worker_ticks_at_or_above(
            self.data_cache_parallel_scheduler_batch_timeline(),
            minimum_worker_count,
        )
    }

    pub fn data_cache_parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        longest_batch_tick_streak_at_or_above(
            self.data_cache_parallel_scheduler_batch_timeline(),
            minimum_worker_count,
        )
    }

    pub fn full_system_parallel_scheduler_batch_ticks_for_worker_count(
        &self,
        worker_count: usize,
    ) -> Tick {
        self.parallel_scheduler_batch_ticks_for_worker_count(worker_count)
            + self.data_cache_parallel_scheduler_batch_ticks_for_worker_count(worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        self.parallel_scheduler_batch_ticks_at_or_above(minimum_worker_count)
            + self.data_cache_parallel_scheduler_batch_ticks_at_or_above(minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks(&self) -> Tick {
        self.parallel_scheduler_batch_worker_ticks()
            + self.data_cache_parallel_scheduler_batch_worker_ticks()
    }

    pub fn full_system_parallel_scheduler_batch_worker_ticks_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        self.parallel_scheduler_batch_worker_ticks_at_or_above(minimum_worker_count)
            + self
                .data_cache_parallel_scheduler_batch_worker_ticks_at_or_above(minimum_worker_count)
    }

    pub fn full_system_parallel_scheduler_longest_batch_tick_streak_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> Tick {
        longest_batch_tick_streak_at_or_above(
            self.full_system_parallel_scheduler_batch_timeline(),
            minimum_worker_count,
        )
    }

    pub fn parallel_scheduler_batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_worker_count_summaries_from_summaries(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| epoch.batch_worker_count_summaries()),
        )
    }

    pub fn data_cache_parallel_scheduler_batch_worker_count_summaries(
        &self,
    ) -> Vec<(usize, usize)> {
        collect_batch_worker_count_summaries(self.data_cache_parallel_scheduler_batches())
    }

    pub fn full_system_parallel_scheduler_batch_worker_count_summaries(
        &self,
    ) -> Vec<(usize, usize)> {
        collect_worker_count_summaries_from_summaries(
            self.parallel_scheduler_batch_worker_count_summaries()
                .into_iter()
                .chain(self.data_cache_parallel_scheduler_batch_worker_count_summaries()),
        )
    }

    pub fn parallel_scheduler_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_count_for_worker_count(worker_count))
            .sum()
    }

    pub fn data_cache_parallel_scheduler_batch_count_for_worker_count(
        &self,
        worker_count: usize,
    ) -> usize {
        batch_count_for_worker_count(self.data_cache_parallel_scheduler_batches(), worker_count)
    }

    pub fn full_system_parallel_scheduler_batch_count_for_worker_count(
        &self,
        worker_count: usize,
    ) -> usize {
        self.parallel_scheduler_batch_count_for_worker_count(worker_count)
            + self.data_cache_parallel_scheduler_batch_count_for_worker_count(worker_count)
    }

    pub fn parallel_scheduler_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_count_at_or_above(minimum_worker_count))
            .sum()
    }

    pub fn data_cache_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        batch_count_at_or_above(
            self.data_cache_parallel_scheduler_batches(),
            minimum_worker_count,
        )
    }

    pub fn full_system_parallel_scheduler_batch_count_at_or_above(
        &self,
        minimum_worker_count: usize,
    ) -> usize {
        self.parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
            + self.data_cache_parallel_scheduler_batch_count_at_or_above(minimum_worker_count)
    }

    pub fn parallel_scheduler_batch_partition_set_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_partition_set_summaries_from_summaries(
            self.parallel_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| epoch.batch_partition_set_summaries()),
        )
    }

    pub fn data_cache_parallel_scheduler_batch_partition_set_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_batch_partition_set_summaries(self.data_cache_parallel_scheduler_batches())
    }

    pub fn full_system_parallel_scheduler_batch_partition_set_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_partition_set_summaries_from_summaries(
            self.parallel_scheduler_batch_partition_set_summaries()
                .into_iter()
                .chain(self.data_cache_parallel_scheduler_batch_partition_set_summaries()),
        )
    }

    pub fn parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        self.parallel_scheduler_epochs()
            .into_iter()
            .map(|epoch| epoch.batch_count_for_partition_set(partitions.iter().copied()))
            .sum()
    }

    pub fn data_cache_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        batch_count_for_partition_set(self.data_cache_parallel_scheduler_batches(), partitions)
    }

    pub fn full_system_parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        let partitions = normalize_partition_set(partitions);
        self.parallel_scheduler_batch_count_for_partition_set(partitions.iter().copied())
            + self.data_cache_parallel_scheduler_batch_count_for_partition_set(
                partitions.iter().copied(),
            )
    }

    pub fn parallel_scheduler_batch_partition_streak_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_batch_partition_streak_summaries(self.parallel_scheduler_batches())
    }

    pub fn data_cache_parallel_scheduler_batch_partition_streak_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_batch_partition_streak_summaries(self.data_cache_parallel_scheduler_batches())
    }

    pub fn full_system_parallel_scheduler_batch_partition_streak_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_batch_partition_streak_summaries(self.full_system_parallel_scheduler_batches())
    }

    pub fn parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        max_consecutive_batch_count_for_partition_set(
            self.parallel_scheduler_batch_partition_streak_summaries(),
            partitions,
        )
    }

    pub fn data_cache_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        max_consecutive_batch_count_for_partition_set(
            self.data_cache_parallel_scheduler_batch_partition_streak_summaries(),
            partitions,
        )
    }

    pub fn full_system_parallel_scheduler_max_consecutive_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        max_consecutive_batch_count_for_partition_set(
            self.full_system_parallel_scheduler_batch_partition_streak_summaries(),
            partitions,
        )
    }
}

fn collect_batch_timeline(
    scope: RiscvSystemParallelBatchScope,
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
) -> Vec<RiscvSystemParallelBatchTimelineRecord> {
    let mut timeline = batches
        .into_iter()
        .map(|batch| RiscvSystemParallelBatchTimelineRecord::new(scope, &batch))
        .collect::<Vec<_>>();
    sort_batch_timeline(&mut timeline);
    timeline
}

fn sort_batch_timeline(timeline: &mut [RiscvSystemParallelBatchTimelineRecord]) {
    timeline.sort_by_key(|record| {
        (
            record.start_tick(),
            record.horizon(),
            record.scope().sort_rank(),
            record.partitions().to_vec(),
        )
    });
}

fn collect_batch_worker_count_tick_summaries(
    records: impl IntoIterator<Item = RiscvSystemParallelBatchTimelineRecord>,
) -> Vec<(usize, Tick)> {
    let mut summaries = BTreeMap::<usize, Tick>::new();
    for record in records {
        let duration = record.duration_ticks();
        if record.worker_count() != 0 && duration != 0 {
            let ticks = summaries.entry(record.worker_count()).or_default();
            *ticks = ticks.saturating_add(duration);
        }
    }
    summaries.into_iter().collect()
}

fn batch_ticks_for_worker_count(
    records: impl IntoIterator<Item = RiscvSystemParallelBatchTimelineRecord>,
    worker_count: usize,
) -> Tick {
    records
        .into_iter()
        .filter(|record| record.worker_count() == worker_count)
        .map(|record| record.duration_ticks())
        .fold(0, Tick::saturating_add)
}

fn batch_ticks_at_or_above(
    records: impl IntoIterator<Item = RiscvSystemParallelBatchTimelineRecord>,
    minimum_worker_count: usize,
) -> Tick {
    records
        .into_iter()
        .filter(|record| record.worker_count() >= minimum_worker_count)
        .map(|record| record.duration_ticks())
        .fold(0, Tick::saturating_add)
}

fn batch_worker_ticks(
    records: impl IntoIterator<Item = RiscvSystemParallelBatchTimelineRecord>,
) -> Tick {
    records
        .into_iter()
        .map(|record| {
            record
                .duration_ticks()
                .saturating_mul(record.worker_count() as Tick)
        })
        .fold(0, Tick::saturating_add)
}

fn batch_worker_ticks_at_or_above(
    records: impl IntoIterator<Item = RiscvSystemParallelBatchTimelineRecord>,
    minimum_worker_count: usize,
) -> Tick {
    records
        .into_iter()
        .filter(|record| record.worker_count() >= minimum_worker_count)
        .map(|record| {
            record
                .duration_ticks()
                .saturating_mul(record.worker_count() as Tick)
        })
        .fold(0, Tick::saturating_add)
}

fn longest_batch_tick_streak_at_or_above(
    records: impl IntoIterator<Item = RiscvSystemParallelBatchTimelineRecord>,
    minimum_worker_count: usize,
) -> Tick {
    let mut longest = 0;
    let mut current_start = None;
    let mut current_end = 0;
    for record in records {
        if record.worker_count() < minimum_worker_count || record.duration_ticks() == 0 {
            continue;
        }
        let start_tick = record.start_tick();
        let horizon = record.horizon();
        match current_start {
            Some(streak_start) if start_tick <= current_end => {
                current_end = current_end.max(horizon);
                longest = longest.max(current_end.saturating_sub(streak_start));
            }
            Some(streak_start) => {
                longest = longest.max(current_end.saturating_sub(streak_start));
                current_start = Some(start_tick);
                current_end = horizon;
            }
            None => {
                current_start = Some(start_tick);
                current_end = horizon;
            }
        }
    }
    if let Some(streak_start) = current_start {
        longest = longest.max(current_end.saturating_sub(streak_start));
    }
    longest
}

fn collect_batch_worker_count_summaries(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
) -> Vec<(usize, usize)> {
    let mut summaries = BTreeMap::<usize, usize>::new();
    for batch in batches {
        let worker_count = batch.worker_count();
        if worker_count != 0 {
            *summaries.entry(worker_count).or_default() += 1;
        }
    }
    summaries.into_iter().collect()
}

fn collect_worker_count_summaries_from_summaries(
    summaries: impl IntoIterator<Item = (usize, usize)>,
) -> Vec<(usize, usize)> {
    let mut collected = BTreeMap::<usize, usize>::new();
    for (worker_count, count) in summaries {
        if worker_count != 0 && count != 0 {
            *collected.entry(worker_count).or_default() += count;
        }
    }
    collected.into_iter().collect()
}

fn collect_worker_count_tick_summaries_from_summaries(
    summaries: impl IntoIterator<Item = (usize, Tick)>,
) -> Vec<(usize, Tick)> {
    let mut collected = BTreeMap::<usize, Tick>::new();
    for (worker_count, ticks) in summaries {
        if worker_count != 0 && ticks != 0 {
            let stored = collected.entry(worker_count).or_default();
            *stored = stored.saturating_add(ticks);
        }
    }
    collected.into_iter().collect()
}

fn batch_count_at_or_above(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
    minimum_worker_count: usize,
) -> usize {
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .count()
}

fn batch_count_for_worker_count(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
    worker_count: usize,
) -> usize {
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() == worker_count)
        .count()
}

fn collect_batch_partition_set_summaries(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
) -> Vec<(Vec<PartitionId>, usize)> {
    let mut summaries = BTreeMap::<Vec<PartitionId>, usize>::new();
    for batch in batches {
        let partitions = batch.partition_set();
        if !partitions.is_empty() {
            *summaries.entry(partitions).or_default() += 1;
        }
    }
    summaries.into_iter().collect()
}

fn collect_partition_set_summaries_from_summaries(
    summaries: impl IntoIterator<Item = (Vec<PartitionId>, usize)>,
) -> Vec<(Vec<PartitionId>, usize)> {
    let mut collected = BTreeMap::<Vec<PartitionId>, usize>::new();
    for (partitions, count) in summaries {
        if !partitions.is_empty() && count != 0 {
            *collected.entry(partitions).or_default() += count;
        }
    }
    collected.into_iter().collect()
}

fn batch_count_for_partition_set(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let expected = normalize_partition_set(partitions);
    batches
        .into_iter()
        .filter(|batch| batch.partition_set() == expected)
        .count()
}

fn collect_batch_partition_streak_summaries(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
) -> Vec<(Vec<PartitionId>, usize)> {
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

fn max_consecutive_batch_count_for_partition_set(
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

fn batch_start_tick(batch: &ParallelEpochBatchRecord) -> Tick {
    batch
        .workers()
        .iter()
        .map(|worker| worker.start_tick())
        .min()
        .unwrap_or_else(|| batch.horizon())
}

fn normalize_partition_set(partitions: impl IntoIterator<Item = PartitionId>) -> Vec<PartitionId> {
    partitions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
