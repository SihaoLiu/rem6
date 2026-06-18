use std::path::{Path, PathBuf};

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

    pub(super) fn resolve_path(&mut self, base: &Path) {
        let Self::Path(path) = self else {
            return;
        };
        if path.is_relative() {
            *path = base.join(&path);
        }
    }
}
