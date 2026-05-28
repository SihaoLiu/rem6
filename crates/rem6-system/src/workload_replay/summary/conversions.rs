use rem6_workload::{
    WorkloadDataCacheProtocol, WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerLaneRecord,
};

use crate::{
    RiscvDataCacheProtocol, RiscvSystemParallelBatchScope, RiscvSystemParallelBatchTimelineRecord,
    RiscvSystemParallelBatchWorkerLaneRecord,
};

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
