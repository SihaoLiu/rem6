use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_pci::{PciError, PciLegacyInterruptRouter, PciLegacyInterruptRouterSnapshot};

const PCI_LEGACY_INTERRUPT_ROUTER_CHUNK: &str = "legacy-intx-router";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciLegacyInterruptRouterCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: PciLegacyInterruptRouterSnapshot,
}

impl PciLegacyInterruptRouterCheckpointRecord {
    pub fn new(
        component: CheckpointComponentId,
        snapshot: PciLegacyInterruptRouterSnapshot,
    ) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &PciLegacyInterruptRouterSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct PciLegacyInterruptRouterCheckpointPort {
    component: CheckpointComponentId,
    router: Arc<Mutex<PciLegacyInterruptRouter>>,
}

impl PciLegacyInterruptRouterCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        router: Arc<Mutex<PciLegacyInterruptRouter>>,
    ) -> Self {
        Self { component, router }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn router(&self) -> Arc<Mutex<PciLegacyInterruptRouter>> {
        Arc::clone(&self.router)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<PciLegacyInterruptRouterCheckpointRecord, CheckpointError> {
        let snapshot = self
            .router
            .lock()
            .expect("PCI legacy interrupt router lock")
            .snapshot();
        registry.write_chunk(
            &self.component,
            PCI_LEGACY_INTERRUPT_ROUTER_CHUNK,
            snapshot.to_bytes(),
        )?;
        Ok(PciLegacyInterruptRouterCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PciLegacyInterruptRouterCheckpointRecord, PciLegacyInterruptRouterCheckpointError>
    {
        let record = self.decode_from(registry)?;
        self.validate_record(&record)?;
        self.restore_record(&record)?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PciLegacyInterruptRouterCheckpointRecord, PciLegacyInterruptRouterCheckpointError>
    {
        let payload = registry
            .chunk(&self.component, PCI_LEGACY_INTERRUPT_ROUTER_CHUNK)
            .ok_or_else(|| PciLegacyInterruptRouterCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PCI_LEGACY_INTERRUPT_ROUTER_CHUNK.to_string(),
            })?;
        let snapshot = PciLegacyInterruptRouterSnapshot::from_bytes(payload).map_err(|error| {
            PciLegacyInterruptRouterCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            }
        })?;
        Ok(PciLegacyInterruptRouterCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_record(
        &self,
        record: &PciLegacyInterruptRouterCheckpointRecord,
    ) -> Result<(), PciLegacyInterruptRouterCheckpointError> {
        self.router
            .lock()
            .expect("PCI legacy interrupt router lock")
            .clone()
            .restore(record.snapshot())
            .map_err(|error| PciLegacyInterruptRouterCheckpointError::Pci {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_record(
        &self,
        record: &PciLegacyInterruptRouterCheckpointRecord,
    ) -> Result<(), PciLegacyInterruptRouterCheckpointError> {
        self.router
            .lock()
            .expect("PCI legacy interrupt router lock")
            .restore(record.snapshot())
            .map_err(|error| PciLegacyInterruptRouterCheckpointError::Pci {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct PciLegacyInterruptRouterCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, PciLegacyInterruptRouterCheckpointPort>,
}

impl PciLegacyInterruptRouterCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = PciLegacyInterruptRouterCheckpointPort>,
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

    pub fn insert_port(
        &mut self,
        port: PciLegacyInterruptRouterCheckpointPort,
    ) -> Result<(), CheckpointError> {
        let component = port.component().clone();
        if self.ports.contains_key(&component) {
            return Err(CheckpointError::DuplicateComponent { component });
        }
        self.ports.insert(component, port);
        Ok(())
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
    ) -> Result<Vec<PciLegacyInterruptRouterCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<
        Vec<PciLegacyInterruptRouterCheckpointRecord>,
        PciLegacyInterruptRouterCheckpointError,
    > {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.validate_record(record)?;
        }
        for (port, record) in self.ports.values().zip(&records) {
            port.restore_record(record)?;
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), PciLegacyInterruptRouterCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_record(&record)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PciLegacyInterruptRouterCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Pci {
        component: CheckpointComponentId,
        error: PciError,
    },
}

impl PciLegacyInterruptRouterCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Pci { component, .. } => component,
        }
    }
}

impl fmt::Display for PciLegacyInterruptRouterCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "PCI legacy interrupt router checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "PCI legacy interrupt router checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Pci { component, error } => write!(
                formatter,
                "PCI legacy interrupt router checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for PciLegacyInterruptRouterCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pci { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}
