use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_coherence::{
    HarnessError, MsiBankCycleHistory, MsiBankDirectoryHarness, MsiBankDirectoryHarnessSnapshot,
};

const MSI_BANK_CHUNK: &str = "msi-bank";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MsiBankCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: MsiBankDirectoryHarnessSnapshot,
}

impl MsiBankCheckpointRecord {
    pub fn new(
        component: CheckpointComponentId,
        snapshot: MsiBankDirectoryHarnessSnapshot,
    ) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &MsiBankDirectoryHarnessSnapshot {
        &self.snapshot
    }

    pub fn parallel_cycle_history(&self) -> MsiBankCycleHistory {
        self.snapshot.parallel_cycle_history()
    }
}

#[derive(Clone)]
pub struct MsiBankCheckpointPort {
    component: CheckpointComponentId,
    harness: Arc<Mutex<MsiBankDirectoryHarness>>,
}

impl fmt::Debug for MsiBankCheckpointPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MsiBankCheckpointPort")
            .field("component", &self.component)
            .finish_non_exhaustive()
    }
}

impl MsiBankCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        harness: Arc<Mutex<MsiBankDirectoryHarness>>,
    ) -> Self {
        Self { component, harness }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn harness(&self) -> Arc<Mutex<MsiBankDirectoryHarness>> {
        Arc::clone(&self.harness)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<MsiBankCheckpointRecord, MsiBankCheckpointError> {
        let snapshot = self
            .harness
            .lock()
            .expect("MSI bank checkpoint lock")
            .snapshot();
        registry
            .write_chunk(&self.component, MSI_BANK_CHUNK, snapshot.to_bytes())
            .map_err(MsiBankCheckpointError::Checkpoint)?;
        Ok(MsiBankCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<MsiBankCheckpointRecord, MsiBankCheckpointError> {
        let payload = registry
            .chunk(&self.component, MSI_BANK_CHUNK)
            .ok_or_else(|| MsiBankCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: MSI_BANK_CHUNK.to_string(),
            })?;
        let snapshot = MsiBankDirectoryHarnessSnapshot::from_bytes(payload).map_err(|reason| {
            MsiBankCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason,
            }
        })?;
        self.harness
            .lock()
            .expect("MSI bank checkpoint lock")
            .restore(&snapshot)
            .map_err(|error| MsiBankCheckpointError::Harness {
                component: self.component.clone(),
                error: Box::new(error),
            })?;
        Ok(MsiBankCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }
}

#[derive(Clone, Debug, Default)]
pub struct MsiBankCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, MsiBankCheckpointPort>,
}

impl MsiBankCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = MsiBankCheckpointPort>,
    {
        let mut by_component = BTreeMap::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            by_component.insert(component, port);
        }
        Ok(Self {
            ports: by_component,
        })
    }

    pub fn component_count(&self) -> usize {
        self.ports.len()
    }

    pub fn components(&self) -> Vec<CheckpointComponentId> {
        self.ports.keys().cloned().collect()
    }

    pub fn register_all(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        for port in self.ports.values() {
            port.register(registry)?;
        }
        Ok(())
    }

    pub fn capture_all_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Vec<MsiBankCheckpointRecord>, MsiBankCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<MsiBankCheckpointRecord>, MsiBankCheckpointError> {
        self.ports
            .values()
            .map(|port| port.restore_from(registry))
            .collect()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MsiBankCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
    Harness {
        component: CheckpointComponentId,
        error: Box<HarnessError>,
    },
}

impl MsiBankCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Harness { component, .. } => Some(component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for MsiBankCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "MSI bank checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "MSI bank checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Harness { component, error } => write!(
                formatter,
                "MSI bank checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for MsiBankCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Harness { error, .. } => Some(error.as_ref()),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}
