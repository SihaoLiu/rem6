use std::collections::BTreeSet;

use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, ParallelRemoteSendRecord, PartitionId,
};

pub(crate) fn parallel_active_partition_count(
    activities: &[(PartitionId, ParallelPartitionActivity)],
    flows: &[ParallelRemoteFlowRecord],
    sends: &[ParallelRemoteSendRecord],
) -> usize {
    let mut partitions = BTreeSet::new();
    collect_active_partitions(&mut partitions, activities, flows, sends);
    partitions.len()
}

pub(crate) fn combined_parallel_active_partition_count(
    left_activities: &[(PartitionId, ParallelPartitionActivity)],
    left_flows: &[ParallelRemoteFlowRecord],
    left_sends: &[ParallelRemoteSendRecord],
    right_activities: &[(PartitionId, ParallelPartitionActivity)],
    right_flows: &[ParallelRemoteFlowRecord],
    right_sends: &[ParallelRemoteSendRecord],
) -> usize {
    let mut partitions = BTreeSet::new();
    collect_active_partitions(&mut partitions, left_activities, left_flows, left_sends);
    collect_active_partitions(&mut partitions, right_activities, right_flows, right_sends);
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
        .filter_map(|(_, activity)| {
            let worker_count = activity.worker_count();
            (worker_count >= 2).then_some(worker_count)
        })
        .sum()
}

pub(crate) fn parallel_partition_activity_for_partition(
    activities: &[(PartitionId, ParallelPartitionActivity)],
    flows: &[ParallelRemoteFlowRecord],
    sends: &[ParallelRemoteSendRecord],
    partition: PartitionId,
) -> Option<ParallelPartitionActivity> {
    let explicit = activities
        .iter()
        .find(|(existing, _)| *existing == partition)
        .map(|(_, activity)| *activity);
    merge_parallel_partition_activity_evidence_options(
        merge_parallel_partition_activity_evidence_options(
            explicit,
            parallel_remote_flow_partition_activity(flows, partition),
        ),
        parallel_remote_send_partition_activity(sends, partition),
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

pub(crate) fn merge_parallel_partition_activity_evidence_options(
    left: Option<ParallelPartitionActivity>,
    right: Option<ParallelPartitionActivity>,
) -> Option<ParallelPartitionActivity> {
    match (left, right) {
        (Some(left), Some(right)) => Some(ParallelPartitionActivity::with_remote_counts(
            left.worker_count().max(right.worker_count()),
            left.dispatch_count().max(right.dispatch_count()),
            left.remote_send_count().max(right.remote_send_count()),
            left.remote_receive_count()
                .max(right.remote_receive_count()),
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
    sends: &[ParallelRemoteSendRecord],
) {
    partitions.extend(activities.iter().map(|(partition, _)| *partition));
    for flow in flows {
        partitions.insert(flow.source());
        partitions.insert(flow.target());
    }
    for send in sends {
        partitions.insert(send.source());
        partitions.insert(send.target());
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

fn parallel_remote_send_partition_activity(
    sends: &[ParallelRemoteSendRecord],
    partition: PartitionId,
) -> Option<ParallelPartitionActivity> {
    let remote_send_count = sends
        .iter()
        .filter(|send| send.source() == partition)
        .count();
    let remote_receive_count = sends
        .iter()
        .filter(|send| send.target() == partition)
        .count();
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
