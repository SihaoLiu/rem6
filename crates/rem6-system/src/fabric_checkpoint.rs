use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::sync::{Arc, Mutex};

use rem6_checkpoint::{CheckpointComponentId, CheckpointError, CheckpointRegistry};
use rem6_fabric::{FabricError, FabricLaneSnapshot, FabricLinkId, FabricModel, VirtualNetworkId};

const FABRIC_CHUNK: &str = "fabric";
const FORMAT_VERSION: u64 = 1;
const U32_BYTES: usize = 4;
const U64_BYTES: usize = 8;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FabricCheckpointRecord {
    component: CheckpointComponentId,
    lanes: Vec<FabricLaneSnapshot>,
}

impl FabricCheckpointRecord {
    pub fn new(component: CheckpointComponentId, lanes: Vec<FabricLaneSnapshot>) -> Self {
        Self { component, lanes }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn lanes(&self) -> &[FabricLaneSnapshot] {
        &self.lanes
    }
}

#[derive(Clone, Debug)]
pub struct FabricCheckpointPort {
    component: CheckpointComponentId,
    fabric: Arc<Mutex<FabricModel>>,
}

impl FabricCheckpointPort {
    pub fn new(component: CheckpointComponentId, fabric: Arc<Mutex<FabricModel>>) -> Self {
        Self { component, fabric }
    }

    pub fn component(&self) -> &CheckpointComponentId {
        &self.component
    }

    pub fn fabric(&self) -> Arc<Mutex<FabricModel>> {
        Arc::clone(&self.fabric)
    }

    pub fn register(&self, registry: &mut CheckpointRegistry) -> Result<(), CheckpointError> {
        registry.register(self.component.clone())
    }

    pub fn capture_into(
        &self,
        registry: &mut CheckpointRegistry,
    ) -> Result<FabricCheckpointRecord, FabricCheckpointError> {
        let lanes = self
            .fabric
            .lock()
            .expect("fabric checkpoint lock")
            .lane_snapshots();
        registry
            .write_chunk(&self.component, FABRIC_CHUNK, encode_lanes(&lanes))
            .map_err(FabricCheckpointError::Checkpoint)?;
        Ok(FabricCheckpointRecord::new(self.component.clone(), lanes))
    }

    pub fn restore_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<FabricCheckpointRecord, FabricCheckpointError> {
        let record = self.decode_from(registry)?;
        self.restore_lanes(record.lanes())?;
        Ok(record)
    }

    fn decode_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<FabricCheckpointRecord, FabricCheckpointError> {
        let payload = registry
            .chunk(&self.component, FABRIC_CHUNK)
            .ok_or_else(|| FabricCheckpointError::MissingChunk {
                component: self.component.clone(),
                name: FABRIC_CHUNK.to_string(),
            })?;
        let lanes = decode_lanes(&self.component, payload)?;
        Ok(FabricCheckpointRecord::new(self.component.clone(), lanes))
    }

    fn validate_lanes(&self, lanes: &[FabricLaneSnapshot]) -> Result<(), FabricCheckpointError> {
        let mut fabric = self.fabric.lock().expect("fabric checkpoint lock").clone();
        fabric
            .restore_lane_snapshots(lanes.iter().cloned())
            .map_err(|error| FabricCheckpointError::Fabric {
                component: self.component.clone(),
                error,
            })
    }

    fn restore_lanes(&self, lanes: &[FabricLaneSnapshot]) -> Result<(), FabricCheckpointError> {
        self.fabric
            .lock()
            .expect("fabric checkpoint lock")
            .restore_lane_snapshots(lanes.iter().cloned())
            .map_err(|error| FabricCheckpointError::Fabric {
                component: self.component.clone(),
                error,
            })
    }
}

#[derive(Clone, Debug, Default)]
pub struct FabricCheckpointBank {
    ports: BTreeMap<CheckpointComponentId, FabricCheckpointPort>,
}

impl FabricCheckpointBank {
    pub fn new<I>(ports: I) -> Result<Self, CheckpointError>
    where
        I: IntoIterator<Item = FabricCheckpointPort>,
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
    ) -> Result<Vec<FabricCheckpointRecord>, FabricCheckpointError> {
        self.ports
            .values()
            .map(|port| port.capture_into(registry))
            .collect()
    }

    pub fn restore_all_from(
        &self,
        registry: &CheckpointRegistry,
    ) -> Result<Vec<FabricCheckpointRecord>, FabricCheckpointError> {
        let mut decoded = Vec::new();
        for port in self.ports.values() {
            let record = port.decode_from(registry)?;
            port.validate_lanes(record.lanes())?;
            decoded.push((port, record));
        }

        let mut restored = Vec::new();
        for (port, record) in decoded {
            port.restore_lanes(record.lanes())?;
            restored.push(record);
        }
        Ok(restored)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FabricCheckpointError {
    MissingChunk {
        component: CheckpointComponentId,
        name: String,
    },
    InvalidChunk {
        component: CheckpointComponentId,
        reason: String,
    },
    Checkpoint(CheckpointError),
    Fabric {
        component: CheckpointComponentId,
        error: FabricError,
    },
}

impl FabricCheckpointError {
    pub fn component(&self) -> Option<&CheckpointComponentId> {
        match self {
            Self::MissingChunk { component, .. }
            | Self::InvalidChunk { component, .. }
            | Self::Fabric { component, .. } => Some(component),
            Self::Checkpoint(_) => None,
        }
    }
}

impl fmt::Display for FabricCheckpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingChunk { component, name } => write!(
                formatter,
                "fabric checkpoint component {} is missing chunk {name}",
                component.as_str()
            ),
            Self::InvalidChunk { component, reason } => write!(
                formatter,
                "fabric checkpoint component {} has invalid chunk: {reason}",
                component.as_str()
            ),
            Self::Checkpoint(error) => write!(formatter, "{error}"),
            Self::Fabric { component, error } => write!(
                formatter,
                "fabric checkpoint component {} restore failed: {error}",
                component.as_str()
            ),
        }
    }
}

impl Error for FabricCheckpointError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Checkpoint(error) => Some(error),
            Self::Fabric { error, .. } => Some(error),
            Self::MissingChunk { .. } | Self::InvalidChunk { .. } => None,
        }
    }
}

fn encode_lanes(lanes: &[FabricLaneSnapshot]) -> Vec<u8> {
    let mut payload = Vec::new();
    write_u64(&mut payload, FORMAT_VERSION);
    write_u64(&mut payload, lanes.len() as u64);
    for lane in lanes {
        write_string(&mut payload, lane.link().as_str());
        write_u32(&mut payload, u32::from(lane.virtual_network().get()));
        write_u64(&mut payload, lane.next_available_tick());
        write_u64(&mut payload, lane.credit_return_ticks().len() as u64);
        for tick in lane.credit_return_ticks() {
            write_u64(&mut payload, *tick);
        }
    }
    payload
}

fn decode_lanes(
    component: &CheckpointComponentId,
    payload: &[u8],
) -> Result<Vec<FabricLaneSnapshot>, FabricCheckpointError> {
    let mut cursor = PayloadCursor::new(component.clone(), payload);
    let version = cursor.read_u64("fabric checkpoint version")?;
    if version != FORMAT_VERSION {
        return Err(cursor.invalid(format!("unsupported fabric checkpoint version {version}")));
    }

    let lane_count = cursor.read_count("fabric lane count")?;
    let mut lanes = Vec::with_capacity(lane_count);
    for _ in 0..lane_count {
        let link = FabricLinkId::new(cursor.read_string("fabric link id")?)
            .map_err(|error| cursor.invalid(error.to_string()))?;
        let raw_virtual_network = cursor.read_u32("fabric virtual network")?;
        let virtual_network = u16::try_from(raw_virtual_network)
            .map(VirtualNetworkId::new)
            .map_err(|_| {
                cursor.invalid(format!(
                    "fabric virtual network {raw_virtual_network} exceeds u16"
                ))
            })?;
        let next_available_tick = cursor.read_u64("fabric next available tick")?;
        let credit_count = cursor.read_count("fabric credit return count")?;
        let mut credit_return_ticks = Vec::with_capacity(credit_count);
        for _ in 0..credit_count {
            credit_return_ticks.push(cursor.read_u64("fabric credit return tick")?);
        }
        lanes.push(FabricLaneSnapshot::new(
            link,
            virtual_network,
            next_available_tick,
            credit_return_ticks,
        ));
    }
    cursor.finish()?;
    Ok(lanes)
}

fn write_u32(payload: &mut Vec<u8>, value: u32) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_u64(payload: &mut Vec<u8>, value: u64) {
    payload.extend_from_slice(&value.to_le_bytes());
}

fn write_string(payload: &mut Vec<u8>, value: &str) {
    write_u64(payload, value.len() as u64);
    payload.extend_from_slice(value.as_bytes());
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

    fn read_u32(&mut self, field: &str) -> Result<u32, FabricCheckpointError> {
        let bytes = self.read_exact(field, U32_BYTES)?;
        Ok(u32::from_le_bytes(bytes.try_into().expect("u32 slice")))
    }

    fn read_u64(&mut self, field: &str) -> Result<u64, FabricCheckpointError> {
        let bytes = self.read_exact(field, U64_BYTES)?;
        Ok(u64::from_le_bytes(bytes.try_into().expect("u64 slice")))
    }

    fn read_count(&mut self, field: &str) -> Result<usize, FabricCheckpointError> {
        let value = self.read_u64(field)?;
        usize::try_from(value).map_err(|_| self.invalid(format!("{field} exceeds usize")))
    }

    fn read_string(&mut self, field: &str) -> Result<String, FabricCheckpointError> {
        let len = self.read_count(field)?;
        let bytes = self.read_exact(field, len)?;
        std::str::from_utf8(bytes)
            .map(str::to_string)
            .map_err(|error| self.invalid(format!("{field} is not UTF-8: {error}")))
    }

    fn read_exact(&mut self, field: &str, len: usize) -> Result<&'a [u8], FabricCheckpointError> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or_else(|| self.invalid(format!("{field} length overflows")))?;
        if end > self.payload.len() {
            return Err(self.invalid(format!("{field} is truncated")));
        }

        let bytes = &self.payload[self.offset..end];
        self.offset = end;
        Ok(bytes)
    }

    fn finish(self) -> Result<(), FabricCheckpointError> {
        if self.offset == self.payload.len() {
            Ok(())
        } else {
            Err(self.invalid(format!(
                "{} trailing bytes",
                self.payload.len() - self.offset
            )))
        }
    }

    fn invalid(&self, reason: String) -> FabricCheckpointError {
        FabricCheckpointError::InvalidChunk {
            component: self.component.clone(),
            reason,
        }
    }
}
