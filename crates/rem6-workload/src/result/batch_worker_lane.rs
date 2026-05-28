use rem6_kernel::{PartitionId, Tick};

use crate::parallel_batch::{
    collect_parallel_batch_worker_lane_records, collect_parallel_batch_worker_lane_tick_summaries,
    parallel_batch_worker_lane_partition_ticks, WorkloadParallelBatchScope,
    WorkloadParallelBatchWorkerLaneRecord,
};

use super::WorkloadParallelExecutionSummary;

impl WorkloadParallelExecutionSummary {
    pub fn with_parallel_scheduler_planned_batch_worker_lanes(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchWorkerLaneRecord>,
    ) -> Self {
        self.parallel_scheduler_planned_batch_worker_lanes =
            collect_scoped_parallel_batch_worker_lanes(
                WorkloadParallelBatchScope::Scheduler,
                records,
            );
        self
    }

    pub fn with_data_cache_parallel_scheduler_planned_batch_worker_lanes(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchWorkerLaneRecord>,
    ) -> Self {
        self.data_cache_parallel_scheduler_planned_batch_worker_lanes =
            collect_scoped_parallel_batch_worker_lanes(
                WorkloadParallelBatchScope::DataCacheScheduler,
                records,
            );
        self
    }

    pub fn with_gpu_dma_scheduler_planned_batch_worker_lanes(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchWorkerLaneRecord>,
    ) -> Self {
        self.gpu_dma_scheduler_planned_batch_worker_lanes =
            collect_scoped_parallel_batch_worker_lanes(
                WorkloadParallelBatchScope::GpuDmaScheduler,
                records,
            );
        self
    }

    pub fn with_accelerator_dma_scheduler_planned_batch_worker_lanes(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchWorkerLaneRecord>,
    ) -> Self {
        self.accelerator_dma_scheduler_planned_batch_worker_lanes =
            collect_scoped_parallel_batch_worker_lanes(
                WorkloadParallelBatchScope::AcceleratorDmaScheduler,
                records,
            );
        self
    }

    pub fn with_full_system_parallel_scheduler_planned_batch_worker_lanes(
        mut self,
        records: impl IntoIterator<Item = WorkloadParallelBatchWorkerLaneRecord>,
    ) -> Self {
        self.full_system_parallel_scheduler_planned_batch_worker_lanes =
            collect_parallel_batch_worker_lane_records(records);
        self
    }

    pub fn parallel_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> &[WorkloadParallelBatchWorkerLaneRecord] {
        &self.parallel_scheduler_planned_batch_worker_lanes
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> &[WorkloadParallelBatchWorkerLaneRecord] {
        &self.data_cache_parallel_scheduler_planned_batch_worker_lanes
    }

    pub fn gpu_dma_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> &[WorkloadParallelBatchWorkerLaneRecord] {
        &self.gpu_dma_scheduler_planned_batch_worker_lanes
    }

    pub fn accelerator_dma_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> &[WorkloadParallelBatchWorkerLaneRecord] {
        &self.accelerator_dma_scheduler_planned_batch_worker_lanes
    }

    pub fn dma_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerLaneRecord> {
        collect_parallel_batch_worker_lane_records(
            self.gpu_dma_scheduler_planned_batch_worker_lanes
                .iter()
                .copied()
                .chain(
                    self.accelerator_dma_scheduler_planned_batch_worker_lanes
                        .iter()
                        .copied(),
                ),
        )
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerLaneRecord> {
        let scoped_lanes = self.scoped_full_system_parallel_scheduler_planned_batch_worker_lanes();
        if !self.explicit_full_system_parallel_scheduler_planned_batch_worker_lanes_cover_scoped() {
            return scoped_lanes;
        }
        collect_parallel_batch_worker_lane_records(
            scoped_lanes.into_iter().chain(
                self.full_system_parallel_scheduler_planned_batch_worker_lanes
                    .iter()
                    .copied(),
            ),
        )
    }

    pub fn parallel_scheduler_planned_batch_worker_lane_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_lane_tick_summaries(
            self.parallel_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
    }

    pub fn parallel_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        parallel_batch_worker_lane_partition_ticks(
            self.parallel_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
            lane,
            partition,
        )
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_lane_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_lane_tick_summaries(
            self.data_cache_parallel_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
    }

    pub fn data_cache_parallel_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        parallel_batch_worker_lane_partition_ticks(
            self.data_cache_parallel_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
            lane,
            partition,
        )
    }

    pub fn gpu_dma_scheduler_planned_batch_worker_lane_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_lane_tick_summaries(
            self.gpu_dma_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
    }

    pub fn gpu_dma_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        parallel_batch_worker_lane_partition_ticks(
            self.gpu_dma_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
            lane,
            partition,
        )
    }

    pub fn accelerator_dma_scheduler_planned_batch_worker_lane_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_lane_tick_summaries(
            self.accelerator_dma_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
    }

    pub fn accelerator_dma_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        parallel_batch_worker_lane_partition_ticks(
            self.accelerator_dma_scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
            lane,
            partition,
        )
    }

    pub fn dma_scheduler_planned_batch_worker_lane_tick_summaries(&self) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_lane_tick_summaries(
            self.dma_scheduler_planned_batch_worker_lanes(),
        )
    }

    pub fn dma_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        parallel_batch_worker_lane_partition_ticks(
            self.dma_scheduler_planned_batch_worker_lanes(),
            lane,
            partition,
        )
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_lane_tick_summaries(
        &self,
    ) -> Vec<(usize, Tick)> {
        collect_parallel_batch_worker_lane_tick_summaries(
            self.full_system_parallel_scheduler_planned_batch_worker_lanes(),
        )
    }

    pub fn full_system_parallel_scheduler_planned_batch_worker_lane_partition_ticks(
        &self,
        lane: usize,
        partition: PartitionId,
    ) -> Tick {
        parallel_batch_worker_lane_partition_ticks(
            self.full_system_parallel_scheduler_planned_batch_worker_lanes(),
            lane,
            partition,
        )
    }

    fn scoped_full_system_parallel_scheduler_planned_batch_worker_lanes(
        &self,
    ) -> Vec<WorkloadParallelBatchWorkerLaneRecord> {
        collect_parallel_batch_worker_lane_records(
            self.parallel_scheduler_planned_batch_worker_lanes
                .iter()
                .copied()
                .chain(
                    self.data_cache_parallel_scheduler_planned_batch_worker_lanes
                        .iter()
                        .copied(),
                )
                .chain(self.dma_scheduler_planned_batch_worker_lanes()),
        )
    }

    fn explicit_full_system_parallel_scheduler_planned_batch_worker_lanes_cover_scoped(
        &self,
    ) -> bool {
        let scoped_lanes = self.scoped_full_system_parallel_scheduler_planned_batch_worker_lanes();
        scoped_lanes.iter().all(|scoped| {
            self.full_system_parallel_scheduler_planned_batch_worker_lanes
                .iter()
                .any(|record| record == scoped)
        })
    }
}

fn collect_scoped_parallel_batch_worker_lanes(
    scope: WorkloadParallelBatchScope,
    records: impl IntoIterator<Item = WorkloadParallelBatchWorkerLaneRecord>,
) -> Vec<WorkloadParallelBatchWorkerLaneRecord> {
    collect_parallel_batch_worker_lane_records(records.into_iter().map(|record| {
        WorkloadParallelBatchWorkerLaneRecord::new(
            scope,
            record.lane(),
            record.partition(),
            record.start_tick(),
            record.horizon(),
        )
    }))
}
