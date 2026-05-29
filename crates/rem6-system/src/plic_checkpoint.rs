use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_interrupt::{
    InterruptLineId, InterruptPriority, InterruptTargetId, PlicContextSnapshot, PlicError,
    PlicMmioDevice, PlicSnapshot,
};
use rem6_kernel::PartitionId;
use rem6_memory::Address;

const PLIC_CHUNK: &str = "plic";
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlicCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: PlicSnapshot,
}

impl PlicCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: PlicSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &PlicSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct PlicCheckpointPort {
    component: CheckpointComponentId,
    device: PlicMmioDevice,
}

impl PlicCheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: PlicMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> PlicMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<PlicCheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(&self.component, PLIC_CHUNK, encode_plic(&snapshot))?;
        Ok(PlicCheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PlicCheckpointRecord, PlicCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_snapshot(record.snapshot())?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<PlicCheckpointRecord, PlicCheckpointError> {
        let payload = registry.chunk(&self.component, PLIC_CHUNK).ok_or_else(|| {
            PlicCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PLIC_CHUNK.to_string(),
            }
        })?;
        let snapshot = decode_plic(&self.component, payload)?;
        Ok(PlicCheckpointRecord::new(self.component.clone(), snapshot))
    }

    fn validate_snapshot(&self, snapshot: &PlicSnapshot) -> Result<(), PlicCheckpointError> {
        self.device
            .validate_snapshot(snapshot)
            .map_err(|error| PlicCheckpointError::Plic {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(&self, snapshot: &PlicSnapshot) -> Result<(), PlicCheckpointError> {
        self.device
            .restore(snapshot)
            .map_err(|error| PlicCheckpointError::Plic {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct PlicCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, PlicCheckpointPort>,
}

impl PlicCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = PlicCheckpointPort>,
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
    ) -> Result<Vec<PlicCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<PlicCheckpointRecord>, PlicCheckpointError> {
        self.validate_restore_from(registry)?;
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
            decoded.push((port, record));
        }

        let mut records = Vec::with_capacity(decoded.len());
        for (port, record) in decoded {
            port.restore_snapshot(record.snapshot())?;
            records.push(record);
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), PlicCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlicCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Plic {
        component: CheckpointComponentId,
        error: PlicError,
    },
}

impl PlicCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Plic { component, .. } => component,
        }
    }
}

impl fmt::Display for PlicCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "PLIC checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "PLIC checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Plic { component, error } => write!(
                formatter,
                "PLIC checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for PlicCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Plic { error, .. } => Some(error),
            _ => None,
        }
    }
}

fn encode_plic(snapshot: &PlicSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, snapshot.base().get());
    write_u64(&mut payload, snapshot.contexts().len() as u64);
    for context in snapshot.contexts() {
        write_u64(&mut payload, context.context());
        write_u32(&mut payload, context.target().get());
        write_u32(&mut payload, context.target_partition().index());
        write_u32(&mut payload, context.threshold().get());
        write_u64(&mut payload, context.enabled().len() as u64);
        for line in context.enabled() {
            write_u64(&mut payload, line.get());
        }
    }
    payload
}

fn decode_plic(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<PlicSnapshot, PlicCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let base = Address::new(cursor.read_u64("PLIC base")?);
    let contexts = read_contexts(&mut cursor)?;
    cursor.finish()?;
    Ok(PlicSnapshot::new(base, contexts))
}

fn read_contexts(
    cursor: &mut PayloadCursor<'_>,
) -> Result<Vec<PlicContextSnapshot>, PlicCheckpointError> {
    let count = cursor.read_count("PLIC context count")?;
    let mut contexts = Vec::with_capacity(count);
    for _ in 0..count {
        let context = cursor.read_u64("PLIC context id")?;
        let target = InterruptTargetId::new(cursor.read_u32("PLIC context target")?);
        let target_partition = PartitionId::new(cursor.read_u32("PLIC context target partition")?);
        let threshold = InterruptPriority::new(cursor.read_u32("PLIC context threshold")?);
        let enabled_count = cursor.read_count("PLIC enabled line count")?;
        let mut enabled = Vec::with_capacity(enabled_count);
        for _ in 0..enabled_count {
            enabled.push(InterruptLineId::new(cursor.read_u64("PLIC enabled line")?));
        }
        contexts.push(PlicContextSnapshot::new(
            context,
            target,
            target_partition,
            enabled,
            threshold,
        ));
    }
    Ok(contexts)
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_count(&mut self, name: &str) -> Result<usize, PlicCheckpointError> {
        let value = self.read_u64(name)?;
        usize::try_from(value).map_err(|_| self.invalid(format!("{name} {value} overflows usize")))
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, PlicCheckpointError> {
        let bytes = self.read_exact(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("u32 bytes")))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, PlicCheckpointError> {
        let bytes = self.read_exact(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("u64 bytes")))
    }

    fn finish(self) -> Result<(), PlicCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }

        Err(PlicCheckpointError::InvalidChunk {
            component: self.component,
            reason: format!(
                "trailing {} bytes after PLIC checkpoint payload",
                self.payload.len() - self.offset
            ),
        })
    }

    fn read_exact(&mut self, name: &str, bytes: usize) -> Result<&'a [u8], PlicCheckpointError> {
        let end = self.offset.checked_add(bytes).ok_or_else(|| {
            self.invalid(format!(
                "{name} offset overflow while reading {bytes} bytes"
            ))
        })?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} expected {bytes} bytes at offset {}, payload has {} bytes",
                self.offset,
                self.payload.len()
            )));
        }

        let slice = &self.payload[self.offset..end];
        self.offset = end;
        Ok(slice)
    }

    fn invalid(&self, reason: String) -> PlicCheckpointError {
        PlicCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
