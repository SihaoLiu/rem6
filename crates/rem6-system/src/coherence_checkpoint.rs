use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_coherence::{
    HarnessError, MsiBankCycleHistory, MsiBankDirectoryHarness, MsiBankDirectoryHarnessSnapshot,
};

use crate::CheckpointRuntimeState;

const MSI_BANK_CHUNK: &str = "msi-bank";
const MSI_BANK_RUNTIME_STATE_CHUNK: &str = "msi-bank-runtime-state";

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
    runtime_state: Option<CheckpointRuntimeState>,
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
        Self {
            component,
            harness,
            runtime_state: None,
        }
    }

    pub fn with_runtime_state(mut self, runtime_state: CheckpointRuntimeState) -> Self {
        self.runtime_state = Some(runtime_state);
        self
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
        if let Some(runtime_state) = &self.runtime_state {
            let payload =
                runtime_state
                    .capture()
                    .map_err(|reason| MsiBankCheckpointError::RuntimeState {
                        component: self.component.clone(),
                        operation: "capture",
                        reason,
                    })?;
            registry
                .write_chunk(&self.component, MSI_BANK_RUNTIME_STATE_CHUNK, payload)
                .map_err(MsiBankCheckpointError::Checkpoint)?;
        }
        Ok(MsiBankCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<MsiBankCheckpointRecord, MsiBankCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_snapshot(record.snapshot())?;
        self.validate_runtime_state(registry)?;
        self.restore_snapshot(record.snapshot())?;
        self.restore_runtime_state(registry);
        Ok(record)
    }

    fn decode_from(
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
        Ok(MsiBankCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &MsiBankDirectoryHarnessSnapshot,
    ) -> Result<(), MsiBankCheckpointError> {
        let mut harness = self
            .harness
            .lock()
            .expect("MSI bank checkpoint lock")
            .clone();
        harness
            .restore(snapshot)
            .map_err(|error| MsiBankCheckpointError::Harness {
                component: self.component.clone(),
                error: Box::new(error),
            })
    }

    fn runtime_state_payload<'a>(
        &self,
        registry: &'a CheckpointRegistry,
    ) -> Result<Option<&'a [u8]>, MsiBankCheckpointError> {
        let Some(_runtime_state) = &self.runtime_state else {
            return Ok(None);
        };
        registry
            .chunk(&self.component, MSI_BANK_RUNTIME_STATE_CHUNK)
            .map(Some)
            .ok_or_else(|| MsiBankCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: MSI_BANK_RUNTIME_STATE_CHUNK.to_string(),
            })
    }

    fn validate_runtime_state(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), MsiBankCheckpointError> {
        let (Some(runtime_state), Some(payload)) =
            (&self.runtime_state, self.runtime_state_payload(registry)?)
        else {
            return Ok(());
        };
        runtime_state
            .validate(payload)
            .map_err(|reason| MsiBankCheckpointError::RuntimeState {
                component: self.component.clone(),
                operation: "validate restore",
                reason,
            })
    }

    fn restore_runtime_state(&self, registry: &CheckpointRegistry) {
        let Some(runtime_state) = &self.runtime_state else {
            return;
        };
        let payload = registry
            .chunk(&self.component, MSI_BANK_RUNTIME_STATE_CHUNK)
            .expect("validated MSI bank runtime checkpoint state");
        runtime_state.restore(payload);
    }

    fn restore_snapshot(
        &self,
        snapshot: &MsiBankDirectoryHarnessSnapshot,
    ) -> Result<(), MsiBankCheckpointError> {
        self.harness
            .lock()
            .expect("MSI bank checkpoint lock")
            .restore(snapshot)
            .map_err(|error| MsiBankCheckpointError::Harness {
                component: self.component.clone(),
                error: Box::new(error),
            })
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
        let mut by_component = BTreeMap::<CheckpointComponentId, MsiBankCheckpointPort>::new();
        for port in ports {
            let component = port.component().clone();
            if by_component.contains_key(&component) {
                return Err(CheckpointError::DuplicateComponent { component });
            }
            if by_component
                .values()
                .any(|existing| Arc::ptr_eq(&existing.harness, &port.harness))
            {
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
        self.validate_restore_from(registry)?;
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            decoded.push((port, record));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            port.restore_runtime_state(registry);
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), MsiBankCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            port.validate_runtime_state(registry)?;
        }
        Ok(())
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
    RuntimeState {
        component: CheckpointComponentId,
        operation: &'static str,
        reason: String,
    },
}

impl MsiBankCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Harness { component, .. }
            | Self::RuntimeState { component, .. } => Some(component),
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
            Self::RuntimeState {
                component,
                operation,
                reason,
            } => write!(
                formatter,
                "MSI bank checkpoint component {} runtime state {operation} failed: {reason}",
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
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } | Self::RuntimeState { .. } => {
                None
            }
        }
    }
}
