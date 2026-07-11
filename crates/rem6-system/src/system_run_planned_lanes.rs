use std::collections::BTreeMap;

use rem6_kernel::{ParallelEpochPlannedWorkerRecord, PartitionId, Tick};

use crate::{RiscvSystemParallelBatchScope, RiscvSystemRun};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct RiscvSystemParallelBatchWorkerLaneRecord {
    scope: RiscvSystemParallelBatchScope,
    lane: usize,
    partition: PartitionId,
    start_tick: Tick,
    horizon: Tick,
    duration_ticks: Tick,
}

impl RiscvSystemParallelBatchWorkerLaneRecord {
    pub const fn new(
        scope: RiscvSystemParallelBatchScope,
        lane: usize,
        partition: PartitionId,
        start_tick: Tick,
        horizon: Tick,
    ) -> Self {
        Self {
            scope,
            lane,
            partition,
            start_tick,
            horizon,
            duration_ticks: horizon.saturating_sub(start_tick),
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

    pub const fn horizon(self) -> Tick {
        self.horizon
    }

    pub const fn duration_ticks(self) -> Tick {
        self.duration_ticks
    }

    pub const fn is_empty(self) -> bool {
        self.horizon <= self.start_tick
    }
}

impl RiscvSystemRun {
    pub fn parallel_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> Vec<RiscvSystemParallelBatchWorkerLaneRecord> {
        collect_system_planned_worker_lanes(
            RiscvSystemParallelBatchScope::Scheduler,
            self.parallel_safe_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| epoch.plan().parallel_batch_planned_workers()),
        )
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> Vec<RiscvSystemParallelBatchWorkerLaneRecord> {
        collect_system_planned_worker_lanes(
            RiscvSystemParallelBatchScope::DataCacheScheduler,
            self.data_cache_parallel_scheduler_epochs()
                .into_iter()
                .flat_map(|epoch| epoch.planned_batch_planned_workers()),
        )
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> Vec<RiscvSystemParallelBatchWorkerLaneRecord> {
        let mut lanes = self.parallel_scheduler_planned_batch_worker_lanes();
        lanes.extend(self.data_cache_parallel_scheduler_planned_batch_worker_lanes());
        sort_system_planned_worker_lanes(&mut lanes);
        lanes
    }

    pub fn parallel_scheduler_planned_batch_worker_lane_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_worker_lane_tick_summaries(self.parallel_scheduler_planned_batch_worker_lanes())
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_lane_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_worker_lane_tick_summaries(
            self.data_cache_parallel_scheduler_planned_batch_worker_lanes(),
        )
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_lane_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_worker_lane_tick_summaries(
            self.full_system_parallel_scheduler_planned_batch_worker_lanes(),
        )
    }

    pub fn parallel_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        worker_lane_partition_ticks(
            self.parallel_scheduler_planned_batch_worker_lanes(),
            lane,
            partition,
        )
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        worker_lane_partition_ticks(
            self.data_cache_parallel_scheduler_planned_batch_worker_lanes(),
            lane,
            partition,
        )
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        worker_lane_partition_ticks(
            self.full_system_parallel_scheduler_planned_batch_worker_lanes(),
            lane,
            partition,
        )
    }
}

fn collect_system_planned_worker_lanes(
    scope: RiscvSystemParallelBatchScope,
    lanes: impl IntoIterator<Item = ParallelEpochPlannedWorkerRecord>,
) -> Vec<RiscvSystemParallelBatchWorkerLaneRecord> {
    let mut lanes = lanes
        .into_iter()
        .map(|lane| {
            RiscvSystemParallelBatchWorkerLaneRecord::new(
                scope,
                lane.lane(),
                lane.partition(),
                lane.start_tick(),
                lane.horizon(),
            )
        })
        .filter(|lane| !lane.is_empty())
        .collect::<Vec<_>>();
    sort_system_planned_worker_lanes(&mut lanes);
    lanes
}

fn sort_system_planned_worker_lanes(lanes: &mut [RiscvSystemParallelBatchWorkerLaneRecord]) {
    lanes.sort_by_key(|lane| {
        (
            lane.start_tick(),
            lane.horizon(),
            lane.scope().sort_rank(),
            lane.lane(),
            lane.partition(),
        )
    });
}

fn collect_worker_lane_tick_summaries(
    lanes: impl IntoIterator<Item = RiscvSystemParallelBatchWorkerLaneRecord>,
) -> Vec<(usize, Tick)> {
    let mut summaries = BTreeMap::<usize, Tick>::new();
    for lane in lanes {
        let ticks = summaries.entry(lane.lane()).or_default();
        *ticks = ticks.saturating_add(lane.duration_ticks());
    }
    summaries.into_iter().collect()
}

fn worker_lane_partition_ticks(
    lanes: impl IntoIterator<Item = RiscvSystemParallelBatchWorkerLaneRecord>,
    lane: usize,
    partition: PartitionId,
) -> Tick {
    lanes
        .into_iter()
        .filter(|record| record.lane() == lane && record.partition() == partition)
        .map(RiscvSystemParallelBatchWorkerLaneRecord::duration_ticks)
        .fold(0, Tick::saturating_add)
}
