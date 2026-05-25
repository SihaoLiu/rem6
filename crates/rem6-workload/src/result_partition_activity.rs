use rem6_kernel::{ParallelPartitionActivity, ParallelRemoteFlowRecord, PartitionId};

pub(crate) fn parallel_partition_dispatch_count(
    activities: &[(PartitionId, ParallelPartitionActivity)],
) -> usize {
    activities
        .iter()
        .map(|(_, activity)| activity.dispatch_count())
        .sum()
}

pub(crate) fn parallel_partition_activity_for_partition(
    activities: &[(PartitionId, ParallelPartitionActivity)],
    flows: &[ParallelRemoteFlowRecord],
    partition: PartitionId,
) -> Option<ParallelPartitionActivity> {
    let explicit = activities
        .iter()
        .find(|(existing, _)| *existing == partition)
        .map(|(_, activity)| *activity);
    merge_parallel_partition_activity_options(
        explicit,
        parallel_remote_flow_partition_activity(flows, partition),
    )
}

pub(crate) fn merge_parallel_partition_activity_options(
    left: Option<ParallelPartitionActivity>,
    right: Option<ParallelPartitionActivity>,
) -> Option<ParallelPartitionActivity> {
    match (left, right) {
        (Some(left), Some(right)) => Some(ParallelPartitionActivity::with_remote_counts(
            left.worker_count() + right.worker_count(),
            left.dispatch_count() + right.dispatch_count(),
            left.remote_send_count() + right.remote_send_count(),
            left.remote_receive_count() + right.remote_receive_count(),
            left.max_pending_events().max(right.max_pending_events()),
        )),
        (Some(activity), None) | (None, Some(activity)) => Some(activity),
        (None, None) => None,
    }
}

fn parallel_remote_flow_partition_activity(
    flows: &[ParallelRemoteFlowRecord],
    partition: PartitionId,
) -> Option<ParallelPartitionActivity> {
    let remote_send_count: usize = flows
        .iter()
        .filter(|flow| flow.source() == partition)
        .map(|flow| flow.send_count())
        .sum();
    let remote_receive_count: usize = flows
        .iter()
        .filter(|flow| flow.target() == partition)
        .map(|flow| flow.send_count())
        .sum();
    if remote_send_count == 0 && remote_receive_count == 0 {
        return None;
    }
    Some(ParallelPartitionActivity::with_remote_counts(
        0,
        0,
        remote_send_count,
        remote_receive_count,
        0,
    ))
}
