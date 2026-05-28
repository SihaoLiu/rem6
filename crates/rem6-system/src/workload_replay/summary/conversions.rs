use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerLaneRecord,
};

use crate::{
    RiscvDataCacheProtocol, RiscvSystemParallelBatchScope, RiscvSystemParallelBatchTimelineRecord,
    RiscvSystemParallelBatchWorkerLaneRecord, RiscvSystemRun,
};

use super::WorkloadReplayActivityRefs;

pub(super) fn workload_data_cache_protocol(
    protocol: RiscvDataCacheProtocol,
) -> WorkloadDataCacheProtocol {
    match protocol {
        RiscvDataCacheProtocol::Msi => WorkloadDataCacheProtocol::Msi,
        RiscvDataCacheProtocol::Mesi => WorkloadDataCacheProtocol::Mesi,
        RiscvDataCacheProtocol::Moesi => WorkloadDataCacheProtocol::Moesi,
        RiscvDataCacheProtocol::Chi => WorkloadDataCacheProtocol::Chi,
    }
}

pub(super) fn workload_parallel_batch_timeline_record(
    record: RiscvSystemParallelBatchTimelineRecord,
) -> WorkloadParallelBatchTimelineRecord {
    WorkloadParallelBatchTimelineRecord::new(
        workload_parallel_batch_scope(record.scope()),
        record.start_tick(),
        record.horizon(),
        record.partitions().iter().copied(),
        record.worker_count(),
    )
}

pub(super) fn workload_parallel_batch_worker_lane_record(
    record: RiscvSystemParallelBatchWorkerLaneRecord,
) -> WorkloadParallelBatchWorkerLaneRecord {
    WorkloadParallelBatchWorkerLaneRecord::new(
        workload_parallel_batch_scope(record.scope()),
        record.lane(),
        record.partition(),
        record.start_tick(),
        record.horizon(),
    )
}

fn workload_parallel_batch_scope(
    scope: RiscvSystemParallelBatchScope,
) -> WorkloadParallelBatchScope {
    match scope {
        RiscvSystemParallelBatchScope::Scheduler => WorkloadParallelBatchScope::Scheduler,
        RiscvSystemParallelBatchScope::DataCacheScheduler => {
            WorkloadParallelBatchScope::DataCacheScheduler
        }
    }
}

pub(super) fn full_system_planned_batch_timeline(
    run: &RiscvSystemRun,
    activities: &WorkloadReplayActivityRefs<'_>,
) -> Vec<WorkloadParallelBatchTimelineRecord> {
    run.full_system_parallel_scheduler_planned_batch_timeline()
        .into_iter()
        .map(workload_parallel_batch_timeline_record)
        .chain(
            activities
                .gpu_dma
                .scheduler_planned_batch_timeline
                .iter()
                .cloned(),
        )
        .chain(
            activities
                .accelerator_dma
                .scheduler_planned_batch_timeline
                .iter()
                .cloned(),
        )
        .collect()
}

pub(super) fn full_system_planned_batch_worker_lanes(
    run: &RiscvSystemRun,
    activities: &WorkloadReplayActivityRefs<'_>,
) -> Vec<WorkloadParallelBatchWorkerLaneRecord> {
    run.full_system_parallel_scheduler_planned_batch_worker_lanes()
        .into_iter()
        .map(workload_parallel_batch_worker_lane_record)
        .chain(
            activities
                .gpu_dma
                .scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
        .chain(
            activities
                .accelerator_dma
                .scheduler_planned_batch_worker_lanes
                .iter()
                .copied(),
        )
        .collect()
}

pub(super) fn full_system_planned_batch_worker_capacity_ticks(
    run: &RiscvSystemRun,
    activities: &WorkloadReplayActivityRefs<'_>,
) -> rem6_kernel::Tick {
    run.full_system_parallel_scheduler_planned_batch_worker_capacity_ticks()
        .saturating_add(
            activities
                .gpu_dma
                .scheduler_planned_batch_worker_capacity_ticks,
        )
        .saturating_add(
            activities
                .accelerator_dma
                .scheduler_planned_batch_worker_capacity_ticks,
        )
}
