use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::PartitionId;

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

pub(crate) fn parallel_batch_activity_count_at_or_above(
    counts: &[WorkloadParallelBatchWorkerCount],
    sets: &[WorkloadParallelBatchPartitionSet],
    minimum_worker_count: usize,
) -> usize {
    if counts.is_empty() {
        parallel_batch_partition_set_count_at_or_above(sets, minimum_worker_count)
    } else {
        parallel_batch_count_at_or_above(counts, minimum_worker_count)
    }
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
) -> usize {
    if counts.is_empty() {
        max_parallel_batch_partition_set_worker_count(sets)
    } else {
        max_parallel_batch_worker_count(counts)
    }
}

fn max_parallel_batch_partition_set_worker_count(
    sets: &[WorkloadParallelBatchPartitionSet],
) -> usize {
    sets.iter()
        .map(|set| set.partitions().len())
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

pub(crate) fn parallel_batch_count_for_partition_set(
    sets: &[WorkloadParallelBatchPartitionSet],
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let partitions = normalize_partition_set(partitions);
    sets.iter()
        .find(|set| set.partitions() == partitions.as_slice())
        .map(WorkloadParallelBatchPartitionSet::batch_count)
        .unwrap_or(0)
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
