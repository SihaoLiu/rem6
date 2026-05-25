use std::collections::BTreeSet;

use rem6_kernel::{ParallelPartitionActivity, ParallelRemoteFlowRecord, PartitionId};

pub(crate) fn parallel_active_partition_count(
    activities: &[(PartitionId, ParallelPartitionActivity)],
    flows: &[ParallelRemoteFlowRecord],
) -> usize {
    let mut partitions = BTreeSet::new();
    collect_active_partitions(&mut partitions, activities, flows);
    partitions.len()
}

pub(crate) fn combined_parallel_active_partition_count(
    left_activities: &[(PartitionId, ParallelPartitionActivity)],
    left_flows: &[ParallelRemoteFlowRecord],
    right_activities: &[(PartitionId, ParallelPartitionActivity)],
    right_flows: &[ParallelRemoteFlowRecord],
) -> usize {
    let mut partitions = BTreeSet::new();
    collect_active_partitions(&mut partitions, left_activities, left_flows);
    collect_active_partitions(&mut partitions, right_activities, right_flows);
    partitions.len()
}

pub(crate) fn parallel_partition_dispatch_count(
    activities: &[(PartitionId, ParallelPartitionActivity)],
) -> usize {
    activities
        .iter()
        .map(|(_, activity)| activity.dispatch_count())
        .sum()
}

pub(crate) fn parallel_partition_worker_count(
    activities: &[(PartitionId, ParallelPartitionActivity)],
) -> usize {
    activities
        .iter()
        .map(|(_, activity)| activity.worker_count())
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

fn collect_active_partitions(
    partitions: &mut BTreeSet<PartitionId>,
    activities: &[(PartitionId, ParallelPartitionActivity)],
    flows: &[ParallelRemoteFlowRecord],
) {
    partitions.extend(activities.iter().map(|(partition, _)| *partition));
    for flow in flows {
        partitions.insert(flow.source());
        partitions.insert(flow.target());
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
