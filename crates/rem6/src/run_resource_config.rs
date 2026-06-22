use std::path::{Path, PathBuf};

use rem6_workload::{
    WorkloadAcquiredResource, WorkloadAcquiredSuiteResource, WorkloadResourceKind,
    WorkloadResourcePayload,
};

use crate::resource_acquire_cli::{
    acquire_manifest_required_resources, acquire_suite_required_resources,
    reject_runtime_remote_uri_resources,
};
use crate::{KernelResourceSelector, Rem6CliError, Rem6ResourceAcquireConfig};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RunResourcePayloads {
    resource_config: PathBuf,
    payloads: Vec<RunResourcePayload>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct RunResourcePayload {
    workload_id: Option<String>,
    kind: WorkloadResourceKind,
    payload: WorkloadResourcePayload,
}

impl RunResourcePayloads {
    pub(crate) fn kernel_binary(
        &self,
        selector: Option<&KernelResourceSelector>,
    ) -> Result<Vec<u8>, Rem6CliError> {
        if let Some(selector) = selector {
            return self.selected_kernel_binary(selector);
        }

        let payloads = self
            .payloads
            .iter()
            .filter(|payload| payload.kind == WorkloadResourceKind::Kernel)
            .collect::<Vec<_>>();
        if payloads.len() != 1 {
            return Err(Rem6CliError::Execute {
                error: format!(
                    "run resource config {} acquired {} required kernel resources; expected exactly one",
                    self.resource_config.display(),
                    payloads.len(),
                ),
            });
        }
        Ok(payloads[0].payload.data().to_vec())
    }

    fn selected_kernel_binary(
        &self,
        selector: &KernelResourceSelector,
    ) -> Result<Vec<u8>, Rem6CliError> {
        let payload = match selector {
            KernelResourceSelector::Resource(id) => self.payload_by_id("kernel", id)?,
            KernelResourceSelector::SuiteResource(selector) => {
                self.payload_by_suite_id("kernel", selector.workload_id(), selector.resource_id())?
            }
        };
        if payload.kind != WorkloadResourceKind::Kernel {
            return Err(Rem6CliError::Execute {
                error: format!(
                    "kernel resource {} in run resource config {} has kind {}; expected kernel",
                    selector.source_name(),
                    self.resource_config.display(),
                    payload.kind.as_str(),
                ),
            });
        }
        Ok(payload.payload.data().to_vec())
    }

    pub(crate) fn readfile_payload(&self, id: &str) -> Result<&[u8], Rem6CliError> {
        self.input_payload("readfile", id)
    }

    pub(crate) fn readfile_suite_payload(
        &self,
        workload_id: &str,
        id: &str,
    ) -> Result<&[u8], Rem6CliError> {
        self.input_suite_payload("readfile", workload_id, id)
    }

    pub(crate) fn input_payload(&self, use_case: &str, id: &str) -> Result<&[u8], Rem6CliError> {
        let payload = self.payload_by_id(use_case, id)?;
        if payload.kind != WorkloadResourceKind::Input {
            return Err(Rem6CliError::Execute {
                error: format!(
                    "{use_case} resource {id} in run resource config {} has kind {}; expected input",
                    self.resource_config.display(),
                    payload.kind.as_str(),
                ),
            });
        }
        Ok(payload.payload.data())
    }

    pub(crate) fn input_suite_payload(
        &self,
        use_case: &str,
        workload_id: &str,
        id: &str,
    ) -> Result<&[u8], Rem6CliError> {
        let payload = self.payload_by_suite_id(use_case, workload_id, id)?;
        if payload.kind != WorkloadResourceKind::Input {
            return Err(Rem6CliError::Execute {
                error: format!(
                    "{use_case} suite resource {workload_id}/{id} in run resource config {} has kind {}; expected input",
                    self.resource_config.display(),
                    payload.kind.as_str(),
                ),
            });
        }
        Ok(payload.payload.data())
    }

    pub(crate) fn blob_payload(&self, id: &str) -> Result<&[u8], Rem6CliError> {
        let payload = self.payload_by_id("load blob", id)?;
        Ok(payload.payload.data())
    }

    pub(crate) fn blob_suite_payload(
        &self,
        workload_id: &str,
        id: &str,
    ) -> Result<&[u8], Rem6CliError> {
        let payload = self.payload_by_suite_id("load blob", workload_id, id)?;
        Ok(payload.payload.data())
    }

    fn payload_by_id(&self, use_case: &str, id: &str) -> Result<&RunResourcePayload, Rem6CliError> {
        let payloads = self
            .payloads
            .iter()
            .filter(|payload| payload.matches_id(id))
            .collect::<Vec<_>>();
        let payload = match payloads.as_slice() {
            [payload] => payload,
            [] => {
                return Err(Rem6CliError::Execute {
                    error: format!(
                        "{use_case} resource {id} was not acquired by run resource config {}",
                        self.resource_config.display(),
                    ),
                })
            }
            _ => {
                return Err(Rem6CliError::Execute {
                    error: format!(
                        "{use_case} resource {id} is ambiguous in run resource config {}; expected exactly one",
                        self.resource_config.display(),
                    ),
                })
            }
        };
        Ok(payload)
    }

    fn payload_by_suite_id(
        &self,
        use_case: &str,
        workload_id: &str,
        id: &str,
    ) -> Result<&RunResourcePayload, Rem6CliError> {
        let payloads = self
            .payloads
            .iter()
            .filter(|payload| payload.matches_suite_id(workload_id, id))
            .collect::<Vec<_>>();
        let payload = match payloads.as_slice() {
            [payload] => payload,
            [] => {
                return Err(Rem6CliError::Execute {
                    error: format!(
                        "{use_case} suite resource {workload_id}/{id} was not acquired by run resource config {}",
                        self.resource_config.display(),
                    ),
                })
            }
            _ => {
                return Err(Rem6CliError::Execute {
                    error: format!(
                        "{use_case} suite resource {workload_id}/{id} is ambiguous in run resource config {}; expected exactly one",
                        self.resource_config.display(),
                    ),
                })
            }
        };
        Ok(payload)
    }
}

pub(crate) fn run_resource_payloads_from_config(
    resource_config: &Path,
) -> Result<RunResourcePayloads, Rem6CliError> {
    let acquire_config = Rem6ResourceAcquireConfig::parse_args([
        "resource-acquire".to_string(),
        "--config".to_string(),
        resource_config.display().to_string(),
    ])?;
    reject_runtime_remote_uri_resources("run", resource_config, &acquire_config)?;
    if acquire_config.suite_id().is_some() {
        let (_plan, acquired) = acquire_suite_required_resources(&acquire_config)?;
        let payloads = acquired
            .into_iter()
            .map(RunResourcePayload::from_suite_acquired)
            .collect::<Vec<_>>();
        return Ok(RunResourcePayloads {
            resource_config: resource_config.to_path_buf(),
            payloads,
        });
    }
    let (_manifest, acquired) = acquire_manifest_required_resources(&acquire_config)?;
    let payloads = acquired
        .into_iter()
        .map(RunResourcePayload::from_acquired)
        .collect::<Vec<_>>();
    Ok(RunResourcePayloads {
        resource_config: resource_config.to_path_buf(),
        payloads,
    })
}

impl RunResourcePayload {
    fn from_acquired(resource: WorkloadAcquiredResource) -> Self {
        Self::new(None, resource)
    }

    fn from_suite_acquired(resource: WorkloadAcquiredSuiteResource) -> Self {
        let workload_id = resource.workload_id().as_str().to_string();
        Self::new(Some(workload_id), resource.into_acquired())
    }

    fn new(workload_id: Option<String>, resource: WorkloadAcquiredResource) -> Self {
        let kind = resource.kind();
        Self {
            workload_id,
            kind,
            payload: resource.into_payload(),
        }
    }

    fn matches_id(&self, id: &str) -> bool {
        self.payload.resource().as_str() == id
    }

    fn matches_suite_id(&self, workload_id: &str, id: &str) -> bool {
        self.workload_id.as_deref() == Some(workload_id) && self.matches_id(id)
    }
}
