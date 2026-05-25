use std::collections::BTreeMap;

use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::{
    ParallelPartitionActivity, ParallelRemoteFlowRecord, PartitionFrontier, PartitionId,
};

use crate::{WorkloadDramQosPrioritySummary, WorkloadDramQosRequestorSummary};

pub(crate) fn collect_priority_summaries(
    summaries: impl IntoIterator<Item = WorkloadDramQosPrioritySummary>,
) -> Vec<WorkloadDramQosPrioritySummary> {
    let mut by_priority = BTreeMap::<QosPriority, (usize, u64)>::new();
    for summary in summaries {
        if summary.is_empty() {
            continue;
        }
        let entry = by_priority.entry(summary.priority()).or_default();
        entry.0 += summary.access_count();
        entry.1 += summary.byte_count();
    }
    by_priority
        .into_iter()
        .map(|(priority, (access_count, byte_count))| {
            WorkloadDramQosPrioritySummary::new(priority, access_count, byte_count)
        })
        .collect()
}

pub(crate) fn collect_parallel_remote_flows(
    flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
) -> Vec<ParallelRemoteFlowRecord> {
    let mut by_route = BTreeMap::<(PartitionId, PartitionId), ParallelRemoteFlowRecord>::new();
    for flow in flows {
        if flow.send_count() == 0 {
            continue;
        }
        by_route
            .entry((flow.source(), flow.target()))
            .and_modify(|stored| *stored = stored.merged_with(flow))
            .or_insert(flow);
    }
    by_route.into_values().collect()
}

pub(crate) fn collect_partition_frontiers(
    frontiers: impl IntoIterator<Item = PartitionFrontier>,
) -> Vec<PartitionFrontier> {
    let mut frontiers: Vec<_> = frontiers.into_iter().collect();
    frontiers.sort_by_key(|frontier| {
        (
            frontier.partition(),
            frontier.now(),
            frontier.safe_until(),
            frontier.next_tick(),
            frontier.pending_events(),
        )
    });
    frontiers
}

pub(crate) fn collect_conservative_partition_frontiers(
    frontiers: impl IntoIterator<Item = PartitionFrontier>,
) -> Vec<PartitionFrontier> {
    let mut by_partition = BTreeMap::<PartitionId, PartitionFrontier>::new();
    for frontier in frontiers {
        by_partition
            .entry(frontier.partition())
            .and_modify(|stored| *stored = merge_partition_frontiers(*stored, frontier))
            .or_insert(frontier);
    }
    by_partition.into_values().collect()
}

fn merge_partition_frontiers(
    left: PartitionFrontier,
    right: PartitionFrontier,
) -> PartitionFrontier {
    let next_tick = match (left.next_tick(), right.next_tick()) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(left), None) => Some(left),
        (None, Some(right)) => Some(right),
        (None, None) => None,
    };
    PartitionFrontier::new(
        left.partition(),
        left.now().min(right.now()),
        left.safe_until().min(right.safe_until()),
        next_tick,
        left.pending_events().max(right.pending_events()),
    )
}

pub(crate) fn parallel_remote_flow_count(
    flows: &[ParallelRemoteFlowRecord],
    source: PartitionId,
    target: PartitionId,
) -> usize {
    flows
        .iter()
        .find(|flow| flow.source() == source && flow.target() == target)
        .map(|flow| flow.send_count())
        .unwrap_or(0)
}

pub(crate) fn collect_parallel_partition_activities(
    activities: impl IntoIterator<Item = (PartitionId, ParallelPartitionActivity)>,
) -> Vec<(PartitionId, ParallelPartitionActivity)> {
    let mut by_partition = BTreeMap::new();
    for (partition, activity) in activities {
        if !activity.has_activity() {
            continue;
        }
        by_partition
            .entry(partition)
            .and_modify(|stored: &mut ParallelPartitionActivity| {
                *stored = merge_parallel_partition_activity(*stored, activity);
            })
            .or_insert(activity);
    }
    by_partition.into_iter().collect()
}

fn merge_parallel_partition_activity(
    left: ParallelPartitionActivity,
    right: ParallelPartitionActivity,
) -> ParallelPartitionActivity {
    ParallelPartitionActivity::with_remote_counts(
        left.worker_count() + right.worker_count(),
        left.dispatch_count() + right.dispatch_count(),
        left.remote_send_count() + right.remote_send_count(),
        left.remote_receive_count() + right.remote_receive_count(),
        left.max_pending_events().max(right.max_pending_events()),
    )
}

pub(crate) fn collect_requestor_summaries(
    summaries: impl IntoIterator<Item = WorkloadDramQosRequestorSummary>,
) -> Vec<WorkloadDramQosRequestorSummary> {
    let mut by_requestor = BTreeMap::<QosRequestorId, (usize, u64)>::new();
    for summary in summaries {
        if summary.is_empty() {
            continue;
        }
        let entry = by_requestor.entry(summary.requestor()).or_default();
        entry.0 += summary.access_count();
        entry.1 += summary.byte_count();
    }
    by_requestor
        .into_iter()
        .map(|(requestor, (access_count, byte_count))| {
            WorkloadDramQosRequestorSummary::new(requestor, access_count, byte_count)
        })
        .collect()
}
