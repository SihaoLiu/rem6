use std::collections::BTreeMap;

use rem6_boot::BootImage;
use rem6_kernel::Tick;

use crate::host_event::{
    execution_mode_switch_matches, planned_checkpoint_labels, planned_checkpoint_restore_labels,
    planned_execution_mode_switches, planned_stop_reason,
};
use crate::{gups, parallel_expectation, replay_verify, traffic_trace_replay};
use crate::{
    CheckpointLineage, HostEventIntent, WorkloadBootImage, WorkloadCheckpointComponentSummary,
    WorkloadCheckpointManifestSummary, WorkloadError, WorkloadExecutionModeSwitch,
    WorkloadExpectedCheckpointComponentSummary, WorkloadExpectedCheckpointManifestSummary,
    WorkloadExpectedCleanParallelDiagnostics, WorkloadExpectedDataCacheProtocolRunCount,
    WorkloadExpectedDataCacheRunAttribution, WorkloadExpectedDramLowPowerActivity,
    WorkloadExpectedFabricHopActivity, WorkloadExpectedFabricLaneActivity,
    WorkloadExpectedFabricLinkActivity, WorkloadExpectedFabricVirtualNetworkActivity,
    WorkloadExpectedGupsRunSummary, WorkloadExpectedParallelBatchActivity,
    WorkloadExpectedParallelBatchPartitionSet, WorkloadExpectedParallelBatchPartitionStreak,
    WorkloadExpectedParallelBatchTimelineRecord, WorkloadExpectedParallelBatchWorkerBucket,
    WorkloadExpectedParallelBatchWorkerTickActivity, WorkloadExpectedParallelBatchWorkerTickBucket,
    WorkloadExpectedParallelBatchWorkerTickStreak, WorkloadExpectedParallelBatchWorkerTicks,
    WorkloadExpectedParallelFrontier, WorkloadExpectedParallelPartitionActivity,
    WorkloadExpectedParallelPartitionUse, WorkloadExpectedParallelProgressTransition,
    WorkloadExpectedParallelRemoteDelayCeiling, WorkloadExpectedParallelRemoteDelayFloor,
    WorkloadExpectedParallelRemoteEndpoints, WorkloadExpectedParallelRemoteFlow,
    WorkloadExpectedParallelRemoteFlowTiming, WorkloadExpectedParallelRemoteSend,
    WorkloadExpectedParallelRemoteTrafficConsistency, WorkloadExpectedParallelSchedulerIdleBound,
    WorkloadExpectedParallelSchedulerProgress, WorkloadExpectedParallelWaitForBlockedNodeWindow,
    WorkloadExpectedParallelWaitForEdgeKindCount, WorkloadExpectedParallelWaitForEdgeKindWindow,
    WorkloadExpectedParallelWaitForTargetNodeWindow, WorkloadExpectedParallelWorkerActivity,
    WorkloadExpectedParallelWorkerUse, WorkloadExpectedPlannedParallelBatchIdleWorkerTicks,
    WorkloadExpectedPlannedParallelBatchUtilization,
    WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks,
    WorkloadExpectedPlannedParallelBatchWorkerSlotTicks, WorkloadExpectedResourceActivity,
    WorkloadExpectedStatsHistory, WorkloadExpectedTrafficTraceReplaySummary, WorkloadGupsRun,
    WorkloadHostEvent, WorkloadLinuxBootHandoff, WorkloadManifest, WorkloadManifestIdentity,
    WorkloadResource, WorkloadResult, WorkloadTopology, WorkloadTrafficTraceReplayRun,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadReplayPlan {
    pub(crate) manifest_identity: WorkloadManifestIdentity,
    pub(crate) boot: WorkloadBootImage,
    pub(crate) linux_boot_handoff: Option<WorkloadLinuxBootHandoff>,
    pub(crate) topology: Option<WorkloadTopology>,
    pub(crate) required_resources: Vec<WorkloadResource>,
    pub(crate) host_events: Vec<WorkloadHostEvent>,
    pub(crate) planned_checkpoint_labels: Vec<String>,
    pub(crate) planned_checkpoint_restore_labels: Vec<String>,
    pub(crate) planned_execution_mode_switches: Vec<WorkloadExecutionModeSwitch>,
    pub(crate) planned_stop_reason: Option<String>,
    pub(crate) expected_clean_parallel_diagnostics: Vec<WorkloadExpectedCleanParallelDiagnostics>,
    pub(crate) expected_parallel_wait_for_edge_kind_counts:
        Vec<WorkloadExpectedParallelWaitForEdgeKindCount>,
    pub(crate) expected_parallel_wait_for_edge_kind_windows:
        Vec<WorkloadExpectedParallelWaitForEdgeKindWindow>,
    pub(crate) expected_parallel_wait_for_blocked_node_windows:
        Vec<WorkloadExpectedParallelWaitForBlockedNodeWindow>,
    pub(crate) expected_parallel_wait_for_target_node_windows:
        Vec<WorkloadExpectedParallelWaitForTargetNodeWindow>,
    pub(crate) expected_data_cache_protocol_run_counts:
        Vec<WorkloadExpectedDataCacheProtocolRunCount>,
    pub(crate) expected_data_cache_run_attribution: Option<WorkloadExpectedDataCacheRunAttribution>,
    pub(crate) expected_stats_history: Option<WorkloadExpectedStatsHistory>,
    pub(crate) expected_parallel_remote_flows: Vec<WorkloadExpectedParallelRemoteFlow>,
    pub(crate) expected_parallel_remote_endpoints: Vec<WorkloadExpectedParallelRemoteEndpoints>,
    pub(crate) expected_parallel_remote_delay_floors: Vec<WorkloadExpectedParallelRemoteDelayFloor>,
    pub(crate) expected_parallel_remote_delay_ceilings:
        Vec<WorkloadExpectedParallelRemoteDelayCeiling>,
    pub(crate) expected_parallel_remote_traffic_consistency:
        Vec<WorkloadExpectedParallelRemoteTrafficConsistency>,
    pub(crate) expected_parallel_remote_sends: Vec<WorkloadExpectedParallelRemoteSend>,
    pub(crate) expected_parallel_remote_flow_timings: Vec<WorkloadExpectedParallelRemoteFlowTiming>,
    pub(crate) expected_parallel_progress_transitions:
        Vec<WorkloadExpectedParallelProgressTransition>,
    pub(crate) traffic_trace_replays: Vec<WorkloadTrafficTraceReplayRun>,
    pub(crate) expected_traffic_trace_replay_summaries:
        Vec<WorkloadExpectedTrafficTraceReplaySummary>,
    pub(crate) gups_runs: Vec<WorkloadGupsRun>,
    pub(crate) expected_gups_run_summaries: Vec<WorkloadExpectedGupsRunSummary>,
    pub(crate) expected_checkpoint_manifest_summaries:
        Vec<WorkloadExpectedCheckpointManifestSummary>,
    pub(crate) expected_checkpoint_restore_manifest_summaries:
        Vec<WorkloadExpectedCheckpointManifestSummary>,
    pub(crate) expected_checkpoint_component_summaries:
        Vec<WorkloadExpectedCheckpointComponentSummary>,
    pub(crate) expected_checkpoint_restore_component_summaries:
        Vec<WorkloadExpectedCheckpointComponentSummary>,
    pub(crate) expected_parallel_worker_use: Vec<WorkloadExpectedParallelWorkerUse>,
    pub(crate) expected_parallel_worker_activity: Vec<WorkloadExpectedParallelWorkerActivity>,
    pub(crate) expected_parallel_scheduler_progress: Vec<WorkloadExpectedParallelSchedulerProgress>,
    pub(crate) expected_parallel_scheduler_idle_bounds:
        Vec<WorkloadExpectedParallelSchedulerIdleBound>,
    pub(crate) expected_parallel_batch_activity: Vec<WorkloadExpectedParallelBatchActivity>,
    pub(crate) expected_parallel_batch_worker_buckets:
        Vec<WorkloadExpectedParallelBatchWorkerBucket>,
    pub(crate) expected_parallel_batch_worker_tick_buckets:
        Vec<WorkloadExpectedParallelBatchWorkerTickBucket>,
    pub(crate) expected_parallel_batch_worker_tick_activity:
        Vec<WorkloadExpectedParallelBatchWorkerTickActivity>,
    pub(crate) expected_parallel_batch_worker_tick_streaks:
        Vec<WorkloadExpectedParallelBatchWorkerTickStreak>,
    pub(crate) expected_parallel_batch_worker_ticks: Vec<WorkloadExpectedParallelBatchWorkerTicks>,
    pub(crate) expected_planned_parallel_batch_utilization:
        Vec<WorkloadExpectedPlannedParallelBatchUtilization>,
    pub(crate) expected_planned_parallel_batch_idle_worker_ticks:
        Vec<WorkloadExpectedPlannedParallelBatchIdleWorkerTicks>,
    pub(crate) expected_planned_parallel_batch_worker_slot_ticks:
        Vec<WorkloadExpectedPlannedParallelBatchWorkerSlotTicks>,
    pub(crate) expected_planned_parallel_batch_worker_lane_partition_ticks:
        Vec<WorkloadExpectedPlannedParallelBatchWorkerLanePartitionTicks>,
    pub(crate) expected_parallel_batch_partition_sets:
        Vec<WorkloadExpectedParallelBatchPartitionSet>,
    pub(crate) expected_parallel_batch_partition_streaks:
        Vec<WorkloadExpectedParallelBatchPartitionStreak>,
    pub(crate) expected_parallel_batch_timeline_records:
        Vec<WorkloadExpectedParallelBatchTimelineRecord>,
    pub(crate) expected_parallel_partition_use: Vec<WorkloadExpectedParallelPartitionUse>,
    pub(crate) expected_parallel_partition_activity: Vec<WorkloadExpectedParallelPartitionActivity>,
    pub(crate) expected_parallel_frontiers: Vec<WorkloadExpectedParallelFrontier>,
    pub(crate) expected_resource_activity: Vec<WorkloadExpectedResourceActivity>,
    pub(crate) expected_dram_low_power_activity: Option<WorkloadExpectedDramLowPowerActivity>,
    pub(crate) expected_fabric_hop_activity: Vec<WorkloadExpectedFabricHopActivity>,
    pub(crate) expected_fabric_lane_activity: Vec<WorkloadExpectedFabricLaneActivity>,
    pub(crate) expected_fabric_link_activity: Vec<WorkloadExpectedFabricLinkActivity>,
    pub(crate) expected_fabric_virtual_network_activity:
        Vec<WorkloadExpectedFabricVirtualNetworkActivity>,
    pub(crate) checkpoint_lineage: Option<CheckpointLineage>,
}

impl WorkloadReplayPlan {
    pub fn from_manifest(manifest: &WorkloadManifest) -> Result<Self, WorkloadError> {
        let host_events = manifest.host_events().to_vec();
        Ok(Self {
            manifest_identity: manifest.identity(),
            boot: manifest.boot().clone(),
            linux_boot_handoff: manifest.linux_boot_handoff().cloned(),
            topology: manifest.topology().cloned(),
            required_resources: manifest.required_resource_details()?,
            planned_checkpoint_labels: planned_checkpoint_labels(&host_events),
            planned_checkpoint_restore_labels: planned_checkpoint_restore_labels(&host_events),
            planned_execution_mode_switches: planned_execution_mode_switches(&host_events),
            planned_stop_reason: planned_stop_reason(&host_events),
            expected_clean_parallel_diagnostics: manifest
                .expected_clean_parallel_diagnostics()
                .to_vec(),
            expected_parallel_wait_for_edge_kind_counts: manifest
                .expected_parallel_wait_for_edge_kind_counts()
                .to_vec(),
            expected_parallel_wait_for_edge_kind_windows: manifest
                .expected_parallel_wait_for_edge_kind_windows()
                .to_vec(),
            expected_parallel_wait_for_blocked_node_windows: manifest
                .expected_parallel_wait_for_blocked_node_windows()
                .to_vec(),
            expected_parallel_wait_for_target_node_windows: manifest
                .expected_parallel_wait_for_target_node_windows()
                .to_vec(),
            expected_data_cache_protocol_run_counts: manifest
                .expected_data_cache_protocol_run_counts()
                .to_vec(),
            expected_data_cache_run_attribution: manifest
                .expected_data_cache_run_attribution()
                .copied(),
            expected_stats_history: manifest.expected_stats_history().cloned(),
            expected_parallel_remote_flows: manifest.expected_parallel_remote_flows().to_vec(),
            expected_parallel_remote_endpoints: manifest
                .expected_parallel_remote_endpoints()
                .to_vec(),
            expected_parallel_remote_delay_floors: manifest
                .expected_parallel_remote_delay_floors()
                .to_vec(),
            expected_parallel_remote_delay_ceilings: manifest
                .expected_parallel_remote_delay_ceilings()
                .to_vec(),
            expected_parallel_remote_traffic_consistency: manifest
                .expected_parallel_remote_traffic_consistency()
                .to_vec(),
            expected_parallel_remote_sends: manifest.expected_parallel_remote_sends().to_vec(),
            expected_parallel_remote_flow_timings: manifest
                .expected_parallel_remote_flow_timings()
                .to_vec(),
            expected_parallel_progress_transitions: manifest
                .expected_parallel_progress_transitions()
                .to_vec(),
            traffic_trace_replays: manifest.traffic_trace_replays().to_vec(),
            expected_traffic_trace_replay_summaries: manifest
                .expected_traffic_trace_replay_summaries()
                .to_vec(),
            gups_runs: manifest.gups_runs().to_vec(),
            expected_gups_run_summaries: manifest.expected_gups_run_summaries().to_vec(),
            expected_checkpoint_manifest_summaries: manifest
                .expected_checkpoint_manifest_summaries()
                .to_vec(),
            expected_checkpoint_restore_manifest_summaries: manifest
                .expected_checkpoint_restore_manifest_summaries()
                .to_vec(),
            expected_checkpoint_component_summaries: manifest
                .expected_checkpoint_component_summaries()
                .to_vec(),
            expected_checkpoint_restore_component_summaries: manifest
                .expected_checkpoint_restore_component_summaries()
                .to_vec(),
            expected_parallel_worker_use: manifest.expected_parallel_worker_use().to_vec(),
            expected_parallel_worker_activity: manifest
                .expected_parallel_worker_activity()
                .to_vec(),
            expected_parallel_scheduler_progress: manifest
                .expected_parallel_scheduler_progress()
                .to_vec(),
            expected_parallel_scheduler_idle_bounds: manifest
                .expected_parallel_scheduler_idle_bounds()
                .to_vec(),
            expected_parallel_batch_activity: manifest.expected_parallel_batch_activity().to_vec(),
            expected_parallel_batch_worker_buckets: manifest
                .expected_parallel_batch_worker_buckets()
                .to_vec(),
            expected_parallel_batch_worker_tick_buckets: manifest
                .expected_parallel_batch_worker_tick_buckets()
                .to_vec(),
            expected_parallel_batch_worker_tick_activity: manifest
                .expected_parallel_batch_worker_tick_activity()
                .to_vec(),
            expected_parallel_batch_worker_tick_streaks: manifest
                .expected_parallel_batch_worker_tick_streaks()
                .to_vec(),
            expected_parallel_batch_worker_ticks: manifest
                .expected_parallel_batch_worker_ticks()
                .to_vec(),
            expected_planned_parallel_batch_utilization: manifest
                .expected_planned_parallel_batch_utilization()
                .to_vec(),
            expected_planned_parallel_batch_idle_worker_ticks: manifest
                .expected_planned_parallel_batch_idle_worker_ticks()
                .to_vec(),
            expected_planned_parallel_batch_worker_slot_ticks: manifest
                .expected_planned_parallel_batch_worker_slot_ticks()
                .to_vec(),
            expected_planned_parallel_batch_worker_lane_partition_ticks: manifest
                .expected_planned_parallel_batch_worker_lane_partition_ticks()
                .to_vec(),
            expected_parallel_batch_partition_sets: manifest
                .expected_parallel_batch_partition_sets()
                .to_vec(),
            expected_parallel_batch_partition_streaks: manifest
                .expected_parallel_batch_partition_streaks()
                .to_vec(),
            expected_parallel_batch_timeline_records: manifest
                .expected_parallel_batch_timeline_records()
                .to_vec(),
            expected_parallel_partition_use: manifest.expected_parallel_partition_use().to_vec(),
            expected_parallel_partition_activity: manifest
                .expected_parallel_partition_activity()
                .to_vec(),
            expected_parallel_frontiers: manifest.expected_parallel_frontiers().to_vec(),
            expected_resource_activity: manifest.expected_resource_activity().to_vec(),
            expected_dram_low_power_activity: manifest.expected_dram_low_power_activity(),
            expected_fabric_hop_activity: manifest.expected_fabric_hop_activity().to_vec(),
            expected_fabric_lane_activity: manifest.expected_fabric_lane_activity().to_vec(),
            expected_fabric_link_activity: manifest.expected_fabric_link_activity().to_vec(),
            expected_fabric_virtual_network_activity: manifest
                .expected_fabric_virtual_network_activity()
                .to_vec(),
            host_events,
            checkpoint_lineage: manifest.checkpoint_lineage().cloned(),
        })
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn boot(&self) -> &WorkloadBootImage {
        &self.boot
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        self.boot.to_boot_image()
    }

    pub fn linux_boot_handoff(&self) -> Option<&WorkloadLinuxBootHandoff> {
        self.linux_boot_handoff.as_ref()
    }

    pub fn topology(&self) -> Option<&WorkloadTopology> {
        self.topology.as_ref()
    }

    pub fn required_resources(&self) -> &[WorkloadResource] {
        &self.required_resources
    }

    pub fn host_events(&self) -> &[WorkloadHostEvent] {
        &self.host_events
    }

    pub fn planned_checkpoint_labels(&self) -> &[String] {
        &self.planned_checkpoint_labels
    }

    pub fn planned_checkpoint_restore_labels(&self) -> &[String] {
        &self.planned_checkpoint_restore_labels
    }

    pub fn planned_execution_mode_switches(&self) -> &[WorkloadExecutionModeSwitch] {
        &self.planned_execution_mode_switches
    }

    pub fn planned_stop_reason(&self) -> Option<&str> {
        self.planned_stop_reason.as_deref()
    }

    pub fn expected_checkpoint_manifest_summaries(
        &self,
    ) -> &[WorkloadExpectedCheckpointManifestSummary] {
        &self.expected_checkpoint_manifest_summaries
    }

    pub fn expected_traffic_trace_replay_summaries(
        &self,
    ) -> &[WorkloadExpectedTrafficTraceReplaySummary] {
        &self.expected_traffic_trace_replay_summaries
    }

    pub fn traffic_trace_replays(&self) -> &[WorkloadTrafficTraceReplayRun] {
        &self.traffic_trace_replays
    }

    pub fn expected_gups_run_summaries(&self) -> &[WorkloadExpectedGupsRunSummary] {
        &self.expected_gups_run_summaries
    }

    pub fn gups_runs(&self) -> &[WorkloadGupsRun] {
        &self.gups_runs
    }

    pub fn expected_checkpoint_restore_manifest_summaries(
        &self,
    ) -> &[WorkloadExpectedCheckpointManifestSummary] {
        &self.expected_checkpoint_restore_manifest_summaries
    }

    pub fn expected_checkpoint_component_summaries(
        &self,
    ) -> &[WorkloadExpectedCheckpointComponentSummary] {
        &self.expected_checkpoint_component_summaries
    }

    pub fn expected_checkpoint_restore_component_summaries(
        &self,
    ) -> &[WorkloadExpectedCheckpointComponentSummary] {
        &self.expected_checkpoint_restore_component_summaries
    }

    pub fn add_expected_data_cache_protocol_run_count(
        mut self,
        expected: WorkloadExpectedDataCacheProtocolRunCount,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_data_cache_protocol_run_counts
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedDataCacheProtocolRunCount {
                protocol: expected.protocol(),
            });
        }
        self.expected_data_cache_protocol_run_counts.push(expected);
        self.expected_data_cache_protocol_run_counts
            .sort_by_key(|count| count.sort_key());
        Ok(self)
    }

    pub fn expected_data_cache_protocol_run_counts(
        &self,
    ) -> &[WorkloadExpectedDataCacheProtocolRunCount] {
        &self.expected_data_cache_protocol_run_counts
    }

    pub fn add_expected_data_cache_run_attribution(
        mut self,
        expected: WorkloadExpectedDataCacheRunAttribution,
    ) -> Result<Self, WorkloadError> {
        if self.expected_data_cache_run_attribution.is_some() {
            return Err(WorkloadError::DuplicateExpectedDataCacheRunAttribution);
        }
        self.expected_data_cache_run_attribution = Some(expected);
        Ok(self)
    }

    pub fn expected_data_cache_run_attribution(
        &self,
    ) -> Option<&WorkloadExpectedDataCacheRunAttribution> {
        self.expected_data_cache_run_attribution.as_ref()
    }

    pub fn add_expected_parallel_worker_use(
        mut self,
        expected: WorkloadExpectedParallelWorkerUse,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_worker_use
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelWorkerUse {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_worker_use.push(expected);
        self.expected_parallel_worker_use
            .sort_by_key(|worker_use| worker_use.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_worker_use(&self) -> &[WorkloadExpectedParallelWorkerUse] {
        &self.expected_parallel_worker_use
    }

    pub fn add_expected_parallel_worker_activity(
        mut self,
        expected: WorkloadExpectedParallelWorkerActivity,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_worker_activity
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelWorkerActivity {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_worker_activity.push(expected);
        self.expected_parallel_worker_activity
            .sort_by_key(|worker_activity| worker_activity.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_worker_activity(&self) -> &[WorkloadExpectedParallelWorkerActivity] {
        &self.expected_parallel_worker_activity
    }

    pub fn add_expected_parallel_scheduler_progress(
        mut self,
        expected: WorkloadExpectedParallelSchedulerProgress,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_scheduler_progress
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelSchedulerProgress {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_scheduler_progress.push(expected);
        self.expected_parallel_scheduler_progress
            .sort_by_key(|progress| progress.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_scheduler_progress(
        &self,
    ) -> &[WorkloadExpectedParallelSchedulerProgress] {
        &self.expected_parallel_scheduler_progress
    }

    pub fn add_expected_parallel_scheduler_idle_bound(
        mut self,
        expected: WorkloadExpectedParallelSchedulerIdleBound,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_scheduler_idle_bounds
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelSchedulerIdleBound {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_scheduler_idle_bounds.push(expected);
        self.expected_parallel_scheduler_idle_bounds
            .sort_by_key(|bound| bound.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_scheduler_idle_bounds(
        &self,
    ) -> &[WorkloadExpectedParallelSchedulerIdleBound] {
        &self.expected_parallel_scheduler_idle_bounds
    }

    pub fn add_expected_parallel_batch_activity(
        mut self,
        expected: WorkloadExpectedParallelBatchActivity,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_activity
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchActivity {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
            });
        }
        self.expected_parallel_batch_activity.push(expected);
        self.expected_parallel_batch_activity
            .sort_by_key(|batch_activity| batch_activity.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_activity(&self) -> &[WorkloadExpectedParallelBatchActivity] {
        &self.expected_parallel_batch_activity
    }

    pub fn add_expected_parallel_batch_partition_set(
        mut self,
        expected: WorkloadExpectedParallelBatchPartitionSet,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_partition_sets
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelBatchPartitionSet {
                scope: expected.scope(),
                partitions: expected.partition_indexes(),
            });
        }
        self.expected_parallel_batch_partition_sets.push(expected);
        self.expected_parallel_batch_partition_sets
            .sort_by_key(|batch_set| batch_set.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_partition_sets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchPartitionSet] {
        &self.expected_parallel_batch_partition_sets
    }

    pub fn add_expected_parallel_batch_partition_streak(
        mut self,
        expected: WorkloadExpectedParallelBatchPartitionStreak,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_partition_streaks
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchPartitionStreak {
                    scope: expected.scope(),
                    partitions: expected.partition_indexes(),
                },
            );
        }
        self.expected_parallel_batch_partition_streaks
            .push(expected);
        self.expected_parallel_batch_partition_streaks
            .sort_by_key(|streak| streak.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_partition_streaks(
        &self,
    ) -> &[WorkloadExpectedParallelBatchPartitionStreak] {
        &self.expected_parallel_batch_partition_streaks
    }

    pub fn add_expected_parallel_batch_timeline_record(
        mut self,
        expected: WorkloadExpectedParallelBatchTimelineRecord,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_batch_timeline_records
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedParallelBatchTimelineRecord {
                    scope: expected.scope(),
                    batch_scope: expected.batch_scope(),
                    start_tick: expected.start_tick(),
                    horizon: expected.horizon(),
                    partitions: expected.partition_indexes(),
                    worker_count: expected.worker_count(),
                },
            );
        }
        self.expected_parallel_batch_timeline_records.push(expected);
        self.expected_parallel_batch_timeline_records
            .sort_by_key(|record| record.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_batch_timeline_records(
        &self,
    ) -> &[WorkloadExpectedParallelBatchTimelineRecord] {
        &self.expected_parallel_batch_timeline_records
    }

    pub fn add_expected_parallel_partition_use(
        mut self,
        expected: WorkloadExpectedParallelPartitionUse,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_partition_use
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelPartitionUse {
                scope: expected.scope(),
            });
        }
        self.expected_parallel_partition_use.push(expected);
        self.expected_parallel_partition_use
            .sort_by_key(|partition_use| partition_use.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_partition_use(&self) -> &[WorkloadExpectedParallelPartitionUse] {
        &self.expected_parallel_partition_use
    }

    pub fn add_expected_parallel_partition_activity(
        mut self,
        expected: WorkloadExpectedParallelPartitionActivity,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_partition_activity
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelPartitionActivity {
                scope: expected.scope(),
                partition: expected.partition().index(),
            });
        }
        self.expected_parallel_partition_activity.push(expected);
        self.expected_parallel_partition_activity
            .sort_by_key(|activity| activity.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_partition_activity(
        &self,
    ) -> &[WorkloadExpectedParallelPartitionActivity] {
        &self.expected_parallel_partition_activity
    }

    pub fn checkpoint_lineage(&self) -> Option<&CheckpointLineage> {
        self.checkpoint_lineage.as_ref()
    }

    pub fn verify_result(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        if result.manifest_identity() != self.manifest_identity {
            return Err(WorkloadError::ManifestIdentityMismatch {
                expected: self.manifest_identity.clone(),
                actual: result.manifest_identity(),
            });
        }

        result.verify_stats_timing()?;
        self.verify_all_planned_events_reached(result.final_tick())?;
        self.verify_checkpoint_labels(result)?;
        self.verify_checkpoint_restore_labels(result)?;
        self.verify_checkpoint_manifest_summaries(result)?;
        self.verify_checkpoint_restore_manifest_summaries(result)?;
        self.verify_checkpoint_component_summaries(result)?;
        self.verify_checkpoint_restore_component_summaries(result)?;
        self.verify_execution_mode_switches(result)?;
        self.verify_stop_reason(result)?;
        self.verify_expected_stats_history(result)?;
        traffic_trace_replay::verify_expected_traffic_trace_replay_summaries(self, result)?;
        gups::verify_expected_gups_run_summaries(self, result)?;
        replay_verify::verify_expected_parallel_remote_flows(self, result)?;
        replay_verify::verify_expected_parallel_remote_sends(self, result)?;
        replay_verify::verify_expected_parallel_progress_transitions(self, result)?;
        replay_verify::verify_expected_parallel_remote_flow_timings(self, result)?;
        replay_verify::verify_expected_parallel_remote_endpoints(self, result)?;
        replay_verify::verify_expected_parallel_remote_delay_floors(self, result)?;
        replay_verify::verify_expected_parallel_remote_delay_ceilings(self, result)?;
        replay_verify::verify_expected_parallel_remote_traffic_consistency(self, result)?;
        self.verify_expected_parallel_worker_use(result)?;
        self.verify_expected_parallel_worker_activity(result)?;
        replay_verify::verify_expected_data_cache_protocol_run_counts(self, result)?;
        replay_verify::verify_expected_data_cache_run_attribution(self, result)?;
        replay_verify::verify_expected_data_cache_run_accounting(self, result)?;
        replay_verify::verify_expected_parallel_scheduler_progress(self, result)?;
        replay_verify::verify_expected_parallel_scheduler_idle_bounds(self, result)?;
        replay_verify::verify_expected_parallel_batch_activity(self, result)?;
        replay_verify::verify_expected_parallel_batch_worker_buckets(self, result)?;
        replay_verify::verify_expected_parallel_batch_worker_tick_buckets(self, result)?;
        replay_verify::verify_expected_parallel_batch_worker_tick_activity(self, result)?;
        replay_verify::verify_expected_parallel_batch_worker_tick_streaks(self, result)?;
        replay_verify::verify_expected_parallel_batch_worker_ticks(self, result)?;
        replay_verify::verify_expected_planned_parallel_batch_utilization(self, result)?;
        replay_verify::verify_expected_planned_parallel_batch_idle_worker_ticks(self, result)?;
        replay_verify::verify_expected_planned_parallel_batch_worker_slot_ticks(self, result)?;
        replay_verify::verify_expected_planned_parallel_batch_worker_lane_partition_ticks(
            self, result,
        )?;
        replay_verify::verify_expected_parallel_batch_partition_sets(self, result)?;
        replay_verify::verify_expected_parallel_batch_partition_streaks(self, result)?;
        replay_verify::verify_expected_parallel_batch_timeline_records(self, result)?;
        self.verify_expected_parallel_partition_use(result)?;
        self.verify_expected_parallel_partition_activity(result)?;
        replay_verify::verify_expected_parallel_frontiers(self, result)?;
        replay_verify::verify_expected_resource_activity(self, result)?;
        replay_verify::verify_expected_dram_low_power_activity(self, result)?;
        parallel_expectation::verify_expected_fabric_hop_activity(self, result)?;
        replay_verify::verify_expected_fabric_lane_activity(self, result)?;
        replay_verify::verify_expected_fabric_link_activity(self, result)?;
        replay_verify::verify_expected_fabric_virtual_network_activity(self, result)?;
        replay_verify::verify_expected_clean_parallel_diagnostics(self, result)?;
        replay_verify::verify_expected_parallel_wait_for_edge_kind_counts(self, result)?;
        replay_verify::verify_expected_parallel_wait_for_edge_kind_windows(self, result)?;
        replay_verify::verify_expected_parallel_wait_for_blocked_node_windows(self, result)?;
        replay_verify::verify_expected_parallel_wait_for_target_node_windows(self, result)?;
        Ok(())
    }

    fn verify_all_planned_events_reached(&self, final_tick: Tick) -> Result<(), WorkloadError> {
        if let Some(event) = self
            .host_events
            .iter()
            .find(|event| event.tick() > final_tick)
        {
            return Err(WorkloadError::PlannedHostEventAfterFinalTick {
                event_tick: event.tick(),
                final_tick,
            });
        }

        Ok(())
    }

    fn verify_checkpoint_labels(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        let planned_counts = label_counts(&self.planned_checkpoint_labels);
        let actual_counts = label_counts(result.checkpoint_labels());
        for label in result.checkpoint_labels() {
            let planned_count = planned_counts.get(label.as_str()).copied().unwrap_or(0);
            let actual_count = actual_counts.get(label.as_str()).copied().unwrap_or(0);
            if actual_count > planned_count {
                return Err(WorkloadError::UnexpectedCheckpointLabel {
                    label: label.clone(),
                });
            }
        }

        for label in &self.planned_checkpoint_labels {
            let planned_count = planned_counts.get(label.as_str()).copied().unwrap_or(0);
            let actual_count = actual_counts.get(label.as_str()).copied().unwrap_or(0);
            if actual_count < planned_count {
                return Err(WorkloadError::MissingCheckpointLabel {
                    label: label.clone(),
                });
            }
        }

        Ok(())
    }

    fn verify_checkpoint_restore_labels(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        let planned_counts = label_counts(&self.planned_checkpoint_restore_labels);
        let actual_counts = label_counts(result.restored_checkpoint_labels());
        for label in result.restored_checkpoint_labels() {
            let planned_count = planned_counts.get(label.as_str()).copied().unwrap_or(0);
            let actual_count = actual_counts.get(label.as_str()).copied().unwrap_or(0);
            if actual_count > planned_count {
                return Err(WorkloadError::UnexpectedCheckpointRestoreLabel {
                    label: label.clone(),
                });
            }
        }

        for label in &self.planned_checkpoint_restore_labels {
            let planned_count = planned_counts.get(label.as_str()).copied().unwrap_or(0);
            let actual_count = actual_counts.get(label.as_str()).copied().unwrap_or(0);
            if actual_count < planned_count {
                return Err(WorkloadError::MissingCheckpointRestoreLabel {
                    label: label.clone(),
                });
            }
        }

        Ok(())
    }

    fn verify_checkpoint_manifest_summaries(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        let mut remaining_planned_ticks = planned_checkpoint_ticks_by_label(&self.host_events);
        for actual in result.checkpoint_manifest_summaries() {
            if actual.tick() > result.final_tick() {
                return Err(WorkloadError::checkpoint_manifest_summary_after_final_tick(
                    actual.label(),
                    actual.tick(),
                    result.final_tick(),
                ));
            }
            let remaining_ticks = remaining_planned_ticks
                .entry(actual.label().to_string())
                .or_default();
            let Some(position) = remaining_ticks
                .iter()
                .position(|tick| *tick == actual.tick())
            else {
                return Err(WorkloadError::checkpoint_manifest_summary_tick_mismatch(
                    actual.label(),
                    actual.tick(),
                    remaining_ticks.iter().copied(),
                ));
            };
            remaining_ticks.remove(position);
        }

        for expected in &self.expected_checkpoint_manifest_summaries {
            let actual = result
                .checkpoint_manifest_summaries()
                .iter()
                .find(|summary| summary.label() == expected.label())
                .ok_or_else(|| WorkloadError::MissingCheckpointManifestSummary {
                    label: expected.label().to_string(),
                })?;

            verify_checkpoint_summary_minimum(expected, actual).map_err(
                |(
                    minimum_component_count,
                    actual_component_count,
                    minimum_chunk_count,
                    actual_chunk_count,
                    minimum_payload_bytes,
                    actual_payload_bytes,
                )| {
                    WorkloadError::CheckpointManifestSummaryBelowMinimum {
                        label: expected.label().to_string(),
                        minimum_component_count,
                        actual_component_count,
                        minimum_chunk_count,
                        actual_chunk_count,
                        minimum_payload_bytes,
                        actual_payload_bytes,
                    }
                },
            )?;
        }

        Ok(())
    }

    fn verify_checkpoint_restore_manifest_summaries(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        let mut remaining_planned_ticks =
            planned_checkpoint_restore_ticks_by_label(&self.host_events);
        for actual in result.restored_checkpoint_manifest_summaries() {
            if actual.tick() > result.final_tick() {
                return Err(
                    WorkloadError::checkpoint_restore_manifest_summary_after_final_tick(
                        actual.label(),
                        actual.tick(),
                        result.final_tick(),
                    ),
                );
            }
            let remaining_ticks = remaining_planned_ticks
                .entry(actual.label().to_string())
                .or_default();
            let Some(position) = remaining_ticks
                .iter()
                .position(|tick| *tick == actual.tick())
            else {
                return Err(
                    WorkloadError::checkpoint_restore_manifest_summary_tick_mismatch(
                        actual.label(),
                        actual.tick(),
                        remaining_ticks.iter().copied(),
                    ),
                );
            };
            remaining_ticks.remove(position);
        }

        for expected in &self.expected_checkpoint_restore_manifest_summaries {
            let actual = result
                .restored_checkpoint_manifest_summaries()
                .iter()
                .find(|summary| summary.label() == expected.label())
                .ok_or_else(|| WorkloadError::MissingCheckpointRestoreManifestSummary {
                    label: expected.label().to_string(),
                })?;

            verify_checkpoint_summary_minimum(expected, actual).map_err(
                |(
                    minimum_component_count,
                    actual_component_count,
                    minimum_chunk_count,
                    actual_chunk_count,
                    minimum_payload_bytes,
                    actual_payload_bytes,
                )| {
                    WorkloadError::CheckpointRestoreManifestSummaryBelowMinimum {
                        label: expected.label().to_string(),
                        minimum_component_count,
                        actual_component_count,
                        minimum_chunk_count,
                        actual_chunk_count,
                        minimum_payload_bytes,
                        actual_payload_bytes,
                    }
                },
            )?;
        }

        Ok(())
    }

    fn verify_checkpoint_component_summaries(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        for expected in &self.expected_checkpoint_component_summaries {
            let manifest = result
                .checkpoint_manifest_summaries()
                .iter()
                .find(|summary| summary.label() == expected.label())
                .ok_or_else(|| WorkloadError::MissingCheckpointManifestSummary {
                    label: expected.label().to_string(),
                })?;
            let actual = manifest
                .component_summaries()
                .iter()
                .find(|summary| summary.component() == expected.component())
                .ok_or_else(|| WorkloadError::MissingCheckpointComponentSummary {
                    label: expected.label().to_string(),
                    component: expected.component().to_string(),
                })?;

            verify_checkpoint_component_minimum(expected, actual).map_err(
                |(
                    minimum_chunk_count,
                    actual_chunk_count,
                    minimum_payload_bytes,
                    actual_payload_bytes,
                )| {
                    WorkloadError::CheckpointComponentSummaryBelowMinimum {
                        label: expected.label().to_string(),
                        component: expected.component().to_string(),
                        minimum_chunk_count,
                        actual_chunk_count,
                        minimum_payload_bytes,
                        actual_payload_bytes,
                    }
                },
            )?;
            if let Some(chunk) = missing_required_checkpoint_chunk(expected, actual) {
                return Err(WorkloadError::MissingCheckpointComponentChunkSummary {
                    label: expected.label().to_string(),
                    component: expected.component().to_string(),
                    chunk: chunk.to_string(),
                });
            }
            if let Some((chunk, actual_payload_bytes)) =
                checkpoint_chunk_payload_below_minimum(expected, actual)
            {
                return Err(
                    WorkloadError::checkpoint_component_chunk_summary_below_minimum(
                        expected.label(),
                        expected.component(),
                        chunk.name(),
                        chunk.minimum_payload_bytes(),
                        actual_payload_bytes,
                    ),
                );
            }
        }

        Ok(())
    }

    fn verify_checkpoint_restore_component_summaries(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        for expected in &self.expected_checkpoint_restore_component_summaries {
            let manifest = result
                .restored_checkpoint_manifest_summaries()
                .iter()
                .find(|summary| summary.label() == expected.label())
                .ok_or_else(|| WorkloadError::MissingCheckpointRestoreManifestSummary {
                    label: expected.label().to_string(),
                })?;
            let actual = manifest
                .component_summaries()
                .iter()
                .find(|summary| summary.component() == expected.component())
                .ok_or_else(|| WorkloadError::MissingCheckpointRestoreComponentSummary {
                    label: expected.label().to_string(),
                    component: expected.component().to_string(),
                })?;

            verify_checkpoint_component_minimum(expected, actual).map_err(
                |(
                    minimum_chunk_count,
                    actual_chunk_count,
                    minimum_payload_bytes,
                    actual_payload_bytes,
                )| {
                    WorkloadError::CheckpointRestoreComponentSummaryBelowMinimum {
                        label: expected.label().to_string(),
                        component: expected.component().to_string(),
                        minimum_chunk_count,
                        actual_chunk_count,
                        minimum_payload_bytes,
                        actual_payload_bytes,
                    }
                },
            )?;
            if let Some(chunk) = missing_required_checkpoint_chunk(expected, actual) {
                return Err(
                    WorkloadError::MissingCheckpointRestoreComponentChunkSummary {
                        label: expected.label().to_string(),
                        component: expected.component().to_string(),
                        chunk: chunk.to_string(),
                    },
                );
            }
            if let Some((chunk, actual_payload_bytes)) =
                checkpoint_chunk_payload_below_minimum(expected, actual)
            {
                return Err(
                    WorkloadError::checkpoint_restore_component_chunk_summary_below_minimum(
                        expected.label(),
                        expected.component(),
                        chunk.name(),
                        chunk.minimum_payload_bytes(),
                        actual_payload_bytes,
                    ),
                );
            }
        }

        Ok(())
    }

    fn verify_execution_mode_switches(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        for actual in result.execution_mode_switches() {
            if !self
                .planned_execution_mode_switches
                .iter()
                .any(|planned| execution_mode_switch_matches(planned, actual))
            {
                return Err(WorkloadError::UnexpectedExecutionModeSwitch {
                    tick: actual.tick(),
                    target: actual.target().to_string(),
                    mode: actual.mode().clone(),
                });
            }
        }

        for planned in &self.planned_execution_mode_switches {
            if !result
                .execution_mode_switches()
                .iter()
                .any(|actual| execution_mode_switch_matches(planned, actual))
            {
                return Err(WorkloadError::MissingExecutionModeSwitch {
                    tick: planned.tick(),
                    target: planned.target().to_string(),
                    mode: planned.mode().clone(),
                });
            }
        }

        Ok(())
    }

    fn verify_stop_reason(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        let Some(expected) = &self.planned_stop_reason else {
            return match result.stop_reason() {
                Some(actual) => Err(WorkloadError::UnexpectedStopReason {
                    actual: actual.to_string(),
                }),
                None => Ok(()),
            };
        };

        if result.stop_reason() == Some(expected.as_str()) {
            return Ok(());
        }

        Err(WorkloadError::StopReasonMismatch {
            expected: expected.clone(),
            actual: result.stop_reason().map(str::to_string),
        })
    }

    fn verify_expected_parallel_worker_use(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_worker_use.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_parallel_worker_use[0];
            return Err(WorkloadError::MissingParallelWorkerSummary {
                scope: expected.scope(),
                minimum_max_workers: expected.minimum_max_workers(),
            });
        };

        for expected in &self.expected_parallel_worker_use {
            replay_verify::validate_worker_scope_batch_timeline_evidence(
                summary,
                expected.scope(),
            )?;
            let actual_max_workers = expected.actual_max_workers(summary);
            if actual_max_workers < expected.minimum_max_workers() {
                return Err(WorkloadError::ExpectedParallelWorkerCountBelowMinimum {
                    scope: expected.scope(),
                    minimum_max_workers: expected.minimum_max_workers(),
                    actual_max_workers,
                });
            }
        }
        Ok(())
    }

    fn verify_expected_parallel_worker_activity(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_worker_activity.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_parallel_worker_activity[0];
            return Err(WorkloadError::MissingParallelWorkerActivitySummary {
                scope: expected.scope(),
                minimum_total_workers: expected.minimum_total_workers(),
            });
        };

        for expected in &self.expected_parallel_worker_activity {
            replay_verify::validate_worker_scope_batch_timeline_evidence(
                summary,
                expected.scope(),
            )?;
            let actual_total_workers = expected.actual_total_workers(summary);
            if actual_total_workers < expected.minimum_total_workers() {
                return Err(WorkloadError::ExpectedParallelWorkerActivityBelowMinimum {
                    scope: expected.scope(),
                    minimum_total_workers: expected.minimum_total_workers(),
                    actual_total_workers,
                });
            }
        }
        Ok(())
    }

    fn verify_expected_parallel_partition_use(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_partition_use.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_parallel_partition_use[0];
            return Err(WorkloadError::MissingParallelPartitionSummary {
                scope: expected.scope(),
                minimum_active_partitions: expected.minimum_active_partitions(),
            });
        };

        for expected in &self.expected_parallel_partition_use {
            replay_verify::validate_remote_partition_scope_evidence(summary, expected.scope())?;
            replay_verify::validate_partition_scope_batch_timeline_evidence(
                summary,
                expected.scope(),
            )?;
            replay_verify::validate_partition_scope_count_evidence(summary, expected.scope())?;
            replay_verify::validate_partition_scope_activity_evidence(summary, expected.scope())?;
            let actual_active_partitions = expected.actual_active_partitions(summary);
            if actual_active_partitions < expected.minimum_active_partitions() {
                return Err(WorkloadError::ExpectedParallelPartitionCountBelowMinimum {
                    scope: expected.scope(),
                    minimum_active_partitions: expected.minimum_active_partitions(),
                    actual_active_partitions,
                });
            }
        }
        Ok(())
    }

    fn verify_expected_parallel_partition_activity(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_partition_activity.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_parallel_partition_activity[0];
            return Err(WorkloadError::MissingParallelPartitionActivitySummary {
                scope: expected.scope(),
                partition: expected.partition().index(),
            });
        };

        for expected in &self.expected_parallel_partition_activity {
            replay_verify::validate_remote_partition_scope_evidence(summary, expected.scope())?;
            replay_verify::validate_partition_scope_batch_timeline_evidence(
                summary,
                expected.scope(),
            )?;
            replay_verify::validate_partition_scope_activity_evidence(summary, expected.scope())?;
            let actual = expected.actual_activity(summary);
            let actual_worker_count = actual.map(|activity| activity.worker_count()).unwrap_or(0);
            let actual_dispatch_count = actual
                .map(|activity| activity.dispatch_count())
                .unwrap_or(0);
            let actual_remote_send_count = actual
                .map(|activity| activity.remote_send_count())
                .unwrap_or(0);
            let actual_remote_receive_count = actual
                .map(|activity| activity.remote_receive_count())
                .unwrap_or(0);
            if actual_worker_count < expected.minimum_worker_count()
                || actual_dispatch_count < expected.minimum_dispatch_count()
                || actual_remote_send_count < expected.minimum_remote_send_count()
                || actual_remote_receive_count < expected.minimum_remote_receive_count()
            {
                return Err(
                    WorkloadError::ExpectedParallelPartitionActivityBelowMinimum {
                        scope: expected.scope(),
                        partition: expected.partition().index(),
                        minimum_worker_count: expected.minimum_worker_count(),
                        actual_worker_count,
                        minimum_dispatch_count: expected.minimum_dispatch_count(),
                        actual_dispatch_count,
                        minimum_remote_send_count: expected.minimum_remote_send_count(),
                        actual_remote_send_count,
                        minimum_remote_receive_count: expected.minimum_remote_receive_count(),
                        actual_remote_receive_count,
                    },
                );
            }
        }
        Ok(())
    }
}

fn verify_checkpoint_summary_minimum(
    expected: &WorkloadExpectedCheckpointManifestSummary,
    actual: &WorkloadCheckpointManifestSummary,
) -> Result<(), (usize, usize, usize, usize, usize, usize)> {
    if actual.component_count() >= expected.minimum_component_count()
        && actual.chunk_count() >= expected.minimum_chunk_count()
        && actual.payload_bytes() >= expected.minimum_payload_bytes()
    {
        return Ok(());
    }

    Err((
        expected.minimum_component_count(),
        actual.component_count(),
        expected.minimum_chunk_count(),
        actual.chunk_count(),
        expected.minimum_payload_bytes(),
        actual.payload_bytes(),
    ))
}

fn verify_checkpoint_component_minimum(
    expected: &WorkloadExpectedCheckpointComponentSummary,
    actual: &WorkloadCheckpointComponentSummary,
) -> Result<(), (usize, usize, usize, usize)> {
    if actual.chunk_count() >= expected.minimum_chunk_count()
        && actual.payload_bytes() >= expected.minimum_payload_bytes()
    {
        return Ok(());
    }

    Err((
        expected.minimum_chunk_count(),
        actual.chunk_count(),
        expected.minimum_payload_bytes(),
        actual.payload_bytes(),
    ))
}

fn missing_required_checkpoint_chunk<'a>(
    expected: &'a WorkloadExpectedCheckpointComponentSummary,
    actual: &WorkloadCheckpointComponentSummary,
) -> Option<&'a str> {
    expected
        .required_chunk_names()
        .iter()
        .map(String::as_str)
        .chain(
            expected
                .required_chunk_payloads()
                .iter()
                .map(|chunk| chunk.name()),
        )
        .find(|chunk| actual.chunk_summary(chunk).is_none())
}

fn checkpoint_chunk_payload_below_minimum<'a>(
    expected: &'a WorkloadExpectedCheckpointComponentSummary,
    actual: &WorkloadCheckpointComponentSummary,
) -> Option<(&'a crate::WorkloadExpectedCheckpointChunkSummary, usize)> {
    expected.required_chunk_payloads().iter().find_map(|chunk| {
        let actual_payload_bytes = actual.chunk_summary(chunk.name())?.payload_bytes();
        (actual_payload_bytes < chunk.minimum_payload_bytes())
            .then_some((chunk, actual_payload_bytes))
    })
}

fn label_counts(labels: &[String]) -> BTreeMap<&str, usize> {
    let mut counts = BTreeMap::new();
    for label in labels {
        *counts.entry(label.as_str()).or_insert(0) += 1;
    }
    counts
}

fn planned_checkpoint_ticks_by_label(events: &[WorkloadHostEvent]) -> BTreeMap<String, Vec<Tick>> {
    let mut ticks_by_label = BTreeMap::new();
    for event in events {
        if let HostEventIntent::Checkpoint { label } = event.intent() {
            ticks_by_label
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(event.tick());
        }
    }
    ticks_by_label
}

fn planned_checkpoint_restore_ticks_by_label(
    events: &[WorkloadHostEvent],
) -> BTreeMap<String, Vec<Tick>> {
    let mut ticks_by_label = BTreeMap::new();
    for event in events {
        if let HostEventIntent::RestoreCheckpoint { label } = event.intent() {
            ticks_by_label
                .entry(label.clone())
                .or_insert_with(Vec::new)
                .push(event.tick());
        }
    }
    ticks_by_label
}
