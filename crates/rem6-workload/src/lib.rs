use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use rem6_boot::BootImage;
use rem6_kernel::Tick;
use rem6_memory::{Address, AddressRange};
use rem6_stats::StatSnapshot;

const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostEventIntent {
    RoiBegin { label: String },
    RoiEnd { label: String },
    StatsReset { label: String },
    StatsDump { label: String },
    Checkpoint { label: String },
    Stop { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadHostEvent {
    tick: Tick,
    intent: HostEventIntent,
}

impl WorkloadHostEvent {
    pub const fn new(tick: Tick, intent: HostEventIntent) -> Self {
        Self { tick, intent }
    }

    pub const fn tick(&self) -> Tick {
        self.tick
    }

    pub const fn intent(&self) -> &HostEventIntent {
        &self.intent
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckpointLineage {
    CreatedByWorkload {
        label: String,
    },
    RestoredFrom {
        label: String,
        manifest_identity: String,
    },
}

impl CheckpointLineage {
    pub fn label(&self) -> &str {
        match self {
            Self::CreatedByWorkload { label } | Self::RestoredFrom { label, .. } => label,
        }
    }

    pub fn manifest_identity(&self) -> Option<&str> {
        match self {
            Self::CreatedByWorkload { .. } => None,
            Self::RestoredFrom {
                manifest_identity, ..
            } => Some(manifest_identity),
        }
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
    resources: Vec<WorkloadResource>,
    required_resources: Vec<WorkloadResourceId>,
    host_events: Vec<WorkloadHostEvent>,
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

    pub fn resources(&self) -> &[WorkloadResource] {
        &self.resources
    }

    pub fn required_resources(&self) -> &[WorkloadResourceId] {
        &self.required_resources
    }

    pub fn host_events(&self) -> &[WorkloadHostEvent] {
        &self.host_events
    }

    pub fn checkpoint_lineage(&self) -> Option<&CheckpointLineage> {
        self.checkpoint_lineage.as_ref()
    }

    pub fn identity(&self) -> WorkloadManifestIdentity {
        self.identity.clone()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadManifestBuilder {
    id: WorkloadId,
    boot: WorkloadBootImage,
    resources: BTreeMap<WorkloadResourceId, WorkloadResource>,
    required_resources: BTreeSet<WorkloadResourceId>,
    host_events: Vec<WorkloadHostEvent>,
    checkpoint_lineage: Option<CheckpointLineage>,
}

impl WorkloadManifestBuilder {
    fn new(id: WorkloadId, boot: WorkloadBootImage) -> Self {
        Self {
            id,
            boot,
            resources: BTreeMap::new(),
            required_resources: BTreeSet::new(),
            host_events: Vec::new(),
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

    pub fn add_required_resource(mut self, resource: WorkloadResourceId) -> Self {
        self.required_resources.insert(resource);
        self
    }

    pub fn add_host_event(mut self, event: WorkloadHostEvent) -> Self {
        self.host_events.push(event);
        self
    }

    pub fn with_checkpoint_lineage(mut self, lineage: CheckpointLineage) -> Self {
        self.checkpoint_lineage = Some(lineage);
        self
    }

    pub fn build(mut self) -> Result<WorkloadManifest, WorkloadError> {
        for resource in &self.required_resources {
            if !self.resources.contains_key(resource) {
                return Err(WorkloadError::MissingRequiredResource {
                    resource: resource.clone(),
                });
            }
        }

        self.host_events.sort_by_key(host_event_sort_key);
        let resources = self.resources.into_values().collect::<Vec<_>>();
        let required_resources = self.required_resources.into_iter().collect::<Vec<_>>();
        let identity = manifest_identity(
            &self.id,
            &self.boot,
            &resources,
            &required_resources,
            &self.host_events,
            self.checkpoint_lineage.as_ref(),
        );

        Ok(WorkloadManifest {
            id: self.id,
            boot: self.boot,
            resources,
            required_resources,
            host_events: self.host_events,
            checkpoint_lineage: self.checkpoint_lineage,
            identity,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResult {
    manifest_identity: WorkloadManifestIdentity,
    final_tick: Tick,
    stop_reason: Option<String>,
    stats_snapshot: Option<StatSnapshot>,
    checkpoint_labels: Vec<String>,
}

impl WorkloadResult {
    pub const fn new(manifest_identity: WorkloadManifestIdentity, final_tick: Tick) -> Self {
        Self {
            manifest_identity,
            final_tick,
            stop_reason: None,
            stats_snapshot: None,
            checkpoint_labels: Vec::new(),
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

    pub fn with_checkpoint_label(mut self, label: impl Into<String>) -> Self {
        self.checkpoint_labels.push(label.into());
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

    pub fn checkpoint_labels(&self) -> &[String] {
        &self.checkpoint_labels
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadError {
    EmptyWorkloadId,
    EmptyResourceId,
    EmptyResourceDigest { resource: WorkloadResourceId },
    EmptyResourceLocator { resource: WorkloadResourceId },
    DuplicateResource { resource: WorkloadResourceId },
    MissingRequiredResource { resource: WorkloadResourceId },
}

impl fmt::Display for WorkloadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyWorkloadId => write!(formatter, "workload id must not be empty"),
            Self::EmptyResourceId => write!(formatter, "resource id must not be empty"),
            Self::EmptyResourceDigest { resource } => write!(
                formatter,
                "resource {} must include a digest",
                resource.as_str()
            ),
            Self::EmptyResourceLocator { resource } => write!(
                formatter,
                "resource {} must include a locator",
                resource.as_str()
            ),
            Self::DuplicateResource { resource } => {
                write!(
                    formatter,
                    "resource {} is already defined",
                    resource.as_str()
                )
            }
            Self::MissingRequiredResource { resource } => write!(
                formatter,
                "required resource {} is not defined",
                resource.as_str()
            ),
        }
    }
}

impl Error for WorkloadError {}

fn host_event_sort_key(event: &WorkloadHostEvent) -> (Tick, u8, String) {
    let (rank, label) = match event.intent() {
        HostEventIntent::RoiBegin { label } => (0, label.as_str()),
        HostEventIntent::RoiEnd { label } => (1, label.as_str()),
        HostEventIntent::StatsReset { label } => (2, label.as_str()),
        HostEventIntent::StatsDump { label } => (3, label.as_str()),
        HostEventIntent::Checkpoint { label } => (4, label.as_str()),
        HostEventIntent::Stop { reason } => (5, reason.as_str()),
    };
    (event.tick(), rank, label.to_string())
}

fn manifest_identity(
    id: &WorkloadId,
    boot: &WorkloadBootImage,
    resources: &[WorkloadResource],
    required_resources: &[WorkloadResourceId],
    host_events: &[WorkloadHostEvent],
    checkpoint_lineage: Option<&CheckpointLineage>,
) -> WorkloadManifestIdentity {
    let mut hash = FNV_OFFSET;
    hash_str(&mut hash, "rem6.workload.manifest.v1");
    hash_str(&mut hash, id.as_str());
    hash_u64(&mut hash, boot.entry().get());
    hash_u64(&mut hash, boot.segments().len() as u64);
    for segment in boot.segments() {
        hash_u64(&mut hash, segment.range().start().get());
        hash_u64(&mut hash, segment.range().size().bytes());
        hash_bytes(&mut hash, segment.data());
    }
    hash_u64(&mut hash, resources.len() as u64);
    for resource in resources {
        hash_str(&mut hash, resource.id().as_str());
        hash_u64(&mut hash, resource.kind() as u64);
        hash_str(&mut hash, resource.digest());
        hash_str(&mut hash, resource.locator());
    }
    hash_u64(&mut hash, required_resources.len() as u64);
    for resource in required_resources {
        hash_str(&mut hash, resource.as_str());
    }
    hash_u64(&mut hash, host_events.len() as u64);
    for event in host_events {
        hash_u64(&mut hash, event.tick());
        hash_host_event(&mut hash, event.intent());
    }
    hash_checkpoint_lineage(&mut hash, checkpoint_lineage);
    WorkloadManifestIdentity::new(hash)
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
        HostEventIntent::Checkpoint { label } => {
            hash_str(hash, "checkpoint");
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

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV_PRIME);
    }
}
