use std::path::{Path, PathBuf};

use crate::config::parse::parse_number;
use crate::Rem6CliError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryDumpRequest {
    address: u64,
    bytes: u64,
}

impl MemoryDumpRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let Some((address, bytes)) = value.split_once(':') else {
            return Err(Rem6CliError::InvalidMemoryDump {
                value: value.to_string(),
            });
        };
        let address = parse_number(address).ok_or_else(|| Rem6CliError::InvalidMemoryDump {
            value: value.to_string(),
        })?;
        let bytes = parse_number(bytes)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| Rem6CliError::InvalidMemoryDump {
                value: value.to_string(),
            })?;
        Ok(Self { address, bytes })
    }

    pub const fn address(self) -> u64 {
        self.address
    }

    pub const fn bytes(self) -> u64 {
        self.bytes
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LoadBlobRequest {
    address: u64,
    source: LoadBlobSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuiteResourceSelector {
    workload_id: String,
    resource_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LoadBlobSource {
    Path(PathBuf),
    Resource(String),
    SuiteResource(SuiteResourceSelector),
}

impl SuiteResourceSelector {
    pub fn parse_source(value: &str) -> Option<Self> {
        value.strip_prefix("suite-resource:").and_then(Self::parse)
    }

    pub(in crate::config) fn parse(value: &str) -> Option<Self> {
        let (workload_id, resource_id) = value.split_once('/')?;
        if workload_id.is_empty() || resource_id.is_empty() {
            return None;
        }
        Some(Self {
            workload_id: workload_id.to_string(),
            resource_id: resource_id.to_string(),
        })
    }

    pub fn workload_id(&self) -> &str {
        &self.workload_id
    }

    pub fn resource_id(&self) -> &str {
        &self.resource_id
    }

    pub fn qualified_id(&self) -> String {
        format!("{}/{}", self.workload_id, self.resource_id)
    }

    pub fn source_name(&self) -> String {
        format!("suite-resource:{}", self.qualified_id())
    }
}

impl LoadBlobRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let Some((address, path)) = value.split_once(':') else {
            return Err(Rem6CliError::InvalidLoadBlob {
                value: value.to_string(),
            });
        };
        let address = parse_number(address).ok_or_else(|| Rem6CliError::InvalidLoadBlob {
            value: value.to_string(),
        })?;
        if path.is_empty() {
            return Err(Rem6CliError::InvalidLoadBlob {
                value: value.to_string(),
            });
        }
        let source = if let Some(selector) = path.strip_prefix("suite-resource:") {
            SuiteResourceSelector::parse(selector)
                .map(LoadBlobSource::SuiteResource)
                .ok_or_else(|| Rem6CliError::InvalidLoadBlob {
                    value: value.to_string(),
                })?
        } else {
            path.strip_prefix("resource:")
                .map(|resource| {
                    if resource.is_empty() {
                        return Err(Rem6CliError::InvalidLoadBlob {
                            value: value.to_string(),
                        });
                    }
                    Ok(LoadBlobSource::Resource(resource.to_string()))
                })
                .unwrap_or_else(|| Ok(LoadBlobSource::Path(PathBuf::from(path))))?
        };
        Ok(Self { address, source })
    }

    pub const fn address(&self) -> u64 {
        self.address
    }

    pub const fn source(&self) -> &LoadBlobSource {
        &self.source
    }

    pub fn source_name(&self) -> String {
        match &self.source {
            LoadBlobSource::Path(path) => path.display().to_string(),
            LoadBlobSource::Resource(resource) => format!("resource:{resource}"),
            LoadBlobSource::SuiteResource(selector) => selector.source_name(),
        }
    }

    pub(super) fn resolve_path(&mut self, base: &Path) {
        let LoadBlobSource::Path(path) = &mut self.source else {
            return;
        };
        if path.is_relative() {
            *path = base.join(&*path);
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadfileRequest {
    base: u64,
    size: u64,
    source: ReadfileSource,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadfileSource {
    Path(PathBuf),
    Resource(String),
    SuiteResource(SuiteResourceSelector),
}

impl ReadfileRequest {
    pub fn parse(value: &str) -> Result<Self, Rem6CliError> {
        let mut parts = value.splitn(3, ':');
        let Some(base) = parts.next() else {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        };
        let Some(size) = parts.next() else {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        };
        let Some(path) = parts.next() else {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        };
        let base = parse_number(base).ok_or_else(|| Rem6CliError::InvalidReadfile {
            value: value.to_string(),
        })?;
        let size = parse_number(size)
            .filter(|bytes| *bytes > 0)
            .ok_or_else(|| Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            })?;
        if path.is_empty() {
            return Err(Rem6CliError::InvalidReadfile {
                value: value.to_string(),
            });
        }
        let source = if let Some(selector) = path.strip_prefix("suite-resource:") {
            SuiteResourceSelector::parse(selector)
                .map(ReadfileSource::SuiteResource)
                .ok_or_else(|| Rem6CliError::InvalidReadfile {
                    value: value.to_string(),
                })?
        } else {
            path.strip_prefix("resource:")
                .map(|resource| {
                    if resource.is_empty() {
                        return Err(Rem6CliError::InvalidReadfile {
                            value: value.to_string(),
                        });
                    }
                    Ok(ReadfileSource::Resource(resource.to_string()))
                })
                .unwrap_or_else(|| Ok(ReadfileSource::Path(PathBuf::from(path))))?
        };
        Ok(Self { base, size, source })
    }

    pub const fn base(&self) -> u64 {
        self.base
    }

    pub const fn size(&self) -> u64 {
        self.size
    }

    pub const fn source(&self) -> &ReadfileSource {
        &self.source
    }

    pub fn source_name(&self) -> String {
        match &self.source {
            ReadfileSource::Path(path) => path.display().to_string(),
            ReadfileSource::Resource(resource) => format!("resource:{resource}"),
            ReadfileSource::SuiteResource(selector) => selector.source_name(),
        }
    }

    pub(super) fn resolve_path(&mut self, base: &Path) {
        let ReadfileSource::Path(path) = &mut self.source else {
            return;
        };
        if path.is_relative() {
            *path = base.join(&path);
        }
    }
}
