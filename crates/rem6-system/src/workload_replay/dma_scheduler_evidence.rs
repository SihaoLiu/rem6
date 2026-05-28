use std::collections::{BTreeMap, BTreeSet};

use rem6_kernel::{
    ParallelEpochBatchRecord, ParallelEpochPlannedWorkerRecord, ParallelRemoteFlowRecord,
    ParallelRemoteSendRecord, PartitionFrontier, PartitionId, RecordedConservativeRunSummary, Tick,
};
use rem6_workload::{
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerCount, WorkloadParallelBatchWorkerLaneRecord,
};

#[derive(Debug, Default)]
pub(super) struct DmaSchedulerEvidence {
    pub(super) epoch_count: usize,
    pub(super) empty_epoch_count: usize,
    pub(super) dispatch_count: usize,
    pub(super) batch_count: usize,
    pub(super) batch_timeline: Vec<WorkloadParallelBatchTimelineRecord>,
    pub(super) batch_worker_counts: BTreeMap<usize, usize>,
    pub(super) batch_worker_count_ticks: BTreeMap<usize, Tick>,
    pub(super) planned_batch_worker_lanes: Vec<WorkloadParallelBatchWorkerLaneRecord>,
    pub(super) recorded_batch_worker_capacity_ticks: Tick,
    pub(super) recorded_batch_worker_slot_tick_summaries: BTreeMap<usize, (Tick, Tick)>,
    pub(super) initial_frontiers: Vec<PartitionFrontier>,
    pub(super) final_frontiers: Vec<PartitionFrontier>,
    pub(super) remote_flows: Vec<ParallelRemoteFlowRecord>,
    pub(super) remote_sends: Vec<ParallelRemoteSendRecord>,
}

impl DmaSchedulerEvidence {
    pub(super) fn merge_run(
        &mut self,
        scope: WorkloadParallelBatchScope,
        run: &RecordedConservativeRunSummary,
    ) {
        self.epoch_count += run.epoch_count();
        self.empty_epoch_count += run.empty_epoch_count();
        self.dispatch_count += run.dispatch_count();
        self.batch_count += run.batch_count();
        self.recorded_batch_worker_capacity_ticks = self
            .recorded_batch_worker_capacity_ticks
            .saturating_add(run.batch_worker_capacity_ticks());
        self.batch_timeline.extend(
            run.batches()
                .iter()
                .map(|batch| dma_scheduler_batch_timeline_record(scope, batch)),
        );
        self.planned_batch_worker_lanes.extend(
            run.planned_batch_planned_workers()
                .into_iter()
                .map(|record| dma_scheduler_planned_worker_lane_record(scope, record)),
        );
        self.initial_frontiers
            .extend(run.initial_frontiers().iter().copied());
        self.final_frontiers
            .extend(run.final_frontiers().iter().copied());
        self.remote_flows.extend(run.remote_flows());
        self.remote_sends.extend(run.remote_sends());
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
        for (worker_slot, active_ticks, idle_ticks) in run.batch_worker_slot_tick_summaries() {
            let stored = self
                .recorded_batch_worker_slot_tick_summaries
                .entry(worker_slot)
                .or_default();
            stored.0 = stored.0.saturating_add(active_ticks);
            stored.1 = stored.1.saturating_add(idle_ticks);
        }
    }
}

pub(super) fn dma_scheduler_planned_batch_worker_lanes(
    mut records: Vec<WorkloadParallelBatchWorkerLaneRecord>,
) -> Vec<WorkloadParallelBatchWorkerLaneRecord> {
    records.retain(|record| !record.is_empty());
    records.sort_by_key(|record| {
        (
            record.start_tick(),
            record.horizon(),
            record.scope(),
            record.lane(),
            record.partition(),
        )
    });
    records
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

pub(super) fn dma_scheduler_frontiers(
    mut records: Vec<PartitionFrontier>,
) -> Vec<PartitionFrontier> {
    records.sort_by_key(|frontier| {
        (
            frontier.partition(),
            frontier.now(),
            frontier.safe_until(),
            frontier.next_tick(),
            frontier.pending_events(),
        )
    });
    records
}

pub(super) fn dma_scheduler_remote_flows(
    records: Vec<ParallelRemoteFlowRecord>,
) -> Vec<ParallelRemoteFlowRecord> {
    let mut by_route = BTreeMap::<(PartitionId, PartitionId), ParallelRemoteFlowRecord>::new();
    for record in records {
        if record.send_count() == 0 {
            continue;
        }
        by_route
            .entry((record.source(), record.target()))
            .and_modify(|stored| *stored = stored.merged_with(record))
            .or_insert(record);
    }
    by_route.into_values().collect()
}

pub(super) fn dma_scheduler_remote_sends(
    mut records: Vec<ParallelRemoteSendRecord>,
) -> Vec<ParallelRemoteSendRecord> {
    records.sort_by_key(|record| {
        (
            record.source(),
            record.target(),
            record.source_tick(),
            record.delivery_tick(),
            record.order(),
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

pub(super) fn dma_scheduler_recorded_batch_worker_slot_tick_summaries(
    summaries: BTreeMap<usize, (Tick, Tick)>,
) -> Vec<(usize, Tick, Tick)> {
    summaries
        .into_iter()
        .filter(|(_, (active_ticks, idle_ticks))| *active_ticks != 0 || *idle_ticks != 0)
        .map(|(worker_slot, (active_ticks, idle_ticks))| (worker_slot, active_ticks, idle_ticks))
        .collect()
}

fn dma_scheduler_batch_timeline_record(
    scope: WorkloadParallelBatchScope,
    batch: &ParallelEpochBatchRecord,
) -> WorkloadParallelBatchTimelineRecord {
    WorkloadParallelBatchTimelineRecord::new(
        scope,
        batch.start_tick(),
        batch.horizon(),
        normalize_partition_set(batch.worker_partitions()),
        batch.worker_count(),
    )
}

fn dma_scheduler_planned_worker_lane_record(
    scope: WorkloadParallelBatchScope,
    record: ParallelEpochPlannedWorkerRecord,
) -> WorkloadParallelBatchWorkerLaneRecord {
    WorkloadParallelBatchWorkerLaneRecord::new(
        scope,
        record.lane(),
        record.partition(),
        record.start_tick(),
        record.horizon(),
    )
}

fn normalize_partition_set(partitions: impl IntoIterator<Item = PartitionId>) -> Vec<PartitionId> {
    partitions
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
