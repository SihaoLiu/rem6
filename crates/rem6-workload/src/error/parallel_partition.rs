use crate::WorkloadParallelBatchPartitionScope;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelPartitionCountMergeSummary {
    pub scope: WorkloadParallelBatchPartitionScope,
    pub merged_active_partitions: usize,
    pub lower_bound_active_partitions: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadParallelPartitionActivityMergeSummary {
    pub scope: WorkloadParallelBatchPartitionScope,
    pub partition: u32,
    pub merged_worker_count: usize,
    pub lower_bound_worker_count: usize,
    pub merged_dispatch_count: usize,
    pub lower_bound_dispatch_count: usize,
    pub merged_remote_send_count: usize,
    pub lower_bound_remote_send_count: usize,
    pub merged_remote_receive_count: usize,
    pub lower_bound_remote_receive_count: usize,
    pub merged_max_pending_events: usize,
    pub lower_bound_max_pending_events: usize,
}
