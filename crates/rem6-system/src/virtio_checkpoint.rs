use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_memory::Address;
use rem6_virtio::{
    VirtioError, VirtioPciDeviceConfigDevice, VirtioPciDeviceConfigSnapshot, VirtioPciIsrDevice,
    VirtioPciIsrSnapshot, VirtioPciNotifyDevice, VirtioPciNotifySnapshot, VirtioSplitQueue,
    VirtioSplitQueueSnapshot,
};

const VIRTIO_PCI_DEVICE_CONFIG_CHUNK: &str = "device-config";
const VIRTIO_PCI_ISR_CHUNK: &str = "pci-isr";
const VIRTIO_PCI_NOTIFY_CHUNK: &str = "pci-notify";
const VIRTIO_SPLIT_QUEUE_CHUNK: &str = "split-queue";
const U16_BYTES: usize = 2;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioSplitQueueCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: VirtioSplitQueueSnapshot,
}

impl VirtioSplitQueueCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: VirtioSplitQueueSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &VirtioSplitQueueSnapshot {
        &self.snapshot
    }
}

#[derive(Clone)]
pub struct VirtioSplitQueueCheckpointPort {
    component: CheckpointComponentId,
    queue: Arc<Mutex<VirtioSplitQueue>>,
}

impl fmt::Debug for VirtioSplitQueueCheckpointPort {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("VirtioSplitQueueCheckpointPort")
            .field("component", &self.component)
            .finish_non_exhaustive()
    }
}

impl VirtioSplitQueueCheckpointPort {
    pub fn new(component: CheckpointComponentId, queue: Arc<Mutex<VirtioSplitQueue>>) -> Self {
        Self { component, queue }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn queue(&self) -> Arc<Mutex<VirtioSplitQueue>> {
        Arc::clone(&self.queue)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<VirtioSplitQueueCheckpointRecord, VirtioSplitQueueCheckpointError> {
        let snapshot = self
            .queue
            .lock()
            .expect("VirtIO split queue checkpoint lock")
            .snapshot();
        registry
            .write_chunk(
                &self.component,
                VIRTIO_SPLIT_QUEUE_CHUNK,
                encode_split_queue(&snapshot),
            )
            .map_err(VirtioSplitQueueCheckpointError::Checkpoint)?;
        Ok(VirtioSplitQueueCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioSplitQueueCheckpointRecord, VirtioSplitQueueCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioSplitQueueCheckpointRecord, VirtioSplitQueueCheckpointError> {
        let payload = registry
            .chunk(&self.component, VIRTIO_SPLIT_QUEUE_CHUNK)
            .ok_or_else(|| VirtioSplitQueueCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: VIRTIO_SPLIT_QUEUE_CHUNK.to_string(),
            })?;
        let snapshot = decode_split_queue(&self.component, payload)?;
        Ok(VirtioSplitQueueCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn validate_snapshot(
        &self,
        snapshot: &VirtioSplitQueueSnapshot,
    ) -> Result<(), VirtioSplitQueueCheckpointError> {
        self.queue
            .lock()
            .expect("VirtIO split queue checkpoint lock")
            .validate_snapshot_shape(snapshot)
            .map_err(|error| VirtioSplitQueueCheckpointError::Virtio {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &VirtioSplitQueueSnapshot,
    ) -> Result<(), VirtioSplitQueueCheckpointError> {
        self.queue
            .lock()
            .expect("VirtIO split queue checkpoint lock")
            .restore(snapshot)
            .map_err(|error| VirtioSplitQueueCheckpointError::Virtio {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct VirtioSplitQueueCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, VirtioSplitQueueCheckpointPort>,
}

impl VirtioSplitQueueCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = VirtioSplitQueueCheckpointPort>,
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
    ) -> Result<Vec<VirtioSplitQueueCheckpointRecord>, VirtioSplitQueueCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<VirtioSplitQueueCheckpointRecord>, VirtioSplitQueueCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            decoded.push((port, record));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            restored.push(record);
        }
        Ok(restored)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), VirtioSplitQueueCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioSplitQueueCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
    Virtio {
        component: CheckpointComponentId,
        error: VirtioError,
    },
}

impl VirtioSplitQueueCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Virtio { component, .. } => Some(component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for VirtioSplitQueueCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "VirtIO split queue checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "VirtIO split queue checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Virtio { component, error } => write!(
                formatter,
                "VirtIO split queue checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for VirtioSplitQueueCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Virtio { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciIsrCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: VirtioPciIsrSnapshot,
}

impl VirtioPciIsrCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: VirtioPciIsrSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &VirtioPciIsrSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct VirtioPciIsrCheckpointPort {
    component: CheckpointComponentId,
    isr: VirtioPciIsrDevice,
}

impl VirtioPciIsrCheckpointPort {
    pub fn new(component: CheckpointComponentId, isr: VirtioPciIsrDevice) -> Self {
        Self { component, isr }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn isr(&self) -> VirtioPciIsrDevice {
        self.isr.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<VirtioPciIsrCheckpointRecord, VirtioPciIsrCheckpointError> {
        let snapshot = self.isr.snapshot();
        registry
            .write_chunk(&self.component, VIRTIO_PCI_ISR_CHUNK, snapshot.to_bytes())
            .map_err(VirtioPciIsrCheckpointError::Checkpoint)?;
        Ok(VirtioPciIsrCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioPciIsrCheckpointRecord, VirtioPciIsrCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_record(&record);
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioPciIsrCheckpointRecord, VirtioPciIsrCheckpointError> {
        let payload = registry
            .chunk(&self.component, VIRTIO_PCI_ISR_CHUNK)
            .ok_or_else(|| VirtioPciIsrCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: VIRTIO_PCI_ISR_CHUNK.to_string(),
            })?;
        let snapshot = VirtioPciIsrSnapshot::from_bytes(payload).map_err(|error| {
            VirtioPciIsrCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            }
        })?;
        Ok(VirtioPciIsrCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn restore_record(&self, record: &VirtioPciIsrCheckpointRecord) {
        self.isr.restore(record.snapshot());
    }
}

#[derive(Clone, Debug, Default)]
pub struct VirtioPciIsrCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, VirtioPciIsrCheckpointPort>,
}

impl VirtioPciIsrCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = VirtioPciIsrCheckpointPort>,
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
    ) -> Result<Vec<VirtioPciIsrCheckpointRecord>, VirtioPciIsrCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<VirtioPciIsrCheckpointRecord>, VirtioPciIsrCheckpointError> {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.restore_record(record);
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), VirtioPciIsrCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioPciIsrCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
}

impl VirtioPciIsrCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                Some(component)
            }
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for VirtioPciIsrCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "VirtIO PCI ISR checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "VirtIO PCI ISR checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for VirtioPciIsrCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciNotifyCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: VirtioPciNotifySnapshot,
}

impl VirtioPciNotifyCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: VirtioPciNotifySnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &VirtioPciNotifySnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct VirtioPciNotifyCheckpointPort {
    component: CheckpointComponentId,
    notify: VirtioPciNotifyDevice,
}

impl VirtioPciNotifyCheckpointPort {
    pub fn new(component: CheckpointComponentId, notify: VirtioPciNotifyDevice) -> Self {
        Self { component, notify }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn notify(&self) -> VirtioPciNotifyDevice {
        self.notify.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<VirtioPciNotifyCheckpointRecord, VirtioPciNotifyCheckpointError> {
        let snapshot = self.notify.snapshot();
        registry
            .write_chunk(
                &self.component,
                VIRTIO_PCI_NOTIFY_CHUNK,
                snapshot.to_bytes(),
            )
            .map_err(VirtioPciNotifyCheckpointError::Checkpoint)?;
        Ok(VirtioPciNotifyCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioPciNotifyCheckpointRecord, VirtioPciNotifyCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_record(&record);
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioPciNotifyCheckpointRecord, VirtioPciNotifyCheckpointError> {
        let payload = registry
            .chunk(&self.component, VIRTIO_PCI_NOTIFY_CHUNK)
            .ok_or_else(|| VirtioPciNotifyCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: VIRTIO_PCI_NOTIFY_CHUNK.to_string(),
            })?;
        let snapshot = VirtioPciNotifySnapshot::from_bytes(payload).map_err(|error| {
            VirtioPciNotifyCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            }
        })?;
        Ok(VirtioPciNotifyCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn restore_record(&self, record: &VirtioPciNotifyCheckpointRecord) {
        self.notify.restore(record.snapshot());
    }
}

#[derive(Clone, Debug, Default)]
pub struct VirtioPciNotifyCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, VirtioPciNotifyCheckpointPort>,
}

impl VirtioPciNotifyCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = VirtioPciNotifyCheckpointPort>,
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
    ) -> Result<Vec<VirtioPciNotifyCheckpointRecord>, VirtioPciNotifyCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<VirtioPciNotifyCheckpointRecord>, VirtioPciNotifyCheckpointError> {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.restore_record(record);
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), VirtioPciNotifyCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioPciNotifyCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
}

impl VirtioPciNotifyCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                Some(component)
            }
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for VirtioPciNotifyCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "VirtIO PCI notify checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "VirtIO PCI notify checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for VirtioPciNotifyCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VirtioPciDeviceConfigCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: VirtioPciDeviceConfigSnapshot,
}

impl VirtioPciDeviceConfigCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: VirtioPciDeviceConfigSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &VirtioPciDeviceConfigSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct VirtioPciDeviceConfigCheckpointPort {
    component: CheckpointComponentId,
    config: VirtioPciDeviceConfigDevice,
}

impl VirtioPciDeviceConfigCheckpointPort {
    pub fn new(component: CheckpointComponentId, config: VirtioPciDeviceConfigDevice) -> Self {
        Self { component, config }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn config(&self) -> VirtioPciDeviceConfigDevice {
        self.config.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<VirtioPciDeviceConfigCheckpointRecord, VirtioPciDeviceConfigCheckpointError> {
        let snapshot = self.config.snapshot();
        registry
            .write_chunk(
                &self.component,
                VIRTIO_PCI_DEVICE_CONFIG_CHUNK,
                snapshot.to_bytes(),
            )
            .map_err(VirtioPciDeviceConfigCheckpointError::Checkpoint)?;
        Ok(VirtioPciDeviceConfigCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioPciDeviceConfigCheckpointRecord, VirtioPciDeviceConfigCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_record(&record);
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<VirtioPciDeviceConfigCheckpointRecord, VirtioPciDeviceConfigCheckpointError> {
        let payload = registry
            .chunk(&self.component, VIRTIO_PCI_DEVICE_CONFIG_CHUNK)
            .ok_or_else(|| VirtioPciDeviceConfigCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: VIRTIO_PCI_DEVICE_CONFIG_CHUNK.to_string(),
            })?;
        let snapshot = VirtioPciDeviceConfigSnapshot::from_bytes(payload).map_err(|error| {
            VirtioPciDeviceConfigCheckpointError::InvalidChunk {
                component: self.component.clone(),
                reason: error.to_string(),
            }
        })?;
        Ok(VirtioPciDeviceConfigCheckpointRecord::new(
            self.component.clone(),
            snapshot,
        ))
    }

    fn restore_record(&self, record: &VirtioPciDeviceConfigCheckpointRecord) {
        self.config.restore(record.snapshot());
    }
}

#[derive(Clone, Debug, Default)]
pub struct VirtioPciDeviceConfigCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, VirtioPciDeviceConfigCheckpointPort>,
}

impl VirtioPciDeviceConfigCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = VirtioPciDeviceConfigCheckpointPort>,
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
    ) -> Result<Vec<VirtioPciDeviceConfigCheckpointRecord>, VirtioPciDeviceConfigCheckpointError>
    {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<VirtioPciDeviceConfigCheckpointRecord>, VirtioPciDeviceConfigCheckpointError>
    {
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.restore_record(record);
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), VirtioPciDeviceConfigCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VirtioPciDeviceConfigCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
}

impl VirtioPciDeviceConfigCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. } | Self::InvalidChunk { component, .. } => {
                Some(component)
            }
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for VirtioPciDeviceConfigCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "VirtIO PCI device config checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "VirtIO PCI device config checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
        }
    }
}

impl Error for VirtioPciDeviceConfigCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

fn encode_split_queue(snapshot: &VirtioSplitQueueSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u16(&mut payload, snapshot.queue_size());
    write_u64(&mut payload, snapshot.descriptor_table().get());
    write_u64(&mut payload, snapshot.available_ring().get());
    write_u64(&mut payload, snapshot.used_ring().get());
    write_u16(&mut payload, snapshot.last_available_index());
    payload.push(u8::from(snapshot.event_index_enabled()));
    payload
}

fn decode_split_queue(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<VirtioSplitQueueSnapshot, VirtioSplitQueueCheckpointError> {
    let mut cursor = VirtioSplitQueueCheckpointCursor::new(component.clone(), payload);
    let queue_size = cursor.read_u16("queue size")?;
    let descriptor_table = Address::new(cursor.read_u64("descriptor table")?);
    let available_ring = Address::new(cursor.read_u64("available ring")?);
    let used_ring = Address::new(cursor.read_u64("used ring")?);
    let last_available_index = cursor.read_u16("last available index")?;
    let event_index = cursor.read_bool("event index")?;
    cursor.finish()?;
    VirtioSplitQueueSnapshot::new(
        queue_size,
        descriptor_table,
        available_ring,
        used_ring,
        last_available_index,
        event_index,
    )
    .map_err(|error| VirtioSplitQueueCheckpointError::InvalidChunk {
        component: component.clone(),
        reason: error.to_string(),
    })
}

fn write_u16(payload: &mut Vec<u8>, value: u16) {
    payload.extend(value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend(value.to_le_bytes());
}

struct VirtioSplitQueueCheckpointCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> VirtioSplitQueueCheckpointCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_u16(&mut self, name: &str) -> Result<u16, VirtioSplitQueueCheckpointError> {
        let bytes = self.read_exact(name, U16_BYTES)?;
        Ok(u16::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, VirtioSplitQueueCheckpointError> {
        let bytes = self.read_exact(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(bytes.try_into().unwrap()))
    }

    fn read_bool(&mut self, name: &str) -> Result<bool, VirtioSplitQueueCheckpointError> {
        let bytes = self.read_exact(name, 1)?;
        match bytes[0] {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} has invalid bool value {value}"))),
        }
    }

    fn finish(&self) -> Result<(), VirtioSplitQueueCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "payload has {} trailing bytes",
            self.payload.len() - self.offset
        )))
    }

    fn read_exact(
        &mut self,
        name: &str,
        bytes: usize,
    ) -> Result<&'a [u8], VirtioSplitQueueCheckpointError> {
        let end = self
            .offset
            .checked_add(bytes)
            .ok_or_else(|| self.invalid(format!("{name} offset overflow")))?;
        let chunk = self
            .payload
            .get(self.offset..end)
            .ok_or_else(|| self.invalid(format!("{name} is truncated")))?;
        self.offset = end;
        Ok(chunk)
    }

    fn invalid(&self, reason: String) -> VirtioSplitQueueCheckpointError {
        VirtioSplitQueueCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
