use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_memory::{AccessSize, Address};
use rem6_platform::PlatformReadfileMmioDevice;

const READFILE_CHUNK: &str = "readfile";
const U64_BYTES: usize = 8;
const HEADER_BYTES: usize = U64_BYTES * 3;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadfileCheckpointRecord {
    component: CheckpointComponentId,
    base: Address,
    size: AccessSize,
    payload: Vec<u8>,
}

impl ReadfileCheckpointRecord {
    pub fn new(
        component: CheckpointComponentId,
        base: Address,
        size: AccessSize,
        payload: Vec<u8>,
    ) -> Self {
        Self {
            component,
            base,
            size,
            payload,
        }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub const fn base(&self) -> Address {
        self.base
    }

    pub const fn size(&self) -> AccessSize {
        self.size
    }

    pub fn payload(&self) -> &[u8] {
        &self.payload
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReadfileCheckpointPort {
    component: CheckpointComponentId,
    device: PlatformReadfileMmioDevice,
}

impl ReadfileCheckpointPort {
    pub const fn new(component: CheckpointComponentId, device: PlatformReadfileMmioDevice) -> Self {
        Self { component, device }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn device(&self) -> &PlatformReadfileMmioDevice {
        &self.device
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<ReadfileCheckpointRecord, CheckpointError> {
        let record = self.record_from_device();
        registry.write_chunk(&self.component, READFILE_CHUNK, encode_record(&record))?;
        Ok(record)
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<ReadfileCheckpointRecord, ReadfileCheckpointError> {
        let record = self.decode_from(registry)?;
        self.validate_record_matches_device(&record)?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<ReadfileCheckpointRecord, ReadfileCheckpointError> {
        let payload = registry
            .chunk(&self.component, READFILE_CHUNK)
            .ok_or_else(|| ReadfileCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: READFILE_CHUNK.to_string(),
            })?;
        decode_record(&self.component, payload)
    }

    fn record_from_device(&self) -> ReadfileCheckpointRecord {
        ReadfileCheckpointRecord::new(
            self.component.clone(),
            self.device.base(),
            self.device.range().size(),
            self.device.payload().to_vec(),
        )
    }

    fn validate_record_matches_device(
        &self,
        record: &ReadfileCheckpointRecord,
    ) -> Result<(), ReadfileCheckpointError> {
        let expected = self.record_from_device();
        if &expected == record {
            return Ok(());
        }

        Err(ReadfileCheckpointError::DeviceMismatch {
            component: self.component.clone(),
        })
    }
}

#[derive(Clone, Debug, Default)]
pub struct ReadfileCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, ReadfileCheckpointPort>,
}

impl ReadfileCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = ReadfileCheckpointPort>,
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
    ) -> Result<Vec<ReadfileCheckpointRecord>, CheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<ReadfileCheckpointRecord>, ReadfileCheckpointError> {
        self.validate_restore_from(registry)?;
        self.ports
            .values()
            .map(|port| port.restore_from(registry))
            .collect()
    }

    pub fn validate_restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<(), ReadfileCheckpointError> {
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_record_matches_device(&record)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReadfileCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    DeviceMismatch {
        component: CheckpointComponentId,
    },
}

impl ReadfileCheckpointError {
    pub fn component(&self) -> &CheckpointComponentId {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::DeviceMismatch { component } => component,
        }
    }
}

impl fmt::Display for ReadfileCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "readfile checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "readfile checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::DeviceMismatch { component } => write!(
                formatter,
                "readfile checkpoint component {} does not match attached device",
                component.as_str()
            ),
        }
    }
}

impl Error for ReadfileCheckpointError {}

fn encode_record(record: &ReadfileCheckpointRecord) -> Vec<u8> {
    let mut payload = Vec::with_capacity(HEADER_BYTES + record.payload().len());
    write_u64(&mut payload, record.base().get());
    write_u64(&mut payload, record.size().bytes());
    write_u64(&mut payload, record.payload().len() as u64);
    payload.extend_from_slice(record.payload());
    payload
}

fn decode_record(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<ReadfileCheckpointRecord, ReadfileCheckpointError> {
    if payload.len() < HEADER_BYTES {
        return Err(ReadfileCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: format!(
                "expected at least {HEADER_BYTES} bytes, got {}",
                payload.len()
            ),
        });
    }

    let base = Address::new(read_u64(component, payload, 0, "base")?);
    let size =
        AccessSize::new(read_u64(component, payload, U64_BYTES, "size")?).map_err(|error| {
            ReadfileCheckpointError::InvalidChunk {
                component: component.clone(),
                reason: error.to_string(),
            }
        })?;
    let payload_len = read_u64(component, payload, U64_BYTES * 2, "payload length")?;
    let payload_len: usize =
        payload_len
            .try_into()
            .map_err(|_| ReadfileCheckpointError::InvalidChunk {
                component: component.clone(),
                reason: "payload length does not fit host usize".to_string(),
            })?;
    let expected = HEADER_BYTES.checked_add(payload_len).ok_or_else(|| {
        ReadfileCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: "payload length overflows record size".to_string(),
        }
    })?;
    if payload.len() != expected {
        return Err(ReadfileCheckpointError::InvalidChunk {
            component: component.clone(),
            reason: format!("expected {expected} bytes, got {}", payload.len()),
        });
    }

    Ok(ReadfileCheckpointRecord::new(
        component.clone(),
        base,
        size,
        payload[HEADER_BYTES..].to_vec(),
    ))
}

fn read_u64(
    component: &CheckpointComponentId,
    payload: &[u8],
    offset: usize,
    field: &'static str,
) -> Result<u64, ReadfileCheckpointError> {
    let end = offset + U64_BYTES;
    let bytes: [u8; U64_BYTES] =
        payload[offset..end]
            .try_into()
            .map_err(|_| ReadfileCheckpointError::InvalidChunk {
                component: component.clone(),
                reason: format!("{field} is truncated"),
            })?;
    Ok(u64::from_le_bytes(bytes))
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn component(name: &str) -> CheckpointComponentId {
        CheckpointComponentId::new(name).unwrap()
    }

    fn device(base: u64, size: u64, payload: &[u8]) -> PlatformReadfileMmioDevice {
        PlatformReadfileMmioDevice::new(
            Address::new(base),
            AccessSize::new(size).unwrap(),
            payload.to_vec(),
        )
        .unwrap()
    }

    fn record_payload(base: u64, size: u64, payload_len: u64, payload: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::new();
        write_u64(&mut bytes, base);
        write_u64(&mut bytes, size);
        write_u64(&mut bytes, payload_len);
        bytes.extend_from_slice(payload);
        bytes
    }

    #[test]
    fn readfile_checkpoint_bank_rejects_duplicate_components() {
        let component = component("readfile.a000");
        let first = ReadfileCheckpointPort::new(component.clone(), device(0xa000, 0x100, b"one"));
        let second = ReadfileCheckpointPort::new(component.clone(), device(0xa000, 0x100, b"two"));

        assert_eq!(
            ReadfileCheckpointBank::new([first, second]).unwrap_err(),
            CheckpointError::DuplicateComponent { component }
        );
    }

    #[test]
    fn readfile_checkpoint_restore_rejects_missing_chunk() {
        let component = component("readfile.a000");
        let port = ReadfileCheckpointPort::new(component.clone(), device(0xa000, 0x100, b"boot"));
        let mut registry = CheckpointRegistry::new();
        port.register(&mut registry).unwrap();

        assert_eq!(
            port.restore_from(&registry).unwrap_err(),
            ReadfileCheckpointError::MissingChunk {
                component,
                name: READFILE_CHUNK.to_string()
            }
        );
    }

    #[test]
    fn readfile_checkpoint_restore_rejects_truncated_chunk() {
        let component = component("readfile.a000");

        assert!(matches!(
            decode_record(&component, &[0; HEADER_BYTES - 1]).unwrap_err(),
            ReadfileCheckpointError::InvalidChunk { .. }
        ));
    }

    #[test]
    fn readfile_checkpoint_restore_rejects_invalid_window_size() {
        let component = component("readfile.a000");

        assert!(matches!(
            decode_record(&component, &record_payload(0xa000, 0, 0, &[])).unwrap_err(),
            ReadfileCheckpointError::InvalidChunk { .. }
        ));
    }

    #[test]
    fn readfile_checkpoint_restore_rejects_payload_length_mismatch() {
        let component = component("readfile.a000");

        assert!(matches!(
            decode_record(&component, &record_payload(0xa000, 0x100, 4, b"abc")).unwrap_err(),
            ReadfileCheckpointError::InvalidChunk { .. }
        ));
    }

    #[test]
    fn readfile_checkpoint_restore_rejects_device_mismatch() {
        let component = component("readfile.a000");
        let first = ReadfileCheckpointPort::new(component.clone(), device(0xa000, 0x100, b"boot"));
        let second =
            ReadfileCheckpointPort::new(component.clone(), device(0xa000, 0x100, b"other"));
        let mut registry = CheckpointRegistry::new();
        first.register(&mut registry).unwrap();
        first.capture_into(&mut registry).unwrap();

        assert_eq!(
            second.restore_from(&registry).unwrap_err(),
            ReadfileCheckpointError::DeviceMismatch { component }
        );
    }
}
