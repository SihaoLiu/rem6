use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_memory::Address;
use rem6_timer::{
    Pl031Error, Pl031Rtc, Pl031RtcMmioDevice, Pl031RtcMmioSnapshot, Pl031Snapshot,
    Pl031SnapshotFields,
};

const PL031_CHUNK: &str = "pl031";
const U8_BYTES: usize = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pl031CheckpointRecord {
    component: CheckpointComponentId,
    snapshot: Pl031RtcMmioSnapshot,
}

impl Pl031CheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: Pl031RtcMmioSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &Pl031RtcMmioSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct Pl031CheckpointPort {
    component: CheckpointComponentId,
    device: Pl031RtcMmioDevice,
}

impl Pl031CheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: Pl031RtcMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> Pl031RtcMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<Pl031CheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(&self.component, PL031_CHUNK, encode_pl031(&snapshot))?;
        Ok(Pl031CheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Pl031CheckpointRecord, Pl031CheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Pl031CheckpointRecord, Pl031CheckpointError> {
        let payload = registry
            .chunk(&self.component, PL031_CHUNK)
            .ok_or_else(|| Pl031CheckpointError::MissingChunk {
                component: self.component.clone(),
                name: PL031_CHUNK.to_string(),
            })?;
        let snapshot = decode_pl031(&self.component, payload)?;
        self.validate_snapshot(&snapshot)?;
        Ok(Pl031CheckpointRecord::new(self.component.clone(), snapshot))
    }

    fn validate_snapshot(
        &self,
        snapshot: &Pl031RtcMmioSnapshot,
    ) -> Result<(), Pl031CheckpointError> {
        let probe = Pl031RtcMmioDevice::new(Address::new(0), default_pl031());
        probe
            .restore(snapshot)
            .map_err(|error| Pl031CheckpointError::Pl031 {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &Pl031RtcMmioSnapshot,
    ) -> Result<(), Pl031CheckpointError> {
        self.device
            .restore(snapshot)
            .map_err(|error| Pl031CheckpointError::Pl031 {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct Pl031CheckpointBank {
    ports: BTreeMap<CheckpointComponentId, Pl031CheckpointPort>,
}

impl Pl031CheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = Pl031CheckpointPort>,
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
    ) -> Result<Vec<Pl031CheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<Pl031CheckpointRecord>, Pl031CheckpointError> {
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
    ) -> Result<(), Pl031CheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Pl031CheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Pl031 {
        component: CheckpointComponentId,
        error: Pl031Error,
    },
}

impl Pl031CheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Pl031 { component, .. } => component,
        }
    }
}

impl fmt::Display for Pl031CheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "PL031 checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "PL031 checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Pl031 { component, error } => write!(
                formatter,
                "PL031 checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for Pl031CheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Pl031 { error, .. } => Some(error),
            _ => None,
        }
    }
}

fn encode_pl031(snapshot: &Pl031RtcMmioSnapshot) -> Vec<u8> {
    let rtc = snapshot.rtc();
    let mut payload = Vec::new();
    write_u32(&mut payload, rtc.time_value());
    write_u64(&mut payload, rtc.last_written_tick());
    write_u32(&mut payload, rtc.load_value());
    write_u32(&mut payload, rtc.match_value());
    write_bool(&mut payload, rtc.raw_interrupt());
    write_bool(&mut payload, rtc.interrupt_mask());
    write_bool(&mut payload, rtc.pending_interrupt());
    write_u64(&mut payload, rtc.ticks_per_second());
    write_u64(&mut payload, rtc.generation());
    payload
}

fn decode_pl031(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Pl031RtcMmioSnapshot, Pl031CheckpointError> {
    let mut cursor = PayloadCursor::new(component, payload);
    let time_value = cursor.read_u32("time_value")?;
    let last_written_tick = cursor.read_u64("last_written_tick")?;
    let load_value = cursor.read_u32("load_value")?;
    let match_value = cursor.read_u32("match_value")?;
    let raw_interrupt = cursor.read_bool("raw_interrupt")?;
    let interrupt_mask = cursor.read_bool("interrupt_mask")?;
    let pending_interrupt = cursor.read_bool("pending_interrupt")?;
    let ticks_per_second = cursor.read_u64("ticks_per_second")?;
    let generation = cursor.read_u64("generation")?;
    cursor.finish()?;

    Ok(Pl031RtcMmioSnapshot::new(Pl031Snapshot::from_fields(
        Pl031SnapshotFields {
            time_value,
            last_written_tick,
            load_value,
            match_value,
            raw_interrupt,
            interrupt_mask,
            pending_interrupt,
            ticks_per_second,
            generation,
        },
    )))
}

fn default_pl031() -> Pl031Rtc {
    Pl031Rtc::new(0, 1).expect("static PL031 initialization is valid")
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

    fn read_bool(&mut self, name: &str) -> Result<bool, Pl031CheckpointError> {
        match self.read_u8(name)? {
            0 => Ok(false),
            1 => Ok(true),
            value => Err(self.invalid(format!("{name} has invalid bool byte {value}"))),
        }
    }

    fn read_u8(&mut self, name: &str) -> Result<u8, Pl031CheckpointError> {
        let bytes = self.read_bytes(name, U8_BYTES)?;
        Ok(bytes[0])
    }

    fn read_u32(&mut self, name: &str) -> Result<u32, Pl031CheckpointError> {
        let bytes: [u8; U32_BYTES] = self
            .read_bytes(name, U32_BYTES)?
            .try_into()
            .expect("cursor returned exact u32 bytes");
        Ok(u32::from_le_bytes(bytes))
    }

    fn read_u64(&mut self, name: &str) -> Result<u64, Pl031CheckpointError> {
        let bytes: [u8; U64_BYTES] = self
            .read_bytes(name, U64_BYTES)?
            .try_into()
            .expect("cursor returned exact u64 bytes");
        Ok(u64::from_le_bytes(bytes))
    }

    fn read_bytes(&mut self, name: &str, size: usize) -> Result<&'a [u8], Pl031CheckpointError> {
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

    fn finish(&self) -> Result<(), Pl031CheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "trailing {} bytes after PL031 checkpoint payload",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> Pl031CheckpointError {
        Pl031CheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
