use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_pci::{PciError, PciHostBridge, PciHostBridgeTopologySnapshot};

const PCI_HOST_TOPOLOGY_CHUNK: &str = "host-topology";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PciHostCheckpointRecord {
    component: CheckpointComponentId,
    topology: PciHostBridgeTopologySnapshot,
}

impl PciHostCheckpointRecord {
    pub fn new(component: CheckpointComponentId, topology: PciHostBridgeTopologySnapshot) -> Self {
        Self {
            component,
            topology,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn topology(&self) -> &PciHostBridgeTopologySnapshot {
        &self.topology
    }
}

#[derive(Clone, Debug)]
pub struct PciHostCheckpointPort {
    component: CheckpointComponentId,
    host: Arc<Mutex<PciHostBridge>>,
}

impl PciHostCheckpointPort {
    pub fn new(component: CheckpointComponentId, host: Arc<Mutex<PciHostBridge>>) -> Self {
        Self { component, host }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn host(&self) -> Arc<Mutex<PciHostBridge>> {
        Arc::clone(&self.host)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<PciHostCheckpointRecord, PciHostCheckpointError> {
        let topology = self
            .host
            .lock()
            .expect("PCI host checkpoint lock")
            .topology_snapshot();
        registry
            .write_chunk(
                &self.component,
                PCI_HOST_TOPOLOGY_CHUNK,
                topology.to_bytes(),
            )
            .map_err(PciHostCheckpointError::Checkpoint)?;
        Ok(PciHostCheckpointRecord::new(
            self.component.clone(),
            topology,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PciHostCheckpointRecord, PciHostCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_topology(record.topology())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PciHostCheckpointRecord, PciHostCheckpointError> {
        let payload = registry
            .chunk(&self.component, PCI_HOST_TOPOLOGY_CHUNK)
            .ok_or_else(|| PciHostCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PCI_HOST_TOPOLOGY_CHUNK.to_string(),
            })?;
        let topology = PciHostBridgeTopologySnapshot::from_bytes(payload).map_err(|error| {
            PciHostCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            }
        })?;
        Ok(PciHostCheckpointRecord::new(
            self.component.clone(),
            topology,
        ))
    }

    fn validate_topology(
        &self,
        topology: &PciHostBridgeTopologySnapshot,
    ) -> Result<(), PciHostCheckpointError> {
        self.host
            .lock()
            .expect("PCI host checkpoint lock")
            .validate_topology_snapshot(topology)
            .map_err(|error| PciHostCheckpointError::Pci {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct PciHostCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, PciHostCheckpointPort>,
}

impl PciHostCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = PciHostCheckpointPort>,
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
    ) -> Result<Vec<PciHostCheckpointRecord>, PciHostCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn decode_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<PciHostCheckpointRecord>, PciHostCheckpointError> {
        self.ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<PciHostCheckpointRecord>, PciHostCheckpointError> {
        self.validate_restore_from(registry)?;
        let records = self.decode_all_from(registry)?;
        for record in &records {
            let port = self
                .ports
                .get(record.component())
                .expect("decoded PCI host checkpoint record has registered port");
            port.validate_topology(record.topology())?;
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), PciHostCheckpointError> {
        for record in self.decode_all_from(registry)? {
            let port = self
                .ports
                .get(record.component())
                .expect("decoded PCI host checkpoint record has registered port");
            port.validate_topology(record.topology())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PciHostCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
    Pci {
        component: CheckpointComponentId,
        error: PciError,
    },
}

impl PciHostCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Pci { component, .. } => Some(component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for PciHostCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "PCI host checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "PCI host checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Pci { component, error } => write!(
                formatter,
                "PCI host checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for PciHostCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Pci { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}
