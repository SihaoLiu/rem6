use std::collections::{BTreeMap, BTreeSet};

use rem6_fabric::{QosPriority, QosRequestorId};
use rem6_kernel::{
    ParallelPartitionActivity, ParallelProgressTransitionRecord, ParallelRemoteFlowRecord,
    ParallelRemoteSendRecord, PartitionFrontier, PartitionId,
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

pub(crate) fn is_parallel_remote_flow_evidence(flow: ParallelRemoteFlowRecord) -> bool {
    flow.send_count() != 0 && flow.source() != flow.target()
}

pub(crate) fn is_parallel_remote_send_evidence(send: ParallelRemoteSendRecord) -> bool {
    send.source() != send.target()
}

pub(crate) fn collect_parallel_remote_flow_evidence(
    flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
) -> Vec<ParallelRemoteFlowRecord> {
    let mut by_route = BTreeMap::<(PartitionId, PartitionId), ParallelRemoteFlowRecord>::new();
    for flow in flows {
        if !is_parallel_remote_flow_evidence(flow) {
            continue;
        }
        let route = (flow.source(), flow.target());
        by_route
            .entry(route)
            .and_modify(|stored| *stored = stored.merged_with(flow))
            .or_insert(flow);
    }
    let mut send_flows = BTreeMap::<(PartitionId, PartitionId), ParallelRemoteFlowRecord>::new();
    for send in sends {
        if !is_parallel_remote_send_evidence(send) {
            continue;
        }
        let route = (send.source(), send.target());
        let flow = parallel_remote_flow_from_send(send);
        send_flows
            .entry(route)
            .and_modify(|stored| *stored = stored.merged_with(flow))
            .or_insert(flow);
    }
    for (route, send_flow) in send_flows {
        by_route
            .entry(route)
            .and_modify(|stored| {
                *stored = stronger_parallel_remote_flow_evidence(*stored, send_flow)
            })
            .or_insert(send_flow);
    }
    by_route.into_values().collect()
}

fn stronger_parallel_remote_flow_evidence(
    explicit_flow: ParallelRemoteFlowRecord,
    send_flow: ParallelRemoteFlowRecord,
) -> ParallelRemoteFlowRecord {
    if send_flow.send_count() >= explicit_flow.send_count() {
        send_flow
    } else {
        explicit_flow
    }
}

fn parallel_remote_flow_from_send(send: ParallelRemoteSendRecord) -> ParallelRemoteFlowRecord {
    ParallelRemoteFlowRecord::with_delay_bounds(
        send.source(),
        send.target(),
        1,
        send.delivery_tick(),
        send.delivery_tick(),
        send.delay(),
        send.delay(),
    )
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

pub(crate) fn parallel_remote_flow_evidence_count(
    flows: &[ParallelRemoteFlowRecord],
    sends: &[ParallelRemoteSendRecord],
    source: PartitionId,
    target: PartitionId,
) -> usize {
    let evidence =
        collect_parallel_remote_flow_evidence(flows.iter().copied(), sends.iter().copied());
    parallel_remote_flow_count(&evidence, source, target)
}

pub(crate) fn collect_parallel_remote_sends(
    sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
) -> Vec<ParallelRemoteSendRecord> {
    let mut sends = sends.into_iter().collect::<Vec<_>>();
    sends.sort_by_key(|send| {
        (
            send.source(),
            send.target(),
            send.source_tick(),
            send.delivery_tick(),
            send.order(),
        )
    });
    sends
}

pub(crate) fn collect_parallel_progress_transitions(
    transitions: impl IntoIterator<Item = ParallelProgressTransitionRecord>,
) -> Vec<ParallelProgressTransitionRecord> {
    let mut transitions = transitions.into_iter().collect::<Vec<_>>();
    transitions.sort_by_key(|transition| {
        (
            transition.partition(),
            transition.tick(),
            transition.order(),
            transition.kind(),
            transition.subject().clone(),
        )
    });
    transitions
}

pub(crate) fn parallel_remote_send_count(
    sends: &[ParallelRemoteSendRecord],
    source: PartitionId,
    target: PartitionId,
) -> usize {
    sends
        .iter()
        .filter(|send| {
            is_parallel_remote_send_evidence(**send)
                && send.source() == source
                && send.target() == target
        })
        .count()
}

pub(crate) fn collect_parallel_remote_source_partitions(
    flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
) -> Vec<PartitionId> {
    let mut partitions = BTreeSet::new();
    for flow in flows {
        if is_parallel_remote_flow_evidence(flow) {
            partitions.insert(flow.source());
        }
    }
    for send in sends {
        if is_parallel_remote_send_evidence(send) {
            partitions.insert(send.source());
        }
    }
    partitions.into_iter().collect()
}

pub(crate) fn collect_parallel_remote_target_partitions(
    flows: impl IntoIterator<Item = ParallelRemoteFlowRecord>,
    sends: impl IntoIterator<Item = ParallelRemoteSendRecord>,
) -> Vec<PartitionId> {
    let mut partitions = BTreeSet::new();
    for flow in flows {
        if is_parallel_remote_flow_evidence(flow) {
            partitions.insert(flow.target());
        }
    }
    for send in sends {
        if is_parallel_remote_send_evidence(send) {
            partitions.insert(send.target());
        }
    }
    partitions.into_iter().collect()
}

pub(crate) fn collect_parallel_partition_activities(
    activities: impl IntoIterator<Item = (PartitionId, ParallelPartitionActivity)>,
) -> Vec<(PartitionId, ParallelPartitionActivity)> {
    let mut by_partition = BTreeMap::new();
    for (partition, activity) in activities {
        let activity = normalize_parallel_partition_activity(activity);
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

fn normalize_parallel_partition_activity(
    activity: ParallelPartitionActivity,
) -> ParallelPartitionActivity {
    if activity.worker_count() == 0 && activity.dispatch_count() != 0 {
        return ParallelPartitionActivity::with_remote_counts(
            0,
            0,
            activity.remote_send_count(),
            activity.remote_receive_count(),
            activity.max_pending_events(),
        );
    }
    activity
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
