use rem6_kernel::PartitionId;
use rem6_memory::{AccessSize, Address};
use rem6_mmio::{MmioBus, MmioRoute};
use rem6_platform::PlatformReadfileMmioDevice;

use crate::run_resource_config::RunResourcePayloads;
use crate::{execute_error, ReadfileRequest, ReadfileSource, Rem6CliError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) struct LoadedReadfile {
    summary: Rem6ReadfileSummary,
    payload: Vec<u8>,
}

impl LoadedReadfile {
    pub(super) fn summary(&self) -> &Rem6ReadfileSummary {
        &self.summary
    }
}

pub(super) fn read_readfiles(
    requests: &[ReadfileRequest],
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<LoadedReadfile>, Rem6CliError> {
    requests
        .iter()
        .map(|request| {
            let payload = read_readfile_payload(request, resource_payloads)?;
            let summary = Rem6ReadfileSummary::new(
                request.base(),
                request.size(),
                request.source_name(),
                payload.len() as u64,
            );
            Ok(LoadedReadfile { summary, payload })
        })
        .collect()
}

fn read_readfile_payload(
    request: &ReadfileRequest,
    resource_payloads: Option<&RunResourcePayloads>,
) -> Result<Vec<u8>, Rem6CliError> {
    match request.source() {
        ReadfileSource::Path(path) => {
            std::fs::read(path).map_err(|error| Rem6CliError::ReadReadfile {
                path: path.to_path_buf(),
                error: error.to_string(),
            })
        }
        ReadfileSource::Resource(resource) => {
            let payloads = resource_payloads.ok_or_else(|| Rem6CliError::Execute {
                error: format!("readfile resource {resource} requires --resource-config"),
            })?;
            Ok(payloads.readfile_payload(resource)?.to_vec())
        }
        ReadfileSource::SuiteResource(selector) => {
            let payloads = resource_payloads.ok_or_else(|| Rem6CliError::Execute {
                error: format!(
                    "readfile suite resource {} requires --resource-config",
                    selector.qualified_id()
                ),
            })?;
            Ok(payloads
                .readfile_suite_payload(selector.workload_id(), selector.resource_id())?
                .to_vec())
        }
    }
}

pub(super) fn readfile_mmio_bus(
    readfiles: &[LoadedReadfile],
    core_count: u32,
    target_partition: PartitionId,
    route_delay: u64,
) -> Result<Option<MmioBus>, Rem6CliError> {
    if readfiles.is_empty() {
        return Ok(None);
    }

    let mut bus = MmioBus::new();
    for readfile in readfiles {
        let size = AccessSize::new(readfile.summary.size()).map_err(execute_error)?;
        let device = PlatformReadfileMmioDevice::new(
            Address::new(readfile.summary.base()),
            size,
            readfile.payload.clone(),
        )
        .map_err(execute_error)?;
        for cpu_index in 0..core_count {
            let route = MmioRoute::new(
                PartitionId::new(cpu_index),
                target_partition,
                route_delay,
                route_delay,
            )
            .map_err(execute_error)?;
            bus.insert_device(device.range(), route, device.clone())
                .map_err(execute_error)?;
        }
    }

    Ok(Some(bus))
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ReadfileSummary {
    base: u64,
    size: u64,
    source: String,
    bytes: u64,
}

impl Rem6ReadfileSummary {
    pub fn new(base: u64, size: u64, source: String, bytes: u64) -> Self {
        Self {
            base,
            size,
            source,
            bytes,
        }
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub fn path(&self) -> &str {
        &self.source
    }

    pub const fn bytes(&self) -> u64 {
        self.bytes
    }
}
