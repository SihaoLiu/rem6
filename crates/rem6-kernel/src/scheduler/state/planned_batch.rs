use std::collections::BTreeMap;

use crate::scheduler::PartitionId;
use crate::Tick;

use super::batch_evidence::normalize_partition_set;
use super::{
    ParallelEpochPlan, ReadyPartition, RecordedConservativeRunSummary, RecordedRunSummary,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParallelEpochPlannedBatch {
    horizon: Tick,
    ready_partitions: Vec<ReadyPartition>,
}

impl ParallelEpochPlannedBatch {
    pub fn new(horizon: Tick, ready_partitions: Vec<ReadyPartition>) -> Self {
        Self {
            horizon,
            ready_partitions,
        }
    }

    pub fn horizon(&self) -> Tick {
        self.horizon
    }

    pub fn ready_partitions(&self) -> &[ReadyPartition] {
        &self.ready_partitions
    }

    pub fn worker_count(&self) -> usize {
        self.ready_partitions.len()
    }

    pub fn worker_partitions(&self) -> Vec<PartitionId> {
        self.ready_partitions
            .iter()
            .map(|ready| ready.partition)
            .collect()
    }

    pub fn partition_set(&self) -> Vec<PartitionId> {
        normalize_partition_set(self.worker_partitions())
    }

    pub fn contains_worker(&self, partition: PartitionId) -> bool {
        self.ready_partitions
            .iter()
            .any(|ready| ready.partition == partition)
    }

    pub fn start_tick(&self) -> Tick {
        self.ready_partitions
            .iter()
            .map(|ready| ready.next_tick)
            .min()
            .unwrap_or(self.horizon)
    }

    pub fn duration_ticks(&self) -> Tick {
        self.horizon.saturating_sub(self.start_tick())
    }

    pub fn worker_ticks(&self) -> Tick {
        self.duration_ticks()
            .saturating_mul(self.worker_count() as Tick)
    }
}

impl ParallelEpochPlan {
    pub fn parallel_worker_limit(&self) -> usize {
        self.parallel_worker_limit
    }

    pub fn parallel_batches(&self) -> &[ParallelEpochPlannedBatch] {
        &self.parallel_batches
    }

    pub fn parallel_batch_count(&self) -> usize {
        self.parallel_batches.len()
    }

    pub fn parallel_batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_planned_batch_worker_count_summaries(&self.parallel_batches)
    }

    pub fn parallel_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        planned_batch_count_for_worker_count(&self.parallel_batches, worker_count)
    }

    pub fn parallel_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        planned_batch_count_at_or_above(&self.parallel_batches, minimum_worker_count)
    }

    pub fn parallel_batch_worker_count_total(&self) -> usize {
        planned_batch_worker_count_total(&self.parallel_batches)
    }

    pub fn parallel_batch_max_workers(&self) -> usize {
        planned_batch_max_workers(&self.parallel_batches)
    }

    pub fn parallel_batch_worker_count_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_planned_batch_worker_count_tick_summaries(&self.parallel_batches)
    }

    pub fn parallel_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        planned_batch_ticks_for_worker_count(&self.parallel_batches, worker_count)
    }

    pub fn parallel_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        planned_batch_ticks_at_or_above(&self.parallel_batches, minimum_worker_count)
    }

    pub fn parallel_batch_worker_ticks(&self) -> Tick {
        planned_batch_worker_ticks(&self.parallel_batches)
    }

    pub fn parallel_batch_worker_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        planned_batch_worker_ticks_at_or_above(&self.parallel_batches, minimum_worker_count)
    }

    pub fn parallel_batch_partition_set_summaries(&self) -> Vec<(Vec<PartitionId>, usize)> {
        collect_planned_batch_partition_set_summaries(&self.parallel_batches)
    }

    pub fn parallel_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        planned_batch_count_for_partition_set(&self.parallel_batches, partitions)
    }
}

impl RecordedRunSummary {
    pub fn planned_parallel_worker_limit(&self) -> usize {
        self.planned_parallel_worker_limit
    }

    pub fn planned_batches(&self) -> &[ParallelEpochPlannedBatch] {
        &self.planned_batches
    }

    pub fn planned_batch_count(&self) -> usize {
        self.planned_batches.len()
    }

    pub fn planned_batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_planned_batch_worker_count_summaries(&self.planned_batches)
    }

    pub fn planned_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        planned_batch_count_for_worker_count(&self.planned_batches, worker_count)
    }

    pub fn planned_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        planned_batch_count_at_or_above(&self.planned_batches, minimum_worker_count)
    }

    pub fn planned_batch_worker_count_total(&self) -> usize {
        planned_batch_worker_count_total(&self.planned_batches)
    }

    pub fn planned_batch_max_workers(&self) -> usize {
        planned_batch_max_workers(&self.planned_batches)
    }

    pub fn planned_batch_worker_count_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_planned_batch_worker_count_tick_summaries(&self.planned_batches)
    }

    pub fn planned_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        planned_batch_ticks_for_worker_count(&self.planned_batches, worker_count)
    }

    pub fn planned_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        planned_batch_ticks_at_or_above(&self.planned_batches, minimum_worker_count)
    }

    pub fn planned_batch_worker_ticks(&self) -> Tick {
        planned_batch_worker_ticks(&self.planned_batches)
    }

    pub fn planned_batch_worker_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        planned_batch_worker_ticks_at_or_above(&self.planned_batches, minimum_worker_count)
    }

    pub fn planned_batch_partition_set_summaries(&self) -> Vec<(Vec<PartitionId>, usize)> {
        collect_planned_batch_partition_set_summaries(&self.planned_batches)
    }

    pub fn planned_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        planned_batch_count_for_partition_set(&self.planned_batches, partitions)
    }
}

impl RecordedConservativeRunSummary {
    pub fn planned_batches(&self) -> Vec<ParallelEpochPlannedBatch> {
        self.epochs
            .iter()
            .flat_map(|epoch| epoch.planned_batches().iter().cloned())
            .collect()
    }

    pub fn planned_batch_count(&self) -> usize {
        self.epochs
            .iter()
            .map(RecordedRunSummary::planned_batch_count)
            .sum()
    }

    pub fn planned_batch_worker_count_summaries(&self) -> Vec<(usize, usize)> {
        collect_planned_batch_worker_count_summaries(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
        )
    }

    pub fn planned_batch_count_for_worker_count(&self, worker_count: usize) -> usize {
        planned_batch_count_for_worker_count(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
            worker_count,
        )
    }

    pub fn planned_batch_count_at_or_above(&self, minimum_worker_count: usize) -> usize {
        planned_batch_count_at_or_above(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
            minimum_worker_count,
        )
    }

    pub fn planned_batch_worker_count_total(&self) -> usize {
        planned_batch_worker_count_total(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
        )
    }

    pub fn planned_batch_max_workers(&self) -> usize {
        planned_batch_max_workers(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
        )
    }

    pub fn planned_batch_worker_count_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_planned_batch_worker_count_tick_summaries(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
        )
    }

    pub fn planned_batch_ticks_for_worker_count(&self, worker_count: usize) -> Tick {
        planned_batch_ticks_for_worker_count(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
            worker_count,
        )
    }

    pub fn planned_batch_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        planned_batch_ticks_at_or_above(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
            minimum_worker_count,
        )
    }

    pub fn planned_batch_worker_ticks(&self) -> Tick {
        planned_batch_worker_ticks(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
        )
    }

    pub fn planned_batch_worker_ticks_at_or_above(&self, minimum_worker_count: usize) -> Tick {
        planned_batch_worker_ticks_at_or_above(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
            minimum_worker_count,
        )
    }

    pub fn planned_batch_partition_set_summaries(&self) -> Vec<(Vec<PartitionId>, usize)> {
        collect_planned_batch_partition_set_summaries(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
        )
    }

    pub fn planned_batch_count_for_partition_set(
        &self,
        partitions: impl IntoIterator<Item = PartitionId>,
    ) -> usize {
        planned_batch_count_for_partition_set(
            self.epochs
                .iter()
                .flat_map(|epoch| epoch.planned_batches().iter()),
            partitions,
        )
    }
}

pub(super) fn collect_parallel_epoch_planned_batches(
    horizon: Tick,
    ready_partitions: &[ReadyPartition],
    parallel_worker_limit: usize,
) -> Vec<ParallelEpochPlannedBatch> {
    ready_partitions
        .chunks(parallel_worker_limit.max(1))
        .map(|ready| ParallelEpochPlannedBatch::new(horizon, ready.to_vec()))
        .collect()
}

fn collect_planned_batch_worker_count_summaries<'a, I>(batches: I) -> Vec<(usize, usize)>
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
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

fn planned_batch_count_for_worker_count<'a, I>(batches: I, worker_count: usize) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() == worker_count)
        .count()
}

fn planned_batch_count_at_or_above<'a, I>(batches: I, minimum_worker_count: usize) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .count()
}

fn planned_batch_worker_count_total<'a, I>(batches: I) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .map(ParallelEpochPlannedBatch::worker_count)
        .sum()
}

fn planned_batch_max_workers<'a, I>(batches: I) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .map(ParallelEpochPlannedBatch::worker_count)
        .max()
        .unwrap_or(0)
}

fn collect_planned_batch_worker_count_tick_summaries<'a, I>(batches: I) -> Vec<(usize, Tick)>
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    let mut summaries = BTreeMap::<usize, Tick>::new();
    for batch in batches {
        let worker_count = batch.worker_count();
        let duration_ticks = batch.duration_ticks();
        if worker_count != 0 && duration_ticks != 0 {
            let ticks = summaries.entry(worker_count).or_default();
            *ticks = ticks.saturating_add(duration_ticks);
        }
    }
    summaries.into_iter().collect()
}

fn planned_batch_ticks_for_worker_count<'a, I>(batches: I, worker_count: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() == worker_count)
        .map(ParallelEpochPlannedBatch::duration_ticks)
        .fold(0, Tick::saturating_add)
}

fn planned_batch_ticks_at_or_above<'a, I>(batches: I, minimum_worker_count: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .map(ParallelEpochPlannedBatch::duration_ticks)
        .fold(0, Tick::saturating_add)
}

fn planned_batch_worker_ticks<'a, I>(batches: I) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .map(ParallelEpochPlannedBatch::worker_ticks)
        .fold(0, Tick::saturating_add)
}

fn planned_batch_worker_ticks_at_or_above<'a, I>(batches: I, minimum_worker_count: usize) -> Tick
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    batches
        .into_iter()
        .filter(|batch| batch.worker_count() >= minimum_worker_count)
        .map(ParallelEpochPlannedBatch::worker_ticks)
        .fold(0, Tick::saturating_add)
}

fn collect_planned_batch_partition_set_summaries<'a, I>(
    batches: I,
) -> Vec<(Vec<PartitionId>, usize)>
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
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

fn planned_batch_count_for_partition_set<'a, I>(
    batches: I,
    partitions: impl IntoIterator<Item = PartitionId>,
) -> usize
where
    I: IntoIterator<Item = &'a ParallelEpochPlannedBatch>,
{
    let expected = normalize_partition_set(partitions);
    batches
        .into_iter()
        .filter(|batch| batch.partition_set() == expected)
        .count()
}
