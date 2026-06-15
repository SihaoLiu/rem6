use std::path::Path;

use rem6_workload::{WorkloadResourceKind, WorkloadResourcePayload};

use crate::resource_acquire_cli::acquire_manifest_required_resources;
use crate::{Rem6CliError, Rem6ResourceAcquireConfig};

pub(crate) fn run_kernel_binary_from_resource_config(
    resource_config: &Path,
) -> Result<Vec<u8>, Rem6CliError> {
    let acquire_config = Rem6ResourceAcquireConfig::parse_args([
        "resource-acquire".to_string(),
        "--config".to_string(),
        resource_config.display().to_string(),
    ])?;
    let (_manifest, acquired) = acquire_manifest_required_resources(&acquire_config)?;
    let payloads = acquired
        .into_iter()
        .filter(|resource| resource.kind() == WorkloadResourceKind::Kernel)
        .map(|resource| resource.into_payload())
        .collect::<Vec<_>>();
    unique_run_kernel_payload(payloads, resource_config)
}

fn unique_run_kernel_payload(
    mut payloads: Vec<WorkloadResourcePayload>,
    resource_config: &Path,
) -> Result<Vec<u8>, Rem6CliError> {
    if payloads.len() != 1 {
        return Err(Rem6CliError::Execute {
            error: format!(
                "run resource config {} acquired {} required kernel resources; expected exactly one",
                resource_config.display(),
                payloads.len(),
            ),
        });
    }
    let payload = payloads.remove(0);
    Ok(payload.data().to_vec())
}
