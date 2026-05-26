use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{ParallelPartitionActivity, PartitionId, Tick};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadParallelBatchScope {
    Scheduler,
    DataCacheScheduler,
    GpuDmaScheduler,
    AcceleratorDmaScheduler,
}

impl WorkloadParallelBatchScope {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduler => "scheduler",
            Self::DataCacheScheduler => "data-cache-scheduler",
            Self::GpuDmaScheduler => "gpu-dma-scheduler",
            Self::AcceleratorDmaScheduler => "accelerator-dma-scheduler",
        }
    }

    pub(crate) const fn sort_rank(self) -> u8 {
        match self {
            Self::Scheduler => 0,
            Self::DataCacheScheduler => 1,
            Self::GpuDmaScheduler => 2,
            Self::AcceleratorDmaScheduler => 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelBatchTimelineRecord {
    scope: WorkloadParallelBatchScope,
    start_tick: Tick,
    horizon: Tick,
    partitions: Vec<PartitionId>,
    worker_count: usize,
}

impl WorkloadParallelBatchTimelineRecord {
    pub fn new(
        scope: WorkloadParallelBatchScope,
        start_tick: Tick,
        horizon: Tick,
        partitions: impl IntoIterator<Item = PartitionId>,
        worker_count: usize,
    ) -> Self {
        Self {
            scope,
            start_tick,
            horizon,
            partitions: normalize_partition_set(partitions),
            worker_count,
        }
    }

    pub const fn scope(&self) -> WorkloadParallelBatchScope {
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

    pub fn is_empty(&self) -> bool {
        self.worker_count == 0 || self.partitions.is_empty() || self.horizon <= self.start_tick
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorkloadParallelBatchWorkerCount {
    worker_count: usize,
    batch_count: usize,
}

impl WorkloadParallelBatchWorkerCount {
    pub const fn new(worker_count: usize, batch_count: usize) -> Self {
        Self {
            worker_count,
            batch_count,
        }
    }

    pub const fn worker_count(&self) -> usize {
        self.worker_count
    }

    pub const fn batch_count(&self) -> usize {
        self.batch_count
    }

    pub const fn is_empty(&self) -> bool {
        self.worker_count == 0 || self.batch_count == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelBatchPartitionSet {
    partitions: Vec<PartitionId>,
    batch_count: usize,
}

impl WorkloadParallelBatchPartitionSet {
    pub fn new(partitions: impl IntoIterator<Item = PartitionId>, batch_count: usize) -> Self {
        Self {
            partitions: normalize_partition_set(partitions),
            batch_count,
        }
    }

    pub fn partitions(&self) -> &[PartitionId] {
        &self.partitions
    }

    pub const fn batch_count(&self) -> usize {
        self.batch_count
    }

    pub fn is_empty(&self) -> bool {
        self.partitions.is_empty() || self.batch_count == 0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelBatchPartitionStreak {
    partitions: Vec<PartitionId>,
    consecutive_batch_count: usize,
}

impl WorkloadParallelBatchPartitionStreak {
    pub fn new(
        partitions: impl IntoIterator<Item = PartitionId>,
        consecutive_batch_count: usize,
    ) -> Self {
        Self {
            partitions: normalize_partition_set(partitions),
            consecutive_batch_count,
        }
    }

    pub fn partitions(&self) -> &[PartitionId] {
        &self.partitions
    }

    pub const fn consecutive_batch_count(&self) -> usize {
        self.consecutive_batch_count
    }

    pub fn is_empty(&self) -> bool {
        self.partitions.is_empty() || self.consecutive_batch_count == 0
    }
}

pub(crate) fn normalize_partition_set(
    partitions: impl IntoIterator<Item = PartitionId>,
) -> Vec<PartitionId> {
    partitions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

pub(crate) fn collect_parallel_batch_worker_counts(
    counts: impl IntoIterator<Item = WorkloadParallelBatchWorkerCount>,
) -> Vec<WorkloadParallelBatchWorkerCount> {
    let mut by_worker_count = BTreeMap::<usize, usize>::new();
    for count in counts {
        if count.is_empty() {
            continue;
        }
        *by_worker_count.entry(count.worker_count()).or_default() += count.batch_count();
    }
    by_worker_count
        .into_iter()
        .map(|(worker_count, batch_count)| {
            WorkloadParallelBatchWorkerCount::new(worker_count, batch_count)
        })
        .collect()
}

pub(crate) fn collect_parallel_batch_timeline(
    records: impl IntoIterator<Item = WorkloadParallelBatchTimelineRecord>,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    let mut timeline = records
        .into_iter()
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    sort_parallel_batch_timeline(&mut timeline);
    timeline
}

pub(crate) fn collect_parallel_batch_worker_counts_from_timeline(
    records: &[WorkloadParallelBatchTimelineRecord],
) -> Vec<WorkloadParallelBatchWorkerCount> {
    collect_parallel_batch_worker_counts(
        records
            .iter()
            .map(|record| WorkloadParallelBatchWorkerCount::new(record.worker_count(), 1)),
    )
}

pub(crate) fn collect_parallel_batch_worker_count_tick_summaries(
    records: &[WorkloadParallelBatchTimelineRecord],
) -> Vec<(usize, Tick)> {
    let mut by_worker_count = BTreeMap::<usize, Tick>::new();
    for record in records {
        let duration = record.duration_ticks();
        if record.worker_count() != 0 && duration != 0 {
            let ticks = by_worker_count.entry(record.worker_count()).or_default();
            *ticks = ticks.saturating_add(duration);
        }
    }
    by_worker_count.into_iter().collect()
}

pub(crate) fn parallel_batch_ticks_for_worker_count(
    records: &[WorkloadParallelBatchTimelineRecord],
    worker_count: usize,
) -> Tick {
    records
        .iter()
        .filter(|record| record.worker_count() == worker_count)
        .map(WorkloadParallelBatchTimelineRecord::duration_ticks)
        .fold(0, Tick::saturating_add)
}

pub(crate) fn parallel_batch_ticks_at_or_above(
    records: &[WorkloadParallelBatchTimelineRecord],
    minimum_worker_count: usize,
) -> Tick {
    records
        .iter()
        .filter(|record| record.worker_count() >= minimum_worker_count)
        .map(WorkloadParallelBatchTimelineRecord::duration_ticks)
        .fold(0, Tick::saturating_add)
}

pub(crate) fn parallel_batch_worker_ticks(records: &[WorkloadParallelBatchTimelineRecord]) -> Tick {
    records
        .iter()
        .map(|record| {
            record
                .duration_ticks()
                .saturating_mul(record.worker_count() as Tick)
        })
        .fold(0, Tick::saturating_add)
}

pub(crate) fn parallel_batch_worker_ticks_at_or_above(
    records: &[WorkloadParallelBatchTimelineRecord],
    minimum_worker_count: usize,
) -> Tick {
    records
        .iter()
        .filter(|record| record.worker_count() >= minimum_worker_count)
        .map(|record| {
            record
                .duration_ticks()
                .saturating_mul(record.worker_count() as Tick)
        })
        .fold(0, Tick::saturating_add)
}

pub(crate) fn parallel_batch_longest_tick_streak_at_or_above(
    records: &[WorkloadParallelBatchTimelineRecord],
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

pub(crate) fn collect_parallel_batch_partition_sets_from_timeline(
    records: &[WorkloadParallelBatchTimelineRecord],
) -> Vec<WorkloadParallelBatchPartitionSet> {
    collect_parallel_batch_partition_sets(records.iter().map(|record| {
        WorkloadParallelBatchPartitionSet::new(record.partitions().iter().copied(), 1)
    }))
}

pub(crate) fn collect_parallel_batch_partition_streaks_from_timeline(
    records: &[WorkloadParallelBatchTimelineRecord],
) -> Vec<WorkloadParallelBatchPartitionStreak> {
    collect_parallel_batch_partition_streaks_from_sequence(records.iter().map(|record| {
        WorkloadParallelBatchPartitionSet::new(record.partitions().iter().copied(), 1)
    }))
}

fn sort_parallel_batch_timeline(timeline: &mut [WorkloadParallelBatchTimelineRecord]) {
    timeline.sort_by_key(|record| {
        (
            record.start_tick(),
            record.horizon(),
            record.scope().sort_rank(),
            record.partitions().to_vec(),
            record.worker_count(),
        )
    });
}

pub(crate) fn collect_parallel_batch_partition_sets(
    sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
) -> Vec<WorkloadParallelBatchPartitionSet> {
    let mut by_partitions = BTreeMap::<Vec<PartitionId>, usize>::new();
    for set in sets {
        if set.is_empty() {
            continue;
        }
        *by_partitions.entry(set.partitions().to_vec()).or_default() += set.batch_count();
    }
    by_partitions
        .into_iter()
        .map(|(partitions, batch_count)| {
            WorkloadParallelBatchPartitionSet::new(partitions, batch_count)
        })
        .collect()
}

pub(crate) fn collect_parallel_batch_partition_streaks(
    streaks: impl IntoIterator<Item = WorkloadParallelBatchPartitionStreak>,
) -> Vec<WorkloadParallelBatchPartitionStreak> {
    let mut by_partitions = BTreeMap::<Vec<PartitionId>, usize>::new();
    for streak in streaks {
        if streak.is_empty() {
            continue;
        }
        by_partitions
            .entry(streak.partitions().to_vec())
            .and_modify(|stored| {
                *stored = (*stored).max(streak.consecutive_batch_count());
            })
            .or_insert(streak.consecutive_batch_count());
    }
    by_partitions
        .into_iter()
        .map(|(partitions, consecutive_batch_count)| {
            WorkloadParallelBatchPartitionStreak::new(partitions, consecutive_batch_count)
        })
        .collect()
}

pub(crate) fn collect_parallel_batch_partition_streaks_from_sequence(
    sets: impl IntoIterator<Item = WorkloadParallelBatchPartitionSet>,
) -> Vec<WorkloadParallelBatchPartitionStreak> {
    let mut streaks = Vec::new();
    let mut current: Option<(Vec<PartitionId>, usize)> = None;
    for set in sets {
        if set.is_empty() {
            if let Some((partitions, consecutive_batch_count)) = current.take() {
                streaks.push(WorkloadParallelBatchPartitionStreak::new(
                    partitions,
                    consecutive_batch_count,
                ));
            }
            continue;
        }
        let partitions = set.partitions().to_vec();
        match current.as_mut() {
            Some((current_partitions, current_count))
                if current_partitions.as_slice() == partitions.as_slice() =>
            {
                *current_count += set.batch_count();
            }
            _ => {
                if let Some((partitions, consecutive_batch_count)) = current.take() {
                    streaks.push(WorkloadParallelBatchPartitionStreak::new(
                        partitions,
                        consecutive_batch_count,
                    ));
                }
                current = Some((partitions, set.batch_count()));
            }
        }
    }
    if let Some((partitions, consecutive_batch_count)) = current {
        streaks.push(WorkloadParallelBatchPartitionStreak::new(
            partitions,
            consecutive_batch_count,
        ));
    }
    collect_parallel_batch_partition_streaks(streaks)
}

pub(crate) fn parallel_batch_count_at_or_above(
    counts: &[WorkloadParallelBatchWorkerCount],
    minimum_worker_count: usize,
) -> usize {
    counts
        .iter()
        .filter(|count| count.worker_count() >= minimum_worker_count)
        .map(WorkloadParallelBatchWorkerCount::batch_count)
        .sum()
}

pub(crate) fn parallel_batch_count_for_worker_count(
    counts: &[WorkloadParallelBatchWorkerCount],
    worker_count: usize,
) -> usize {
    counts
        .iter()
        .filter(|count| count.worker_count() == worker_count)
        .map(WorkloadParallelBatchWorkerCount::batch_count)
        .sum()
}

pub(crate) fn parallel_batch_activity_count_at_or_above(
    counts: &[WorkloadParallelBatchWorkerCount],
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
    minimum_worker_count: usize,
) -> usize {
    parallel_batch_count_at_or_above(counts, minimum_worker_count).max(
        parallel_batch_partition_set_count_at_or_above(sets, minimum_worker_count).max(
            parallel_batch_partition_streak_count_at_or_above(streaks, minimum_worker_count),
        ),
    )
}

pub(crate) fn parallel_batch_activity_count_for_worker_count(
    counts: &[WorkloadParallelBatchWorkerCount],
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
    worker_count: usize,
) -> usize {
    parallel_batch_count_for_worker_count(counts, worker_count).max(
        parallel_batch_partition_set_count_for_worker_count(sets, worker_count).max(
            parallel_batch_partition_streak_count_for_worker_count(streaks, worker_count),
        ),
    )
}

fn parallel_batch_partition_set_count_at_or_above(
    sets: &[WorkloadParallelBatchPartitionSet],
    minimum_worker_count: usize,
) -> usize {
    sets.iter()
        .filter(|set| set.partitions().len() >= minimum_worker_count)
        .map(WorkloadParallelBatchPartitionSet::batch_count)
        .sum()
}

fn parallel_batch_partition_set_count_for_worker_count(
    sets: &[WorkloadParallelBatchPartitionSet],
    worker_count: usize,
) -> usize {
    sets.iter()
        .filter(|set| set.partitions().len() == worker_count)
        .map(WorkloadParallelBatchPartitionSet::batch_count)
        .sum()
}

fn parallel_batch_partition_streak_count_at_or_above(
    streaks: &[WorkloadParallelBatchPartitionStreak],
    minimum_worker_count: usize,
) -> usize {
    streaks
        .iter()
        .filter(|streak| streak.partitions().len() >= minimum_worker_count)
        .map(WorkloadParallelBatchPartitionStreak::consecutive_batch_count)
        .sum()
}

fn parallel_batch_partition_streak_count_for_worker_count(
    streaks: &[WorkloadParallelBatchPartitionStreak],
    worker_count: usize,
) -> usize {
    streaks
        .iter()
        .filter(|streak| streak.partitions().len() == worker_count)
        .map(WorkloadParallelBatchPartitionStreak::consecutive_batch_count)
        .sum()
}

pub(crate) fn max_parallel_batch_worker_count(
    counts: &[WorkloadParallelBatchWorkerCount],
) -> usize {
    counts
        .iter()
        .map(WorkloadParallelBatchWorkerCount::worker_count)
        .max()
        .unwrap_or(0)
}

pub(crate) fn max_parallel_batch_activity_worker_count(
    counts: &[WorkloadParallelBatchWorkerCount],
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
) -> usize {
    max_parallel_batch_worker_count(counts)
        .max(max_parallel_batch_partition_set_worker_count(sets))
        .max(max_parallel_batch_partition_streak_worker_count(streaks))
}

fn max_parallel_batch_partition_set_worker_count(
    sets: &[WorkloadParallelBatchPartitionSet],
) -> usize {
    sets.iter()
        .map(|set| set.partitions().len())
        .max()
        .unwrap_or(0)
}

fn max_parallel_batch_partition_streak_worker_count(
    streaks: &[WorkloadParallelBatchPartitionStreak],
) -> usize {
    streaks
        .iter()
        .map(|streak| streak.partitions().len())
        .max()
        .unwrap_or(0)
}

pub(crate) fn total_parallel_batch_worker_count(
    counts: &[WorkloadParallelBatchWorkerCount],
) -> usize {
    counts
        .iter()
        .map(|count| count.worker_count() * count.batch_count())
        .sum()
}

pub(crate) fn total_parallel_batch_count(counts: &[WorkloadParallelBatchWorkerCount]) -> usize {
    counts
        .iter()
        .map(WorkloadParallelBatchWorkerCount::batch_count)
        .sum()
}

pub(crate) fn strongest_parallel_batch_count(
    counts: &[WorkloadParallelBatchWorkerCount],
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
) -> usize {
    total_parallel_batch_count(counts)
        .max(total_parallel_batch_partition_set_count(sets))
        .max(total_parallel_batch_partition_streak_count(streaks))
}

pub(crate) fn total_parallel_batch_activity_worker_count(
    counts: &[WorkloadParallelBatchWorkerCount],
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
) -> usize {
    total_parallel_batch_worker_count(counts)
        .max(total_parallel_batch_partition_set_worker_count(sets))
        .max(total_parallel_batch_partition_streak_worker_count(streaks))
}

pub(crate) fn parallel_batch_active_partition_count(
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
) -> usize {
    let mut partitions = BTreeSet::new();
    collect_parallel_batch_active_partitions(&mut partitions, sets);
    collect_parallel_batch_streak_active_partitions(&mut partitions, streaks);
    partitions.len()
}

pub(crate) fn parallel_batch_partition_activity_for_partition(
    sets: &[WorkloadParallelBatchPartitionSet],
    partition: PartitionId,
) -> Option<ParallelPartitionActivity> {
    let batch_count = sets
        .iter()
        .filter(|set| set.partitions().contains(&partition))
        .map(WorkloadParallelBatchPartitionSet::batch_count)
        .sum();
    if batch_count == 0 {
        return None;
    }
    Some(ParallelPartitionActivity::with_remote_counts(
        batch_count,
        batch_count,
        0,
        0,
        0,
    ))
}

pub(crate) fn parallel_batch_streak_activity_for_partition(
    streaks: &[WorkloadParallelBatchPartitionStreak],
    partition: PartitionId,
) -> Option<ParallelPartitionActivity> {
    let batch_count = streaks
        .iter()
        .filter(|streak| streak.partitions().contains(&partition))
        .map(WorkloadParallelBatchPartitionStreak::consecutive_batch_count)
        .sum();
    if batch_count == 0 {
        return None;
    }
    Some(ParallelPartitionActivity::with_remote_counts(
        batch_count,
        batch_count,
        0,
        0,
        0,
    ))
}

fn collect_parallel_batch_active_partitions(
    partitions: &mut BTreeSet<PartitionId>,
    sets: &[WorkloadParallelBatchPartitionSet],
) {
    for set in sets {
        partitions.extend(set.partitions().iter().copied());
    }
}

fn collect_parallel_batch_streak_active_partitions(
    partitions: &mut BTreeSet<PartitionId>,
    streaks: &[WorkloadParallelBatchPartitionStreak],
) {
    for streak in streaks {
        partitions.extend(streak.partitions().iter().copied());
    }
}

fn total_parallel_batch_partition_set_worker_count(
    sets: &[WorkloadParallelBatchPartitionSet],
) -> usize {
    sets.iter()
        .map(|set| set.partitions().len() * set.batch_count())
        .sum()
}

fn total_parallel_batch_partition_streak_worker_count(
    streaks: &[WorkloadParallelBatchPartitionStreak],
) -> usize {
    streaks
        .iter()
        .map(|streak| streak.partitions().len() * streak.consecutive_batch_count())
        .sum()
}

fn total_parallel_batch_partition_set_count(sets: &[WorkloadParallelBatchPartitionSet]) -> usize {
    sets.iter()
        .map(WorkloadParallelBatchPartitionSet::batch_count)
        .sum()
}

fn total_parallel_batch_partition_streak_count(
    streaks: &[WorkloadParallelBatchPartitionStreak],
) -> usize {
    streaks
        .iter()
        .map(WorkloadParallelBatchPartitionStreak::consecutive_batch_count)
        .sum()
}

pub(crate) fn parallel_batch_count_for_partition_set(
    sets: &[WorkloadParallelBatchPartitionSet],
    streaks: &[WorkloadParallelBatchPartitionStreak],
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let partitions = normalize_partition_set(partitions);
    let set_count = sets
        .iter()
        .find(|set| set.partitions() == partitions.as_slice())
        .map(WorkloadParallelBatchPartitionSet::batch_count)
        .unwrap_or(0);
    let streak_count = streaks
        .iter()
        .find(|streak| streak.partitions() == partitions.as_slice())
        .map(WorkloadParallelBatchPartitionStreak::consecutive_batch_count)
        .unwrap_or(0);
    set_count.max(streak_count)
}

pub(crate) fn parallel_batch_streak_count_for_partition_set(
    streaks: &[WorkloadParallelBatchPartitionStreak],
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let partitions = normalize_partition_set(partitions);
    streaks
        .iter()
        .find(|streak| streak.partitions() == partitions.as_slice())
        .map(WorkloadParallelBatchPartitionStreak::consecutive_batch_count)
        .unwrap_or(0)
}
