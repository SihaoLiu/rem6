use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_net::{
    SinicError, SinicFifoDevice, SinicFifoDeviceSnapshot, SinicRegisterBlock,
    SinicRegisterBlockSnapshot,
};

const SINIC_REGISTER_CHUNK: &str = "sinic-register";
const SINIC_FIFO_CHUNK: &str = "sinic-fifo";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicRegisterCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: SinicRegisterBlockSnapshot,
}

impl SinicRegisterCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: SinicRegisterBlockSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &SinicRegisterBlockSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct SinicRegisterCheckpointPort {
    component: CheckpointComponentId,
    registers: Arc<Mutex<SinicRegisterBlock>>,
}

impl SinicRegisterCheckpointPort {
    pub fn new(
        component: CheckpointComponentId,
        registers: Arc<Mutex<SinicRegisterBlock>>,
    ) -> Self {
        Self {
            component,
            registers,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn registers(&self) -> Arc<Mutex<SinicRegisterBlock>> {
        Arc::clone(&self.registers)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<SinicRegisterCheckpointRecord, CheckpointError> {
        let snapshot = self
            .registers
            .lock()
            .expect("SINIC register block lock")
            .snapshot();
        registry.write_chunk(
            &self.component,
            SINIC_REGISTER_CHUNK,
            snapshot.encode_checkpoint_payload(),
        )?;
        Ok(SinicRegisterCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<SinicRegisterCheckpointRecord, SinicRegisterCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_record(&record)?;
        self.restore_record(&record)?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<SinicRegisterCheckpointRecord, SinicRegisterCheckpointError> {
        let payload = registry
            .chunk(&self.component, SINIC_REGISTER_CHUNK)
            .ok_or_else(|| SinicRegisterCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: SINIC_REGISTER_CHUNK.to_string(),
            })?;
        let snapshot =
            SinicRegisterBlockSnapshot::decode_checkpoint_payload(payload).map_err(|error| {
                SinicRegisterCheckpointError::InvalidChunk {
                    component: self.component.clone(),
                    reason: error.to_string(),
                }
            })?;
        Ok(SinicRegisterCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_record(
        &self,
        record: &SinicRegisterCheckpointRecord,
    ) -> Result<(), SinicRegisterCheckpointError> {
        self.registers
            .lock()
            .expect("SINIC register block lock")
            .clone()
            .restore(record.snapshot())
            .map_err(|error| SinicRegisterCheckpointError::Sinic {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_record(
        &self,
        record: &SinicRegisterCheckpointRecord,
    ) -> Result<(), SinicRegisterCheckpointError> {
        self.registers
            .lock()
            .expect("SINIC register block lock")
            .restore(record.snapshot())
            .map_err(|error| SinicRegisterCheckpointError::Sinic {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct SinicRegisterCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, SinicRegisterCheckpointPort>,
}

impl SinicRegisterCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = SinicRegisterCheckpointPort>,
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
        port: SinicRegisterCheckpointPort,
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
    ) -> Result<Vec<SinicRegisterCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<SinicRegisterCheckpointRecord>, SinicRegisterCheckpointError> {
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
    ) -> Result<(), SinicRegisterCheckpointError> {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.validate_record(record)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SinicRegisterCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Sinic {
        component: CheckpointComponentId,
        error: SinicError,
    },
}

impl SinicRegisterCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Sinic { component, .. } => component,
        }
    }
}

impl fmt::Display for SinicRegisterCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "SINIC register checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "SINIC register checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Sinic { component, error } => write!(
                formatter,
                "SINIC register checkpoint component {} cannot restore snapshot: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for SinicRegisterCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sinic { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SinicFifoCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: SinicFifoDeviceSnapshot,
}

impl SinicFifoCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: SinicFifoDeviceSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &SinicFifoDeviceSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct SinicFifoCheckpointPort {
    component: CheckpointComponentId,
    device: Arc<Mutex<SinicFifoDevice>>,
}

impl SinicFifoCheckpointPort {
    pub fn new(component: CheckpointComponentId, device: Arc<Mutex<SinicFifoDevice>>) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> Arc<Mutex<SinicFifoDevice>> {
        Arc::clone(&self.device)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<SinicFifoCheckpointRecord, CheckpointError> {
        let snapshot = self
            .device
            .lock()
            .expect("SINIC FIFO device lock")
            .snapshot();
        registry.write_chunk(
            &self.component,
            SINIC_FIFO_CHUNK,
            snapshot.encode_checkpoint_payload(),
        )?;
        Ok(SinicFifoCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<SinicFifoCheckpointRecord, SinicFifoCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_record(&record)?;
        self.restore_record(&record)?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<SinicFifoCheckpointRecord, SinicFifoCheckpointError> {
        let payload = registry
            .chunk(&self.component, SINIC_FIFO_CHUNK)
            .ok_or_else(|| SinicFifoCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: SINIC_FIFO_CHUNK.to_string(),
            })?;
        let snapshot =
            SinicFifoDeviceSnapshot::decode_checkpoint_payload(payload).map_err(|error| {
                SinicFifoCheckpointError::InvalidChunk {
                    component: self.component.clone(),
                    reason: error.to_string(),
                }
            })?;
        Ok(SinicFifoCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_record(
        &self,
        record: &SinicFifoCheckpointRecord,
    ) -> Result<(), SinicFifoCheckpointError> {
        self.device
            .lock()
            .expect("SINIC FIFO device lock")
            .clone()
            .restore(record.snapshot())
            .map_err(|error| SinicFifoCheckpointError::Sinic {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_record(
        &self,
        record: &SinicFifoCheckpointRecord,
    ) -> Result<(), SinicFifoCheckpointError> {
        self.device
            .lock()
            .expect("SINIC FIFO device lock")
            .restore(record.snapshot())
            .map_err(|error| SinicFifoCheckpointError::Sinic {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct SinicFifoCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, SinicFifoCheckpointPort>,
}

impl SinicFifoCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = SinicFifoCheckpointPort>,
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

    pub fn insert_port(&mut self, port: SinicFifoCheckpointPort) -> Result<(), CheckpointError> {
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
    ) -> Result<Vec<SinicFifoCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<SinicFifoCheckpointRecord>, SinicFifoCheckpointError> {
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
    ) -> Result<(), SinicFifoCheckpointError> {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.validate_record(record)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SinicFifoCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Sinic {
        component: CheckpointComponentId,
        error: SinicError,
    },
}

impl SinicFifoCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Sinic { component, .. } => component,
        }
    }
}

impl fmt::Display for SinicFifoCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "SINIC FIFO checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "SINIC FIFO checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Sinic { component, error } => write!(
                formatter,
                "SINIC FIFO checkpoint component {} cannot restore snapshot: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for SinicFifoCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sinic { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}
