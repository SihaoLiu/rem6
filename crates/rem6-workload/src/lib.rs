use std::collections::{BTreeMap, BTreeSet};

use rem6_boot::BootImage;
use rem6_kernel::Tick;
use rem6_memory::{Address, AddressRange};
use rem6_stats::StatSnapshot;

mod boot_handoff;
mod error;
mod heterogeneous;
mod host_event;
mod identity;
mod parallel_expectation;
mod qos;
mod resource_payload;
mod result;
mod topology;

pub use boot_handoff::{WorkloadLinuxBootHandoff, WorkloadLinuxInitrd};
pub use error::WorkloadError;
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
    WorkloadHostActionSummary, WorkloadHostEvent, WorkloadStatsScope,
};
use identity::{manifest_identity, ManifestIdentityInput};
pub use parallel_expectation::{
    WorkloadExpectedCleanParallelDiagnostics, WorkloadExpectedParallelBatchActivity,
    WorkloadExpectedParallelBatchPartitionSet, WorkloadExpectedParallelPartitionActivity,
    WorkloadExpectedParallelPartitionUse, WorkloadExpectedParallelRemoteFlow,
    WorkloadExpectedParallelRemoteFlowTiming, WorkloadExpectedParallelWorkerActivity,
    WorkloadExpectedParallelWorkerUse, WorkloadParallelDiagnosticScope,
    WorkloadParallelRemoteFlowScope,
};
pub use qos::{
    WorkloadQosPolicy, WorkloadQosQueuePolicyKind, WorkloadQosRequestorPriority,
    WorkloadQosTurnaroundPolicyKind,
};
pub use resource_payload::{WorkloadResolvedResources, WorkloadResourcePayload};
pub use result::{
    WorkloadDataCacheProtocol, WorkloadDataCacheProtocolCount, WorkloadDramQosPrioritySummary,
    WorkloadDramQosRequestorSummary, WorkloadParallelBatchPartitionSet,
    WorkloadParallelBatchWorkerCount, WorkloadParallelExecutionSummary,
};
pub use topology::{
    WorkloadHostPlacement, WorkloadMemoryRoute, WorkloadMemoryTarget, WorkloadRiscvCore,
    WorkloadRiscvDataCache, WorkloadRouteFabric, WorkloadRouteHop, WorkloadRouteId,
    WorkloadRouteLatency, WorkloadTopology,
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
pub struct WorkloadBootImage {
    entry: Address,
    segments: Vec<WorkloadBootSegment>,
}

impl WorkloadBootImage {
    pub fn from_boot_image(image: &BootImage) -> Self {
        Self {
            entry: image.entry(),
            segments: image
                .segments()
                .iter()
                .map(|segment| WorkloadBootSegment::new(segment.range(), segment.data().to_vec()))
                .collect(),
        }
    }

    pub const fn entry(&self) -> Address {
        self.entry
    }

    pub fn segments(&self) -> &[WorkloadBootSegment] {
        &self.segments
    }

    pub fn to_boot_image(&self) -> Result<BootImage, WorkloadError> {
        let mut image = BootImage::new(self.entry);
        for segment in &self.segments {
            image = image
                .add_segment(segment.range().start(), segment.data().to_vec())
                .map_err(WorkloadError::Boot)?;
        }
        Ok(image)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadBootSegment {
    range: AddressRange,
    data: Vec<u8>,
}

impl WorkloadBootSegment {
    pub const fn new(range: AddressRange, data: Vec<u8>) -> Self {
        Self { range, data }
    }

    pub const fn range(&self) -> AddressRange {
        self.range
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct WorkloadManifestIdentity(String);

impl WorkloadManifestIdentity {
    fn new(hash: u64) -> Self {
        Self(format!("wl-{hash:016x}"))
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
    expected_parallel_remote_flows: Vec<WorkloadExpectedParallelRemoteFlow>,
    expected_parallel_remote_flow_timings: Vec<WorkloadExpectedParallelRemoteFlowTiming>,
    expected_parallel_worker_use: Vec<WorkloadExpectedParallelWorkerUse>,
    expected_parallel_worker_activity: Vec<WorkloadExpectedParallelWorkerActivity>,
    expected_parallel_batch_activity: Vec<WorkloadExpectedParallelBatchActivity>,
    expected_parallel_batch_partition_sets: Vec<WorkloadExpectedParallelBatchPartitionSet>,
    expected_parallel_partition_use: Vec<WorkloadExpectedParallelPartitionUse>,
    expected_parallel_partition_activity: Vec<WorkloadExpectedParallelPartitionActivity>,
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

    pub fn expected_parallel_remote_flows(&self) -> &[WorkloadExpectedParallelRemoteFlow] {
        &self.expected_parallel_remote_flows
    }

    pub fn expected_parallel_remote_flow_timings(
        &self,
    ) -> &[WorkloadExpectedParallelRemoteFlowTiming] {
        &self.expected_parallel_remote_flow_timings
    }

    pub fn expected_parallel_worker_use(&self) -> &[WorkloadExpectedParallelWorkerUse] {
        &self.expected_parallel_worker_use
    }

    pub fn expected_parallel_worker_activity(&self) -> &[WorkloadExpectedParallelWorkerActivity] {
        &self.expected_parallel_worker_activity
    }

    pub fn expected_parallel_batch_activity(&self) -> &[WorkloadExpectedParallelBatchActivity] {
        &self.expected_parallel_batch_activity
    }

    pub fn expected_parallel_batch_partition_sets(
        &self,
    ) -> &[WorkloadExpectedParallelBatchPartitionSet] {
        &self.expected_parallel_batch_partition_sets
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
    expected_parallel_remote_flows: Vec<WorkloadExpectedParallelRemoteFlow>,
    expected_parallel_remote_flow_timings: Vec<WorkloadExpectedParallelRemoteFlowTiming>,
    expected_parallel_worker_use: Vec<WorkloadExpectedParallelWorkerUse>,
    expected_parallel_worker_activity: Vec<WorkloadExpectedParallelWorkerActivity>,
    expected_parallel_batch_activity: Vec<WorkloadExpectedParallelBatchActivity>,
    expected_parallel_batch_partition_sets: Vec<WorkloadExpectedParallelBatchPartitionSet>,
    expected_parallel_partition_use: Vec<WorkloadExpectedParallelPartitionUse>,
    expected_parallel_partition_activity: Vec<WorkloadExpectedParallelPartitionActivity>,
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
            expected_parallel_remote_flows: Vec::new(),
            expected_parallel_remote_flow_timings: Vec::new(),
            expected_parallel_worker_use: Vec::new(),
            expected_parallel_worker_activity: Vec::new(),
            expected_parallel_batch_activity: Vec::new(),
            expected_parallel_batch_partition_sets: Vec::new(),
            expected_parallel_partition_use: Vec::new(),
            expected_parallel_partition_activity: Vec::new(),
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

    pub fn add_expected_parallel_remote_flow_timing(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlowTiming,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flow_timings
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlowTiming {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flow_timings.push(expected);
        self.expected_parallel_remote_flow_timings
            .sort_by_key(|timing| timing.sort_key());
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

    pub fn add_expected_parallel_remote_flow(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlow {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flows.push(expected);
        self.expected_parallel_remote_flows
            .sort_by_key(|flow| flow.sort_key());
        Ok(self)
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
            expected_parallel_remote_flows: &self.expected_parallel_remote_flows,
            expected_parallel_remote_flow_timings: &self.expected_parallel_remote_flow_timings,
            expected_parallel_worker_use: &self.expected_parallel_worker_use,
            expected_parallel_worker_activity: &self.expected_parallel_worker_activity,
            expected_parallel_batch_activity: &self.expected_parallel_batch_activity,
            expected_parallel_batch_partition_sets: &self.expected_parallel_batch_partition_sets,
            expected_parallel_partition_use: &self.expected_parallel_partition_use,
            expected_parallel_partition_activity: &self.expected_parallel_partition_activity,
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
            expected_parallel_remote_flows: self.expected_parallel_remote_flows,
            expected_parallel_remote_flow_timings: self.expected_parallel_remote_flow_timings,
            expected_parallel_worker_use: self.expected_parallel_worker_use,
            expected_parallel_worker_activity: self.expected_parallel_worker_activity,
            expected_parallel_batch_activity: self.expected_parallel_batch_activity,
            expected_parallel_batch_partition_sets: self.expected_parallel_batch_partition_sets,
            expected_parallel_partition_use: self.expected_parallel_partition_use,
            expected_parallel_partition_activity: self.expected_parallel_partition_activity,
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
    expected_parallel_remote_flows: Vec<WorkloadExpectedParallelRemoteFlow>,
    expected_parallel_remote_flow_timings: Vec<WorkloadExpectedParallelRemoteFlowTiming>,
    expected_parallel_worker_use: Vec<WorkloadExpectedParallelWorkerUse>,
    expected_parallel_worker_activity: Vec<WorkloadExpectedParallelWorkerActivity>,
    expected_parallel_batch_activity: Vec<WorkloadExpectedParallelBatchActivity>,
    expected_parallel_batch_partition_sets: Vec<WorkloadExpectedParallelBatchPartitionSet>,
    expected_parallel_partition_use: Vec<WorkloadExpectedParallelPartitionUse>,
    expected_parallel_partition_activity: Vec<WorkloadExpectedParallelPartitionActivity>,
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
            expected_parallel_remote_flows: manifest.expected_parallel_remote_flows().to_vec(),
            expected_parallel_remote_flow_timings: manifest
                .expected_parallel_remote_flow_timings()
                .to_vec(),
            expected_parallel_worker_use: manifest.expected_parallel_worker_use().to_vec(),
            expected_parallel_worker_activity: manifest
                .expected_parallel_worker_activity()
                .to_vec(),
            expected_parallel_batch_activity: manifest.expected_parallel_batch_activity().to_vec(),
            expected_parallel_batch_partition_sets: manifest
                .expected_parallel_batch_partition_sets()
                .to_vec(),
            expected_parallel_partition_use: manifest.expected_parallel_partition_use().to_vec(),
            expected_parallel_partition_activity: manifest
                .expected_parallel_partition_activity()
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

    pub fn add_expected_parallel_remote_flow(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlow,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flows
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlow {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flows.push(expected);
        self.expected_parallel_remote_flows
            .sort_by_key(|flow| flow.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_remote_flows(&self) -> &[WorkloadExpectedParallelRemoteFlow] {
        &self.expected_parallel_remote_flows
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

    pub fn add_expected_parallel_remote_flow_timing(
        mut self,
        expected: WorkloadExpectedParallelRemoteFlowTiming,
    ) -> Result<Self, WorkloadError> {
        if self
            .expected_parallel_remote_flow_timings
            .iter()
            .any(|existing| existing.sort_key() == expected.sort_key())
        {
            return Err(WorkloadError::DuplicateExpectedParallelRemoteFlowTiming {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
            });
        }
        self.expected_parallel_remote_flow_timings.push(expected);
        self.expected_parallel_remote_flow_timings
            .sort_by_key(|timing| timing.sort_key());
        Ok(self)
    }

    pub fn expected_parallel_remote_flow_timings(
        &self,
    ) -> &[WorkloadExpectedParallelRemoteFlowTiming] {
        &self.expected_parallel_remote_flow_timings
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

    pub fn checkpoint_lineage(&self) -> Option<&CheckpointLineage> {
        self.checkpoint_lineage.as_ref()
    }

    pub fn verify_result(&self, result: &WorkloadResult) -> Result<(), WorkloadError> {
        if result.manifest_identity != self.manifest_identity {
            return Err(WorkloadError::ManifestIdentityMismatch {
                expected: self.manifest_identity.clone(),
                actual: result.manifest_identity.clone(),
            });
        }

        result.verify_stats_timing()?;
        self.verify_all_planned_events_reached(result.final_tick())?;
        self.verify_checkpoint_labels(result)?;
        self.verify_checkpoint_restore_labels(result)?;
        self.verify_execution_mode_switches(result)?;
        self.verify_stop_reason(result)?;
        self.verify_expected_parallel_remote_flows(result)?;
        self.verify_expected_parallel_remote_flow_timings(result)?;
        self.verify_expected_parallel_worker_use(result)?;
        self.verify_expected_parallel_worker_activity(result)?;
        self.verify_expected_parallel_batch_activity(result)?;
        self.verify_expected_parallel_batch_partition_sets(result)?;
        self.verify_expected_parallel_partition_use(result)?;
        self.verify_expected_parallel_partition_activity(result)?;
        self.verify_expected_clean_parallel_diagnostics(result)?;
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

    fn verify_expected_parallel_remote_flows(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_remote_flows.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_parallel_remote_flows[0];
            return Err(WorkloadError::MissingParallelExecutionSummary {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
                expected_send_count: expected.send_count(),
            });
        };

        for expected in &self.expected_parallel_remote_flows {
            let actual_send_count = expected.actual_send_count(summary);
            if actual_send_count != expected.send_count() {
                return Err(WorkloadError::ExpectedParallelRemoteFlowCountMismatch {
                    scope: expected.scope(),
                    source: expected.source().index(),
                    target: expected.target().index(),
                    expected_send_count: expected.send_count(),
                    actual_send_count,
                });
            }
        }
        Ok(())
    }

    fn verify_expected_parallel_remote_flow_timings(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_remote_flow_timings.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_parallel_remote_flow_timings[0];
            return Err(WorkloadError::MissingParallelRemoteFlowTimingSummary {
                scope: expected.scope(),
                source: expected.source().index(),
                target: expected.target().index(),
                expected_send_count: expected.send_count(),
                expected_first_tick: expected.first_tick(),
                expected_last_tick: expected.last_tick(),
            });
        };

        for expected in &self.expected_parallel_remote_flow_timings {
            let actual = expected.actual_record(summary);
            let actual_send_count = actual.map(|record| record.send_count()).unwrap_or(0);
            let actual_first_tick = actual.map(|record| record.first_tick());
            let actual_last_tick = actual.map(|record| record.last_tick());
            if actual_send_count != expected.send_count()
                || actual_first_tick != Some(expected.first_tick())
                || actual_last_tick != Some(expected.last_tick())
            {
                return Err(WorkloadError::ExpectedParallelRemoteFlowTimingMismatch {
                    scope: expected.scope(),
                    source: expected.source().index(),
                    target: expected.target().index(),
                    expected_send_count: expected.send_count(),
                    actual_send_count,
                    expected_first_tick: expected.first_tick(),
                    actual_first_tick,
                    expected_last_tick: expected.last_tick(),
                    actual_last_tick,
                });
            }
        }
        Ok(())
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

    fn verify_expected_parallel_batch_activity(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_batch_activity.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_parallel_batch_activity[0];
            return Err(WorkloadError::MissingParallelBatchActivitySummary {
                scope: expected.scope(),
                minimum_worker_count: expected.minimum_worker_count(),
                minimum_batch_count: expected.minimum_batch_count(),
            });
        };

        for expected in &self.expected_parallel_batch_activity {
            let actual_batch_count = expected.actual_batch_count(summary);
            if actual_batch_count < expected.minimum_batch_count() {
                return Err(WorkloadError::ExpectedParallelBatchActivityBelowMinimum {
                    scope: expected.scope(),
                    minimum_worker_count: expected.minimum_worker_count(),
                    minimum_batch_count: expected.minimum_batch_count(),
                    actual_batch_count,
                });
            }
        }
        Ok(())
    }

    fn verify_expected_parallel_batch_partition_sets(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_parallel_batch_partition_sets.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = &self.expected_parallel_batch_partition_sets[0];
            return Err(WorkloadError::MissingParallelBatchPartitionSetSummary {
                scope: expected.scope(),
                partitions: expected.partition_indexes(),
                minimum_batch_count: expected.minimum_batch_count(),
            });
        };

        for expected in &self.expected_parallel_batch_partition_sets {
            let actual_batch_count = expected.actual_batch_count(summary);
            if actual_batch_count < expected.minimum_batch_count() {
                return Err(
                    WorkloadError::ExpectedParallelBatchPartitionSetBelowMinimum {
                        scope: expected.scope(),
                        partitions: expected.partition_indexes(),
                        minimum_batch_count: expected.minimum_batch_count(),
                        actual_batch_count,
                    },
                );
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

    fn verify_expected_clean_parallel_diagnostics(
        &self,
        result: &WorkloadResult,
    ) -> Result<(), WorkloadError> {
        if self.expected_clean_parallel_diagnostics.is_empty() {
            return Ok(());
        }
        let Some(summary) = result.parallel_execution_summary() else {
            let expected = self.expected_clean_parallel_diagnostics[0];
            return Err(WorkloadError::MissingParallelDiagnosticSummary {
                scope: expected.scope(),
            });
        };

        for expected in &self.expected_clean_parallel_diagnostics {
            let (wait_for_edge_count, deadlock_diagnostic_count) = expected.actual_counts(summary);
            if wait_for_edge_count != 0 || deadlock_diagnostic_count != 0 {
                return Err(WorkloadError::ExpectedCleanParallelDiagnosticsViolation {
                    scope: expected.scope(),
                    wait_for_edge_count,
                    deadlock_diagnostic_count,
                });
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResult {
    manifest_identity: WorkloadManifestIdentity,
    final_tick: Tick,
    stop_reason: Option<String>,
    stats_snapshot: Option<StatSnapshot>,
    parallel_execution_summary: Option<WorkloadParallelExecutionSummary>,
    host_action_summary: Option<WorkloadHostActionSummary>,
    checkpoint_labels: Vec<String>,
    restored_checkpoint_labels: Vec<String>,
    execution_mode_switches: Vec<WorkloadExecutionModeSwitch>,
}

impl WorkloadResult {
    pub const fn new(manifest_identity: WorkloadManifestIdentity, final_tick: Tick) -> Self {
        Self {
            manifest_identity,
            final_tick,
            stop_reason: None,
            stats_snapshot: None,
            parallel_execution_summary: None,
            host_action_summary: None,
            checkpoint_labels: Vec::new(),
            restored_checkpoint_labels: Vec::new(),
            execution_mode_switches: Vec::new(),
        }
    }

    pub fn with_stop_reason(mut self, reason: impl Into<String>) -> Self {
        self.stop_reason = Some(reason.into());
        self
    }

    pub fn with_stats_snapshot(mut self, snapshot: StatSnapshot) -> Self {
        self.stats_snapshot = Some(snapshot);
        self
    }

    pub fn with_parallel_execution_summary(
        mut self,
        summary: WorkloadParallelExecutionSummary,
    ) -> Self {
        self.parallel_execution_summary = Some(summary);
        self
    }

    pub fn with_host_action_summary(mut self, summary: WorkloadHostActionSummary) -> Self {
        self.host_action_summary = Some(summary);
        self
    }

    pub fn with_checkpoint_label(mut self, label: impl Into<String>) -> Self {
        self.checkpoint_labels.push(label.into());
        self
    }

    pub fn with_restored_checkpoint_label(mut self, label: impl Into<String>) -> Self {
        self.restored_checkpoint_labels.push(label.into());
        self
    }

    pub fn with_execution_mode_switch(
        mut self,
        tick: Tick,
        target: impl Into<String>,
        mode: WorkloadExecutionMode,
    ) -> Self {
        self.execution_mode_switches
            .push(WorkloadExecutionModeSwitch::new(tick, target, mode));
        self
    }

    pub fn with_execution_mode_switch_stats_scope(
        mut self,
        tick: Tick,
        target: impl Into<String>,
        mode: WorkloadExecutionMode,
        stats_epoch: u64,
        stats_reset_tick: Tick,
    ) -> Self {
        self.execution_mode_switches.push(
            WorkloadExecutionModeSwitch::new(tick, target, mode)
                .with_stats_scope(stats_epoch, stats_reset_tick),
        );
        self
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn final_tick(&self) -> Tick {
        self.final_tick
    }

    pub fn stop_reason(&self) -> Option<&str> {
        self.stop_reason.as_deref()
    }

    pub const fn stats_snapshot(&self) -> Option<&StatSnapshot> {
        self.stats_snapshot.as_ref()
    }

    pub const fn parallel_execution_summary(&self) -> Option<&WorkloadParallelExecutionSummary> {
        self.parallel_execution_summary.as_ref()
    }

    pub const fn host_action_summary(&self) -> Option<&WorkloadHostActionSummary> {
        self.host_action_summary.as_ref()
    }

    pub fn checkpoint_labels(&self) -> &[String] {
        &self.checkpoint_labels
    }

    pub fn restored_checkpoint_labels(&self) -> &[String] {
        &self.restored_checkpoint_labels
    }

    pub fn execution_mode_switches(&self) -> &[WorkloadExecutionModeSwitch] {
        &self.execution_mode_switches
    }

    pub fn verify_manifest(&self, manifest: &WorkloadManifest) -> Result<(), WorkloadError> {
        let expected = manifest.identity();
        if self.manifest_identity != expected {
            return Err(WorkloadError::ManifestIdentityMismatch {
                expected,
                actual: self.manifest_identity.clone(),
            });
        }

        self.verify_stats_timing()
    }

    fn verify_stats_timing(&self) -> Result<(), WorkloadError> {
        let Some(snapshot) = &self.stats_snapshot else {
            return Ok(());
        };

        if snapshot.tick() <= self.final_tick {
            return Ok(());
        }

        Err(WorkloadError::StatsAfterFinalTick {
            stats_tick: snapshot.tick(),
            final_tick: self.final_tick,
        })
    }
}
