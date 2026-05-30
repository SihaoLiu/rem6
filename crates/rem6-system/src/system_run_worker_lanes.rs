use std::collections::BTreeMap;

use rem6_kernel::{ParallelWorkerRecord, PartitionId, Tick};

use crate::{RiscvSystemParallelBatchScope, RiscvSystemRun};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvSystemParallelWorkerLaneRecord {
    scope: RiscvSystemParallelBatchScope,
    lane: usize,
    partition: PartitionId,
    start_tick: Tick,
    safe_until: Tick,
    next_tick: Option<Tick>,
    pending_events: usize,
}

impl RiscvSystemParallelWorkerLaneRecord {
    pub const fn new(
        scope: RiscvSystemParallelBatchScope,
        lane: usize,
        partition: PartitionId,
        start_tick: Tick,
        safe_until: Tick,
        next_tick: Option<Tick>,
        pending_events: usize,
    ) -> Self {
        Self {
            scope,
            lane,
            partition,
            start_tick,
            safe_until,
            next_tick,
            pending_events,
        }
    }

    pub const fn scope(self) -> RiscvSystemParallelBatchScope {
        self.scope
    }

    pub const fn lane(self) -> usize {
        self.lane
    }

    pub const fn partition(self) -> PartitionId {
        self.partition
    }

    pub const fn start_tick(self) -> Tick {
        self.start_tick
    }

    pub const fn safe_until(self) -> Tick {
        self.safe_until
    }

    pub const fn next_tick(self) -> Option<Tick> {
        self.next_tick
    }

    pub const fn pending_events(self) -> usize {
        self.pending_events
    }

    pub const fn duration_ticks(self) -> Tick {
        self.safe_until.saturating_sub(self.start_tick)
    }

    pub const fn is_empty(self) -> bool {
        self.safe_until <= self.start_tick
    }
}

impl RiscvSystemRun {
    pub fn parallel_scheduler_worker_lanes(&self) -> Vec<RiscvSystemParallelWorkerLaneRecord> {
        collect_system_worker_lanes(
            RiscvSystemParallelBatchScope::Scheduler,
            self.parallel_scheduler_workers(),
        )
    }

    pub fn data_cache_parallel_scheduler_worker_lanes(
        &self,
    ) -> Vec<RiscvSystemParallelWorkerLaneRecord> {
        collect_system_worker_lanes(
            RiscvSystemParallelBatchScope::DataCacheScheduler,
            self.data_cache_parallel_scheduler_batches()
                .into_iter()
                .flat_map(|batch| batch.workers().to_vec()),
        )
    }

    pub fn full_system_parallel_scheduler_worker_lanes(
        &self,
    ) -> Vec<RiscvSystemParallelWorkerLaneRecord> {
        let mut lanes = self.parallel_scheduler_worker_lanes();
        lanes.extend(self.data_cache_parallel_scheduler_worker_lanes());
        sort_system_worker_lanes(&mut lanes);
        lanes
    }

    pub fn parallel_scheduler_worker_lane_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_worker_lane_tick_summaries(self.parallel_scheduler_worker_lanes())
    }

    pub fn data_cache_parallel_scheduler_worker_lane_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_worker_lane_tick_summaries(self.data_cache_parallel_scheduler_worker_lanes())
    }

    pub fn full_system_parallel_scheduler_worker_lane_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_worker_lane_tick_summaries(self.full_system_parallel_scheduler_worker_lanes())
    }

    pub fn parallel_scheduler_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        worker_lane_partition_ticks(self.parallel_scheduler_worker_lanes(), lane, partition)
    }

    pub fn data_cache_parallel_scheduler_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        worker_lane_partition_ticks(
            self.data_cache_parallel_scheduler_worker_lanes(),
            lane,
            partition,
        )
    }

    pub fn full_system_parallel_scheduler_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        worker_lane_partition_ticks(
            self.full_system_parallel_scheduler_worker_lanes(),
            lane,
            partition,
        )
    }
}

fn collect_system_worker_lanes(
    scope: RiscvSystemParallelBatchScope,
    workers: impl IntoIterator<Item = ParallelWorkerRecord>,
) -> Vec<RiscvSystemParallelWorkerLaneRecord> {
    let mut lanes = workers
        .into_iter()
        .map(|worker| {
            RiscvSystemParallelWorkerLaneRecord::new(
                scope,
                worker.lane(),
                worker.partition(),
                worker.start_tick(),
                worker.safe_until(),
                worker.next_tick(),
                worker.pending_events(),
            )
        })
        .filter(|lane| !lane.is_empty())
        .collect::<Vec<_>>();
    sort_system_worker_lanes(&mut lanes);
    lanes
}

fn sort_system_worker_lanes(lanes: &mut [RiscvSystemParallelWorkerLaneRecord]) {
    lanes.sort_by_key(|lane| {
        (
            lane.start_tick(),
            lane.safe_until(),
            lane.scope().sort_rank(),
            lane.lane(),
            lane.partition(),
            lane.next_tick(),
            lane.pending_events(),
        )
    });
}

fn collect_worker_lane_tick_summaries(
    lanes: impl IntoIterator<Item = RiscvSystemParallelWorkerLaneRecord>,
) -> Vec<(usize, Tick)> {
    let mut summaries = BTreeMap::<usize, Tick>::new();
    for lane in lanes {
        let ticks = summaries.entry(lane.lane()).or_default();
        *ticks = ticks.saturating_add(lane.duration_ticks());
    }
    summaries.into_iter().collect()
}

fn worker_lane_partition_ticks(
    lanes: impl IntoIterator<Item = RiscvSystemParallelWorkerLaneRecord>,
    lane: usize,
    partition: PartitionId,
) -> Tick {
    lanes
        .into_iter()
        .filter(|record| record.lane() == lane && record.partition() == partition)
        .map(RiscvSystemParallelWorkerLaneRecord::duration_ticks)
        .fold(0, Tick::saturating_add)
}
