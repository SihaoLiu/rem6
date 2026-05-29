use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_memory::Address;
use rem6_timer::{
    Sp805Error, Sp805Watchdog, Sp805WatchdogMmioDevice, Sp805WatchdogMmioSnapshot,
    Sp805WatchdogSnapshot, Sp805WatchdogSnapshotFields,
};

const SP805_CHUNK: &str = "sp805";
const U8_BYTES: usize = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp805CheckpointRecord {
    component: CheckpointComponentId,
    snapshot: Sp805WatchdogMmioSnapshot,
}

impl Sp805CheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: Sp805WatchdogMmioSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &Sp805WatchdogMmioSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct Sp805CheckpointPort {
    component: CheckpointComponentId,
    device: Sp805WatchdogMmioDevice,
}

impl Sp805CheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: Sp805WatchdogMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> Sp805WatchdogMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Sp805CheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(&self.component, SP805_CHUNK, encode_sp805(&snapshot))?;
        Ok(Sp805CheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Sp805CheckpointRecord, Sp805CheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Sp805CheckpointRecord, Sp805CheckpointError> {
        let payload = registry
            .chunk(&self.component, SP805_CHUNK)
            .ok_or_else(|| Sp805CheckpointError::MissingChunk {
                component: self.component.clone(),
                name: SP805_CHUNK.to_string(),
            })?;
        let snapshot = decode_sp805(&self.component, payload)?;
        self.validate_snapshot(&snapshot)?;
        Ok(Sp805CheckpointRecord::new(self.component.clone(), snapshot))
    }

    fn validate_snapshot(
        &self,
        snapshot: &Sp805WatchdogMmioSnapshot,
    ) -> Result<(), Sp805CheckpointError> {
        let probe = Sp805WatchdogMmioDevice::new(Address::new(0), default_sp805());
        probe
            .restore(snapshot)
            .map_err(|error| Sp805CheckpointError::Sp805 {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &Sp805WatchdogMmioSnapshot,
    ) -> Result<(), Sp805CheckpointError> {
        self.device
            .restore(snapshot)
            .map_err(|error| Sp805CheckpointError::Sp805 {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Sp805CheckpointBank {
    ports: BTreeMap<CheckpointComponentId, Sp805CheckpointPort>,
}

impl Sp805CheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = Sp805CheckpointPort>,
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
    ) -> Result<Vec<Sp805CheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<Sp805CheckpointRecord>, Sp805CheckpointError> {
        self.validate_restore_from(registry)?;
        let records = self
            .ports
            .values()
            .map(|port| port.decode_from(registry))
            .collect::<Result<Vec<_>, _>>()?;
        for (port, record) in self.ports.values().zip(&records) {
            port.restore_snapshot(record.snapshot())?;
        }
        Ok(records)
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), Sp805CheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sp805CheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Sp805 {
        component: CheckpointComponentId,
        error: Sp805Error,
    },
}

impl Sp805CheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Sp805 { component, .. } => component,
        }
    }
}

impl fmt::Display for Sp805CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "SP805 checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "SP805 checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Sp805 { component, error } => write!(
                formatter,
                "SP805 checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for Sp805CheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sp805 { error, .. } => Some(error),
            _ => None,
        }
    }
}

fn encode_sp805(snapshot: &Sp805WatchdogMmioSnapshot) -> Vec<u8> {
    let watchdog = snapshot.watchdog();
    let mut payload = Vec::new();
    write_u32(&mut payload, watchdog.timeout_interval());
    write_bool(&mut payload, watchdog.timeout_start_tick().is_some());
    if let Some(tick) = watchdog.timeout_start_tick() {
        write_u64(&mut payload, tick);
    }
    write_u32(&mut payload, watchdog.persisted_value());
    write_bool(&mut payload, watchdog.enabled());
    write_bool(&mut payload, watchdog.reset_enabled());
    write_bool(&mut payload, watchdog.write_access_enabled());
    write_bool(&mut payload, watchdog.integration_test_enabled());
    write_bool(&mut payload, watchdog.raw_interrupt());
    write_u64(&mut payload, watchdog.clock_tick());
    write_u64(&mut payload, watchdog.generation());
    write_u64(&mut payload, watchdog.reset_assertions().len() as u64);
    for tick in watchdog.reset_assertions() {
        write_u64(&mut payload, *tick);
    }
    payload
}

fn decode_sp805(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Sp805WatchdogMmioSnapshot, Sp805CheckpointError> {
    let mut cursor = PayloadCursor::new(component, payload);
    let timeout_interval = cursor.read_u32("timeout_interval")?;
    let has_timeout_start_tick = cursor.read_bool("has_timeout_start_tick")?;
    let timeout_start_tick = if has_timeout_start_tick {
        Some(cursor.read_u64("timeout_start_tick")?)
    } else {
        None
    };
    let persisted_value = cursor.read_u32("persisted_value")?;
    let enabled = cursor.read_bool("enabled")?;
    let reset_enabled = cursor.read_bool("reset_enabled")?;
    let write_access_enabled = cursor.read_bool("write_access_enabled")?;
    let integration_test_enabled = cursor.read_bool("integration_test_enabled")?;
    let raw_interrupt = cursor.read_bool("raw_interrupt")?;
    let clock_tick = cursor.read_u64("clock_tick")?;
    let generation = cursor.read_u64("generation")?;
    let reset_count = cursor.read_count("reset_assertion_count")?;
    let mut reset_assertions = Vec::with_capacity(reset_count);
    for _ in 0..reset_count {
        reset_assertions.push(cursor.read_u64("reset_assertion")?);
    }
    cursor.finish()?;

    Ok(Sp805WatchdogMmioSnapshot::new(
        Sp805WatchdogSnapshot::from_fields(Sp805WatchdogSnapshotFields {
            timeout_interval,
            timeout_start_tick,
            persisted_value,
            enabled,
            reset_enabled,
            write_access_enabled,
            integration_test_enabled,
            raw_interrupt,
            clock_tick,
            generation,
            reset_assertions,
        }),
    ))
}

fn default_sp805() -> Sp805Watchdog {
    Sp805Watchdog::new(1).expect("static SP805 initialization is valid")
}

fn write_bool(payload: &mut Vec<u8>, value: bool) {
    payload.push(u8::from(value));
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

struct PayloadCursor<'a> {
    component: &'a CheckpointComponentId,
    payload: &'a [u8],
    offset: usize,
}

impl<'a> PayloadCursor<'a> {
    const fn new(component: &'a CheckpointComponentId, payload: &'a [u8]) -> Self {
        Self {
            component,
            payload,
            offset: 0,
        }
    }

    fn read_bool(&mut self, name: &str) -> Result<bool, Sp805CheckpointError> {
        match self.read_u8(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} has invalid bool byte {value}"))),
        }
    }

    fn read_u8(&mut self, name: &str) -> Result<u8, Sp805CheckpointError> {
        let bytes = self.read_bytes(name, U8_BYTES)?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, Sp805CheckpointError> {
        let bytes: [u8; U32_BYTES] = self
            .read_bytes(name, U32_BYTES)?
            .try_into()
            .expect("cursor returned exact u32 bytes");
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, Sp805CheckpointError> {
        let bytes: [u8; U64_BYTES] = self
            .read_bytes(name, U64_BYTES)?
            .try_into()
            .expect("cursor returned exact u64 bytes");
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_count(&mut self, name: &str) -> Result<usize, Sp805CheckpointError> {
        let value = self.read_u64(name)?;
        usize::try_from(value).map_err(|_| self.invalid(format!("{name} does not fit usize")))
    }

    fn read_bytes(&mut self, name: &str, size: usize) -> Result<&'a [u8], Sp805CheckpointError> {
        let end = self
            .offset
            .checked_add(size)
            .ok_or_else(|| self.invalid(format!("{name} size overflows host usize")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!(
                "{name} needs {size} bytes at offset {} but payload has {}",
                self.offset,
                self.payload.len()
            )));
        }
        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(&self) -> Result<(), Sp805CheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "trailing {} bytes after SP805 checkpoint payload",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> Sp805CheckpointError {
        Sp805CheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
