use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_memory::Address;
use rem6_timer::{
    Mc146818Rtc, Mc146818RtcMmioDevice, Mc146818RtcMmioSnapshot, RtcDateTime, RtcEncoding,
    RtcError, RtcSnapshot, RTC_CMOS_REGISTER_COUNT,
};

const RTC_CHUNK: &str = "rtc";
const RTC_CLOCK_REGISTER_COUNT: usize = 10;
const U8_BYTES: usize = 1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RtcCheckpointRecord {
    component: CheckpointComponentId,
    snapshot: Mc146818RtcMmioSnapshot,
}

impl RtcCheckpointRecord {
    pub fn new(component: CheckpointComponentId, snapshot: Mc146818RtcMmioSnapshot) -> Self {
        Self {
            component,
            snapshot,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn snapshot(&self) -> &Mc146818RtcMmioSnapshot {
        &self.snapshot
    }
}

#[derive(Clone, Debug)]
pub struct RtcCheckpointPort {
    component: CheckpointComponentId,
    device: Mc146818RtcMmioDevice,
}

impl RtcCheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: Mc146818RtcMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> Mc146818RtcMmioDevice {
        self.device.clone()
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<RtcCheckpointRecord, CheckpointError> {
        let snapshot = self.device.snapshot();
        registry.write_chunk(&self.component, RTC_CHUNK, encode_rtc(&snapshot))?;
        Ok(RtcCheckpointRecord::new(self.component.clone(), snapshot))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<RtcCheckpointRecord, RtcCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_snapshot(record.snapshot())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<RtcCheckpointRecord, RtcCheckpointError> {
        let payload = registry.chunk(&self.component, RTC_CHUNK).ok_or_else(|| {
            RtcCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: RTC_CHUNK.to_string(),
            }
        })?;
        let snapshot = decode_rtc(&self.component, payload)?;
        self.validate_snapshot(&snapshot)?;
        Ok(RtcCheckpointRecord::new(self.component.clone(), snapshot))
    }

    fn validate_snapshot(
        &self,
        snapshot: &Mc146818RtcMmioSnapshot,
    ) -> Result<(), RtcCheckpointError> {
        let probe = Mc146818RtcMmioDevice::new(Address::new(0), default_rtc());
        probe
            .restore(snapshot)
            .map_err(|error| RtcCheckpointError::Rtc {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_snapshot(
        &self,
        snapshot: &Mc146818RtcMmioSnapshot,
    ) -> Result<(), RtcCheckpointError> {
        self.device
            .restore(snapshot)
            .map_err(|error| RtcCheckpointError::Rtc {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct RtcCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, RtcCheckpointPort>,
}

impl RtcCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = RtcCheckpointPort>,
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
    ) -> Result<Vec<RtcCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<RtcCheckpointRecord>, RtcCheckpointError> {
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
    ) -> Result<(), RtcCheckpointError> {
        for port in self.ports.values() {
            port.decode_from(registry)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RtcCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Rtc {
        component: CheckpointComponentId,
        error: RtcError,
    },
}

impl RtcCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Rtc { component, .. } => component,
        }
    }
}

impl fmt::Display for RtcCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "RTC checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "RTC checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Rtc { component, error } => write!(
                formatter,
                "RTC checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for RtcCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Rtc { error, .. } => Some(error),
            _ => None,
        }
    }
}

fn encode_rtc(snapshot: &Mc146818RtcMmioSnapshot) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u8(&mut payload, snapshot.selected_address());
    payload.extend_from_slice(snapshot.cmos_data());
    payload.extend_from_slice(snapshot.rtc().clock_data());
    write_u8(&mut payload, snapshot.rtc().status_a());
    write_u8(&mut payload, snapshot.rtc().status_b());
    payload
}

fn decode_rtc(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Mc146818RtcMmioSnapshot, RtcCheckpointError> {
    let mut cursor = PayloadCursor::new(component, payload);
    let selected_address = cursor.read_u8("selected_address")?;
    let mut cmos_data = [0; RTC_CMOS_REGISTER_COUNT];
    cmos_data.copy_from_slice(cursor.read_bytes("cmos_data", RTC_CMOS_REGISTER_COUNT)?);
    let mut clock_data = [0; RTC_CLOCK_REGISTER_COUNT];
    clock_data.copy_from_slice(cursor.read_bytes("clock_data", RTC_CLOCK_REGISTER_COUNT)?);
    let status_a = cursor.read_u8("status_a")?;
    let status_b = cursor.read_u8("status_b")?;
    cursor.finish()?;

    Ok(Mc146818RtcMmioSnapshot::new(
        selected_address,
        cmos_data,
        RtcSnapshot::new(clock_data, status_a, status_b),
    ))
}

fn default_rtc() -> Mc146818Rtc {
    Mc146818Rtc::new(
        RtcDateTime::new(2000, 1, 1, 0, 0, 0, 6).expect("static RTC date is valid"),
        RtcEncoding::Bcd,
    )
    .expect("static RTC initialization is valid")
}

fn write_u8(payload: &mut Vec<u8>, value: u8) {
    payload.push(value);
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

    fn read_u8(&mut self, name: &str) -> Result<u8, RtcCheckpointError> {
        let bytes = self.read_bytes(name, U8_BYTES)?;
        Ok(bytes[0])
    }

    fn read_bytes(&mut self, name: &str, size: usize) -> Result<&'a [u8], RtcCheckpointError> {
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

    fn finish(&self) -> Result<(), RtcCheckpointError> {
        if self.offset == self.payload.len() {
            return Ok(());
        }
        Err(self.invalid(format!(
            "trailing {} bytes after RTC checkpoint payload",
            self.payload.len() - self.offset
        )))
    }

    fn invalid(&self, reason: String) -> RtcCheckpointError {
        RtcCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
