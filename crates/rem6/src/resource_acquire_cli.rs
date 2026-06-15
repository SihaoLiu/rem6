use rem6_boot::BootImage;
use rem6_memory::Address;
use rem6_workload::{
    WorkloadAcquiredResource, WorkloadAcquiredSuiteResource, WorkloadId,
    WorkloadInMemoryResourceAcquisitionExecutor, WorkloadManifest, WorkloadResolvedResources,
    WorkloadResource, WorkloadResourceAcquisition, WorkloadResourceArtifact, WorkloadResourceId,
    WorkloadSuite, WorkloadSuiteId, WorkloadSuiteReplayPlan,
};

use crate::cli_output::emit_cli_output;
use crate::config::StatsFormat;
use crate::formatting::json_escape;
use crate::resource_acquire_config::{
    Rem6ResourceAcquireConfig, Rem6ResourceAcquireManifestConfig, Rem6ResourceAcquireResourceConfig,
};
use crate::stats_output::{resource_acquire_stats_output, Rem6ResourceAcquireStatsInputs};
use crate::{execute_error, Rem6CliError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ResourceAcquireArtifact {
    pub(crate) schema: &'static str,
    pub(crate) config: Rem6ResourceAcquireConfig,
    pub(crate) mode: &'static str,
    pub(crate) manifest_identity: String,
    pub(crate) suite_id: Option<String>,
    pub(crate) suite_identity: Option<String>,
    pub(crate) suite_manifests: u64,
    pub(crate) suite_required_resources: u64,
    pub(crate) suite_acquired_resources: u64,
    pub(crate) suite_acquired_bytes: u64,
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
    pub(crate) workload_id: Option<String>,
    pub(crate) manifest_identity: Option<String>,
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
    if config.suite_id().is_some() {
        run_suite_resource_acquire_config(config)
    } else {
        run_manifest_resource_acquire_config(config)
    }
}

fn run_manifest_resource_acquire_config(
    config: Rem6ResourceAcquireConfig,
) -> Result<Rem6ResourceAcquireArtifact, Rem6CliError> {
    let (manifest, executor) = build_manifest_and_artifacts(
        &config.manifests()[0],
        WorkloadInMemoryResourceAcquisitionExecutor::new(),
    )?;
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
        mode: "manifest",
        manifest_identity: manifest.identity().as_str().to_string(),
        suite_id: None,
        suite_identity: None,
        suite_manifests: 0,
        suite_required_resources: 0,
        suite_acquired_resources: 0,
        suite_acquired_bytes: 0,
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

fn run_suite_resource_acquire_config(
    config: Rem6ResourceAcquireConfig,
) -> Result<Rem6ResourceAcquireArtifact, Rem6CliError> {
    let suite_id = config
        .suite_id()
        .ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "resource_acquire.suite_id",
        })?
        .to_string();
    let mut suite_builder =
        WorkloadSuite::builder(WorkloadSuiteId::new(&suite_id).map_err(execute_error)?);
    let mut executor = WorkloadInMemoryResourceAcquisitionExecutor::new();
    for manifest_config in config.manifests() {
        let (manifest, next_executor) = build_manifest_and_artifacts(manifest_config, executor)?;
        executor = next_executor;
        suite_builder = suite_builder
            .add_manifest(manifest)
            .map_err(execute_error)?;
    }
    let suite = suite_builder.build().map_err(execute_error)?;
    let plan = WorkloadSuiteReplayPlan::from_suite(&suite).map_err(execute_error)?;
    let acquired = executor
        .acquire_suite_replay_plan(&plan)
        .map_err(execute_error)?;
    let resources = acquired
        .iter()
        .map(Rem6ResourceAcquireResourceSummary::from_suite_acquired)
        .collect::<Vec<_>>();
    let suite_required_resources = plan.required_resources().len() as u64;
    let suite_acquired_resources = resources.len() as u64;
    let suite_acquired_bytes = resources
        .iter()
        .map(|resource| resource.size_bytes)
        .sum::<u64>();
    let mut artifact = Rem6ResourceAcquireArtifact {
        schema: "rem6.cli.resource_acquire.v1",
        config,
        mode: "suite",
        manifest_identity: String::new(),
        suite_id: Some(suite_id),
        suite_identity: Some(plan.suite_identity().as_str().to_string()),
        suite_manifests: plan.entries().len() as u64,
        suite_required_resources,
        suite_acquired_resources,
        suite_acquired_bytes,
        required_resources: suite_required_resources,
        acquired_resources: suite_acquired_resources,
        resolved_resources: 0,
        acquired_bytes: suite_acquired_bytes,
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

fn build_manifest_and_artifacts(
    manifest_config: &Rem6ResourceAcquireManifestConfig,
    mut executor: WorkloadInMemoryResourceAcquisitionExecutor,
) -> Result<
    (
        WorkloadManifest,
        WorkloadInMemoryResourceAcquisitionExecutor,
    ),
    Rem6CliError,
> {
    let mut builder = WorkloadManifest::builder(
        WorkloadId::new(manifest_config.workload_id()).map_err(execute_error)?,
        BootImage::new(Address::new(manifest_config.boot_entry())),
    );

    for resource in manifest_config.resources() {
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
    Ok((manifest, executor))
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
        if self.mode == "suite" {
            format!(
                "{{\"schema\":\"{}\",\"mode\":\"suite\",\"suite_id\":\"{}\",\"suite_identity\":\"{}\",\"suite_manifests\":{},\"suite_required_resources\":{},\"suite_acquired_resources\":{},\"suite_acquired_bytes\":{},\"resources\":[{}],\"stats\":{}}}\n",
                self.schema,
                json_escape(self.suite_id.as_deref().unwrap_or("")),
                json_escape(self.suite_identity.as_deref().unwrap_or("")),
                self.suite_manifests,
                self.suite_required_resources,
                self.suite_acquired_resources,
                self.suite_acquired_bytes,
                resources,
                self.stats_json,
            )
        } else {
            format!(
                "{{\"schema\":\"{}\",\"mode\":\"manifest\",\"workload_id\":\"{}\",\"boot_entry\":\"0x{:x}\",\"manifest_identity\":\"{}\",\"required_resources\":{},\"acquired_resources\":{},\"resolved_resources\":{},\"acquired_bytes\":{},\"resources\":[{}],\"stats\":{}}}\n",
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
}

impl Rem6ResourceAcquireResourceSummary {
    fn from_acquired(acquired: &WorkloadAcquiredResource) -> Self {
        Self {
            workload_id: None,
            manifest_identity: None,
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

    fn from_suite_acquired(acquired: &WorkloadAcquiredSuiteResource) -> Self {
        let mut summary = Self::from_acquired(acquired.acquired());
        summary.workload_id = Some(acquired.workload_id().as_str().to_string());
        summary.manifest_identity = Some(acquired.manifest_identity().as_str().to_string());
        summary
    }

    fn to_json(&self) -> String {
        let workload_id = optional_json_string("workload_id", self.workload_id.as_deref());
        let manifest_identity =
            optional_json_string("manifest_identity", self.manifest_identity.as_deref());
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
            "{{{}{}\"resource\":\"{}\",\"kind\":\"{}\",\"digest\":\"{}\",\"size_bytes\":{},\"acquisition_kind\":\"{}\",\"acquisition_locator\":\"{}\",\"acquisition_tool\":{},\"acquisition_revision\":{}}}",
            workload_id,
            manifest_identity,
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

fn optional_json_string(field: &str, value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\":\"{}\",", field, json_escape(value)))
        .unwrap_or_default()
}
