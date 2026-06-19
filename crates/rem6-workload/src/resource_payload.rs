use std::collections::{BTreeMap, BTreeSet};

use crate::{
    WorkloadError, WorkloadLinuxBootHandoff, WorkloadManifest, WorkloadManifestIdentity,
    WorkloadResourceId, WorkloadResourceKind,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResourcePayload {
    resource: WorkloadResourceId,
    digest: String,
    data: Vec<u8>,
}

impl WorkloadResourcePayload {
    pub fn new(
        resource: WorkloadResourceId,
        digest: impl Into<String>,
        data: Vec<u8>,
    ) -> Result<Self, WorkloadError> {
        let digest = digest.into();
        if digest.is_empty() {
            return Err(WorkloadError::EmptyResourceDigest {
                resource: resource.clone(),
            });
        }
        Ok(Self {
            resource,
            digest,
            data,
        })
    }

    pub const fn resource(&self) -> &WorkloadResourceId {
        &self.resource
    }

    pub fn digest(&self) -> &str {
        &self.digest
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkloadResolvedResources {
    manifest_identity: WorkloadManifestIdentity,
    payloads: BTreeMap<WorkloadResourceId, WorkloadResourcePayload>,
}

impl WorkloadResolvedResources {
    pub fn from_manifest(
        manifest: &WorkloadManifest,
        payloads: impl IntoIterator<Item = WorkloadResourcePayload>,
    ) -> Result<Self, WorkloadError> {
        let required = manifest
            .required_resources()
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut resolved = BTreeMap::new();
        for payload in payloads {
            let resource = payload.resource().clone();
            if !required.contains(&resource) {
                return Err(WorkloadError::UnexpectedResourcePayload { resource });
            }
            if resolved.insert(resource.clone(), payload).is_some() {
                return Err(WorkloadError::DuplicateResourcePayload { resource });
            }
        }

        for resource in manifest.required_resource_details()? {
            let payload = resolved.get(resource.id()).ok_or_else(|| {
                WorkloadError::MissingResourcePayload {
                    resource: resource.id().clone(),
                }
            })?;
            if payload.digest() != resource.digest() {
                return Err(WorkloadError::ResourcePayloadDigestMismatch {
                    resource: resource.id().clone(),
                    expected: resource.digest().to_string(),
                    actual: payload.digest().to_string(),
                });
            }
        }
        validate_linux_device_tree_payload(manifest, &resolved)?;
        validate_linux_initrd_payload(manifest, &resolved)?;
        validate_linux_debug_console_input_payload(manifest, &resolved)?;

        Ok(Self {
            manifest_identity: manifest.identity(),
            payloads: resolved,
        })
    }

    pub fn manifest_identity(&self) -> WorkloadManifestIdentity {
        self.manifest_identity.clone()
    }

    pub fn payload(&self, resource: &WorkloadResourceId) -> Option<&WorkloadResourcePayload> {
        self.payloads.get(resource)
    }

    pub fn payload_data(&self, resource: &WorkloadResourceId) -> Option<&[u8]> {
        self.payload(resource).map(WorkloadResourcePayload::data)
    }

    pub fn linux_initrd_data(&self, handoff: &WorkloadLinuxBootHandoff) -> Option<&[u8]> {
        let initrd = handoff.initrd()?;
        self.payload_data(initrd.resource())
    }

    pub fn linux_device_tree_data(&self, handoff: &WorkloadLinuxBootHandoff) -> Option<&[u8]> {
        let resource = handoff.device_tree_resource()?;
        self.payload_data(resource)
    }

    pub fn linux_debug_console_input_data(
        &self,
        handoff: &WorkloadLinuxBootHandoff,
    ) -> Option<&[u8]> {
        let resource = handoff.debug_console_input_resource()?;
        self.payload_data(resource)
    }
}

fn validate_linux_initrd_payload(
    manifest: &WorkloadManifest,
    resolved: &BTreeMap<WorkloadResourceId, WorkloadResourcePayload>,
) -> Result<(), WorkloadError> {
    let Some(initrd) = manifest
        .linux_boot_handoff()
        .and_then(WorkloadLinuxBootHandoff::initrd)
    else {
        return Ok(());
    };
    let resource = manifest.resource(initrd.resource()).ok_or_else(|| {
        WorkloadError::MissingRequiredResource {
            resource: initrd.resource().clone(),
        }
    })?;
    if resource.kind() != WorkloadResourceKind::Initrd {
        return Err(WorkloadError::ResourceKindMismatch {
            resource: resource.id().clone(),
            expected: WorkloadResourceKind::Initrd,
            actual: resource.kind(),
        });
    }
    let payload =
        resolved
            .get(initrd.resource())
            .ok_or_else(|| WorkloadError::MissingResourcePayload {
                resource: initrd.resource().clone(),
            })?;
    let expected_bytes = initrd.size().bytes() as usize;
    if payload.data().len() != expected_bytes {
        return Err(WorkloadError::ResourcePayloadSizeMismatch {
            resource: initrd.resource().clone(),
            expected_bytes,
            actual_bytes: payload.data().len(),
        });
    }
    Ok(())
}

fn validate_linux_device_tree_payload(
    manifest: &WorkloadManifest,
    resolved: &BTreeMap<WorkloadResourceId, WorkloadResourcePayload>,
) -> Result<(), WorkloadError> {
    let Some(resource_id) = manifest
        .linux_boot_handoff()
        .and_then(WorkloadLinuxBootHandoff::device_tree_resource)
    else {
        return Ok(());
    };
    let resource =
        manifest
            .resource(resource_id)
            .ok_or_else(|| WorkloadError::MissingRequiredResource {
                resource: resource_id.clone(),
            })?;
    if resource.kind() != WorkloadResourceKind::DeviceTree {
        return Err(WorkloadError::ResourceKindMismatch {
            resource: resource.id().clone(),
            expected: WorkloadResourceKind::DeviceTree,
            actual: resource.kind(),
        });
    }
    if !resolved.contains_key(resource_id) {
        return Err(WorkloadError::MissingResourcePayload {
            resource: resource_id.clone(),
        });
    }
    Ok(())
}

fn validate_linux_debug_console_input_payload(
    manifest: &WorkloadManifest,
    resolved: &BTreeMap<WorkloadResourceId, WorkloadResourcePayload>,
) -> Result<(), WorkloadError> {
    let Some(resource_id) = manifest
        .linux_boot_handoff()
        .and_then(WorkloadLinuxBootHandoff::debug_console_input_resource)
    else {
        return Ok(());
    };
    let resource =
        manifest
            .resource(resource_id)
            .ok_or_else(|| WorkloadError::MissingRequiredResource {
                resource: resource_id.clone(),
            })?;
    if resource.kind() != WorkloadResourceKind::Input {
        return Err(WorkloadError::ResourceKindMismatch {
            resource: resource.id().clone(),
            expected: WorkloadResourceKind::Input,
            actual: resource.kind(),
        });
    }
    if !resolved.contains_key(resource_id) {
        return Err(WorkloadError::MissingResourcePayload {
            resource: resource_id.clone(),
        });
    }
    Ok(())
}
