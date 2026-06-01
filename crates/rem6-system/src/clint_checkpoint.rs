use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_memory::Address;
use rem6_timer::{ClintHartSnapshot, ClintMmioDevice, ClintSnapshot, TimerError};

const CLINT_CHUNK: &str = "clint";
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;
const CLINT_HART_RECORD_BYTES: usize = U32_BYTES * 2 + U64_BYTES * 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ClintCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: ClintSnapshot,
}

impl ClintCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: ClintSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &ClintSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct ClintCheckpointPort {
    component: CheckpointComponentId,
    device: ClintMmioDevice,
}

impl ClintCheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: ClintMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> ClintMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<ClintCheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(&self.component, CLINT_CHUNK, encode_clint(&snapshot))?;
        Ok(ClintCheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<ClintCheckpointRecord, ClintCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_snapshot(record.snapshot())?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<ClintCheckpointRecord, ClintCheckpointError> {
        let payload = registry
            .chunk(&self.component, CLINT_CHUNK)
            .ok_or_else(|| ClintCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: CLINT_CHUNK.to_string(),
            })?;
        let snapshot = decode_clint(&self.component, payload)?;
        Ok(ClintCheckpointRecord::new(self.component.clone(), snapshot))
    }

    fn validate_snapshot(&self, snapshot: &ClintSnapshot) -> Result<(), ClintCheckpointError> {
        if snapshot.base() != self.device.base() {
            return Err(ClintCheckpointError::Clint {
                component: self.component.clone(),
                error: TimerError::ClintSnapshotBaseMismatch {
                    expected: self.device.base(),
                    actual: snapshot.base(),
                },
            });
        }

        let expected = self
            .device
            .snapshot()
            .harts()
            .iter()
            .map(ClintHartSnapshot::hart)
            .collect::<Vec<_>>();
        let actual = snapshot
            .harts()
            .iter()
            .map(ClintHartSnapshot::hart)
            .collect::<Vec<_>>();
        if actual != expected {
            return Err(ClintCheckpointError::Clint {
                component: self.component.clone(),
                error: TimerError::ClintSnapshotHartMismatch { expected, actual },
            });
        }

        Ok(())
    }

    fn restore_snapshot(&self, snapshot: &ClintSnapshot) -> Result<(), ClintCheckpointError> {
        self.device
            .restore(snapshot)
            .map_err(|error| ClintCheckpointError::Clint {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct ClintCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, ClintCheckpointPort>,
}

impl ClintCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = ClintCheckpointPort>,
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
    ) -> Result<Vec<ClintCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<ClintCheckpointRecord>, ClintCheckpointError> {
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
    ) -> Result<(), ClintCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_snapshot(record.snapshot())?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClintCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Clint {
        component: CheckpointComponentId,
        error: TimerError,
    },
}

impl ClintCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Clint { component, .. } => component,
        }
    }
}

impl fmt::Display for ClintCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "CLINT checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "CLINT checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Clint { component, error } => write!(
                formatter,
                "CLINT checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for ClintCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Clint { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

fn encode_clint(snapshot: &ClintSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, snapshot.base().get());
    write_u64(&mut payload, snapshot.mtime());
    write_u64(&mut payload, snapshot.harts().len() as u64);
    for hart in snapshot.harts() {
        write_u32(&mut payload, hart.hart());
        write_u32(&mut payload, hart.msip());
        write_u64(&mut payload, hart.mtimecmp());
        write_u64(&mut payload, hart.timer_generation());
        write_bool(&mut payload, hart.timer_asserted());
    }
    payload
}

fn decode_clint(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<ClintSnapshot, ClintCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let base = Address::new(cursor.read_u64("CLINT base")?);
    let mtime = cursor.read_u64("CLINT mtime")?;
    let hart_count = cursor.read_bounded_count("CLINT hart count", CLINT_HART_RECORD_BYTES)?;
    let mut harts = Vec::with_capacity(hart_count);
    for _ in 0..hart_count {
        harts.push(ClintHartSnapshot::new(
            cursor.read_u32("CLINT hart id")?,
            cursor.read_u32("CLINT msip")?,
            cursor.read_u64("CLINT mtimecmp")?,
            cursor.read_u64("CLINT timer generation")?,
            cursor.read_bool("CLINT timer asserted")?,
        ));
    }
    cursor.finish()?;
    Ok(ClintSnapshot::with_mtime(base, mtime, harts))
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_bool(payload: &mut Vec<u8>, value: bool) {
    write_u64(payload, u64::from(value));
}

struct PayloadCursor<'a> {
    component: CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    const fn new(component: CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, ClintCheckpointError> {
        let bytes = self.read_bytes(name, U64_BYTES)?;
        Ok(u64::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u64 slice width is fixed"),
        ))
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, ClintCheckpointError> {
        let bytes = self.read_bytes(name, U32_BYTES)?;
        Ok(u32::from_le_bytes(
            bytes
                .try_into()
                .expect("checkpoint u32 slice width is fixed"),
        ))
    }

    fn read_bool(&mut self, name: &str) -> Result<bool, ClintCheckpointError> {
        match self.read_u64(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} has invalid bool value {value}"))),
        }
    }

    fn read_count(&mut self, name: &str) -> Result<usize, ClintCheckpointError> {
        let count = self.read_u64(name)?;
        usize::try_from(count).map_err(|_| self.invalid(format!("{name} is too large: {count}")))
    }

    fn read_bounded_count(
        &mut self,
        name: &str,
        record_bytes: usize,
    ) -> Result<usize, ClintCheckpointError> {
        let count = self.read_count(name)?;
        let capacity = self.remaining() / record_bytes;
        if count > capacity {
            return Err(self.invalid(format!(
                "{name} {count} exceeds remaining payload capacity {capacity} records"
            )));
        }
        Ok(count)
    }

    fn remaining(&self) -> usize {
        self.payload.len().saturating_sub(self.offset)
    }

    fn read_bytes(&mut self, name: &str, size: usize) -> Result<&'a [u8], ClintCheckpointError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| self.invalid(format!("{name} offset overflow")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!("{name} is truncated")));
        }
        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), ClintCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "{} trailing bytes",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> ClintCheckpointError {
        ClintCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
