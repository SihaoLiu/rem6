use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadAcquiredResource, WorkloadId, WorkloadInMemoryResourceAcquisitionExecutor,
    WorkloadManifest, WorkloadResolvedResources, WorkloadResource, WorkloadResourceAcquisition,
    WorkloadResourceArtifact, WorkloadResourceId,
};

use crate::cli_output::emit_cli_output;
use crate::config::StatsFormat;
use crate::formatting::json_escape;
use crate::resource_acquire_config::{
    Rem6ResourceAcquireConfig, Rem6ResourceAcquireResourceConfig,
};
use crate::stats_output::{resource_acquire_stats_output, Rem6ResourceAcquireStatsInputs};
use crate::{execute_error, Rem6CliError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ResourceAcquireArtifact {
    pub(crate) schema: &'static str,
    pub(crate) config: Rem6ResourceAcquireConfig,
    pub(crate) manifest_identity: String,
    pub(crate) required_resources: u64,
    pub(crate) acquired_resources: u64,
    pub(crate) resolved_resources: u64,
    pub(crate) acquired_bytes: u64,
    pub(crate) resources: Vec<Rem6ResourceAcquireResourceSummary>,
    pub(crate) stats_json: String,
    pub(crate) stats_text: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ResourceAcquireResourceSummary {
    pub(crate) resource: String,
    pub(crate) kind: &'static str,
    pub(crate) digest: String,
    pub(crate) size_bytes: u64,
    pub(crate) acquisition_kind: &'static str,
    pub(crate) acquisition_locator: String,
    pub(crate) acquisition_tool: Option<String>,
    pub(crate) acquisition_revision: Option<String>,
}

pub(crate) fn run_resource_acquire_cli(args: Vec<String>) -> Result<String, Rem6CliError> {
    let config = Rem6ResourceAcquireConfig::parse_args(args)?;
    let artifact = run_resource_acquire_config(config)?;
    let stats_format = artifact.config.stats_format();
    let output = match stats_format {
        StatsFormat::Json => artifact.to_json(),
        StatsFormat::Text => artifact.stats_text.clone(),
    };
    emit_cli_output(
        output,
        &artifact.stats_json,
        &artifact.stats_text,
        artifact.config.output(),
        artifact.config.stats_output(),
        stats_format,
    )
}

pub fn run_resource_acquire_config(
    config: Rem6ResourceAcquireConfig,
) -> Result<Rem6ResourceAcquireArtifact, Rem6CliError> {
    let mut builder = WorkloadManifest::builder(
        WorkloadId::new(config.workload_id()).map_err(execute_error)?,
        BootImage::new(Address::new(config.boot_entry())),
    );
    let mut executor = WorkloadInMemoryResourceAcquisitionExecutor::new();

    for resource in config.resources() {
        let resource_id = WorkloadResourceId::new(resource.id()).map_err(execute_error)?;
        let acquisition = resource_acquisition(resource)?;
        let workload_resource = WorkloadResource::new(
            resource_id.clone(),
            resource.kind(),
            resource.digest(),
            resource.locator(),
        )
        .map_err(execute_error)?
        .with_acquisition(acquisition.clone());
        builder = builder
            .add_resource(workload_resource)
            .map_err(execute_error)?;
        if resource.required() {
            builder = builder.add_required_resource(resource_id);
        }

        let data = std::fs::read(resource.artifact()).map_err(|error| {
            Rem6CliError::ReadResourceArtifact {
                path: resource.artifact().to_path_buf(),
                error: error.to_string(),
            }
        })?;
        let size_bytes = resource.artifact_size().unwrap_or(data.len());
        executor = executor
            .add_artifact(WorkloadResourceArtifact::new(
                acquisition,
                resource.artifact_digest(),
                size_bytes,
                data,
            ))
            .map_err(execute_error)?;
    }

    let manifest = builder.build().map_err(execute_error)?;
    let acquired = executor
        .acquire_manifest(&manifest)
        .map_err(execute_error)?;
    let resources = acquired
        .iter()
        .map(Rem6ResourceAcquireResourceSummary::from_acquired)
        .collect::<Vec<_>>();
    let _resolved = WorkloadResolvedResources::from_manifest(
        &manifest,
        acquired
            .into_iter()
            .map(WorkloadAcquiredResource::into_payload),
    )
    .map_err(execute_error)?;
    let required_resources = manifest.required_resources().len() as u64;
    let acquired_resources = resources.len() as u64;
    let resolved_resources = acquired_resources;
    let acquired_bytes = resources
        .iter()
        .map(|resource| resource.size_bytes)
        .sum::<u64>();
    let mut artifact = Rem6ResourceAcquireArtifact {
        schema: "rem6.cli.resource_acquire.v1",
        config,
        manifest_identity: manifest.identity().as_str().to_string(),
        required_resources,
        acquired_resources,
        resolved_resources,
        acquired_bytes,
        resources,
        stats_json: String::new(),
        stats_text: String::new(),
    };
    let stats = resource_acquire_stats_output(Rem6ResourceAcquireStatsInputs {
        artifact: &artifact,
    })?;
    artifact.stats_json = stats.json;
    artifact.stats_text = stats.text;
    Ok(artifact)
}

fn resource_acquisition(
    resource: &Rem6ResourceAcquireResourceConfig,
) -> Result<WorkloadResourceAcquisition, Rem6CliError> {
    let mut acquisition = WorkloadResourceAcquisition::new(
        resource.acquisition_kind(),
        resource.acquisition_locator(),
    )
    .map_err(execute_error)?;
    if let Some(tool) = resource.acquisition_tool() {
        acquisition = acquisition.with_tool(tool).map_err(execute_error)?;
    }
    if let Some(revision) = resource.acquisition_revision() {
        acquisition = acquisition.with_revision(revision).map_err(execute_error)?;
    }
    Ok(acquisition)
}

impl Rem6ResourceAcquireArtifact {
    pub fn to_json(&self) -> String {
        let resources = self
            .resources
            .iter()
            .map(Rem6ResourceAcquireResourceSummary::to_json)
            .collect::<Vec<_>>()
            .join(",");
        format!(
            "{{\"schema\":\"{}\",\"workload_id\":\"{}\",\"boot_entry\":\"0x{:x}\",\"manifest_identity\":\"{}\",\"required_resources\":{},\"acquired_resources\":{},\"resolved_resources\":{},\"acquired_bytes\":{},\"resources\":[{}],\"stats\":{}}}\n",
            self.schema,
            json_escape(self.config.workload_id()),
            self.config.boot_entry(),
            json_escape(&self.manifest_identity),
            self.required_resources,
            self.acquired_resources,
            self.resolved_resources,
            self.acquired_bytes,
            resources,
            self.stats_json,
        )
    }
}

impl Rem6ResourceAcquireResourceSummary {
    fn from_acquired(acquired: &WorkloadAcquiredResource) -> Self {
        Self {
            resource: acquired.resource().as_str().to_string(),
            kind: acquired.kind().as_str(),
            digest: acquired.digest().to_string(),
            size_bytes: acquired.size_bytes() as u64,
            acquisition_kind: acquired.acquisition().kind().as_str(),
            acquisition_locator: acquired.acquisition().locator().to_string(),
            acquisition_tool: acquired.acquisition().tool().map(str::to_string),
            acquisition_revision: acquired.acquisition().revision().map(str::to_string),
        }
    }

    fn to_json(&self) -> String {
        let acquisition_tool = self
            .acquisition_tool
            .as_ref()
            .map(|tool| format!("\"{}\"", json_escape(tool)))
            .unwrap_or_else(|| "null".to_string());
        let acquisition_revision = self
            .acquisition_revision
            .as_ref()
            .map(|revision| format!("\"{}\"", json_escape(revision)))
            .unwrap_or_else(|| "null".to_string());
        format!(
            "{{\"resource\":\"{}\",\"kind\":\"{}\",\"digest\":\"{}\",\"size_bytes\":{},\"acquisition_kind\":\"{}\",\"acquisition_locator\":\"{}\",\"acquisition_tool\":{},\"acquisition_revision\":{}}}",
            json_escape(&self.resource),
            self.kind,
            json_escape(&self.digest),
            self.size_bytes,
            self.acquisition_kind,
            json_escape(&self.acquisition_locator),
            acquisition_tool,
            acquisition_revision,
        )
    }
}
