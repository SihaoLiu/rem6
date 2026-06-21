use std::path::{Path, PathBuf};

use rem6_workload::{WorkloadResourceAcquisitionKind, WorkloadResourceKind};
use serde::Deserialize;

use crate::config::StatsFormat;
use crate::Rem6CliError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ResourceAcquireConfig {
    suite_id: Option<String>,
    manifests: Vec<Rem6ResourceAcquireManifestConfig>,
    stats_format: StatsFormat,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ResourceAcquireManifestConfig {
    workload_id: String,
    boot_entry: u64,
    resources: Vec<Rem6ResourceAcquireResourceConfig>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Rem6ResourceAcquireResourceConfig {
    id: String,
    kind: WorkloadResourceKind,
    digest: String,
    locator: String,
    required: bool,
    acquisition_kind: WorkloadResourceAcquisitionKind,
    acquisition_locator: String,
    acquisition_tool: Option<String>,
    acquisition_revision: Option<String>,
    artifact: PathBuf,
    artifact_member: Option<String>,
    artifact_remote_locator: Option<String>,
    artifact_digest: String,
    artifact_size: Option<usize>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6ResourceAcquireToml {
    resource_acquire: Option<Rem6ResourceAcquireFileConfig>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6ResourceAcquireFileConfig {
    suite_id: Option<String>,
    manifests: Option<Vec<Rem6ResourceAcquireFileManifestConfig>>,
    workload_id: Option<String>,
    boot_entry: Option<u64>,
    resources: Option<Vec<Rem6ResourceAcquireFileResourceConfig>>,
    stats_format: Option<String>,
    output: Option<PathBuf>,
    stats_output: Option<PathBuf>,
    #[serde(skip)]
    config_dir: Option<PathBuf>,
}

impl Rem6ResourceAcquireFileConfig {
    fn resolve_path(&self, path: &Path) -> PathBuf {
        resolve_config_path(self.config_dir.as_deref(), path)
    }
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6ResourceAcquireFileManifestConfig {
    workload_id: Option<String>,
    boot_entry: Option<u64>,
    resources: Option<Vec<Rem6ResourceAcquireFileResourceConfig>>,
}

#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rem6ResourceAcquireFileResourceConfig {
    id: Option<String>,
    kind: Option<String>,
    digest: Option<String>,
    locator: Option<String>,
    required: Option<bool>,
    acquisition_kind: Option<String>,
    acquisition_locator: Option<String>,
    acquisition_tool: Option<String>,
    acquisition_revision: Option<String>,
    artifact: Option<PathBuf>,
    artifact_digest: Option<String>,
    artifact_size: Option<usize>,
}

impl Rem6ResourceAcquireConfig {
    pub fn parse_args<I, S>(args: I) -> Result<Self, Rem6CliError>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut args = args.into_iter().map(Into::into);
        let Some(command) = args.next() else {
            return Err(Rem6CliError::MissingCommand);
        };
        if command != "resource-acquire" {
            return Err(Rem6CliError::UnsupportedCommand { command });
        }

        let remaining_args = args.collect::<Vec<_>>();
        let file_config = resource_acquire_file_config_from_args(&remaining_args)?
            .map(|path| load_resource_acquire_file_config(&path))
            .transpose()?
            .unwrap_or_default();

        let mut workload_id = file_config.workload_id.clone();
        let mut boot_entry = file_config.boot_entry;
        let mut stats_format = file_config
            .stats_format
            .as_deref()
            .map(StatsFormat::parse)
            .transpose()?
            .unwrap_or(StatsFormat::Json);
        let mut output = file_config
            .output
            .as_deref()
            .map(|path| file_config.resolve_path(path));
        let mut stats_output = file_config
            .stats_output
            .as_deref()
            .map(|path| file_config.resolve_path(path));

        let mut args = remaining_args.into_iter();
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--config" => {
                    let _ = required_value(&flag, args.next())?;
                }
                "--workload-id" => {
                    workload_id = Some(required_value(&flag, args.next())?);
                }
                "--boot-entry" => {
                    let value = required_value(&flag, args.next())?;
                    boot_entry = Some(parse_number(&value).ok_or_else(|| {
                        Rem6CliError::InvalidStartAddress {
                            value: value.clone(),
                        }
                    })?);
                }
                "--stats-format" => {
                    stats_format = StatsFormat::parse(&required_value(&flag, args.next())?)?;
                }
                "--output" => {
                    output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                "--stats-output" => {
                    stats_output = Some(PathBuf::from(required_value(&flag, args.next())?));
                }
                _ => return Err(Rem6CliError::UnknownFlag { flag }),
            }
        }
        let manifests = match file_config.manifests.as_deref() {
            Some(manifests) => {
                if manifests.is_empty() {
                    return Err(Rem6CliError::MissingRequiredFlag {
                        flag: "resource_acquire.manifests",
                    });
                }
                manifests
                    .iter()
                    .map(|manifest| parse_resource_acquire_manifest(&file_config, manifest))
                    .collect::<Result<Vec<_>, _>>()?
            }
            None if file_config.suite_id.is_some() => {
                return Err(Rem6CliError::MissingRequiredFlag {
                    flag: "resource_acquire.manifests",
                });
            }
            None => {
                let resources = file_config
                    .resources
                    .as_deref()
                    .unwrap_or(&[])
                    .iter()
                    .map(|resource| parse_resource_acquire_resource(&file_config, resource))
                    .collect::<Result<Vec<_>, _>>()?;
                if resources.is_empty() {
                    return Err(Rem6CliError::MissingRequiredFlag {
                        flag: "resource_acquire.resources",
                    });
                }
                vec![Rem6ResourceAcquireManifestConfig {
                    workload_id: workload_id.ok_or(Rem6CliError::MissingRequiredFlag {
                        flag: "resource_acquire.workload_id",
                    })?,
                    boot_entry: boot_entry.ok_or(Rem6CliError::MissingRequiredFlag {
                        flag: "resource_acquire.boot_entry",
                    })?,
                    resources,
                }]
            }
        };

        if let (Some(output), Some(stats_output)) = (&output, &stats_output) {
            if output == stats_output {
                return Err(Rem6CliError::ConflictingOutputPaths {
                    path: output.to_path_buf(),
                });
            }
        }
        let suite_id = if file_config.manifests.is_some() {
            Some(
                file_config
                    .suite_id
                    .clone()
                    .ok_or(Rem6CliError::MissingRequiredFlag {
                        flag: "resource_acquire.suite_id",
                    })?,
            )
        } else {
            None
        };

        Ok(Self {
            suite_id,
            manifests,
            stats_format,
            output,
            stats_output,
        })
    }

    pub fn suite_id(&self) -> Option<&str> {
        self.suite_id.as_deref()
    }

    pub fn manifests(&self) -> &[Rem6ResourceAcquireManifestConfig] {
        &self.manifests
    }

    pub fn workload_id(&self) -> &str {
        self.manifests[0].workload_id()
    }

    pub fn boot_entry(&self) -> u64 {
        self.manifests[0].boot_entry()
    }

    pub fn resources(&self) -> &[Rem6ResourceAcquireResourceConfig] {
        self.manifests[0].resources()
    }

    pub fn resource_count(&self) -> usize {
        self.manifests
            .iter()
            .map(|manifest| manifest.resources().len())
            .sum()
    }

    pub const fn stats_format(&self) -> StatsFormat {
        self.stats_format
    }

    pub fn output(&self) -> Option<&Path> {
        self.output.as_deref()
    }

    pub fn stats_output(&self) -> Option<&Path> {
        self.stats_output.as_deref()
    }
}

impl Rem6ResourceAcquireManifestConfig {
    pub fn workload_id(&self) -> &str {
        &self.workload_id
    }

    pub const fn boot_entry(&self) -> u64 {
        self.boot_entry
    }

    pub fn resources(&self) -> &[Rem6ResourceAcquireResourceConfig] {
        &self.resources
    }
}

impl Rem6ResourceAcquireResourceConfig {
    pub fn id(&self) -> &str {
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

    pub const fn required(&self) -> bool {
        self.required
    }

    pub const fn acquisition_kind(&self) -> WorkloadResourceAcquisitionKind {
        self.acquisition_kind
    }

    pub fn acquisition_locator(&self) -> &str {
        &self.acquisition_locator
    }

    pub fn acquisition_tool(&self) -> Option<&str> {
        self.acquisition_tool.as_deref()
    }

    pub fn acquisition_revision(&self) -> Option<&str> {
        self.acquisition_revision.as_deref()
    }

    pub fn artifact(&self) -> &Path {
        &self.artifact
    }

    pub fn artifact_member(&self) -> Option<&str> {
        self.artifact_member.as_deref()
    }

    pub fn artifact_remote_locator(&self) -> Option<&str> {
        self.artifact_remote_locator.as_deref()
    }

    pub fn artifact_digest(&self) -> &str {
        &self.artifact_digest
    }

    pub const fn artifact_size(&self) -> Option<usize> {
        self.artifact_size
    }
}

fn parse_resource_acquire_manifest(
    file_config: &Rem6ResourceAcquireFileConfig,
    manifest: &Rem6ResourceAcquireFileManifestConfig,
) -> Result<Rem6ResourceAcquireManifestConfig, Rem6CliError> {
    let resources = manifest
        .resources
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|resource| parse_resource_acquire_resource(file_config, resource))
        .collect::<Result<Vec<_>, _>>()?;
    if resources.is_empty() {
        return Err(Rem6CliError::MissingRequiredFlag {
            flag: "resource_acquire.manifests.resources",
        });
    }
    Ok(Rem6ResourceAcquireManifestConfig {
        workload_id: manifest
            .workload_id
            .clone()
            .ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "resource_acquire.manifests.workload_id",
            })?,
        boot_entry: manifest
            .boot_entry
            .ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "resource_acquire.manifests.boot_entry",
            })?,
        resources,
    })
}

fn parse_resource_acquire_resource(
    file_config: &Rem6ResourceAcquireFileConfig,
    resource: &Rem6ResourceAcquireFileResourceConfig,
) -> Result<Rem6ResourceAcquireResourceConfig, Rem6CliError> {
    let id = resource
        .id
        .clone()
        .ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "resource_acquire.resources.id",
        })?;
    let kind = resource
        .kind
        .as_deref()
        .map(|value| {
            parse_resource_kind(value).ok_or_else(|| Rem6CliError::InvalidResourceKind {
                value: value.to_string(),
            })
        })
        .transpose()?
        .ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "resource_acquire.resources.kind",
        })?;
    let digest = resource
        .digest
        .clone()
        .ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "resource_acquire.resources.digest",
        })?;
    let locator = resource
        .locator
        .clone()
        .ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "resource_acquire.resources.locator",
        })?;
    let acquisition_kind = resource
        .acquisition_kind
        .as_deref()
        .map(|value| {
            parse_resource_acquisition_kind(value).ok_or_else(|| {
                Rem6CliError::InvalidResourceAcquisitionKind {
                    value: value.to_string(),
                }
            })
        })
        .transpose()?
        .ok_or(Rem6CliError::MissingRequiredFlag {
            flag: "resource_acquire.resources.acquisition_kind",
        })?;
    let acquisition_locator =
        resource
            .acquisition_locator
            .clone()
            .ok_or(Rem6CliError::MissingRequiredFlag {
                flag: "resource_acquire.resources.acquisition_locator",
            })?;
    let artifact = resource.artifact.as_deref();
    let (artifact, artifact_member, artifact_remote_locator) = match (artifact, acquisition_kind) {
        (Some(_), WorkloadResourceAcquisitionKind::Generated) => {
            return Err(Rem6CliError::Execute {
                error: format!("generated resource {id} must not declare artifact"),
            });
        }
        (Some(artifact), _) => (file_config.resolve_path(artifact), None, None),
        (
            None,
            WorkloadResourceAcquisitionKind::HostFile | WorkloadResourceAcquisitionKind::Preloaded,
        ) => (
            file_config.resolve_path(Path::new(&acquisition_locator)),
            None,
            None,
        ),
        (
            None,
            WorkloadResourceAcquisitionKind::ArchiveTar
            | WorkloadResourceAcquisitionKind::ArchiveZip,
        ) => {
            let (archive, member) =
                acquisition_locator
                    .split_once('#')
                    .ok_or(Rem6CliError::MissingRequiredFlag {
                        flag: "resource_acquire.resources.acquisition_locator archive#member",
                    })?;
            if archive.is_empty() || member.is_empty() {
                return Err(Rem6CliError::MissingRequiredFlag {
                    flag: "resource_acquire.resources.acquisition_locator archive#member",
                });
            }
            (
                file_config.resolve_path(Path::new(archive)),
                Some(member.to_string()),
                None,
            )
        }
        (None, WorkloadResourceAcquisitionKind::ArchiveGzip) => (
            file_config.resolve_path(Path::new(&acquisition_locator)),
            None,
            None,
        ),
        (None, WorkloadResourceAcquisitionKind::RemoteUri) => {
            (PathBuf::new(), None, Some(acquisition_locator.clone()))
        }
        (None, WorkloadResourceAcquisitionKind::Generated) => {
            if resource.artifact_size.is_none() {
                return Err(Rem6CliError::MissingRequiredFlag {
                    flag: "resource_acquire.resources.artifact_size",
                });
            }
            (PathBuf::from(&acquisition_locator), None, None)
        }
        (None, _) => {
            return Err(Rem6CliError::MissingRequiredFlag {
                flag: "resource_acquire.resources.artifact",
            });
        }
    };
    let artifact_digest = match (&resource.artifact_digest, acquisition_kind) {
        (Some(artifact_digest), WorkloadResourceAcquisitionKind::RemoteUri) => {
            if !is_sha256_content_digest(artifact_digest) {
                return Err(Rem6CliError::InvalidRemoteResourceArtifactDigest {
                    resource: id,
                    value: artifact_digest.clone(),
                });
            }
            artifact_digest.clone()
        }
        (None, WorkloadResourceAcquisitionKind::RemoteUri) => {
            return Err(Rem6CliError::MissingRemoteResourceArtifactDigest { resource: id });
        }
        (Some(artifact_digest), _) => artifact_digest.clone(),
        (None, _) => digest.clone(),
    };

    Ok(Rem6ResourceAcquireResourceConfig {
        id,
        kind,
        digest,
        locator,
        required: resource.required.unwrap_or(false),
        acquisition_kind,
        acquisition_locator,
        acquisition_tool: resource.acquisition_tool.clone(),
        acquisition_revision: resource.acquisition_revision.clone(),
        artifact,
        artifact_member,
        artifact_remote_locator,
        artifact_digest,
        artifact_size: resource.artifact_size,
    })
}

fn is_sha256_content_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn resolve_config_path(config_dir: Option<&Path>, path: &Path) -> PathBuf {
    if path.is_relative() {
        config_dir
            .map(|dir| dir.join(path))
            .unwrap_or_else(|| path.to_path_buf())
    } else {
        path.to_path_buf()
    }
}

fn parse_number(value: &str) -> Option<u64> {
    if let Some(hex) = value
        .strip_prefix("0x")
        .or_else(|| value.strip_prefix("0X"))
    {
        u64::from_str_radix(hex, 16).ok()
    } else {
        value.parse().ok()
    }
}

fn parse_resource_kind(value: &str) -> Option<WorkloadResourceKind> {
    match value {
        "kernel" => Some(WorkloadResourceKind::Kernel),
        "disk-image" => Some(WorkloadResourceKind::DiskImage),
        "firmware" => Some(WorkloadResourceKind::Firmware),
        "device-tree" => Some(WorkloadResourceKind::DeviceTree),
        "input" => Some(WorkloadResourceKind::Input),
        "output" => Some(WorkloadResourceKind::Output),
        "initrd" => Some(WorkloadResourceKind::Initrd),
        _ => None,
    }
}

fn parse_resource_acquisition_kind(value: &str) -> Option<WorkloadResourceAcquisitionKind> {
    match value {
        "local-file" => Some(WorkloadResourceAcquisitionKind::LocalFile),
        "host-file" => Some(WorkloadResourceAcquisitionKind::HostFile),
        "archive-tar" => Some(WorkloadResourceAcquisitionKind::ArchiveTar),
        "archive-gzip" => Some(WorkloadResourceAcquisitionKind::ArchiveGzip),
        "archive-zip" => Some(WorkloadResourceAcquisitionKind::ArchiveZip),
        "remote-uri" => Some(WorkloadResourceAcquisitionKind::RemoteUri),
        "generated" => Some(WorkloadResourceAcquisitionKind::Generated),
        "preloaded" => Some(WorkloadResourceAcquisitionKind::Preloaded),
        _ => None,
    }
}

fn resource_acquire_file_config_from_args(
    args: &[String],
) -> Result<Option<PathBuf>, Rem6CliError> {
    config_path_from_args(
        args,
        &[
            "--workload-id",
            "--boot-entry",
            "--stats-format",
            "--output",
            "--stats-output",
        ],
        &[],
    )
}

fn config_path_from_args(
    args: &[String],
    value_flags: &[&str],
    bool_flags: &[&str],
) -> Result<Option<PathBuf>, Rem6CliError> {
    let mut path = None;
    let mut index = 0;
    while let Some(flag) = args.get(index) {
        match flag.as_str() {
            "--config" => {
                let value = args
                    .get(index + 1)
                    .cloned()
                    .ok_or_else(|| Rem6CliError::MissingFlagValue { flag: flag.clone() })?;
                path = Some(PathBuf::from(value));
                index += 2;
            }
            flag if bool_flags.contains(&flag) => {
                index += 1;
            }
            flag if value_flags.contains(&flag) => {
                index += 2;
            }
            _ => {
                index += 1;
            }
        }
    }
    Ok(path)
}

fn load_resource_acquire_file_config(
    path: &Path,
) -> Result<Rem6ResourceAcquireFileConfig, Rem6CliError> {
    let mut resource_acquire = load_file_config(path)?.resource_acquire.unwrap_or_default();
    resource_acquire.config_dir = path.parent().map(Path::to_path_buf);
    Ok(resource_acquire)
}

fn load_file_config(path: &Path) -> Result<Rem6ResourceAcquireToml, Rem6CliError> {
    let text = std::fs::read_to_string(path).map_err(|error| Rem6CliError::ReadConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })?;
    toml::from_str::<Rem6ResourceAcquireToml>(&text).map_err(|error| Rem6CliError::ParseConfig {
        path: path.to_path_buf(),
        error: error.to_string(),
    })
}

fn required_value(flag: &str, value: Option<String>) -> Result<String, Rem6CliError> {
    value.ok_or_else(|| Rem6CliError::MissingFlagValue {
        flag: flag.to_string(),
    })
}
