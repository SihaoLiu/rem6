use std::path::{Component, Path, PathBuf};

use super::SuiteResourceSelector;
use crate::Rem6CliError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RiscvSeFileRequest {
    guest_path: String,
    source: RiscvSeInputSource,
}

impl RiscvSeFileRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let Some((guest_path, source)) = value.split_once('=') else {
            return Err(Rem6CliError::InvalidRiscvSeFile {
                value: value.to_string(),
            });
        };
        if guest_path.is_empty() || guest_path.as_bytes().contains(&0) {
            return Err(Rem6CliError::InvalidRiscvSeFile {
                value: value.to_string(),
            });
        }
        let source =
            RiscvSeInputSource::parse(source).ok_or_else(|| Rem6CliError::InvalidRiscvSeFile {
                value: value.to_string(),
            })?;
        Ok(Self {
            guest_path: guest_path.to_string(),
            source,
        })
    }

    pub fn guest_path(&self) -> &str {
        &self.guest_path
    }

    pub const fn source(&self) -> &RiscvSeInputSource {
        &self.source
    }

    pub(super) fn resolve_host_path(&mut self, base: &Path) {
        self.source.resolve_path(base);
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RiscvSeInputSource {
    Path(PathBuf),
    Resource(String),
    SuiteResource(SuiteResourceSelector),
}

impl RiscvSeInputSource {
    pub(super) fn parse(value: &str) -> Option<Self> {
        if value.is_empty() {
            return None;
        }
        if let Some(path) = value.strip_prefix("path:") {
            if path.is_empty() {
                return None;
            }
            return Some(Self::Path(PathBuf::from(path)));
        }
        if let Some(selector) = value.strip_prefix("suite-resource:") {
            return SuiteResourceSelector::parse(selector).map(Self::SuiteResource);
        }
        if let Some(resource) = value.strip_prefix("resource:") {
            if resource.is_empty() {
                return None;
            }
            return Some(Self::Resource(resource.to_string()));
        }
        Some(Self::Path(PathBuf::from(value)))
    }

    pub fn source_name(&self) -> String {
        match self {
            Self::Path(path) => path.display().to_string(),
            Self::Resource(resource) => format!("resource:{resource}"),
            Self::SuiteResource(selector) => selector.source_name(),
        }
    }

    pub(super) fn resolve_path(&mut self, base: &Path) {
        let Self::Path(path) = self else {
            return;
        };
        if path.is_relative() {
            *path = base.join(&path);
        }
    }
}

pub(super) fn reject_conflicting_riscv_se_output_paths(
    riscv_se_files: &[RiscvSeFileRequest],
    output: Option<&Path>,
    stats_output: Option<&Path>,
    power_output: Option<&Path>,
) -> Result<(), Rem6CliError> {
    let mut path_backed_files = Vec::new();
    let mut guest_paths = Vec::new();
    for file in riscv_se_files {
        if guest_paths.contains(&file.guest_path()) {
            return Err(Rem6CliError::DuplicateRiscvSeGuestFile {
                guest_path: file.guest_path().to_string(),
            });
        }
        guest_paths.push(file.guest_path());
        let RiscvSeInputSource::Path(path) = file.source() else {
            continue;
        };
        let path = path.as_path();
        let normalized_path = normalized_output_path(path);
        if output_path_matches(output, path)
            || output_path_matches(stats_output, path)
            || output_path_matches(power_output, path)
        {
            return Err(Rem6CliError::ConflictingRunOutputPaths {
                path: path.to_path_buf(),
            });
        }
        if path_backed_files.contains(&normalized_path) {
            return Err(Rem6CliError::ConflictingRunOutputPaths {
                path: path.to_path_buf(),
            });
        }
        path_backed_files.push(normalized_path);
    }
    Ok(())
}

fn output_path_matches(candidate: Option<&Path>, path: &Path) -> bool {
    candidate
        .is_some_and(|candidate| normalized_output_path(candidate) == normalized_output_path(path))
}

fn normalized_output_path(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir if ends_with_normal_component(&normalized) => {
                normalized.pop();
            }
            Component::ParentDir if !normalized.has_root() => {
                normalized.push(component.as_os_str());
            }
            Component::ParentDir => {}
            Component::Normal(_) | Component::RootDir | Component::Prefix(_) => {
                normalized.push(component.as_os_str());
            }
        }
    }
    normalized
}

fn ends_with_normal_component(path: &Path) -> bool {
    matches!(path.components().next_back(), Some(Component::Normal(_)))
}
