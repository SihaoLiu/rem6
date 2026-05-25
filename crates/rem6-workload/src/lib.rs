use std::collections::{BTreeMap, BTreeSet};

use rem6_boot::BootImage;
use rem6_kernel::Tick;

mod boot_handoff;
mod boot_image;
mod error;
mod error_support;
mod heterogeneous;
mod host_event;
mod identity;
mod manifest_identity;
mod manifest_parallel_frontier;
mod manifest_progress;
mod manifest_remote_endpoints;
mod manifest_remote_traffic;
mod parallel_batch;
mod parallel_batch_timeline_expectation;
mod parallel_batch_worker_count_expectation;
mod parallel_expectation;
mod qos;
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
use host_event::{
    execution_mode_switch_matches, host_event_sort_key, planned_checkpoint_labels,
    planned_checkpoint_restore_labels, planned_execution_mode_switches, planned_stop_reason,
};
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
pub use parallel_batch_timeline_expectation::WorkloadExpectedParallelBatchTimelineRecord;
pub use parallel_batch_worker_count_expectation::{
    WorkloadExpectedParallelBatchWorkerBucket, WorkloadExpectedParallelBatchWorkerTickActivity,
    WorkloadExpectedParallelBatchWorkerTickBucket, WorkloadExpectedParallelBatchWorkerTickStreak,
    WorkloadExpectedParallelBatchWorkerTicks,
};
pub use parallel_expectation::{
    WorkloadExpectedCleanParallelDiagnostics, WorkloadExpectedDataCacheProtocolRunCount,
    WorkloadExpectedDataCacheRunAttribution, WorkloadExpectedParallelBatchActivity,
    WorkloadExpectedParallelBatchPartitionSet, WorkloadExpectedParallelBatchPartitionStreak,
    WorkloadExpectedParallelFrontier, WorkloadExpectedParallelPartitionActivity,
    WorkloadExpectedParallelPartitionUse, WorkloadExpectedParallelProgressTransition,
    WorkloadExpectedParallelRemoteDelayCeiling, WorkloadExpectedParallelRemoteDelayFloor,
    WorkloadExpectedParallelRemoteEndpoints, WorkloadExpectedParallelRemoteFlow,
    WorkloadExpectedParallelRemoteFlowTiming, WorkloadExpectedParallelRemoteSend,
    WorkloadExpectedParallelRemoteTrafficConsistency, WorkloadExpectedParallelSchedulerIdleBound,
    WorkloadExpectedParallelSchedulerProgress, WorkloadExpectedParallelWorkerActivity,
    WorkloadExpectedParallelWorkerUse, WorkloadExpectedResourceActivity,
    WorkloadParallelDiagnosticScope, WorkloadParallelFrontierStage,
    WorkloadParallelProgressTransitionExpectationError,
    WorkloadParallelProgressTransitionExpectationFailure, WorkloadParallelRemoteFlowScope,
    WorkloadResourceActivityScope,
};
pub use qos::{
    WorkloadQosPolicy, WorkloadQosQueuePolicyKind, WorkloadQosRequestorPriority,
    WorkloadQosTurnaroundPolicyKind,
};
pub use resource_payload::{WorkloadResolvedResources, WorkloadResourcePayload};
pub use result::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadParallelExecutionSummary,
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
pub use workload_result::WorkloadResult;

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

    pub fn expected_clean_parallel_diagnostics(
        &self,
    ) -> &[WorkloadExpectedCleanParallelDiagnostics] {
        &self.expected_clean_parallel_diagnostics
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

    pub fn expected_resource_activity(&self) -> &[WorkloadExpectedResourceActivity] {
        &self.expected_resource_activity
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

    pub fn add_expected_clean_parallel_diagnostics(
        mut self,
        expected: WorkloadExpectedCleanParallelDiagnostics,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_clean_parallel_diagnostics
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedCleanParallelDiagnostics {
                scope: expected.scope(),
            });
        }
        self.expected_clean_parallel_diagnostics.push(expected);
        self.expected_clean_parallel_diagnostics
            .sort_by_key(|diagnostics| diagnostics.sort_key());
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

    pub fn add_expected_resource_activity(
        mut self,
        expected: WorkloadExpectedResourceActivity,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_resource_activity
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedResourceActivity {
                scope: expected.scope(),
            });
        }
        self.expected_resource_activity.push(expected);
        self.expected_resource_activity
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
            checkpoint_lineage: self.checkpoint_lineage,
            identity,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadReplayPlan {
    manifest_identity: WorkloadManifestIdentity,
    boot: WorkloadBootImage,
    linux_boot_handoff: Option<WorkloadLinuxBootHandoff>,
    topology: Option<WorkloadTopology>,
    required_resources: Vec<WorkloadResource>,
    host_events: Vec<WorkloadHostEvent>,
    planned_checkpoint_labels: Vec<String>,
    planned_checkpoint_restore_labels: Vec<String>,
    planned_execution_mode_switches: Vec<WorkloadExecutionModeSwitch>,
    planned_stop_reason: Option<String>,
    expected_clean_parallel_diagnostics: Vec<WorkloadExpectedCleanParallelDiagnostics>,
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
    checkpoint_lineage: Option<CheckpointLineage>,
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
            expected_data_cache_protocol_run_counts: manifest
                .expected_data_cache_protocol_run_counts()
                .to_vec(),
            expected_data_cache_run_attribution: manifest
                .expected_data_cache_run_attribution()
                .copied(),
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

    pub fn add_expected_clean_parallel_diagnostics(
        mut self,
        expected: WorkloadExpectedCleanParallelDiagnostics,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_clean_parallel_diagnostics
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedCleanParallelDiagnostics {
                scope: expected.scope(),
            });
        }
        self.expected_clean_parallel_diagnostics.push(expected);
        self.expected_clean_parallel_diagnostics
            .sort_by_key(|diagnostics| diagnostics.sort_key());
        Ok(self)
    }

    pub fn expected_clean_parallel_diagnostics(
        &self,
    ) -> &[WorkloadExpectedCleanParallelDiagnostics] {
        &self.expected_clean_parallel_diagnostics
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

    pub fn add_expected_resource_activity(
        mut self,
        expected: WorkloadExpectedResourceActivity,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_resource_activity
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedResourceActivity {
                scope: expected.scope(),
            });
        }
        self.expected_resource_activity.push(expected);
        self.expected_resource_activity
            .sort_by_key(|activity| activity.sort_key());
        Ok(self)
    }

    pub fn expected_resource_activity(&self) -> &[WorkloadExpectedResourceActivity] {
        &self.expected_resource_activity
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
        self.verify_execution_mode_switches(result)?;
        self.verify_stop_reason(result)?;
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
        replay_verify::verify_expected_parallel_batch_partition_sets(self, result)?;
        replay_verify::verify_expected_parallel_batch_partition_streaks(self, result)?;
        replay_verify::verify_expected_parallel_batch_timeline_records(self, result)?;
        self.verify_expected_parallel_partition_use(result)?;
        self.verify_expected_parallel_partition_activity(result)?;
        replay_verify::verify_expected_parallel_frontiers(self, result)?;
        replay_verify::verify_expected_resource_activity(self, result)?;
        replay_verify::verify_expected_clean_parallel_diagnostics(self, result)?;
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
        for label in result.checkpoint_labels() {
            if !self
                .planned_checkpoint_labels
                .iter()
                .any(|planned| planned == label)
            {
                return Err(WorkloadError::UnexpectedCheckpointLabel {
                    label: label.clone(),
                });
            }
        }

        for label in &self.planned_checkpoint_labels {
            if !result
                .checkpoint_labels()
                .iter()
                .any(|actual| actual == label)
            {
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
        for label in result.restored_checkpoint_labels() {
            if !self
                .planned_checkpoint_restore_labels
                .iter()
                .any(|planned| planned == label)
            {
                return Err(WorkloadError::UnexpectedCheckpointRestoreLabel {
                    label: label.clone(),
                });
            }
        }

        for label in &self.planned_checkpoint_restore_labels {
            if !result
                .restored_checkpoint_labels()
                .iter()
                .any(|actual| actual == label)
            {
                return Err(WorkloadError::MissingCheckpointRestoreLabel {
                    label: label.clone(),
                });
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
