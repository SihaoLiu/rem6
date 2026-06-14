use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{
    WorkloadError, WorkloadId, WorkloadManifest, WorkloadManifestIdentity, WorkloadReplayPlan,
    WorkloadResource, WorkloadResourceAcquisition, WorkloadResourceId, WorkloadResourceKind,
    WorkloadResourcePayload, WorkloadSuiteReplayPlan,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WorkloadResourceAcquisitionError {
    Workload(WorkloadError),
    DuplicateArtifact {
        locator: String,
    },
    MissingAcquisition {
        resource: WorkloadResourceId,
    },
    MissingArtifact {
        resource: WorkloadResourceId,
        locator: String,
    },
    ProvenanceMismatch {
        resource: WorkloadResourceId,
        expected: Box<WorkloadResourceAcquisition>,
        actual: Box<WorkloadResourceAcquisition>,
    },
    SizeMismatch {
        resource: WorkloadResourceId,
        expected_bytes: usize,
        actual_bytes: usize,
    },
    DigestMismatch {
        resource: WorkloadResourceId,
        expected: String,
        actual: String,
    },
}

impl fmt::Display for WorkloadResourceAcquisitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Workload(error) => write!(formatter, "{error}"),
            Self::DuplicateArtifact { locator } => {
                write!(
                    formatter,
                    "resource artifact {locator} is already available"
                )
            }
            Self::MissingAcquisition { resource } => write!(
                formatter,
                "resource {} has no acquisition declaration",
                resource.as_str()
            ),
            Self::MissingArtifact { resource, locator } => write!(
                formatter,
                "resource {} artifact {locator} is not available",
                resource.as_str()
            ),
            Self::ProvenanceMismatch {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "resource {} artifact provenance {:?} does not match {:?}",
                resource.as_str(),
                actual,
                expected
            ),
            Self::SizeMismatch {
                resource,
                expected_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "resource {} artifact has {actual_bytes} bytes, expected {expected_bytes}",
                resource.as_str()
            ),
            Self::DigestMismatch {
                resource,
                expected,
                actual,
            } => write!(
                formatter,
                "resource {} artifact digest {actual} does not match {expected}",
                resource.as_str()
            ),
        }
    }
}

impl Error for WorkloadResourceAcquisitionError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Workload(error) => Some(error),
            _ => None,
        }
    }
}

impl From<WorkloadError> for WorkloadResourceAcquisitionError {
    fn from(error: WorkloadError) -> Self {
        Self::Workload(error)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResourceArtifact {
    acquisition: WorkloadResourceAcquisition,
    digest: String,
    size_bytes: usize,
    data: Vec<u8>,
}

impl WorkloadResourceArtifact {
    pub fn new(
        acquisition: WorkloadResourceAcquisition,
        digest: impl Into<String>,
        size_bytes: usize,
        data: Vec<u8>,
    ) -> Self {
        Self {
            acquisition,
            digest: digest.into(),
            size_bytes,
            data,
        }
    }

    pub const fn acquisition(&self) -> &WorkloadResourceAcquisition {
        &self.acquisition
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub const fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadAcquiredResource {
    resource: WorkloadResourceId,
    kind: WorkloadResourceKind,
    acquisition: WorkloadResourceAcquisition,
    digest: String,
    size_bytes: usize,
    payload: WorkloadResourcePayload,
}

impl WorkloadAcquiredResource {
    fn new(
        resource: WorkloadResourceId,
        kind: WorkloadResourceKind,
        acquisition: WorkloadResourceAcquisition,
        digest: String,
        size_bytes: usize,
        data: Vec<u8>,
    ) -> Result<Self, WorkloadResourceAcquisitionError> {
        let payload = WorkloadResourcePayload::new(resource.clone(), digest.clone(), data)?;
        Ok(Self {
            resource,
            kind,
            acquisition,
            digest,
            size_bytes,
            payload,
        })
    }

    pub const fn resource(&self) -> &WorkloadResourceId {
        &self.resource
    }

    pub const fn kind(&self) -> WorkloadResourceKind {
        self.kind
    }

    pub fn acquisition(&self) -> &WorkloadResourceAcquisition {
        &self.acquisition
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub const fn size_bytes(&self) -> usize {
        self.size_bytes
    }

    pub const fn payload(&self) -> &WorkloadResourcePayload {
        &self.payload
    }

    pub fn into_payload(self) -> WorkloadResourcePayload {
        self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadAcquiredSuiteResource {
    workload_id: WorkloadId,
    manifest_identity: WorkloadManifestIdentity,
    acquired: WorkloadAcquiredResource,
}

impl WorkloadAcquiredSuiteResource {
    fn new(
        workload_id: WorkloadId,
        manifest_identity: WorkloadManifestIdentity,
        acquired: WorkloadAcquiredResource,
    ) -> Self {
        Self {
            workload_id,
            manifest_identity,
            acquired,
        }
    }

    pub const fn workload_id(&self) -> &WorkloadId {
        &self.workload_id
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub const fn acquired(&self) -> &WorkloadAcquiredResource {
        &self.acquired
    }

    pub fn into_acquired(self) -> WorkloadAcquiredResource {
        self.acquired
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct WorkloadInMemoryResourceAcquisitionExecutor {
    artifacts: BTreeMap<String, WorkloadResourceArtifact>,
}

impl WorkloadInMemoryResourceAcquisitionExecutor {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_artifact(
        mut self,
        artifact: WorkloadResourceArtifact,
    ) -> Result<Self, WorkloadResourceAcquisitionError> {
        let locator = artifact.acquisition().locator().to_string();
        if self.artifacts.insert(locator.clone(), artifact).is_some() {
            return Err(WorkloadResourceAcquisitionError::DuplicateArtifact { locator });
        }
        Ok(self)
    }

    pub fn acquire_manifest(
        &self,
        manifest: &WorkloadManifest,
    ) -> Result<Vec<WorkloadAcquiredResource>, WorkloadResourceAcquisitionError> {
        self.acquire_resources(manifest.required_resource_details()?)
    }

    pub fn acquire_replay_plan(
        &self,
        plan: &WorkloadReplayPlan,
    ) -> Result<Vec<WorkloadAcquiredResource>, WorkloadResourceAcquisitionError> {
        self.acquire_resources(plan.required_resources().iter().cloned())
    }

    pub fn acquire_suite_replay_plan(
        &self,
        plan: &WorkloadSuiteReplayPlan,
    ) -> Result<Vec<WorkloadAcquiredSuiteResource>, WorkloadResourceAcquisitionError> {
        plan.required_resources()
            .iter()
            .map(|required| {
                Ok(WorkloadAcquiredSuiteResource::new(
                    required.workload_id().clone(),
                    required.manifest_identity(),
                    self.acquire_resource(required.resource())?,
                ))
            })
            .collect()
    }

    fn acquire_resources(
        &self,
        resources: impl IntoIterator<Item = WorkloadResource>,
    ) -> Result<Vec<WorkloadAcquiredResource>, WorkloadResourceAcquisitionError> {
        resources
            .into_iter()
            .map(|resource| self.acquire_resource(&resource))
            .collect()
    }

    fn acquire_resource(
        &self,
        resource: &WorkloadResource,
    ) -> Result<WorkloadAcquiredResource, WorkloadResourceAcquisitionError> {
        let expected_acquisition = resource.acquisition().ok_or_else(|| {
            WorkloadResourceAcquisitionError::MissingAcquisition {
                resource: resource.id().clone(),
            }
        })?;
        let locator = expected_acquisition.locator();
        let artifact = self.artifacts.get(locator).ok_or_else(|| {
            WorkloadResourceAcquisitionError::MissingArtifact {
                resource: resource.id().clone(),
                locator: locator.to_string(),
            }
        })?;
        if artifact.acquisition() != expected_acquisition {
            return Err(WorkloadResourceAcquisitionError::ProvenanceMismatch {
                resource: resource.id().clone(),
                expected: Box::new(expected_acquisition.clone()),
                actual: Box::new(artifact.acquisition().clone()),
            });
        }
        if artifact.size_bytes() != artifact.data().len() {
            return Err(WorkloadResourceAcquisitionError::SizeMismatch {
                resource: resource.id().clone(),
                expected_bytes: artifact.size_bytes(),
                actual_bytes: artifact.data().len(),
            });
        }
        if resource.digest() != artifact.digest() {
            return Err(WorkloadResourceAcquisitionError::DigestMismatch {
                resource: resource.id().clone(),
                expected: resource.digest().to_string(),
                actual: artifact.digest().to_string(),
            });
        }

        WorkloadAcquiredResource::new(
            resource.id().clone(),
            resource.kind(),
            expected_acquisition.clone(),
            artifact.digest().to_string(),
            artifact.size_bytes(),
            artifact.data().to_vec(),
        )
    }
}
