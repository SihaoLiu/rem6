use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{ParallelEpochBatchRecord, PartitionId, RecordedConservativeRunSummary, Tick};
use rem6_workload::{
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerCount,
};

#[derive(Debug, Default)]
pub(super) struct DmaSchedulerEvidence {
    pub(super) epoch_count: usize,
    pub(super) dispatch_count: usize,
    pub(super) batch_count: usize,
    pub(super) batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    pub(super) batch_worker_counts: BTreeMap<usize, usize>,
    pub(super) batch_worker_count_ticks: BTreeMap<usize, Tick>,
}

impl DmaSchedulerEvidence {
    pub(super) fn merge_run(
        &mut self,
        scope: WorkloadParallelBatchScope,
        run: &RecordedConservativeRunSummary,
    ) {
        self.epoch_count += run.epoch_count();
        self.dispatch_count += run.dispatch_count();
        self.batch_count += run.batch_count();
        self.batch_timeline.extend(
            run.batches()
                .iter()
                .map(|batch| dma_scheduler_batch_timeline_record(scope, batch)),
        );
        for (worker_count, count) in run.batch_worker_count_summaries() {
            let stored = self.batch_worker_counts.entry(worker_count).or_default();
            *stored = stored.saturating_add(count);
        }
        for (worker_count, ticks) in run.batch_worker_count_tick_summaries() {
            let stored = self
                .batch_worker_count_ticks
                .entry(worker_count)
                .or_default();
            *stored = stored.saturating_add(ticks);
        }
    }
}

pub(super) fn dma_scheduler_batch_timeline(
    mut records: Vec<WorkloadParallelBatchTimelineRecord>,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    records.retain(|record| !record.is_empty());
    records.sort_by_key(|record| {
        (
            record.start_tick(),
            record.horizon(),
            record.scope(),
            record.partitions().to_vec(),
        )
    });
    records
}

pub(super) fn dma_scheduler_batch_worker_counts(
    batch_worker_counts: BTreeMap<usize, usize>,
) -> Vec<WorkloadParallelBatchWorkerCount> {
    batch_worker_counts
        .into_iter()
        .filter(|(worker_count, count)| *worker_count != 0 && *count != 0)
        .map(|(worker_count, count)| WorkloadParallelBatchWorkerCount::new(worker_count, count))
        .collect()
}

pub(super) fn dma_scheduler_batch_worker_count_ticks(
    batch_worker_count_ticks: BTreeMap<usize, Tick>,
) -> Vec<(usize, Tick)> {
    batch_worker_count_ticks
        .into_iter()
        .filter(|(worker_count, ticks)| *worker_count != 0 && *ticks != 0)
        .collect()
}

fn dma_scheduler_batch_timeline_record(
    scope: WorkloadParallelBatchScope,
    batch: &ParallelEpochBatchRecord,
) -> WorkloadParallelBatchTimelineRecord {
    WorkloadParallelBatchTimelineRecord::new(
        scope,
        batch_start_tick(batch),
        batch.horizon(),
        normalize_partition_set(batch.worker_partitions()),
        batch.worker_count(),
    )
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
