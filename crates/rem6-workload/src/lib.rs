use std::collections::{BTreeMap, BTreeSet};

use rem6_boot::BootImage;
mod boot_handoff;
mod boot_image;
mod error;
mod error_support;
mod heterogeneous;
mod host_event;
mod identity;
mod manifest_diagnostics;
mod manifest_fabric_hop_activity;
mod manifest_fabric_lane_activity;
mod manifest_fabric_link_activity;
mod manifest_fabric_virtual_network_activity;
mod manifest_identity;
mod manifest_parallel_frontier;
mod manifest_progress;
mod manifest_remote_endpoints;
mod manifest_remote_traffic;
mod manifest_resource_activity;
mod parallel_batch;
mod parallel_batch_partition_expectation;
mod parallel_batch_timeline_expectation;
mod parallel_batch_worker_count_expectation;
mod parallel_diagnostic_expectation;
mod parallel_expectation;
mod parallel_progress_transition_expectation;
mod qos;
mod replay_plan;
mod replay_verify;
mod resource_payload;
mod result;
mod result_collect;
mod result_partition_activity;
mod suite;
mod topology;
mod workload_result;

pub use boot_handoff::{WorkloadLinuxBootHandoff, WorkloadLinuxInitrd};
pub use boot_image::{WorkloadBootImage, WorkloadBootSegment};
pub use error::{WorkloadError, WorkloadParallelRemoteTrafficConsistencyMismatch};
pub use heterogeneous::{
    WorkloadAcceleratorCommand, WorkloadAcceleratorCommandKind, WorkloadAcceleratorDevice,
    WorkloadAcceleratorDmaCopy, WorkloadGpuDevice, WorkloadGpuDmaCopy, WorkloadGpuKernelLaunch,
};
use host_event::host_event_sort_key;
pub use host_event::{
    CheckpointLineage, HostEventIntent, WorkloadExecutionMode, WorkloadExecutionModeSwitch,
    WorkloadGuestHostCallResponse, WorkloadHostActionSummary, WorkloadHostEvent,
    WorkloadStatsScope,
};
use identity::{manifest_identity, ManifestIdentityInput};
pub use manifest_identity::WorkloadManifestIdentity;
pub use parallel_batch::{
    WorkloadParallelBatchPartitionSet, WorkloadParallelBatchPartitionStreak,
    WorkloadParallelBatchScope, WorkloadParallelBatchTimelineRecord,
    WorkloadParallelBatchWorkerCount,
};
pub use parallel_batch_partition_expectation::{
    WorkloadExpectedParallelBatchPartitionSet, WorkloadExpectedParallelBatchPartitionStreak,
    WorkloadParallelBatchPartitionScope,
};
pub use parallel_batch_timeline_expectation::{
    WorkloadExpectedParallelBatchTimelineRecord, WorkloadParallelBatchTimelineScope,
};
pub use parallel_batch_worker_count_expectation::{
    WorkloadExpectedParallelBatchWorkerBucket, WorkloadExpectedParallelBatchWorkerTickActivity,
    WorkloadExpectedParallelBatchWorkerTickBucket, WorkloadExpectedParallelBatchWorkerTickStreak,
    WorkloadExpectedParallelBatchWorkerTicks, WorkloadParallelBatchWorkerScope,
};
pub use parallel_diagnostic_expectation::{
    WorkloadExpectedCleanParallelDiagnostics, WorkloadExpectedParallelWaitForBlockedNodeWindow,
    WorkloadExpectedParallelWaitForEdgeKindCount, WorkloadExpectedParallelWaitForEdgeKindWindow,
    WorkloadExpectedParallelWaitForTargetNodeWindow, WorkloadParallelDiagnosticScope,
};
pub use parallel_expectation::{
    WorkloadExpectedDataCacheProtocolRunCount, WorkloadExpectedDataCacheRunAttribution,
    WorkloadExpectedFabricHopActivity, WorkloadExpectedFabricLaneActivity,
    WorkloadExpectedFabricLinkActivity, WorkloadExpectedFabricVirtualNetworkActivity,
    WorkloadExpectedParallelBatchActivity, WorkloadExpectedParallelFrontier,
    WorkloadExpectedParallelPartitionActivity, WorkloadExpectedParallelPartitionUse,
    WorkloadExpectedParallelRemoteDelayCeiling, WorkloadExpectedParallelRemoteDelayFloor,
    WorkloadExpectedParallelRemoteEndpoints, WorkloadExpectedParallelRemoteFlow,
    WorkloadExpectedParallelRemoteFlowTiming, WorkloadExpectedParallelRemoteSend,
    WorkloadExpectedParallelRemoteTrafficConsistency, WorkloadExpectedParallelSchedulerIdleBound,
    WorkloadExpectedParallelSchedulerProgress, WorkloadExpectedParallelWorkerActivity,
    WorkloadExpectedParallelWorkerUse, WorkloadExpectedResourceActivity,
    WorkloadParallelFrontierStage, WorkloadParallelRemoteFlowScope, WorkloadParallelSchedulerScope,
    WorkloadResourceActivityScope,
};
pub use parallel_progress_transition_expectation::{
    WorkloadExpectedParallelProgressTransition, WorkloadParallelProgressTransitionExpectationError,
    WorkloadParallelProgressTransitionExpectationFailure,
};
pub use qos::{
    WorkloadQosPolicy, WorkloadQosQueuePolicyKind, WorkloadQosRequestorPriority,
    WorkloadQosTurnaroundPolicyKind,
};
pub use replay_plan::WorkloadReplayPlan;
pub use resource_payload::{WorkloadResolvedResources, WorkloadResourcePayload};
pub use result::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadParallelExecutionSummary,
    WorkloadWaitForBlockedNodeWindow, WorkloadWaitForEdgeKindWindow,
    WorkloadWaitForTargetNodeWindow,
};
pub use suite::{
    WorkloadSuite, WorkloadSuiteBuilder, WorkloadSuiteDispatchLoadExpectation,
    WorkloadSuiteDispatchLoadSummary, WorkloadSuiteDispatchOccupancyWindow,
    WorkloadSuiteDispatchPlan, WorkloadSuiteDispatchRecord, WorkloadSuiteDispatchTimeline,
    WorkloadSuiteDispatchTimelineEntry, WorkloadSuiteDispatchWeight, WorkloadSuiteEntry,
    WorkloadSuiteExecutionEfficiency, WorkloadSuiteExecutionExpectation,
    WorkloadSuiteExecutionOccupancyWindow, WorkloadSuiteExecutionRatio,
    WorkloadSuiteExecutionRecord, WorkloadSuiteExecutionSummary, WorkloadSuiteId,
    WorkloadSuiteIdentity, WorkloadSuiteReplayEntry, WorkloadSuiteReplayPlan, WorkloadSuiteResult,
    WorkloadSuiteResultEntry, WorkloadSuiteWorkerDispatchLoad, WorkloadSuiteWorkerExecutionSummary,
};
pub use topology::{
    WorkloadHostPlacement, WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadRiscvCore,
    WorkloadRiscvDataCache, WorkloadRouteFabric, WorkloadRouteHop, WorkloadRouteId,
    WorkloadRouteLatency, WorkloadTopology,
};
pub use workload_result::{
    WorkloadCheckpointManifestSummary, WorkloadExpectedCheckpointManifestSummary, WorkloadResult,
};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadId(String);

impl WorkloadId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkloadError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkloadError::EmptyWorkloadId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadResourceId(String);

impl WorkloadResourceId {
    pub fn new(value: impl Into<String>) -> Result<Self, WorkloadError> {
        let value = value.into();
        if value.is_empty() {
            return Err(WorkloadError::EmptyResourceId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum WorkloadResourceKind {
    Kernel,
    DiskImage,
    Firmware,
    DeviceTree,
    Input,
    Output,
    Initrd,
}

impl WorkloadResourceKind {
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Kernel => "kernel",
            Self::DiskImage => "disk-image",
            Self::Firmware => "firmware",
            Self::DeviceTree => "device-tree",
            Self::Input => "input",
            Self::Output => "output",
            Self::Initrd => "initrd",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResource {
    id: WorkloadResourceId,
    kind: WorkloadResourceKind,
    digest: String,
    locator: String,
}

impl WorkloadResource {
    pub fn new(
        id: WorkloadResourceId,
        kind: WorkloadResourceKind,
        digest: impl Into<String>,
        locator: impl Into<String>,
    ) -> Result<Self, WorkloadError> {
        let digest = digest.into();
        if digest.is_empty() {
            return Err(WorkloadError::EmptyResourceDigest {
                resource: id.clone(),
            });
        }

        let locator = locator.into();
        if locator.is_empty() {
            return Err(WorkloadError::EmptyResourceLocator {
                resource: id.clone(),
            });
        }

        Ok(Self {
            id,
            kind,
            digest,
            locator,
        })
    }

    pub fn id(&self) -> &WorkloadResourceId {
        &self.id
    }

    pub const fn kind(&self) -> WorkloadResourceKind {
        self.kind
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn locator(&self) -> &str {
        &self.locator
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadManifest {
    id: WorkloadId,
    boot: WorkloadBootImage,
    linux_boot_handoff: Option<WorkloadLinuxBootHandoff>,
    topology: Option<WorkloadTopology>,
    resources: Vec<WorkloadResource>,
    required_resources: Vec<WorkloadResourceId>,
    host_events: Vec<WorkloadHostEvent>,
    expected_clean_parallel_diagnostics: Vec<WorkloadExpectedCleanParallelDiagnostics>,
    expected_parallel_wait_for_edge_kind_counts: Vec<WorkloadExpectedParallelWaitForEdgeKindCount>,
    expected_parallel_wait_for_edge_kind_windows:
        Vec<WorkloadExpectedParallelWaitForEdgeKindWindow>,
    expected_parallel_wait_for_blocked_node_windows:
        Vec<WorkloadExpectedParallelWaitForBlockedNodeWindow>,
    expected_parallel_wait_for_target_node_windows:
        Vec<WorkloadExpectedParallelWaitForTargetNodeWindow>,
    expected_data_cache_protocol_run_counts: Vec<WorkloadExpectedDataCacheProtocolRunCount>,
    expected_data_cache_run_attribution: Option<WorkloadExpectedDataCacheRunAttribution>,
    expected_parallel_remote_flows: Vec<WorkloadExpectedParallelRemoteFlow>,
    expected_parallel_remote_endpoints: Vec<WorkloadExpectedParallelRemoteEndpoints>,
    expected_parallel_remote_delay_floors: Vec<WorkloadExpectedParallelRemoteDelayFloor>,
    expected_parallel_remote_delay_ceilings: Vec<WorkloadExpectedParallelRemoteDelayCeiling>,
    expected_parallel_remote_traffic_consistency:
        Vec<WorkloadExpectedParallelRemoteTrafficConsistency>,
    expected_parallel_remote_sends: Vec<WorkloadExpectedParallelRemoteSend>,
    expected_parallel_remote_flow_timings: Vec<WorkloadExpectedParallelRemoteFlowTiming>,
    expected_parallel_progress_transitions: Vec<WorkloadExpectedParallelProgressTransition>,
    expected_checkpoint_manifest_summaries: Vec<WorkloadExpectedCheckpointManifestSummary>,
    expected_checkpoint_restore_manifest_summaries: Vec<WorkloadExpectedCheckpointManifestSummary>,
    expected_parallel_worker_use: Vec<WorkloadExpectedParallelWorkerUse>,
    expected_parallel_worker_activity: Vec<WorkloadExpectedParallelWorkerActivity>,
    expected_parallel_scheduler_progress: Vec<WorkloadExpectedParallelSchedulerProgress>,
    expected_parallel_scheduler_idle_bounds: Vec<WorkloadExpectedParallelSchedulerIdleBound>,
    expected_parallel_batch_activity: Vec<WorkloadExpectedParallelBatchActivity>,
    expected_parallel_batch_worker_buckets: Vec<WorkloadExpectedParallelBatchWorkerBucket>,
    expected_parallel_batch_worker_tick_buckets: Vec<WorkloadExpectedParallelBatchWorkerTickBucket>,
    expected_parallel_batch_worker_tick_activity:
        Vec<WorkloadExpectedParallelBatchWorkerTickActivity>,
    expected_parallel_batch_worker_tick_streaks: Vec<WorkloadExpectedParallelBatchWorkerTickStreak>,
    expected_parallel_batch_worker_ticks: Vec<WorkloadExpectedParallelBatchWorkerTicks>,
    expected_parallel_batch_partition_sets: Vec<WorkloadExpectedParallelBatchPartitionSet>,
    expected_parallel_batch_partition_streaks: Vec<WorkloadExpectedParallelBatchPartitionStreak>,
    expected_parallel_batch_timeline_records: Vec<WorkloadExpectedParallelBatchTimelineRecord>,
    expected_parallel_partition_use: Vec<WorkloadExpectedParallelPartitionUse>,
    expected_parallel_partition_activity: Vec<WorkloadExpectedParallelPartitionActivity>,
    expected_parallel_frontiers: Vec<WorkloadExpectedParallelFrontier>,
    expected_resource_activity: Vec<WorkloadExpectedResourceActivity>,
    expected_fabric_hop_activity: Vec<WorkloadExpectedFabricHopActivity>,
    expected_fabric_lane_activity: Vec<WorkloadExpectedFabricLaneActivity>,
    expected_fabric_link_activity: Vec<WorkloadExpectedFabricLinkActivity>,
    expected_fabric_virtual_network_activity: Vec<WorkloadExpectedFabricVirtualNetworkActivity>,
    checkpoint_lineage: Option<CheckpointLineage>,
    identity: WorkloadManifestIdentity,
}

impl WorkloadManifest {
    pub fn builder(id: WorkloadId, boot: BootImage) -> WorkloadManifestBuilder {
        WorkloadManifestBuilder::new(id, WorkloadBootImage::from_boot_image(&boot))
    }

    pub fn id(&self) -> &WorkloadId {
        &self.id
    }

    pub const fn boot(&self) -> &WorkloadBootImage {
        &self.boot
    }

    pub fn linux_boot_handoff(&self) -> Option<&WorkloadLinuxBootHandoff> {
        self.linux_boot_handoff.as_ref()
    }

    pub fn topology(&self) -> Option<&WorkloadTopology> {
        self.topology.as_ref()
    }

    pub fn resources(&self) -> &[WorkloadResource] {
        &self.resources
    }

    pub fn resource(&self, id: &WorkloadResourceId) -> Option<&WorkloadResource> {
        self.resources.iter().find(|resource| resource.id() == id)
    }

    pub fn required_resources(&self) -> &[WorkloadResourceId] {
        &self.required_resources
    }

    pub fn required_resource_details(&self) -> Result<Vec<WorkloadResource>, WorkloadError> {
        self.required_resources
            .iter()
            .map(|id| {
                self.resource(id)
                    .cloned()
                    .ok_or_else(|| WorkloadError::MissingRequiredResource {
                        resource: id.clone(),
                    })
            })
            .collect()
    }

    pub fn host_events(&self) -> &[WorkloadHostEvent] {
        &self.host_events
    }

    pub fn expected_data_cache_protocol_run_counts(
        &self,
    ) -> &[WorkloadExpectedDataCacheProtocolRunCount] {
        &self.expected_data_cache_protocol_run_counts
    }

    pub fn expected_data_cache_run_attribution(
        &self,
    ) -> Option<&WorkloadExpectedDataCacheRunAttribution> {
        self.expected_data_cache_run_attribution.as_ref()
    }

    pub fn expected_checkpoint_manifest_summaries(
        &self,
    ) -> &[WorkloadExpectedCheckpointManifestSummary] {
        &self.expected_checkpoint_manifest_summaries
    }

    pub fn expected_checkpoint_restore_manifest_summaries(
        &self,
    ) -> &[WorkloadExpectedCheckpointManifestSummary] {
        &self.expected_checkpoint_restore_manifest_summaries
    }

    pub fn expected_parallel_worker_use(&self) -> &[WorkloadExpectedParallelWorkerUse] {
        &self.expected_parallel_worker_use
    }

    pub fn expected_parallel_worker_activity(&self) -> &[WorkloadExpectedParallelWorkerActivity] {
        &self.expected_parallel_worker_activity
    }

    pub fn expected_parallel_scheduler_progress(
        &self,
    ) -> &[WorkloadExpectedParallelSchedulerProgress] {
        &self.expected_parallel_scheduler_progress
    }

    pub fn expected_parallel_scheduler_idle_bounds(
        &self,
    ) -> &[WorkloadExpectedParallelSchedulerIdleBound] {
        &self.expected_parallel_scheduler_idle_bounds
    }

    pub fn expected_parallel_batch_activity(&self) -> &[WorkloadExpectedParallelBatchActivity] {
        &self.expected_parallel_batch_activity
    }

    pub fn expected_parallel_batch_partition_sets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchPartitionSet] {
        &self.expected_parallel_batch_partition_sets
    }

    pub fn expected_parallel_batch_partition_streaks(
        &self,
    ) -> &[WorkloadExpectedParallelBatchPartitionStreak] {
        &self.expected_parallel_batch_partition_streaks
    }

    pub fn expected_parallel_batch_timeline_records(
        &self,
    ) -> &[WorkloadExpectedParallelBatchTimelineRecord] {
        &self.expected_parallel_batch_timeline_records
    }

    pub fn expected_parallel_partition_use(&self) -> &[WorkloadExpectedParallelPartitionUse] {
        &self.expected_parallel_partition_use
    }

    pub fn expected_parallel_partition_activity(
        &self,
    ) -> &[WorkloadExpectedParallelPartitionActivity] {
        &self.expected_parallel_partition_activity
    }

    pub fn checkpoint_lineage(&self) -> Option<&CheckpointLineage> {
        self.checkpoint_lineage.as_ref()
    }

    pub fn identity(&self) -> WorkloadManifestIdentity {
        self.identity.clone()
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        self.boot.to_boot_image()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadManifestBuilder {
    id: WorkloadId,
    boot: WorkloadBootImage,
    linux_boot_handoff: Option<WorkloadLinuxBootHandoff>,
    topology: Option<WorkloadTopology>,
    resources: BTreeMap<WorkloadResourceId, WorkloadResource>,
    required_resources: BTreeSet<WorkloadResourceId>,
    host_events: Vec<WorkloadHostEvent>,
    expected_clean_parallel_diagnostics: Vec<WorkloadExpectedCleanParallelDiagnostics>,
    expected_parallel_wait_for_edge_kind_counts: Vec<WorkloadExpectedParallelWaitForEdgeKindCount>,
    expected_parallel_wait_for_edge_kind_windows:
        Vec<WorkloadExpectedParallelWaitForEdgeKindWindow>,
    expected_parallel_wait_for_blocked_node_windows:
        Vec<WorkloadExpectedParallelWaitForBlockedNodeWindow>,
    expected_parallel_wait_for_target_node_windows:
        Vec<WorkloadExpectedParallelWaitForTargetNodeWindow>,
    expected_data_cache_protocol_run_counts: Vec<WorkloadExpectedDataCacheProtocolRunCount>,
    expected_data_cache_run_attribution: Option<WorkloadExpectedDataCacheRunAttribution>,
    expected_parallel_remote_flows: Vec<WorkloadExpectedParallelRemoteFlow>,
    expected_parallel_remote_endpoints: Vec<WorkloadExpectedParallelRemoteEndpoints>,
    expected_parallel_remote_delay_floors: Vec<WorkloadExpectedParallelRemoteDelayFloor>,
    expected_parallel_remote_delay_ceilings: Vec<WorkloadExpectedParallelRemoteDelayCeiling>,
    expected_parallel_remote_traffic_consistency:
        Vec<WorkloadExpectedParallelRemoteTrafficConsistency>,
    expected_parallel_remote_sends: Vec<WorkloadExpectedParallelRemoteSend>,
    expected_parallel_remote_flow_timings: Vec<WorkloadExpectedParallelRemoteFlowTiming>,
    expected_parallel_progress_transitions: Vec<WorkloadExpectedParallelProgressTransition>,
    expected_checkpoint_manifest_summaries: Vec<WorkloadExpectedCheckpointManifestSummary>,
    expected_checkpoint_restore_manifest_summaries: Vec<WorkloadExpectedCheckpointManifestSummary>,
    expected_parallel_worker_use: Vec<WorkloadExpectedParallelWorkerUse>,
    expected_parallel_worker_activity: Vec<WorkloadExpectedParallelWorkerActivity>,
    expected_parallel_scheduler_progress: Vec<WorkloadExpectedParallelSchedulerProgress>,
    expected_parallel_scheduler_idle_bounds: Vec<WorkloadExpectedParallelSchedulerIdleBound>,
    expected_parallel_batch_activity: Vec<WorkloadExpectedParallelBatchActivity>,
    expected_parallel_batch_worker_buckets: Vec<WorkloadExpectedParallelBatchWorkerBucket>,
    expected_parallel_batch_worker_tick_buckets: Vec<WorkloadExpectedParallelBatchWorkerTickBucket>,
    expected_parallel_batch_worker_tick_activity:
        Vec<WorkloadExpectedParallelBatchWorkerTickActivity>,
    expected_parallel_batch_worker_tick_streaks: Vec<WorkloadExpectedParallelBatchWorkerTickStreak>,
    expected_parallel_batch_worker_ticks: Vec<WorkloadExpectedParallelBatchWorkerTicks>,
    expected_parallel_batch_partition_sets: Vec<WorkloadExpectedParallelBatchPartitionSet>,
    expected_parallel_batch_partition_streaks: Vec<WorkloadExpectedParallelBatchPartitionStreak>,
    expected_parallel_batch_timeline_records: Vec<WorkloadExpectedParallelBatchTimelineRecord>,
    expected_parallel_partition_use: Vec<WorkloadExpectedParallelPartitionUse>,
    expected_parallel_partition_activity: Vec<WorkloadExpectedParallelPartitionActivity>,
    expected_parallel_frontiers: Vec<WorkloadExpectedParallelFrontier>,
    expected_resource_activity: Vec<WorkloadExpectedResourceActivity>,
    expected_fabric_hop_activity: Vec<WorkloadExpectedFabricHopActivity>,
    expected_fabric_lane_activity: Vec<WorkloadExpectedFabricLaneActivity>,
    expected_fabric_link_activity: Vec<WorkloadExpectedFabricLinkActivity>,
    expected_fabric_virtual_network_activity: Vec<WorkloadExpectedFabricVirtualNetworkActivity>,
    checkpoint_lineage: Option<CheckpointLineage>,
}

impl WorkloadManifestBuilder {
    fn new(id: WorkloadId, boot: WorkloadBootImage) -> Self {
        Self {
            id,
            boot,
            linux_boot_handoff: None,
            topology: None,
            resources: BTreeMap::new(),
            required_resources: BTreeSet::new(),
            host_events: Vec::new(),
            expected_clean_parallel_diagnostics: Vec::new(),
            expected_parallel_wait_for_edge_kind_counts: Vec::new(),
            expected_parallel_wait_for_edge_kind_windows: Vec::new(),
            expected_parallel_wait_for_blocked_node_windows: Vec::new(),
            expected_parallel_wait_for_target_node_windows: Vec::new(),
            expected_data_cache_protocol_run_counts: Vec::new(),
            expected_data_cache_run_attribution: None,
            expected_parallel_remote_flows: Vec::new(),
            expected_parallel_remote_endpoints: Vec::new(),
            expected_parallel_remote_delay_floors: Vec::new(),
            expected_parallel_remote_delay_ceilings: Vec::new(),
            expected_parallel_remote_traffic_consistency: Vec::new(),
            expected_parallel_remote_sends: Vec::new(),
            expected_parallel_remote_flow_timings: Vec::new(),
            expected_parallel_progress_transitions: Vec::new(),
            expected_checkpoint_manifest_summaries: Vec::new(),
            expected_checkpoint_restore_manifest_summaries: Vec::new(),
            expected_parallel_worker_use: Vec::new(),
            expected_parallel_worker_activity: Vec::new(),
            expected_parallel_scheduler_progress: Vec::new(),
            expected_parallel_scheduler_idle_bounds: Vec::new(),
            expected_parallel_batch_activity: Vec::new(),
            expected_parallel_batch_worker_buckets: Vec::new(),
            expected_parallel_batch_worker_tick_buckets: Vec::new(),
            expected_parallel_batch_worker_tick_activity: Vec::new(),
            expected_parallel_batch_worker_tick_streaks: Vec::new(),
            expected_parallel_batch_worker_ticks: Vec::new(),
            expected_parallel_batch_partition_sets: Vec::new(),
            expected_parallel_batch_partition_streaks: Vec::new(),
            expected_parallel_batch_timeline_records: Vec::new(),
            expected_parallel_partition_use: Vec::new(),
            expected_parallel_partition_activity: Vec::new(),
            expected_parallel_frontiers: Vec::new(),
            expected_resource_activity: Vec::new(),
            expected_fabric_hop_activity: Vec::new(),
            expected_fabric_lane_activity: Vec::new(),
            expected_fabric_link_activity: Vec::new(),
            expected_fabric_virtual_network_activity: Vec::new(),
            checkpoint_lineage: None,
        }
    }

    pub fn add_resource(mut self, resource: WorkloadResource) -> Result<Self, WorkloadError> {
        let id = resource.id().clone();
        if self.resources.contains_key(&id) {
            return Err(WorkloadError::DuplicateResource { resource: id });
        }
        self.resources.insert(id, resource);
        Ok(self)
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

    pub fn add_expected_checkpoint_manifest_summary(
        mut self,
        expected: WorkloadExpectedCheckpointManifestSummary,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_checkpoint_manifest_summaries
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedCheckpointManifestSummary {
                label: expected.label().to_string(),
            });
        }
        self.expected_checkpoint_manifest_summaries.push(expected);
        self.expected_checkpoint_manifest_summaries
            .sort_by(|left, right| left.sort_key().cmp(right.sort_key()));
        Ok(self)
    }

    pub fn add_expected_checkpoint_restore_manifest_summary(
        mut self,
        expected: WorkloadExpectedCheckpointManifestSummary,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_checkpoint_restore_manifest_summaries
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(
                WorkloadError::DuplicateExpectedCheckpointRestoreManifestSummary {
                    label: expected.label().to_string(),
                },
            );
        }
        self.expected_checkpoint_restore_manifest_summaries
            .push(expected);
        self.expected_checkpoint_restore_manifest_summaries
            .sort_by(|left, right| left.sort_key().cmp(right.sort_key()));
        Ok(self)
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

    pub fn add_required_resource(mut self, resource: WorkloadResourceId) -> Self {
        self.required_resources.insert(resource);
        self
    }

    pub fn add_host_event(mut self, event: WorkloadHostEvent) -> Self {
        self.host_events.push(event);
        self
    }

    pub fn with_topology(mut self, topology: WorkloadTopology) -> Self {
        self.topology = Some(topology);
        self
    }

    pub fn with_linux_boot_handoff(mut self, handoff: WorkloadLinuxBootHandoff) -> Self {
        self.linux_boot_handoff = Some(handoff);
        self
    }

    pub fn with_checkpoint_lineage(mut self, lineage: CheckpointLineage) -> Self {
        self.checkpoint_lineage = Some(lineage);
        self
    }

    pub fn build(mut self) -> Result<WorkloadManifest, WorkloadError> {
        if let Some(resource) = self
            .linux_boot_handoff
            .as_ref()
            .and_then(WorkloadLinuxBootHandoff::device_tree_resource)
        {
            self.required_resources.insert(resource.clone());
        }
        if let Some(initrd) = self
            .linux_boot_handoff
            .as_ref()
            .and_then(WorkloadLinuxBootHandoff::initrd)
        {
            self.required_resources.insert(initrd.resource().clone());
        }

        for resource in &self.required_resources {
            if !self.resources.contains_key(resource) {
                return Err(WorkloadError::MissingRequiredResource {
                    resource: resource.clone(),
                });
            }
        }
        if let Some(initrd) = self
            .linux_boot_handoff
            .as_ref()
            .and_then(WorkloadLinuxBootHandoff::initrd)
        {
            let resource = self
                .resources
                .get(initrd.resource())
                .expect("required resource was checked above");
            if resource.kind() != WorkloadResourceKind::Initrd {
                return Err(WorkloadError::ResourceKindMismatch {
                    resource: resource.id().clone(),
                    expected: WorkloadResourceKind::Initrd,
                    actual: resource.kind(),
                });
            }
        }
        if let Some(resource_id) = self
            .linux_boot_handoff
            .as_ref()
            .and_then(WorkloadLinuxBootHandoff::device_tree_resource)
        {
            let resource = self
                .resources
                .get(resource_id)
                .expect("required resource was checked above");
            if resource.kind() != WorkloadResourceKind::DeviceTree {
                return Err(WorkloadError::ResourceKindMismatch {
                    resource: resource.id().clone(),
                    expected: WorkloadResourceKind::DeviceTree,
                    actual: resource.kind(),
                });
            }
        }

        self.host_events.sort_by_key(host_event_sort_key);
        let resources = self.resources.into_values().collect::<Vec<_>>();
        let required_resources = self.required_resources.into_iter().collect::<Vec<_>>();
        let identity = manifest_identity(ManifestIdentityInput {
            id: &self.id,
            boot: &self.boot,
            linux_boot_handoff: self.linux_boot_handoff.as_ref(),
            topology: self.topology.as_ref(),
            resources: &resources,
            required_resources: &required_resources,
            host_events: &self.host_events,
            expected_clean_parallel_diagnostics: &self.expected_clean_parallel_diagnostics,
            expected_parallel_wait_for_edge_kind_counts: &self
                .expected_parallel_wait_for_edge_kind_counts,
            expected_parallel_wait_for_edge_kind_windows: &self
                .expected_parallel_wait_for_edge_kind_windows,
            expected_parallel_wait_for_blocked_node_windows: &self
                .expected_parallel_wait_for_blocked_node_windows,
            expected_parallel_wait_for_target_node_windows: &self
                .expected_parallel_wait_for_target_node_windows,
            expected_data_cache_protocol_run_counts: &self.expected_data_cache_protocol_run_counts,
            expected_data_cache_run_attribution: self.expected_data_cache_run_attribution.as_ref(),
            expected_parallel_remote_flows: &self.expected_parallel_remote_flows,
            expected_parallel_remote_endpoints: &self.expected_parallel_remote_endpoints,
            expected_parallel_remote_delay_floors: &self.expected_parallel_remote_delay_floors,
            expected_parallel_remote_delay_ceilings: &self.expected_parallel_remote_delay_ceilings,
            expected_parallel_remote_traffic_consistency: &self
                .expected_parallel_remote_traffic_consistency,
            expected_parallel_remote_sends: &self.expected_parallel_remote_sends,
            expected_parallel_remote_flow_timings: &self.expected_parallel_remote_flow_timings,
            expected_parallel_progress_transitions: &self.expected_parallel_progress_transitions,
            expected_checkpoint_manifest_summaries: &self.expected_checkpoint_manifest_summaries,
            expected_checkpoint_restore_manifest_summaries: &self
                .expected_checkpoint_restore_manifest_summaries,
            expected_parallel_worker_use: &self.expected_parallel_worker_use,
            expected_parallel_worker_activity: &self.expected_parallel_worker_activity,
            expected_parallel_scheduler_progress: &self.expected_parallel_scheduler_progress,
            expected_parallel_scheduler_idle_bounds: &self.expected_parallel_scheduler_idle_bounds,
            expected_parallel_batch_activity: &self.expected_parallel_batch_activity,
            expected_parallel_batch_worker_buckets: &self.expected_parallel_batch_worker_buckets,
            expected_parallel_batch_worker_tick_buckets: &self
                .expected_parallel_batch_worker_tick_buckets,
            expected_parallel_batch_worker_tick_activity: &self
                .expected_parallel_batch_worker_tick_activity,
            expected_parallel_batch_worker_tick_streaks: &self
                .expected_parallel_batch_worker_tick_streaks,
            expected_parallel_batch_worker_ticks: &self.expected_parallel_batch_worker_ticks,
            expected_parallel_batch_partition_sets: &self.expected_parallel_batch_partition_sets,
            expected_parallel_batch_partition_streaks: &self
                .expected_parallel_batch_partition_streaks,
            expected_parallel_batch_timeline_records: &self
                .expected_parallel_batch_timeline_records,
            expected_parallel_partition_use: &self.expected_parallel_partition_use,
            expected_parallel_partition_activity: &self.expected_parallel_partition_activity,
            expected_parallel_frontiers: &self.expected_parallel_frontiers,
            expected_resource_activity: &self.expected_resource_activity,
            expected_fabric_hop_activity: &self.expected_fabric_hop_activity,
            expected_fabric_lane_activity: &self.expected_fabric_lane_activity,
            expected_fabric_link_activity: &self.expected_fabric_link_activity,
            expected_fabric_virtual_network_activity: &self
                .expected_fabric_virtual_network_activity,
            checkpoint_lineage: self.checkpoint_lineage.as_ref(),
        });

        Ok(WorkloadManifest {
            id: self.id,
            boot: self.boot,
            linux_boot_handoff: self.linux_boot_handoff,
            topology: self.topology,
            resources,
            required_resources,
            host_events: self.host_events,
            expected_clean_parallel_diagnostics: self.expected_clean_parallel_diagnostics,
            expected_parallel_wait_for_edge_kind_counts: self
                .expected_parallel_wait_for_edge_kind_counts,
            expected_parallel_wait_for_edge_kind_windows: self
                .expected_parallel_wait_for_edge_kind_windows,
            expected_parallel_wait_for_blocked_node_windows: self
                .expected_parallel_wait_for_blocked_node_windows,
            expected_parallel_wait_for_target_node_windows: self
                .expected_parallel_wait_for_target_node_windows,
            expected_data_cache_protocol_run_counts: self.expected_data_cache_protocol_run_counts,
            expected_data_cache_run_attribution: self.expected_data_cache_run_attribution,
            expected_parallel_remote_flows: self.expected_parallel_remote_flows,
            expected_parallel_remote_endpoints: self.expected_parallel_remote_endpoints,
            expected_parallel_remote_delay_floors: self.expected_parallel_remote_delay_floors,
            expected_parallel_remote_delay_ceilings: self.expected_parallel_remote_delay_ceilings,
            expected_parallel_remote_traffic_consistency: self
                .expected_parallel_remote_traffic_consistency,
            expected_parallel_remote_sends: self.expected_parallel_remote_sends,
            expected_parallel_remote_flow_timings: self.expected_parallel_remote_flow_timings,
            expected_parallel_progress_transitions: self.expected_parallel_progress_transitions,
            expected_checkpoint_manifest_summaries: self.expected_checkpoint_manifest_summaries,
            expected_checkpoint_restore_manifest_summaries: self
                .expected_checkpoint_restore_manifest_summaries,
            expected_parallel_worker_use: self.expected_parallel_worker_use,
            expected_parallel_worker_activity: self.expected_parallel_worker_activity,
            expected_parallel_scheduler_progress: self.expected_parallel_scheduler_progress,
            expected_parallel_scheduler_idle_bounds: self.expected_parallel_scheduler_idle_bounds,
            expected_parallel_batch_activity: self.expected_parallel_batch_activity,
            expected_parallel_batch_worker_buckets: self.expected_parallel_batch_worker_buckets,
            expected_parallel_batch_worker_tick_buckets: self
                .expected_parallel_batch_worker_tick_buckets,
            expected_parallel_batch_worker_tick_activity: self
                .expected_parallel_batch_worker_tick_activity,
            expected_parallel_batch_worker_tick_streaks: self
                .expected_parallel_batch_worker_tick_streaks,
            expected_parallel_batch_worker_ticks: self.expected_parallel_batch_worker_ticks,
            expected_parallel_batch_partition_sets: self.expected_parallel_batch_partition_sets,
            expected_parallel_batch_partition_streaks: self
                .expected_parallel_batch_partition_streaks,
            expected_parallel_batch_timeline_records: self.expected_parallel_batch_timeline_records,
            expected_parallel_partition_use: self.expected_parallel_partition_use,
            expected_parallel_partition_activity: self.expected_parallel_partition_activity,
            expected_parallel_frontiers: self.expected_parallel_frontiers,
            expected_resource_activity: self.expected_resource_activity,
            expected_fabric_hop_activity: self.expected_fabric_hop_activity,
            expected_fabric_lane_activity: self.expected_fabric_lane_activity,
            expected_fabric_link_activity: self.expected_fabric_link_activity,
            expected_fabric_virtual_network_activity: self.expected_fabric_virtual_network_activity,
            checkpoint_lineage: self.checkpoint_lineage,
            identity,
        })
    }
}
