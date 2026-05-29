use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_memory::Address;
use rem6_timer::{
    Sp804DualTimer, Sp804DualTimerMmioDevice, Sp804DualTimerMmioSnapshot, Sp804DualTimerSnapshot,
    Sp804Error, Sp804TimerControl, Sp804TimerSnapshot, Sp804TimerSnapshotFields, SP804_TIMER_COUNT,
};

const SP804_CHUNK: &str = "sp804";
const U8_BYTES: usize = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sp804CheckpointRecord {
    component: CheckpointComponentId,
    snapshot: Sp804DualTimerMmioSnapshot,
}

impl Sp804CheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: Sp804DualTimerMmioSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &Sp804DualTimerMmioSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct Sp804CheckpointPort {
    component: CheckpointComponentId,
    device: Sp804DualTimerMmioDevice,
}

impl Sp804CheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: Sp804DualTimerMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> Sp804DualTimerMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Sp804CheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(&self.component, SP804_CHUNK, encode_sp804(&snapshot))?;
        Ok(Sp804CheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Sp804CheckpointRecord, Sp804CheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Sp804CheckpointRecord, Sp804CheckpointError> {
        let payload = registry
            .chunk(&self.component, SP804_CHUNK)
            .ok_or_else(|| Sp804CheckpointError::MissingChunk {
                component: self.component.clone(),
                name: SP804_CHUNK.to_string(),
            })?;
        let snapshot = decode_sp804(&self.component, payload)?;
        self.validate_snapshot(&snapshot)?;
        Ok(Sp804CheckpointRecord::new(self.component.clone(), snapshot))
    }

    fn validate_snapshot(
        &self,
        snapshot: &Sp804DualTimerMmioSnapshot,
    ) -> Result<(), Sp804CheckpointError> {
        let probe = Sp804DualTimerMmioDevice::new(Address::new(0), default_sp804());
        probe
            .restore(snapshot)
            .map_err(|error| Sp804CheckpointError::Sp804 {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &Sp804DualTimerMmioSnapshot,
    ) -> Result<(), Sp804CheckpointError> {
        self.device
            .restore(snapshot)
            .map_err(|error| Sp804CheckpointError::Sp804 {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Sp804CheckpointBank {
    ports: BTreeMap<CheckpointComponentId, Sp804CheckpointPort>,
}

impl Sp804CheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = Sp804CheckpointPort>,
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
    ) -> Result<Vec<Sp804CheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<Sp804CheckpointRecord>, Sp804CheckpointError> {
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
    ) -> Result<(), Sp804CheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Sp804CheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Sp804 {
        component: CheckpointComponentId,
        error: Sp804Error,
    },
}

impl Sp804CheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Sp804 { component, .. } => component,
        }
    }
}

impl fmt::Display for Sp804CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "SP804 checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "SP804 checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Sp804 { component, error } => write!(
                formatter,
                "SP804 checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for Sp804CheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Sp804 { error, .. } => Some(error),
            _ => None,
        }
    }
}

fn encode_sp804(snapshot: &Sp804DualTimerMmioSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    for timer in snapshot.timers().timers() {
        write_u32(&mut payload, timer.load_value());
        write_u32(&mut payload, timer.background_load_value());
        write_u32(&mut payload, timer.base_value());
        write_u64(&mut payload, timer.last_updated_tick());
        write_u32(&mut payload, timer.control().bits());
        write_bool(&mut payload, timer.raw_interrupt());
        write_bool(&mut payload, timer.pending_interrupt());
        write_u64(&mut payload, timer.clock_tick());
        write_u64(&mut payload, timer.generation());
    }
    payload
}

fn decode_sp804(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Sp804DualTimerMmioSnapshot, Sp804CheckpointError> {
    let mut cursor = PayloadCursor::new(component, payload);
    let mut timers = Vec::with_capacity(SP804_TIMER_COUNT);
    for _ in 0..SP804_TIMER_COUNT {
        let load_value = cursor.read_u32("load_value")?;
        let background_load_value = cursor.read_u32("background_load_value")?;
        let base_value = cursor.read_u32("base_value")?;
        let last_updated_tick = cursor.read_u64("last_updated_tick")?;
        let control = Sp804TimerControl::new(cursor.read_u32("control")?);
        let raw_interrupt = cursor.read_bool("raw_interrupt")?;
        let pending_interrupt = cursor.read_bool("pending_interrupt")?;
        let clock_tick = cursor.read_u64("clock_tick")?;
        let generation = cursor.read_u64("generation")?;
        timers.push(Sp804TimerSnapshot::from_fields(Sp804TimerSnapshotFields {
            load_value,
            background_load_value,
            base_value,
            last_updated_tick,
            control,
            raw_interrupt,
            pending_interrupt,
            clock_tick,
            generation,
        }));
    }
    let timers: [Sp804TimerSnapshot; SP804_TIMER_COUNT] =
        timers.try_into().expect("static SP804 timer count matched");
    cursor.finish()?;

    Ok(Sp804DualTimerMmioSnapshot::new(
        Sp804DualTimerSnapshot::new(timers),
    ))
}

fn default_sp804() -> Sp804DualTimer {
    Sp804DualTimer::new(1, 1).expect("static SP804 initialization is valid")
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

    fn read_bool(&mut self, name: &str) -> Result<bool, Sp804CheckpointError> {
        match self.read_u8(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} has invalid bool byte {value}"))),
        }
    }

    fn read_u8(&mut self, name: &str) -> Result<u8, Sp804CheckpointError> {
        let bytes = self.read_bytes(name, U8_BYTES)?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, Sp804CheckpointError> {
        let bytes: [u8; U32_BYTES] = self
            .read_bytes(name, U32_BYTES)?
            .try_into()
            .expect("cursor returned exact u32 bytes");
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, Sp804CheckpointError> {
        let bytes: [u8; U64_BYTES] = self
            .read_bytes(name, U64_BYTES)?
            .try_into()
            .expect("cursor returned exact u64 bytes");
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_bytes(&mut self, name: &str, size: usize) -> Result<&'a [u8], Sp804CheckpointError> {
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

    fn finish(&self) -> Result<(), Sp804CheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "trailing {} bytes after SP804 checkpoint payload",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> Sp804CheckpointError {
        Sp804CheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
