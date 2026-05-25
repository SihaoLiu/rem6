use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{ParallelEpochBatchRecord, PartitionId};

use crate::RiscvSystemRun;

impl RiscvSystemRun {
    pub fn parallel_scheduler_batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_batch_worker_count_summaries(self.parallel_scheduler_batches())
    }

    pub fn data_cache_parallel_scheduler_batch_worker_count_summaries(
        &self,
    ) -> Vec<(usize, usize)> {
        collect_batch_worker_count_summaries(self.data_cache_parallel_scheduler_batches())
    }

    pub fn full_system_parallel_scheduler_batch_worker_count_summaries(
        &self,
    ) -> Vec<(usize, usize)> {
        collect_batch_worker_count_summaries(self.full_system_parallel_scheduler_batches())
    }

    pub fn parallel_scheduler_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        batch_count_at_or_above(self.parallel_scheduler_batches(), minimum_worker_count)
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
        collect_batch_partition_set_summaries(self.parallel_scheduler_batches())
    }

    pub fn data_cache_parallel_scheduler_batch_partition_set_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_batch_partition_set_summaries(self.data_cache_parallel_scheduler_batches())
    }

    pub fn full_system_parallel_scheduler_batch_partition_set_summaries(
        &self,
    ) -> Vec<(Vec<PartitionId>, usize)> {
        collect_batch_partition_set_summaries(self.full_system_parallel_scheduler_batches())
    }

    pub fn parallel_scheduler_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        batch_count_for_partition_set(self.parallel_scheduler_batches(), partitions)
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

fn batch_count_at_or_above(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
    minimum_worker_count: usize,
) -> usize {
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .count()
}

fn collect_batch_partition_set_summaries(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
) -> Vec<(Vec<PartitionId>, usize)> {
    let mut summaries = BTreeMap::<Vec<PartitionId>, usize>::new();
    for batch in batches {
        let partitions = normalize_partition_set(batch.worker_partitions());
        if !partitions.is_empty() {
            *summaries.entry(partitions).or_default() += 1;
        }
    }
    summaries.into_iter().collect()
}

fn batch_count_for_partition_set(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize {
    let expected = normalize_partition_set(partitions);
    batches
        .into_iter()
        .filter(|batch| normalize_partition_set(batch.worker_partitions()) == expected)
        .count()
}

fn collect_batch_partition_streak_summaries(
    batches: impl IntoIterator<Item = ParallelEpochBatchRecord>,
) -> Vec<(Vec<PartitionId>, usize)> {
    let mut summaries = BTreeMap::<Vec<PartitionId>, usize>::new();
    let mut current: Option<(Vec<PartitionId>, usize)> = None;
    for batch in batches {
        let partitions = normalize_partition_set(batch.worker_partitions());
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

fn normalize_partition_set(partitions: impl IntoIterator<Item = PartitionId>) -> Vec<PartitionId> {
    partitions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
