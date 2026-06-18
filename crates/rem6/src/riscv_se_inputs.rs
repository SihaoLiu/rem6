use std::path::PathBuf;

use crate::config::{RiscvSeFileRequest, RiscvSeInputSource};
use crate::run_resource_config::RunResourcePayloads;
use crate::Rem6CliError;

pub(super) fn read_riscv_se_stdin(
    source: &RiscvSeInputSource,
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<u8>, Rem6CliError> {
    read_input_source("RISC-V SE stdin", source, resource_payloads).map_err(|error| match error {
        RiscvSeInputError::ReadPath { path, error } => {
            Rem6CliError::ReadRiscvSeStdin { path, error }
        }
        RiscvSeInputError::Execute { error } => Rem6CliError::Execute { error },
    })
}

pub(super) fn read_riscv_se_file(
    request: &RiscvSeFileRequest,
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<u8>, Rem6CliError> {
    read_input_source("RISC-V SE file", request.source(), resource_payloads).map_err(|error| {
        match error {
            RiscvSeInputError::ReadPath { path, error } => Rem6CliError::ReadRiscvSeFile {
                guest_path: request.guest_path().to_string(),
                path,
                error,
            },
            RiscvSeInputError::Execute { error } => Rem6CliError::Execute { error },
        }
    })
}

fn read_input_source(
    use_case: &str,
    source: &RiscvSeInputSource,
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<u8>, RiscvSeInputError> {
    match source {
        RiscvSeInputSource::Path(path) => {
            std::fs::read(path).map_err(|error| RiscvSeInputError::ReadPath {
                path: path.to_path_buf(),
                error: error.to_string(),
            })
        }
        RiscvSeInputSource::Resource(resource) => {
            let payloads = resource_payloads.ok_or_else(|| RiscvSeInputError::Execute {
                error: format!("{use_case} resource {resource} requires --resource-config"),
            })?;
            payloads
                .input_payload(use_case, resource)
                .map(Vec::from)
                .map_err(resource_input_error)
        }
        RiscvSeInputSource::SuiteResource(selector) => {
            let payloads = resource_payloads.ok_or_else(|| RiscvSeInputError::Execute {
                error: format!(
                    "{use_case} suite resource {} requires --resource-config",
                    selector.qualified_id()
                ),
            })?;
            payloads
                .input_suite_payload(use_case, selector.workload_id(), selector.resource_id())
                .map(Vec::from)
                .map_err(resource_input_error)
        }
    }
}

fn resource_input_error(error: Rem6CliError) -> RiscvSeInputError {
    match error {
        Rem6CliError::Execute { error } => RiscvSeInputError::Execute { error },
        other => RiscvSeInputError::Execute {
            error: other.to_string(),
        },
    }
}

#[derive(Debug)]
enum RiscvSeInputError {
    ReadPath { path: PathBuf, error: String },
    Execute { error: String },
}
