use rem6_boot::{
    BootElfArchitecture, BootElfClass, BootElfEndian, BootElfMetadata, BootElfOperatingSystem,
};
use rem6_dram::{DramMemoryTechnology, ExternalMemoryProfile, ExternalMemoryTopology};
use rem6_kernel::{Tick, WaitForEdgeKind, WaitForNode};

use crate::{
    CheckpointLineage, HostEventIntent, WorkloadBootImage,
    WorkloadExpectedCheckpointComponentSummary, WorkloadExpectedCheckpointManifestSummary,
    WorkloadExpectedCleanParallelDiagnostics, WorkloadExpectedDataCacheProtocolRunCount,
    WorkloadExpectedDataCacheRunAttribution, WorkloadExpectedDramLowPowerActivity,
    WorkloadExpectedFabricHopActivity, WorkloadExpectedFabricLaneActivity,
    WorkloadExpectedFabricLinkActivity, WorkloadExpectedFabricVirtualNetworkActivity,
    WorkloadExpectedParallelBatchActivity, WorkloadExpectedParallelBatchPartitionSet,
    WorkloadExpectedParallelBatchPartitionStreak, WorkloadExpectedParallelBatchTimelineRecord,
    WorkloadExpectedParallelBatchWorkerBucket, WorkloadExpectedParallelBatchWorkerTickActivity,
    WorkloadExpectedParallelBatchWorkerTickBucket, WorkloadExpectedParallelBatchWorkerTickStreak,
    WorkloadExpectedParallelBatchWorkerTicks, WorkloadExpectedParallelFrontier,
    WorkloadExpectedParallelPartitionActivity, WorkloadExpectedParallelPartitionUse,
    WorkloadExpectedParallelProgressTransition, WorkloadExpectedParallelRemoteDelayCeiling,
    WorkloadExpectedParallelRemoteDelayFloor, WorkloadExpectedParallelRemoteEndpoints,
    WorkloadExpectedParallelRemoteFlow, WorkloadExpectedParallelRemoteFlowTiming,
    WorkloadExpectedParallelRemoteSend, WorkloadExpectedParallelRemoteTrafficConsistency,
    WorkloadExpectedParallelSchedulerIdleBound, WorkloadExpectedParallelSchedulerProgress,
    WorkloadExpectedParallelWaitForBlockedNodeWindow, WorkloadExpectedParallelWaitForEdgeKindCount,
    WorkloadExpectedParallelWaitForEdgeKindWindow, WorkloadExpectedParallelWaitForTargetNodeWindow,
    WorkloadExpectedParallelWorkerActivity, WorkloadExpectedParallelWorkerUse,
    WorkloadExpectedPlannedParallelBatchIdleWorkerTicks,
    WorkloadExpectedPlannedParallelBatchUtilization,
    WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks,
    WorkloadExpectedPlannedParallelBatchWorkerSlotTicks, WorkloadExpectedResourceActivity,
    WorkloadExpectedStatsHistory, WorkloadExpectedTrafficTraceReplaySummary, WorkloadHostEvent,
    WorkloadId, WorkloadLinuxBootHandoff, WorkloadManifestIdentity,
    WorkloadParallelBatchPartitionScope, WorkloadParallelBatchTimelineScope,
    WorkloadParallelBatchWorkerScope, WorkloadParallelFrontierStage,
    WorkloadParallelRemoteFlowScope, WorkloadParallelSchedulerScope, WorkloadResource,
    WorkloadResourceActivityScope, WorkloadResourceId, WorkloadStatsHistoryRecordExpectation,
    WorkloadTopology,
};

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

pub(crate) struct ManifestIdentityInput<'a> {
    pub(crate) id: &'a WorkloadId,
    pub(crate) boot: &'a WorkloadBootImage,
    pub(crate) linux_boot_handoff: Option<&'a WorkloadLinuxBootHandoff>,
    pub(crate) topology: Option<&'a WorkloadTopology>,
    pub(crate) resources: &'a [WorkloadResource],
    pub(crate) required_resources: &'a [WorkloadResourceId],
    pub(crate) host_events: &'a [WorkloadHostEvent],
    pub(crate) expected_clean_parallel_diagnostics:
        &'a [WorkloadExpectedCleanParallelDiagnostics],
    pub(crate) expected_parallel_wait_for_edge_kind_counts:
        &'a [WorkloadExpectedParallelWaitForEdgeKindCount],
    pub(crate) expected_parallel_wait_for_edge_kind_windows:
        &'a [WorkloadExpectedParallelWaitForEdgeKindWindow],
    pub(crate) expected_parallel_wait_for_blocked_node_windows:
        &'a [WorkloadExpectedParallelWaitForBlockedNodeWindow],
    pub(crate) expected_parallel_wait_for_target_node_windows:
        &'a [WorkloadExpectedParallelWaitForTargetNodeWindow],
    pub(crate) expected_data_cache_protocol_run_counts:
        &'a [WorkloadExpectedDataCacheProtocolRunCount],
    pub(crate) expected_data_cache_run_attribution:
        Option<&'a WorkloadExpectedDataCacheRunAttribution>,
    pub(crate) expected_stats_history: Option<&'a WorkloadExpectedStatsHistory>,
    pub(crate) expected_parallel_remote_flows: &'a [WorkloadExpectedParallelRemoteFlow],
    pub(crate) expected_parallel_remote_endpoints: &'a [WorkloadExpectedParallelRemoteEndpoints],
    pub(crate) expected_parallel_remote_delay_floors:
        &'a [WorkloadExpectedParallelRemoteDelayFloor],
    pub(crate) expected_parallel_remote_delay_ceilings:
        &'a [WorkloadExpectedParallelRemoteDelayCeiling],
    pub(crate) expected_parallel_remote_traffic_consistency:
        &'a [WorkloadExpectedParallelRemoteTrafficConsistency],
    pub(crate) expected_parallel_remote_sends: &'a [WorkloadExpectedParallelRemoteSend],
    pub(crate) expected_parallel_remote_flow_timings:
        &'a [WorkloadExpectedParallelRemoteFlowTiming],
    pub(crate) expected_parallel_progress_transitions:
        &'a [WorkloadExpectedParallelProgressTransition],
    pub(crate) expected_traffic_trace_replay_summaries:
        &'a [WorkloadExpectedTrafficTraceReplaySummary],
    pub(crate) expected_checkpoint_manifest_summaries:
        &'a [WorkloadExpectedCheckpointManifestSummary],
    pub(crate) expected_checkpoint_restore_manifest_summaries:
        &'a [WorkloadExpectedCheckpointManifestSummary],
    pub(crate) expected_checkpoint_component_summaries:
        &'a [WorkloadExpectedCheckpointComponentSummary],
    pub(crate) expected_checkpoint_restore_component_summaries:
        &'a [WorkloadExpectedCheckpointComponentSummary],
    pub(crate) expected_parallel_worker_use: &'a [WorkloadExpectedParallelWorkerUse],
    pub(crate) expected_parallel_worker_activity: &'a [WorkloadExpectedParallelWorkerActivity],
    pub(crate) expected_parallel_scheduler_progress:
        &'a [WorkloadExpectedParallelSchedulerProgress],
    pub(crate) expected_parallel_scheduler_idle_bounds:
        &'a [WorkloadExpectedParallelSchedulerIdleBound],
    pub(crate) expected_parallel_batch_activity: &'a [WorkloadExpectedParallelBatchActivity],
    pub(crate) expected_parallel_batch_worker_buckets:
        &'a [WorkloadExpectedParallelBatchWorkerBucket],
    pub(crate) expected_parallel_batch_worker_tick_buckets:
        &'a [WorkloadExpectedParallelBatchWorkerTickBucket],
    pub(crate) expected_parallel_batch_worker_tick_activity:
        &'a [WorkloadExpectedParallelBatchWorkerTickActivity],
    pub(crate) expected_parallel_batch_worker_tick_streaks:
        &'a [WorkloadExpectedParallelBatchWorkerTickStreak],
    pub(crate) expected_parallel_batch_worker_ticks:
        &'a [WorkloadExpectedParallelBatchWorkerTicks],
    pub(crate) expected_planned_parallel_batch_utilization:
        &'a [WorkloadExpectedPlannedParallelBatchUtilization],
    pub(crate) expected_planned_parallel_batch_idle_worker_ticks:
        &'a [WorkloadExpectedPlannedParallelBatchIdleWorkerTicks],
    pub(crate) expected_planned_parallel_batch_worker_slot_ticks:
        &'a [WorkloadExpectedPlannedParallelBatchWorkerSlotTicks],
    pub(crate) expected_planned_parallel_batch_worker_lane_partition_ticks:
        &'a [WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks],
    pub(crate) expected_parallel_batch_partition_sets:
        &'a [WorkloadExpectedParallelBatchPartitionSet],
    pub(crate) expected_parallel_batch_partition_streaks:
        &'a [WorkloadExpectedParallelBatchPartitionStreak],
    pub(crate) expected_parallel_batch_timeline_records:
        &'a [WorkloadExpectedParallelBatchTimelineRecord],
    pub(crate) expected_parallel_partition_use: &'a [WorkloadExpectedParallelPartitionUse],
    pub(crate) expected_parallel_partition_activity:
        &'a [WorkloadExpectedParallelPartitionActivity],
    pub(crate) expected_parallel_frontiers: &'a [WorkloadExpectedParallelFrontier],
    pub(crate) expected_resource_activity: &'a [WorkloadExpectedResourceActivity],
    pub(crate) expected_dram_low_power_activity: Option<WorkloadExpectedDramLowPowerActivity>,
    pub(crate) expected_fabric_hop_activity: &'a [WorkloadExpectedFabricHopActivity],
    pub(crate) expected_fabric_lane_activity: &'a [WorkloadExpectedFabricLaneActivity],
    pub(crate) expected_fabric_link_activity: &'a [WorkloadExpectedFabricLinkActivity],
    pub(crate) expected_fabric_virtual_network_activity:
        &'a [WorkloadExpectedFabricVirtualNetworkActivity],
    pub(crate) checkpoint_lineage: Option<&'a CheckpointLineage>,
}

pub(crate) fn manifest_identity(input: ManifestIdentityInput<'_>) -> WorkloadManifestIdentity {
    let mut hash = FNV_OFFSET;
    hash_str(&mut hash, "rem6.workload.manifest.v2");
    hash_str(&mut hash, input.id.as_str());
    hash_u64(&mut hash, input.boot.entry().get());
    hash_elf_metadata(&mut hash, input.boot.elf_metadata());
    hash_u64(&mut hash, input.boot.segments().len() as u64);
    for segment in input.boot.segments() {
        hash_u64(&mut hash, segment.range().start().get());
        hash_u64(&mut hash, segment.range().size().bytes());
        hash_bytes(&mut hash, segment.data());
    }
    hash_linux_boot_handoff(&mut hash, input.linux_boot_handoff);
    hash_topology(&mut hash, input.topology);
    hash_u64(&mut hash, input.resources.len() as u64);
    for resource in input.resources {
        hash_str(&mut hash, resource.id().as_str());
        hash_u64(&mut hash, resource.kind() as u64);
        hash_str(&mut hash, resource.digest());
        hash_str(&mut hash, resource.locator());
        hash_resource_acquisition(&mut hash, resource.acquisition());
        hash_disk_image_construction(&mut hash, resource.disk_image_construction());
    }
    hash_u64(&mut hash, input.required_resources.len() as u64);
    for resource in input.required_resources {
        hash_str(&mut hash, resource.as_str());
    }
    hash_u64(&mut hash, input.host_events.len() as u64);
    for event in input.host_events {
        hash_u64(&mut hash, event.tick());
        hash_host_event(&mut hash, event.intent());
    }
    hash_u64(
        &mut hash,
        input.expected_clean_parallel_diagnostics.len() as u64,
    );
    for expected in input.expected_clean_parallel_diagnostics {
        hash_expected_clean_parallel_diagnostics(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_wait_for_edge_kind_counts.len() as u64,
    );
    for expected in input.expected_parallel_wait_for_edge_kind_counts {
        hash_expected_parallel_wait_for_edge_kind_count(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_wait_for_edge_kind_windows.len() as u64,
    );
    for expected in input.expected_parallel_wait_for_edge_kind_windows {
        hash_expected_parallel_wait_for_edge_kind_window(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_wait_for_blocked_node_windows.len() as u64,
    );
    for expected in input.expected_parallel_wait_for_blocked_node_windows {
        hash_expected_parallel_wait_for_blocked_node_window(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_wait_for_target_node_windows.len() as u64,
    );
    for expected in input.expected_parallel_wait_for_target_node_windows {
        hash_expected_parallel_wait_for_target_node_window(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_data_cache_protocol_run_counts.len() as u64,
    );
    for expected in input.expected_data_cache_protocol_run_counts {
        hash_expected_data_cache_protocol_run_count(&mut hash, *expected);
    }
    hash_expected_data_cache_run_attribution(&mut hash, input.expected_data_cache_run_attribution);
    hash_expected_stats_history(&mut hash, input.expected_stats_history);
    hash_u64(&mut hash, input.expected_parallel_remote_flows.len() as u64);
    for expected in input.expected_parallel_remote_flows {
        hash_expected_parallel_remote_flow(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_remote_endpoints.len() as u64,
    );
    for expected in input.expected_parallel_remote_endpoints {
        hash_expected_parallel_remote_endpoints(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_remote_delay_floors.len() as u64,
    );
    for expected in input.expected_parallel_remote_delay_floors {
        hash_expected_parallel_remote_delay_floor(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_remote_delay_ceilings.len() as u64,
    );
    for expected in input.expected_parallel_remote_delay_ceilings {
        hash_expected_parallel_remote_delay_ceiling(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_remote_traffic_consistency.len() as u64,
    );
    for expected in input.expected_parallel_remote_traffic_consistency {
        hash_expected_parallel_remote_traffic_consistency(&mut hash, *expected);
    }
    hash_u64(&mut hash, input.expected_parallel_remote_sends.len() as u64);
    for expected in input.expected_parallel_remote_sends {
        hash_expected_parallel_remote_send(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_remote_flow_timings.len() as u64,
    );
    for expected in input.expected_parallel_remote_flow_timings {
        hash_expected_parallel_remote_flow_timing(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_progress_transitions.len() as u64,
    );
    for expected in input.expected_parallel_progress_transitions {
        hash_expected_parallel_progress_transition(&mut hash, expected);
    }
    if !input.expected_traffic_trace_replay_summaries.is_empty() {
        hash_str(&mut hash, "expected_traffic_trace_replay_summaries");
        hash_u64(
            &mut hash,
            input.expected_traffic_trace_replay_summaries.len() as u64,
        );
        for expected in input.expected_traffic_trace_replay_summaries {
            hash_expected_traffic_trace_replay_summary(&mut hash, expected);
        }
    }
    hash_u64(
        &mut hash,
        input.expected_checkpoint_manifest_summaries.len() as u64,
    );
    for expected in input.expected_checkpoint_manifest_summaries {
        hash_expected_checkpoint_manifest_summary(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_checkpoint_restore_manifest_summaries.len() as u64,
    );
    for expected in input.expected_checkpoint_restore_manifest_summaries {
        hash_expected_checkpoint_manifest_summary(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_checkpoint_component_summaries.len() as u64,
    );
    for expected in input.expected_checkpoint_component_summaries {
        hash_expected_checkpoint_component_summary(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_checkpoint_restore_component_summaries.len() as u64,
    );
    for expected in input.expected_checkpoint_restore_component_summaries {
        hash_expected_checkpoint_component_summary(&mut hash, expected);
    }
    hash_u64(&mut hash, input.expected_parallel_worker_use.len() as u64);
    for expected in input.expected_parallel_worker_use {
        hash_expected_parallel_worker_use(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_worker_activity.len() as u64,
    );
    for expected in input.expected_parallel_worker_activity {
        hash_expected_parallel_worker_activity(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_scheduler_progress.len() as u64,
    );
    for expected in input.expected_parallel_scheduler_progress {
        hash_expected_parallel_scheduler_progress(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_scheduler_idle_bounds.len() as u64,
    );
    for expected in input.expected_parallel_scheduler_idle_bounds {
        hash_expected_parallel_scheduler_idle_bound(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_activity.len() as u64,
    );
    for expected in input.expected_parallel_batch_activity {
        hash_expected_parallel_batch_activity(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_worker_buckets.len() as u64,
    );
    for expected in input.expected_parallel_batch_worker_buckets {
        hash_expected_parallel_batch_worker_bucket(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_worker_tick_buckets.len() as u64,
    );
    for expected in input.expected_parallel_batch_worker_tick_buckets {
        hash_expected_parallel_batch_worker_tick_bucket(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_worker_tick_activity.len() as u64,
    );
    for expected in input.expected_parallel_batch_worker_tick_activity {
        hash_expected_parallel_batch_worker_tick_activity(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_worker_tick_streaks.len() as u64,
    );
    for expected in input.expected_parallel_batch_worker_tick_streaks {
        hash_expected_parallel_batch_worker_tick_streak(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_worker_ticks.len() as u64,
    );
    for expected in input.expected_parallel_batch_worker_ticks {
        hash_expected_parallel_batch_worker_ticks(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_planned_parallel_batch_utilization.len() as u64,
    );
    for expected in input.expected_planned_parallel_batch_utilization {
        hash_expected_planned_parallel_batch_utilization(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input
            .expected_planned_parallel_batch_idle_worker_ticks
            .len() as u64,
    );
    for expected in input.expected_planned_parallel_batch_idle_worker_ticks {
        hash_expected_planned_parallel_batch_idle_worker_ticks(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input
            .expected_planned_parallel_batch_worker_slot_ticks
            .len() as u64,
    );
    for expected in input.expected_planned_parallel_batch_worker_slot_ticks {
        hash_expected_planned_parallel_batch_worker_slot_ticks(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input
            .expected_planned_parallel_batch_worker_lane_partition_ticks
            .len() as u64,
    );
    for expected in input.expected_planned_parallel_batch_worker_lane_partition_ticks {
        hash_expected_planned_parallel_batch_worker_lane_partition_ticks(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_partition_sets.len() as u64,
    );
    for expected in input.expected_parallel_batch_partition_sets {
        hash_expected_parallel_batch_partition_set(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_partition_streaks.len() as u64,
    );
    for expected in input.expected_parallel_batch_partition_streaks {
        hash_expected_parallel_batch_partition_streak(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_batch_timeline_records.len() as u64,
    );
    for expected in input.expected_parallel_batch_timeline_records {
        hash_expected_parallel_batch_timeline_record(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_partition_use.len() as u64,
    );
    for expected in input.expected_parallel_partition_use {
        hash_expected_parallel_partition_use(&mut hash, *expected);
    }
    hash_u64(
        &mut hash,
        input.expected_parallel_partition_activity.len() as u64,
    );
    for expected in input.expected_parallel_partition_activity {
        hash_expected_parallel_partition_activity(&mut hash, *expected);
    }
    hash_u64(&mut hash, input.expected_parallel_frontiers.len() as u64);
    for expected in input.expected_parallel_frontiers {
        hash_expected_parallel_frontier(&mut hash, *expected);
    }
    hash_u64(&mut hash, input.expected_resource_activity.len() as u64);
    for expected in input.expected_resource_activity {
        hash_expected_resource_activity(&mut hash, *expected);
    }
    hash_optional_expected_dram_low_power_activity(
        &mut hash,
        input.expected_dram_low_power_activity,
    );
    hash_u64(&mut hash, input.expected_fabric_hop_activity.len() as u64);
    for expected in input.expected_fabric_hop_activity {
        hash_expected_fabric_hop_activity(&mut hash, expected);
    }
    hash_u64(&mut hash, input.expected_fabric_lane_activity.len() as u64);
    for expected in input.expected_fabric_lane_activity {
        hash_expected_fabric_lane_activity(&mut hash, expected);
    }
    hash_u64(&mut hash, input.expected_fabric_link_activity.len() as u64);
    for expected in input.expected_fabric_link_activity {
        hash_expected_fabric_link_activity(&mut hash, expected);
    }
    hash_u64(
        &mut hash,
        input.expected_fabric_virtual_network_activity.len() as u64,
    );
    for expected in input.expected_fabric_virtual_network_activity {
        hash_expected_fabric_virtual_network_activity(&mut hash, expected);
    }
    hash_checkpoint_lineage(&mut hash, input.checkpoint_lineage);
    WorkloadManifestIdentity::new(hash)
}

fn hash_expected_clean_parallel_diagnostics(
    hash: &mut u64,
    expected: WorkloadExpectedCleanParallelDiagnostics,
) {
    hash_str(hash, expected.scope().as_str());
    match expected.livelock_transition_threshold() {
        Some(threshold) => {
            hash_u64(hash, 1);
            hash_u64(hash, threshold);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_expected_parallel_wait_for_edge_kind_count(
    hash: &mut u64,
    expected: WorkloadExpectedParallelWaitForEdgeKindCount,
) {
    hash_str(hash, expected.scope().as_str());
    hash_wait_for_edge_kind(hash, expected.kind());
    hash_u64(hash, expected.minimum_edge_count() as u64);
}

fn hash_expected_parallel_wait_for_edge_kind_window(
    hash: &mut u64,
    expected: WorkloadExpectedParallelWaitForEdgeKindWindow,
) {
    hash_str(hash, expected.scope().as_str());
    hash_wait_for_edge_kind(hash, expected.kind());
    hash_u64(hash, expected.edge_count() as u64);
    hash_u64(hash, expected.first_tick());
    hash_u64(hash, expected.last_tick());
}

fn hash_expected_parallel_wait_for_blocked_node_window(
    hash: &mut u64,
    expected: &WorkloadExpectedParallelWaitForBlockedNodeWindow,
) {
    hash_str(hash, expected.scope().as_str());
    hash_wait_for_node(hash, expected.node());
    hash_u64(hash, expected.edge_count() as u64);
    hash_u64(hash, expected.first_tick());
    hash_u64(hash, expected.last_tick());
}

fn hash_expected_parallel_wait_for_target_node_window(
    hash: &mut u64,
    expected: &WorkloadExpectedParallelWaitForTargetNodeWindow,
) {
    hash_str(hash, expected.scope().as_str());
    hash_wait_for_node(hash, expected.node());
    hash_u64(hash, expected.edge_count() as u64);
    hash_u64(hash, expected.first_tick());
    hash_u64(hash, expected.last_tick());
}

fn hash_wait_for_edge_kind(hash: &mut u64, kind: WaitForEdgeKind) {
    hash_u64(
        hash,
        match kind {
            WaitForEdgeKind::Resource => 0,
            WaitForEdgeKind::Message => 1,
            WaitForEdgeKind::Protocol => 2,
            WaitForEdgeKind::Queue => 3,
            WaitForEdgeKind::Credit => 4,
            WaitForEdgeKind::HostAction => 5,
            WaitForEdgeKind::Barrier => 6,
        },
    );
}

fn hash_wait_for_node(hash: &mut u64, node: &WaitForNode) {
    match node {
        WaitForNode::Partition(partition) => {
            hash_u64(hash, 0);
            hash_u64(hash, partition.index() as u64);
        }
        WaitForNode::Component(label) => {
            hash_u64(hash, 1);
            hash_str(hash, label);
        }
        WaitForNode::Resource(label) => {
            hash_u64(hash, 2);
            hash_str(hash, label);
        }
        WaitForNode::Transaction(label) => {
            hash_u64(hash, 3);
            hash_str(hash, label);
        }
    }
}

fn hash_expected_data_cache_protocol_run_count(
    hash: &mut u64,
    expected: WorkloadExpectedDataCacheProtocolRunCount,
) {
    hash_str(hash, expected.protocol().as_str());
    hash_u64(hash, expected.minimum_run_count() as u64);
}

fn hash_expected_data_cache_run_attribution(
    hash: &mut u64,
    expected: Option<&WorkloadExpectedDataCacheRunAttribution>,
) {
    match expected {
        Some(expected) => {
            hash_u64(hash, 1);
            hash_u64(hash, expected.minimum_attributed_run_count() as u64);
            hash_u64(hash, expected.maximum_unattributed_run_count() as u64);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_expected_stats_history(hash: &mut u64, expected: Option<&WorkloadExpectedStatsHistory>) {
    match expected {
        Some(expected) => {
            hash_u64(hash, 1);
            hash_u64(hash, expected.minimum_reset_count() as u64);
            hash_u64(hash, expected.minimum_dump_count() as u64);
            match (expected.first_tick(), expected.last_tick()) {
                (Some(first_tick), Some(last_tick)) => {
                    hash_u64(hash, 1);
                    hash_u64(hash, first_tick);
                    hash_u64(hash, last_tick);
                }
                _ => hash_u64(hash, 0),
            }
            hash_u64(hash, expected.exact_records().len() as u64);
            for record in expected.exact_records() {
                hash_expected_stats_history_record(hash, *record);
            }
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_expected_stats_history_record(
    hash: &mut u64,
    record: WorkloadStatsHistoryRecordExpectation,
) {
    hash_u64(hash, record.kind_code());
    hash_u64(hash, record.id_value());
    hash_u64(hash, record.tick());
    hash_u64(hash, record.epoch());
    match record.reset_tick() {
        Some(reset_tick) => {
            hash_u64(hash, 1);
            hash_u64(hash, reset_tick);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_expected_parallel_remote_flow(
    hash: &mut u64,
    expected: WorkloadExpectedParallelRemoteFlow,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, u64::from(expected.source().index()));
    hash_u64(hash, u64::from(expected.target().index()));
    hash_u64(hash, expected.send_count() as u64);
}

fn hash_expected_parallel_remote_endpoints(
    hash: &mut u64,
    expected: &WorkloadExpectedParallelRemoteEndpoints,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, expected.source_partitions().len() as u64);
    for partition in expected.source_partitions() {
        hash_u64(hash, u64::from(partition.index()));
    }
    hash_u64(hash, expected.target_partitions().len() as u64);
    for partition in expected.target_partitions() {
        hash_u64(hash, u64::from(partition.index()));
    }
}

fn hash_expected_parallel_remote_delay_floor(
    hash: &mut u64,
    expected: WorkloadExpectedParallelRemoteDelayFloor,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_delay());
}

fn hash_expected_parallel_remote_delay_ceiling(
    hash: &mut u64,
    expected: WorkloadExpectedParallelRemoteDelayCeiling,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, expected.maximum_delay());
}

fn hash_expected_parallel_remote_traffic_consistency(
    hash: &mut u64,
    expected: WorkloadExpectedParallelRemoteTrafficConsistency,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
}

fn hash_expected_parallel_remote_send(
    hash: &mut u64,
    expected: WorkloadExpectedParallelRemoteSend,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, u64::from(expected.source().index()));
    hash_u64(hash, u64::from(expected.target().index()));
    hash_u64(hash, expected.source_tick());
    hash_u64(hash, expected.delivery_tick());
    hash_u64(hash, expected.order());
}

fn hash_expected_parallel_remote_flow_timing(
    hash: &mut u64,
    expected: WorkloadExpectedParallelRemoteFlowTiming,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, u64::from(expected.source().index()));
    hash_u64(hash, u64::from(expected.target().index()));
    hash_u64(hash, expected.send_count() as u64);
    hash_u64(hash, expected.first_tick());
    hash_u64(hash, expected.last_tick());
    match expected.delay_bounds() {
        Some((minimum_delay, maximum_delay)) => {
            hash_u64(hash, 1);
            hash_u64(hash, minimum_delay);
            hash_u64(hash, maximum_delay);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_expected_parallel_progress_transition(
    hash: &mut u64,
    expected: &WorkloadExpectedParallelProgressTransition,
) {
    hash_parallel_remote_flow_scope(hash, expected.scope());
    hash_u64(hash, u64::from(expected.partition().index()));
    hash_str(hash, &expected.subject().to_string());
    hash_str(hash, expected.kind().as_str());
    hash_u64(hash, expected.tick());
    hash_u64(hash, expected.order());
}

fn hash_expected_checkpoint_manifest_summary(
    hash: &mut u64,
    expected: &WorkloadExpectedCheckpointManifestSummary,
) {
    hash_str(hash, expected.label());
    hash_u64(hash, expected.minimum_component_count() as u64);
    hash_u64(hash, expected.minimum_chunk_count() as u64);
    hash_u64(hash, expected.minimum_payload_bytes() as u64);
}

fn hash_expected_traffic_trace_replay_summary(
    hash: &mut u64,
    expected: &WorkloadExpectedTrafficTraceReplaySummary,
) {
    hash_str(hash, expected.route().as_str());
    hash_u64(hash, expected.minimum_scheduled_count() as u64);
    hash_u64(hash, expected.minimum_response_delivery_count() as u64);
    hash_u64(
        hash,
        expected.minimum_trace_completed_response_count() as u64,
    );
    hash_u64(hash, expected.minimum_trace_retry_response_count() as u64);
    hash_u64(
        hash,
        expected.minimum_trace_store_conditional_failed_response_count() as u64,
    );
    hash_u64(hash, expected.minimum_memory_trace_event_count() as u64);
    hash_u64(
        hash,
        expected.minimum_memory_write_completion_count() as u64,
    );
    hash_u64(
        hash,
        expected.minimum_trace_data_cache_response_count() as u64,
    );
    hash_u64(hash, expected.minimum_trace_data_cache_error_count() as u64);
    hash_u64(hash, expected.minimum_memory_failure_count() as u64);
    hash_u64(hash, expected.minimum_trace_error_count() as u64);
    hash_u64(hash, expected.minimum_trace_htm_access_count() as u64);
    hash_u64(hash, expected.minimum_control_ack_count() as u64);
    hash_u64(hash, expected.minimum_sync_control_ack_count() as u64);
    hash_u64(hash, expected.minimum_htm_control_ack_count() as u64);
    hash_u64(hash, expected.minimum_control_failure_count() as u64);
    hash_u64(hash, expected.minimum_sync_control_failure_count() as u64);
    hash_u64(hash, expected.minimum_tlb_control_failure_count() as u64);
    hash_u64(hash, expected.minimum_cache_control_failure_count() as u64);
    hash_u64(hash, expected.minimum_htm_control_failure_count() as u64);
    hash_u64(
        hash,
        expected.minimum_diagnostic_control_failure_count() as u64,
    );
    hash_u64(hash, expected.minimum_sideband_event_count() as u64);
    hash_u64(hash, expected.minimum_tlb_sync_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_tlb_sync_count() as u64);
    hash_u64(hash, expected.minimum_cache_flush_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_cache_flush_count() as u64);
    hash_u64(hash, expected.minimum_trace_l1_invalidation_count() as u64);
    hash_u64(hash, expected.minimum_diagnostic_print_event_count() as u64);
    hash_u64(hash, expected.minimum_trace_diagnostic_count() as u64);
    hash_u64(hash, expected.minimum_htm_abort_event_count() as u64);
}

fn hash_expected_checkpoint_component_summary(
    hash: &mut u64,
    expected: &WorkloadExpectedCheckpointComponentSummary,
) {
    hash_str(hash, expected.label());
    hash_str(hash, expected.component());
    hash_u64(hash, expected.minimum_chunk_count() as u64);
    hash_u64(hash, expected.minimum_payload_bytes() as u64);
    hash_u64(hash, expected.required_chunk_names().len() as u64);
    for chunk in expected.required_chunk_names() {
        hash_str(hash, chunk);
    }
    hash_u64(hash, expected.required_chunk_payloads().len() as u64);
    for chunk in expected.required_chunk_payloads() {
        hash_str(hash, chunk.name());
        hash_u64(hash, chunk.minimum_payload_bytes() as u64);
    }
}

fn hash_resource_acquisition(
    hash: &mut u64,
    acquisition: Option<&crate::WorkloadResourceAcquisition>,
) {
    let Some(acquisition) = acquisition else {
        hash_u64(hash, 0);
        return;
    };
    hash_u64(hash, 1);
    hash_str(hash, acquisition.kind().as_str());
    hash_str(hash, acquisition.locator());
    hash_optional_str(hash, acquisition.tool());
    hash_optional_str(hash, acquisition.revision());
}

fn hash_disk_image_construction(
    hash: &mut u64,
    construction: Option<&crate::WorkloadDiskImageConstruction>,
) {
    let Some(construction) = construction else {
        hash_u64(hash, 0);
        return;
    };
    hash_u64(hash, 1);
    hash_str(hash, construction.image_format());
    hash_u64(hash, construction.virtual_size_bytes());
    hash_u64(hash, construction.steps().len() as u64);
    for step in construction.steps() {
        hash_str(hash, step.tool());
        hash_str(hash, step.operation());
        hash_str(hash, step.input());
        hash_u64(hash, step.arguments().len() as u64);
        for argument in step.arguments() {
            hash_str(hash, argument);
        }
    }
}

fn hash_parallel_remote_flow_scope(hash: &mut u64, scope: WorkloadParallelRemoteFlowScope) {
    hash_str(hash, scope.as_str());
}

fn hash_parallel_batch_timeline_scope(hash: &mut u64, scope: WorkloadParallelBatchTimelineScope) {
    hash_str(hash, scope.as_str());
}

fn hash_parallel_batch_worker_scope(hash: &mut u64, scope: WorkloadParallelBatchWorkerScope) {
    hash_str(hash, scope.as_str());
}

fn hash_parallel_batch_partition_scope(hash: &mut u64, scope: WorkloadParallelBatchPartitionScope) {
    hash_str(hash, scope.as_str());
}

fn hash_parallel_scheduler_scope(hash: &mut u64, scope: WorkloadParallelSchedulerScope) {
    hash_str(hash, scope.as_str());
}

fn hash_parallel_frontier_stage(hash: &mut u64, stage: WorkloadParallelFrontierStage) {
    hash_str(hash, stage.as_str());
}

fn hash_resource_activity_scope(hash: &mut u64, scope: WorkloadResourceActivityScope) {
    hash_str(hash, scope.as_str());
}

fn hash_expected_parallel_worker_use(hash: &mut u64, expected: WorkloadExpectedParallelWorkerUse) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_max_workers() as u64);
}

fn hash_expected_parallel_worker_activity(
    hash: &mut u64,
    expected: WorkloadExpectedParallelWorkerActivity,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_total_workers() as u64);
}

fn hash_expected_parallel_scheduler_progress(
    hash: &mut u64,
    expected: WorkloadExpectedParallelSchedulerProgress,
) {
    hash_parallel_scheduler_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_epoch_count() as u64);
    hash_u64(hash, expected.minimum_dispatch_count() as u64);
}

fn hash_expected_parallel_scheduler_idle_bound(
    hash: &mut u64,
    expected: WorkloadExpectedParallelSchedulerIdleBound,
) {
    hash_parallel_scheduler_scope(hash, expected.scope());
    hash_u64(hash, expected.maximum_empty_epoch_count() as u64);
}

fn hash_expected_parallel_batch_activity(
    hash: &mut u64,
    expected: WorkloadExpectedParallelBatchActivity,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_worker_count() as u64);
    hash_u64(hash, expected.minimum_batch_count() as u64);
}

fn hash_expected_parallel_batch_worker_bucket(
    hash: &mut u64,
    expected: WorkloadExpectedParallelBatchWorkerBucket,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.worker_count() as u64);
    hash_u64(hash, expected.minimum_batch_count() as u64);
}

fn hash_expected_parallel_batch_worker_tick_bucket(
    hash: &mut u64,
    expected: WorkloadExpectedParallelBatchWorkerTickBucket,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.worker_count() as u64);
    hash_u64(hash, expected.minimum_ticks());
}

fn hash_expected_parallel_batch_worker_tick_activity(
    hash: &mut u64,
    expected: WorkloadExpectedParallelBatchWorkerTickActivity,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_worker_count() as u64);
    hash_u64(hash, expected.minimum_ticks());
}

fn hash_expected_parallel_batch_worker_tick_streak(
    hash: &mut u64,
    expected: WorkloadExpectedParallelBatchWorkerTickStreak,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_worker_count() as u64);
    hash_u64(hash, expected.minimum_consecutive_ticks());
}

fn hash_expected_parallel_batch_worker_ticks(
    hash: &mut u64,
    expected: WorkloadExpectedParallelBatchWorkerTicks,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_worker_count() as u64);
    hash_u64(hash, expected.minimum_worker_ticks());
}

fn hash_expected_planned_parallel_batch_utilization(
    hash: &mut u64,
    expected: WorkloadExpectedPlannedParallelBatchUtilization,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_numerator());
    hash_u64(hash, expected.minimum_denominator());
}

fn hash_expected_planned_parallel_batch_idle_worker_ticks(
    hash: &mut u64,
    expected: WorkloadExpectedPlannedParallelBatchIdleWorkerTicks,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.maximum_idle_worker_ticks());
}

fn hash_expected_planned_parallel_batch_worker_slot_ticks(
    hash: &mut u64,
    expected: WorkloadExpectedPlannedParallelBatchWorkerSlotTicks,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.worker_slot() as u64);
    hash_u64(hash, expected.minimum_active_ticks());
    hash_u64(hash, expected.maximum_idle_ticks());
}

fn hash_expected_planned_parallel_batch_worker_lane_partition_ticks(
    hash: &mut u64,
    expected: WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks,
) {
    hash_parallel_batch_worker_scope(hash, expected.scope());
    hash_u64(hash, expected.worker_lane() as u64);
    hash_u64(hash, u64::from(expected.partition().index()));
    hash_u64(hash, expected.minimum_ticks());
}

fn hash_expected_parallel_batch_partition_set(
    hash: &mut u64,
    expected: &WorkloadExpectedParallelBatchPartitionSet,
) {
    hash_parallel_batch_partition_scope(hash, expected.scope());
    hash_u64(hash, expected.partitions().len() as u64);
    for partition in expected.partitions() {
        hash_u64(hash, u64::from(partition.index()));
    }
    hash_u64(hash, expected.minimum_batch_count() as u64);
}

fn hash_expected_parallel_batch_partition_streak(
    hash: &mut u64,
    expected: &WorkloadExpectedParallelBatchPartitionStreak,
) {
    hash_parallel_batch_partition_scope(hash, expected.scope());
    hash_u64(hash, expected.partitions().len() as u64);
    for partition in expected.partitions() {
        hash_u64(hash, u64::from(partition.index()));
    }
    hash_u64(hash, expected.minimum_consecutive_batch_count() as u64);
}

fn hash_expected_parallel_batch_timeline_record(
    hash: &mut u64,
    expected: &WorkloadExpectedParallelBatchTimelineRecord,
) {
    hash_parallel_batch_timeline_scope(hash, expected.scope());
    hash_str(hash, expected.batch_scope().as_str());
    hash_u64(hash, expected.start_tick());
    hash_u64(hash, expected.horizon());
    hash_u64(hash, expected.partitions().len() as u64);
    for partition in expected.partitions() {
        hash_u64(hash, u64::from(partition.index()));
    }
    hash_u64(hash, expected.worker_count() as u64);
}

fn hash_expected_parallel_partition_use(
    hash: &mut u64,
    expected: WorkloadExpectedParallelPartitionUse,
) {
    hash_parallel_batch_partition_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_active_partitions() as u64);
}

fn hash_expected_parallel_partition_activity(
    hash: &mut u64,
    expected: WorkloadExpectedParallelPartitionActivity,
) {
    hash_parallel_batch_partition_scope(hash, expected.scope());
    hash_u64(hash, u64::from(expected.partition().index()));
    hash_u64(hash, expected.minimum_worker_count() as u64);
    hash_u64(hash, expected.minimum_dispatch_count() as u64);
    hash_u64(hash, expected.minimum_remote_send_count() as u64);
    hash_u64(hash, expected.minimum_remote_receive_count() as u64);
}

fn hash_expected_parallel_frontier(hash: &mut u64, expected: WorkloadExpectedParallelFrontier) {
    hash_parallel_scheduler_scope(hash, expected.scope());
    hash_parallel_frontier_stage(hash, expected.stage());
    hash_u64(hash, u64::from(expected.partition().index()));
    hash_u64(hash, expected.minimum_now());
    hash_u64(hash, expected.minimum_safe_until());
}

fn hash_expected_resource_activity(hash: &mut u64, expected: WorkloadExpectedResourceActivity) {
    hash_resource_activity_scope(hash, expected.scope());
    hash_u64(hash, expected.minimum_operation_count() as u64);
    hash_u64(hash, expected.minimum_active_resource_count() as u64);
}

fn hash_optional_expected_dram_low_power_activity(
    hash: &mut u64,
    expected: Option<WorkloadExpectedDramLowPowerActivity>,
) {
    let Some(expected) = expected else {
        hash_u64(hash, 0);
        return;
    };
    hash_u64(hash, 1);
    hash_u64(
        hash,
        expected.minimum_entry_count(rem6_dram::DramLowPowerState::ActivePowerdown) as u64,
    );
    hash_u64(
        hash,
        expected.minimum_cycle_count(rem6_dram::DramLowPowerState::ActivePowerdown),
    );
    hash_u64(
        hash,
        expected.minimum_entry_count(rem6_dram::DramLowPowerState::PrechargePowerdown) as u64,
    );
    hash_u64(
        hash,
        expected.minimum_cycle_count(rem6_dram::DramLowPowerState::PrechargePowerdown),
    );
    hash_u64(
        hash,
        expected.minimum_entry_count(rem6_dram::DramLowPowerState::SelfRefresh) as u64,
    );
    hash_u64(
        hash,
        expected.minimum_cycle_count(rem6_dram::DramLowPowerState::SelfRefresh),
    );
    hash_u64(hash, expected.minimum_exit_count() as u64);
    hash_u64(hash, expected.minimum_exit_latency_cycles());
}

fn hash_expected_fabric_hop_activity(hash: &mut u64, expected: &WorkloadExpectedFabricHopActivity) {
    hash_u64(hash, expected.hop_index() as u64);
    hash_str(hash, expected.link().as_str());
    hash_u64(hash, u64::from(expected.virtual_network().get()));
    hash_u64(hash, expected.minimum_transfer_count() as u64);
    hash_u64(hash, expected.minimum_byte_count());
    hash_u64(hash, expected.minimum_occupied_ticks());
    hash_u64(hash, expected.minimum_queue_delay_ticks());
    hash_optional_tick(hash, expected.required_first_tick());
    hash_optional_tick(hash, expected.required_last_tick());
}

fn hash_expected_fabric_lane_activity(
    hash: &mut u64,
    expected: &WorkloadExpectedFabricLaneActivity,
) {
    hash_str(hash, expected.link().as_str());
    hash_u64(hash, u64::from(expected.virtual_network().get()));
    hash_u64(hash, expected.minimum_transfer_count() as u64);
    hash_u64(hash, expected.minimum_byte_count());
    hash_u64(hash, expected.minimum_occupied_ticks());
    hash_u64(hash, expected.minimum_queue_delay_ticks());
    hash_u64(hash, expected.minimum_max_queue_delay_ticks());
    hash_optional_tick(hash, expected.maximum_queue_delay_ticks());
    hash_optional_tick(hash, expected.maximum_max_queue_delay_ticks());
    hash_optional_tick(hash, expected.required_first_tick());
    hash_optional_tick(hash, expected.required_last_tick());
}

fn hash_optional_tick(hash: &mut u64, tick: Option<Tick>) {
    match tick {
        Some(tick) => {
            hash_u64(hash, 1);
            hash_u64(hash, tick);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_optional_str(hash: &mut u64, value: Option<&str>) {
    match value {
        Some(value) => {
            hash_u64(hash, 1);
            hash_str(hash, value);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_optional_usize(hash: &mut u64, value: Option<usize>) {
    match value {
        Some(value) => {
            hash_u64(hash, 1);
            hash_u64(hash, value as u64);
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_expected_fabric_link_activity(
    hash: &mut u64,
    expected: &WorkloadExpectedFabricLinkActivity,
) {
    hash_str(hash, expected.link().as_str());
    hash_u64(hash, expected.minimum_transfer_count() as u64);
    hash_u64(hash, expected.minimum_active_virtual_network_count() as u64);
    hash_u64(hash, expected.minimum_queue_delay_ticks());
    hash_u64(
        hash,
        expected.minimum_contended_virtual_network_count() as u64,
    );
    hash_optional_tick(hash, expected.maximum_queue_delay_ticks());
    hash_optional_tick(hash, expected.maximum_max_queue_delay_ticks());
    hash_optional_tick(hash, expected.required_first_tick());
    hash_optional_tick(hash, expected.required_last_tick());
}

fn hash_expected_fabric_virtual_network_activity(
    hash: &mut u64,
    expected: &WorkloadExpectedFabricVirtualNetworkActivity,
) {
    hash_u64(hash, u64::from(expected.virtual_network().get()));
    hash_u64(hash, expected.minimum_transfer_count() as u64);
    hash_u64(hash, expected.minimum_active_lane_count() as u64);
    hash_u64(hash, expected.minimum_queue_delay_ticks());
    hash_u64(hash, expected.minimum_contended_lane_count() as u64);
    hash_optional_tick(hash, expected.maximum_queue_delay_ticks());
    hash_optional_tick(hash, expected.maximum_max_queue_delay_ticks());
    hash_optional_usize(hash, expected.maximum_active_lane_count());
    hash_optional_usize(hash, expected.maximum_contended_lane_count());
    hash_optional_tick(hash, expected.required_first_tick());
    hash_optional_tick(hash, expected.required_last_tick());
    hash_u64(hash, expected.required_links().len() as u64);
    for link in expected.required_links() {
        hash_str(hash, link.as_str());
    }
}

fn hash_linux_boot_handoff(hash: &mut u64, handoff: Option<&WorkloadLinuxBootHandoff>) {
    let Some(handoff) = handoff else {
        hash_str(hash, "linux.boot_handoff.none");
        return;
    };

    hash_str(hash, "linux.boot_handoff.v2");
    hash_u64(hash, handoff.dtb_addr().get());
    match handoff.device_tree_resource() {
        Some(resource) => {
            hash_str(hash, "device_tree.some");
            hash_str(hash, resource.as_str());
        }
        None => hash_str(hash, "device_tree.none"),
    }
    match handoff.bootargs() {
        Some(bootargs) => {
            hash_str(hash, "bootargs.some");
            hash_str(hash, bootargs);
        }
        None => hash_str(hash, "bootargs.none"),
    }
    match handoff.initrd() {
        Some(initrd) => {
            hash_str(hash, "initrd.some");
            hash_str(hash, initrd.resource().as_str());
            hash_u64(hash, initrd.start().get());
            hash_u64(hash, initrd.size().bytes());
        }
        None => hash_str(hash, "initrd.none"),
    }
}

fn hash_elf_metadata(hash: &mut u64, metadata: Option<BootElfMetadata>) {
    match metadata {
        Some(metadata) => {
            hash_u64(hash, 1);
            hash_elf_class(hash, metadata.class());
            hash_elf_endian(hash, metadata.endian());
            hash_u64(hash, u64::from(metadata.machine()));
            hash_u64(hash, u64::from(metadata.os_abi()));
            hash_u64(hash, u64::from(metadata.flags()));
            hash_elf_architecture(hash, metadata.architecture());
            hash_elf_operating_system(hash, metadata.operating_system());
        }
        None => hash_u64(hash, 0),
    }
}

fn hash_elf_class(hash: &mut u64, class: BootElfClass) {
    let value = match class {
        BootElfClass::Class32 => 1,
        BootElfClass::Class64 => 2,
    };
    hash_u64(hash, value);
}

fn hash_elf_endian(hash: &mut u64, endian: BootElfEndian) {
    let value = match endian {
        BootElfEndian::Little => 1,
        BootElfEndian::Big => 2,
    };
    hash_u64(hash, value);
}

fn hash_elf_architecture(hash: &mut u64, architecture: BootElfArchitecture) {
    match architecture {
        BootElfArchitecture::Sparc32 => hash_u64(hash, 1),
        BootElfArchitecture::Sparc64 => hash_u64(hash, 2),
        BootElfArchitecture::Mips => hash_u64(hash, 3),
        BootElfArchitecture::I386 => hash_u64(hash, 4),
        BootElfArchitecture::X8664 => hash_u64(hash, 5),
        BootElfArchitecture::Arm => hash_u64(hash, 6),
        BootElfArchitecture::Thumb => hash_u64(hash, 7),
        BootElfArchitecture::Arm64 => hash_u64(hash, 8),
        BootElfArchitecture::Riscv32 => hash_u64(hash, 9),
        BootElfArchitecture::Riscv64 => hash_u64(hash, 10),
        BootElfArchitecture::Power => hash_u64(hash, 11),
        BootElfArchitecture::Power64 => hash_u64(hash, 12),
        BootElfArchitecture::Unknown { machine, class } => {
            hash_u64(hash, 13);
            hash_u64(hash, u64::from(machine));
            hash_elf_class(hash, class);
        }
    }
}

fn hash_elf_operating_system(hash: &mut u64, operating_system: BootElfOperatingSystem) {
    match operating_system {
        BootElfOperatingSystem::Linux => hash_u64(hash, 1),
        BootElfOperatingSystem::Solaris => hash_u64(hash, 2),
        BootElfOperatingSystem::Tru64 => hash_u64(hash, 3),
        BootElfOperatingSystem::LinuxArmOabi => hash_u64(hash, 4),
        BootElfOperatingSystem::LinuxPower64AbiV1 => hash_u64(hash, 5),
        BootElfOperatingSystem::LinuxPower64AbiV2 => hash_u64(hash, 6),
        BootElfOperatingSystem::FreeBsd => hash_u64(hash, 7),
        BootElfOperatingSystem::Unknown { os_abi } => {
            hash_u64(hash, 8);
            hash_u64(hash, u64::from(os_abi));
        }
    }
}

fn hash_topology(hash: &mut u64, topology: Option<&WorkloadTopology>) {
    let Some(topology) = topology else {
        hash_str(hash, "topology.none");
        return;
    };

    hash_str(hash, "topology.riscv.v1");
    hash_u64(hash, u64::from(topology.partition_count()));
    hash_u64(hash, topology.min_remote_delay());
    hash_u64(hash, topology.parallel_worker_limit() as u64);
    hash_u64(hash, u64::from(topology.host().partition()));
    hash_u64(hash, topology.host().latency());
    hash_u64(hash, u64::from(topology.host().source()));
    hash_qos_policy(hash, topology.qos_policy());
    hash_u64(hash, topology.memory_targets().len() as u64);
    for target in topology.memory_targets() {
        hash_u64(hash, u64::from(target.target()));
        hash_u64(hash, target.line_bytes());
        hash_u64(hash, target.range().start().get());
        hash_u64(hash, target.range().size().bytes());
        hash_external_memory_profile(hash, target.external_memory_profile());
    }
    hash_u64(hash, topology.memory_routes().len() as u64);
    for route in topology.memory_routes() {
        hash_str(hash, route.id().as_str());
        hash_str(hash, route.source_endpoint());
        hash_u64(hash, u64::from(route.source_partition()));
        hash_str(hash, route.target_endpoint());
        hash_u64(hash, u64::from(route.target_partition()));
        hash_u64(hash, route.request_latency());
        hash_u64(hash, route.response_latency());
        hash_u64(hash, route.hops().len() as u64);
        for hop in route.hops() {
            hash_str(hash, "route.hop");
            hash_str(hash, hop.endpoint());
            hash_u64(hash, u64::from(hop.partition()));
            hash_u64(hash, hop.request_latency());
            hash_u64(hash, hop.response_latency());
            hash_route_fabric(hash, hop.fabric());
        }
    }
    hash_u64(hash, topology.riscv_cores().len() as u64);
    for core in topology.riscv_cores() {
        hash_u64(hash, u64::from(core.cpu()));
        hash_u64(hash, u64::from(core.partition()));
        hash_u64(hash, u64::from(core.agent()));
        hash_u64(hash, core.entry().get());
        hash_str(hash, core.fetch_endpoint());
        hash_str(hash, core.fetch_route().as_str());
        match (core.data_endpoint(), core.data_route()) {
            (Some(endpoint), Some(route)) => {
                hash_str(hash, "data");
                hash_str(hash, endpoint);
                hash_str(hash, route.as_str());
                if let Some(translation) = core.data_translation() {
                    hash_str(hash, "data.translation");
                    hash_u64(hash, translation.queue().capacity() as u64);
                    hash_u64(hash, translation.queue().latency());
                    match translation.tlb() {
                        Some(tlb) => {
                            hash_str(hash, "data.translation.tlb");
                            hash_u64(hash, tlb.capacity() as u64);
                        }
                        None => hash_str(hash, "data.translation.tlb.none"),
                    }
                    hash_u64(hash, translation.page_size_bytes());
                    for mapping in translation.page_mappings() {
                        hash_str(hash, "data.translation.mapping");
                        hash_u64(hash, mapping.virtual_base().get());
                        hash_u64(hash, mapping.physical_base().get());
                        hash_u64(hash, mapping.pages());
                    }
                }
            }
            (None, None) => hash_str(hash, "data.none"),
            _ => hash_str(hash, "data.invalid"),
        }
    }
    match topology.riscv_data_cache() {
        Some(cache) => {
            hash_str(hash, "riscv.data_cache");
            hash_str(hash, cache.protocol().as_str());
            hash_u64(hash, u64::from(cache.memory_target()));
            for line_address in cache.line_addresses() {
                hash_u64(hash, line_address.get());
            }
            hash_u64(hash, u64::from(cache.directory_partition()));
            hash_str(hash, cache.directory_endpoint());
            hash_str(hash, cache.backing_route().as_str());
        }
        None => hash_str(hash, "riscv.data_cache.none"),
    }
    hash_u64(hash, topology.gpu_devices().len() as u64);
    for device in topology.gpu_devices() {
        hash_str(hash, "gpu.device");
        hash_u64(hash, u64::from(device.device()));
        hash_u64(hash, u64::from(device.partition()));
        hash_u64(hash, u64::from(device.compute_units()));
        hash_u64(hash, u64::from(device.wave_slots_per_compute_unit()));
        hash_str(hash, device.command_endpoint());
        hash_str(hash, device.dma_endpoint());
        hash_str(hash, device.command_route().as_str());
    }
    hash_u64(hash, topology.gpu_kernel_launches().len() as u64);
    for launch in topology.gpu_kernel_launches() {
        hash_str(hash, "gpu.kernel_launch");
        hash_u64(hash, u64::from(launch.device()));
        hash_u64(hash, launch.kernel());
        hash_u64(hash, u64::from(launch.workgroups()));
        hash_u64(hash, launch.workgroup_latency());
    }
    hash_u64(hash, topology.gpu_dma_copies().len() as u64);
    for copy in topology.gpu_dma_copies() {
        hash_str(hash, "gpu.dma_copy");
        hash_u64(hash, u64::from(copy.device()));
        hash_u64(hash, copy.transfer());
        hash_str(hash, copy.route().as_str());
        hash_u64(hash, u64::from(copy.agent()));
        hash_u64(hash, copy.source().get());
        hash_u64(hash, copy.destination().get());
        hash_u64(hash, copy.bytes());
    }
    hash_u64(hash, topology.accelerator_devices().len() as u64);
    for device in topology.accelerator_devices() {
        hash_str(hash, "accelerator.device");
        hash_u64(hash, u64::from(device.engine()));
        hash_u64(hash, u64::from(device.partition()));
        hash_u64(hash, u64::from(device.lanes()));
        hash_str(hash, device.command_endpoint());
        hash_str(hash, device.dma_endpoint());
        hash_str(hash, device.command_route().as_str());
    }
    hash_u64(hash, topology.accelerator_commands().len() as u64);
    for command in topology.accelerator_commands() {
        hash_str(hash, "accelerator.command");
        hash_u64(hash, u64::from(command.engine()));
        hash_u64(hash, command.command());
        hash_accelerator_command_kind(hash, command.kind());
        hash_u64(hash, command.execution_latency());
    }
    hash_u64(hash, topology.accelerator_dma_copies().len() as u64);
    for copy in topology.accelerator_dma_copies() {
        hash_str(hash, "accelerator.dma_copy");
        hash_u64(hash, u64::from(copy.engine()));
        hash_u64(hash, copy.transfer());
        hash_str(hash, copy.route().as_str());
        hash_u64(hash, u64::from(copy.agent()));
        hash_u64(hash, copy.source().get());
        hash_u64(hash, copy.destination().get());
        hash_u64(hash, copy.bytes());
    }
    hash_u64(hash, topology.sinic_pci_devices().len() as u64);
    for device in topology.sinic_pci_devices() {
        hash_str(hash, "sinic.pci.device");
        hash_u64(hash, u64::from(device.nic()));
        hash_u64(hash, u64::from(device.partition()));
        hash_u64(hash, u64::from(device.pci_bus()));
        hash_u64(hash, u64::from(device.pci_device()));
        hash_u64(hash, u64::from(device.pci_function()));
        hash_u64(hash, device.bar_base().get());
        hash_str(hash, device.mmio_endpoint());
        hash_str(hash, device.mmio_route().as_str());
        hash_u64(hash, u64::from(device.interrupt_source()));
    }
}

fn hash_qos_policy(hash: &mut u64, policy: Option<&crate::WorkloadQosPolicy>) {
    let Some(policy) = policy else {
        hash_str(hash, "qos.policy.none");
        return;
    };

    match policy.priority_policy_kind() {
        crate::WorkloadQosPriorityPolicyKind::FixedPriority => {
            hash_str(hash, "qos.policy.fixed_priority.v1");
            hash_u64(hash, u64::from(policy.priority_levels()));
            hash_u64(hash, u64::from(policy.default_priority().get()));
            hash_str(hash, policy.queue_policy().as_str());
            hash_str(hash, policy.turnaround_policy().as_str());
            hash_u64(
                hash,
                if policy.priority_escalation_enabled() {
                    1
                } else {
                    0
                },
            );
            hash_u64(hash, policy.requestor_priorities().len() as u64);
            for requestor in policy.requestor_priorities() {
                hash_u64(hash, u64::from(requestor.requestor().get()));
                hash_u64(hash, u64::from(requestor.priority().get()));
            }
        }
        crate::WorkloadQosPriorityPolicyKind::ProportionalFair => {
            hash_str(hash, "qos.policy.proportional_fair.v1");
            hash_u64(hash, u64::from(policy.priority_levels()));
            hash_u64(
                hash,
                policy
                    .proportional_fair_weight()
                    .expect("proportional-fair policy carries a weight")
                    .to_bits(),
            );
            hash_str(hash, policy.queue_policy().as_str());
            hash_str(hash, policy.turnaround_policy().as_str());
            hash_u64(
                hash,
                if policy.priority_escalation_enabled() {
                    1
                } else {
                    0
                },
            );
            hash_u64(hash, policy.requestor_scores().len() as u64);
            for requestor in policy.requestor_scores() {
                hash_u64(hash, u64::from(requestor.requestor().get()));
                hash_u64(hash, requestor.score_bits());
            }
        }
    }
}

fn hash_accelerator_command_kind(hash: &mut u64, kind: &crate::WorkloadAcceleratorCommandKind) {
    match kind {
        crate::WorkloadAcceleratorCommandKind::GpuKernel { workgroups } => {
            hash_str(hash, "gpu_kernel");
            hash_u64(hash, u64::from(*workgroups));
        }
        crate::WorkloadAcceleratorCommandKind::NpuInference { tiles } => {
            hash_str(hash, "npu_inference");
            hash_u64(hash, u64::from(*tiles));
        }
        crate::WorkloadAcceleratorCommandKind::DmaCopy { bytes } => {
            hash_str(hash, "dma_copy");
            hash_u64(hash, *bytes);
        }
    }
}

fn hash_external_memory_profile(hash: &mut u64, profile: Option<&ExternalMemoryProfile>) {
    let Some(profile) = profile else {
        hash_str(hash, "memory.profile.none");
        return;
    };

    hash_str(hash, "memory.profile.v1");
    hash_u64(hash, u64::from(profile.target().get()));
    hash_u64(hash, profile.line_layout().bytes());
    hash_u64(hash, u64::from(profile.geometry().bank_count()));
    hash_u64(hash, profile.geometry().row_size());
    hash_u64(hash, profile.geometry().line_size());
    match profile.geometry().bank_group_count() {
        Some(bank_group_count) => {
            hash_str(hash, "geometry.bank_groups.some");
            hash_u64(hash, u64::from(bank_group_count));
        }
        None => hash_str(hash, "geometry.bank_groups.none"),
    }
    hash_u64(hash, profile.timing().activate_latency());
    hash_u64(hash, profile.timing().read_latency());
    hash_u64(hash, profile.timing().write_latency());
    hash_u64(hash, profile.timing().precharge_latency());
    hash_u64(hash, profile.timing().bus_turnaround());
    hash_u64(hash, profile.timing().burst_spacing());
    match profile.timing().same_bank_group_burst_spacing() {
        Some(burst_spacing) => {
            hash_str(hash, "timing.same_bank_group_burst_spacing.some");
            hash_u64(hash, burst_spacing);
        }
        None => hash_str(hash, "timing.same_bank_group_burst_spacing.none"),
    }
    match profile.timing().command_window() {
        Some(command_window) => {
            hash_str(hash, "timing.command_window.some");
            hash_u64(hash, command_window.window_cycles());
            hash_u64(hash, u64::from(command_window.max_commands()));
        }
        None => hash_str(hash, "timing.command_window.none"),
    }
    match profile.timing().low_power_timing() {
        Some(low_power_timing) => {
            hash_str(hash, "timing.low_power.some");
            hash_u64(hash, low_power_timing.precharge_powerdown_entry_delay());
            hash_u64(hash, low_power_timing.self_refresh_entry_delay());
            hash_u64(hash, low_power_timing.exit_latency());
            hash_u64(hash, low_power_timing.self_refresh_exit_latency());
        }
        None => hash_str(hash, "timing.low_power.none"),
    }
    match profile.technology() {
        DramMemoryTechnology::Ddr => hash_str(hash, "ddr"),
        DramMemoryTechnology::Hbm => hash_str(hash, "hbm"),
        DramMemoryTechnology::Lpddr => hash_str(hash, "lpddr"),
        DramMemoryTechnology::Nvm => hash_str(hash, "nvm"),
    }
    match profile.topology() {
        ExternalMemoryTopology::Ddr {
            channels,
            ranks_per_channel,
        } => {
            hash_str(hash, "ddr.topology");
            hash_u64(hash, u64::from(channels));
            hash_u64(hash, u64::from(ranks_per_channel));
        }
        ExternalMemoryTopology::Hbm {
            stacks,
            pseudo_channels_per_stack,
        } => {
            hash_str(hash, "hbm.topology");
            hash_u64(hash, u64::from(stacks));
            hash_u64(hash, u64::from(pseudo_channels_per_stack));
        }
        ExternalMemoryTopology::Lpddr {
            channels,
            dies_per_channel,
        } => {
            hash_str(hash, "lpddr.topology");
            hash_u64(hash, u64::from(channels));
            hash_u64(hash, u64::from(dies_per_channel));
        }
        ExternalMemoryTopology::Nvm {
            controllers,
            media_banks_per_controller,
        } => {
            hash_str(hash, "nvm.topology");
            hash_u64(hash, u64::from(controllers));
            hash_u64(hash, u64::from(media_banks_per_controller));
        }
    }
    match profile.nvm_media_timing() {
        Some(nvm_media_timing) => {
            hash_str(hash, "nvm.media");
            hash_u64(hash, nvm_media_timing.read_media_latency());
            hash_u64(hash, nvm_media_timing.write_media_latency());
            hash_u64(hash, nvm_media_timing.send_latency());
            hash_u64(hash, u64::from(nvm_media_timing.max_pending_reads()));
            hash_u64(hash, u64::from(nvm_media_timing.max_pending_writes()));
        }
        None => hash_str(hash, "nvm.media.none"),
    }
}

fn hash_route_fabric(hash: &mut u64, fabric: Option<&crate::WorkloadRouteFabric>) {
    let Some(fabric) = fabric else {
        hash_str(hash, "route.fabric.none");
        return;
    };

    hash_str(hash, "route.fabric.v1");
    hash_str(hash, fabric.link());
    hash_u64(hash, fabric.bandwidth_bytes_per_tick());
    hash_u64(hash, u64::from(fabric.request_virtual_network()));
    hash_u64(hash, u64::from(fabric.response_virtual_network()));
    match fabric.credit_depth() {
        Some(credit_depth) => {
            hash_str(hash, "route.fabric.credit");
            hash_u64(hash, u64::from(credit_depth));
        }
        None => hash_str(hash, "route.fabric.no_credit"),
    }
}

fn hash_host_event(hash: &mut u64, intent: &HostEventIntent) {
    match intent {
        HostEventIntent::RoiBegin { label } => {
            hash_str(hash, "roi_begin");
            hash_str(hash, label);
        }
        HostEventIntent::RoiEnd { label } => {
            hash_str(hash, "roi_end");
            hash_str(hash, label);
        }
        HostEventIntent::StatsReset { label } => {
            hash_str(hash, "stats_reset");
            hash_str(hash, label);
        }
        HostEventIntent::StatsDump { label } => {
            hash_str(hash, "stats_dump");
            hash_str(hash, label);
        }
        HostEventIntent::SwitchExecutionMode { target, mode } => {
            hash_str(hash, "execution_mode");
            hash_str(hash, target);
            hash_str(hash, mode.as_str());
        }
        HostEventIntent::GuestHostCall {
            selector,
            arguments,
            payload,
            response,
        } => {
            hash_str(hash, "guest_host_call");
            hash_u64(hash, *selector);
            hash_u64(hash, arguments.len() as u64);
            for argument in arguments {
                hash_u64(hash, *argument);
            }
            hash_u64(hash, payload.len() as u64);
            for byte in payload {
                hash_u64(hash, u64::from(*byte));
            }
            match response {
                Some(response) => {
                    hash_str(hash, "guest_host_call.response");
                    hash_i32(hash, response.status());
                    hash_u64(hash, response.return_values().len() as u64);
                    for value in response.return_values() {
                        hash_u64(hash, *value);
                    }
                    hash_u64(hash, response.payload().len() as u64);
                    for byte in response.payload() {
                        hash_u64(hash, u64::from(*byte));
                    }
                }
                None => hash_str(hash, "guest_host_call.response.none"),
            }
        }
        HostEventIntent::Checkpoint { label } => {
            hash_str(hash, "checkpoint");
            hash_str(hash, label);
        }
        HostEventIntent::RestoreCheckpoint { label } => {
            hash_str(hash, "restore_checkpoint");
            hash_str(hash, label);
        }
        HostEventIntent::Stop { reason } => {
            hash_str(hash, "stop");
            hash_str(hash, reason);
        }
    }
}

fn hash_checkpoint_lineage(hash: &mut u64, lineage: Option<&CheckpointLineage>) {
    match lineage {
        None => hash_str(hash, "lineage.none"),
        Some(CheckpointLineage::CreatedByWorkload { label }) => {
            hash_str(hash, "lineage.created");
            hash_str(hash, label);
        }
        Some(CheckpointLineage::RestoredFrom {
            label,
            manifest_identity,
        }) => {
            hash_str(hash, "lineage.restored");
            hash_str(hash, label);
            hash_str(hash, manifest_identity);
        }
    }
}

fn hash_str(hash: &mut u64, value: &str) {
    hash_u64(hash, value.len() as u64);
    hash_bytes(hash, value.as_bytes());
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_i32(hash: &mut u64, value: i32) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}
